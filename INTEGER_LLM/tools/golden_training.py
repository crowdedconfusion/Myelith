#!/usr/bin/env python3
"""Erzeugt die Golden Vectors des Trainingspfades (Umfang `training`).

## Warum ein eigener Umfang und nicht mehr Vektoren unter `op`

Der `op`-Umfang traegt seit Monaten den Wert `894d8357ae92b5c1`, und
dieser Wert steht an sechs Stellen im Repositorium fest, darunter beide
CI-Laeufe. Neue Vektoren dort haetten ihn geaendert, also eine bestehende
Zusage gebrochen, um eine neue aufzustellen. Der Trainingspfad bekommt
deshalb seinen eigenen Umfang und seine eigene Zahl; beide stehen im
Protokoll nebeneinander, und eine Abweichung ist damit sofort
eingegrenzt: unter dem Modell, im Rueckwaertspass, oder darueber.

## Warum in Python und nicht in Rust

Aus demselben Grund wie `golden_backward.py`: **Ein Vektor, den der zu
pruefende Code selbst erzeugt, prueft nichts.** Die Sollwerte entstehen
hier aus einer UNABHAENGIGEN Nachbildung; stimmt der Rust-Kernel damit
ueberein, haben zwei getrennte Umsetzungen dasselbe gerechnet.

**Und das ist keine Formalitaet.** Am 2026-09-04 stellte sich heraus,
dass drei der acht Rueckwaertskerne falsch rechneten: `attention_backward`
(Fund 173), `rmsnorm_backward` (Fund 175) und die Umrechnung in
`gewicht_aus_master` (Fund 174). Alle drei hatten keinen Aufrufer
ausserhalb ihrer eigenen Tests, und **der Prueflauf, der 33 von 33
meldet, hat keinen von ihnen je gerechnet.** Genau diese Luecke schliesst
diese Datei.

Usage:
    cd INTEGER_LLM
    python tools/golden_training.py
"""
import hashlib
import json
import math
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ZIEL = REPO / "conformance" / "vectors" / "training"
SPEC = REPO / "theta_v" / "spec.json"

MASK64 = (1 << 64) - 1


def theta_v_hash() -> str:
    return "sha256:" + hashlib.sha256(SPEC.read_bytes()).hexdigest()


def pack(daten, dtype: str) -> bytes:
    formate = {"int8": "b", "int16": "<h", "int32": "<i", "int64": "<q"}
    return b"".join(struct.pack(formate[dtype], int(v)) for v in daten)


def tensor(daten, dtype: str) -> dict:
    return {
        "dtype": dtype,
        "shape": [len(daten)],
        "hash": hashlib.sha256(pack(daten, dtype)).hexdigest(),
        "data": [int(v) for v in daten],
    }


# --------------------------------------------------------------------------
# Die Grundoperationen, nachgebildet
# --------------------------------------------------------------------------

def rshift_round(v: int, s: int) -> int:
    """Rundung zur naechsten GERADEN Zahl auf der Zweierkomplement-Darstellung.

    Nachbildung von `fixed_point::rshift_round_i64`. Der Vertrag steht in
    `golden_backward.py` ausfuehrlich; er ist hier woertlich derselbe.
    """
    if s == 0:
        return v
    mask = (1 << s) - 1
    half = 1 << (s - 1)
    quotient = v >> s
    rest = v & mask
    if rest > half or (rest == half and (quotient & 1) != 0):
        return quotient + 1
    return quotient


def verschiebe(v: int, s: int) -> int:
    """Rechts mit Rundung, links exakt."""
    return rshift_round(v, s) if s >= 0 else v << (-s)


def rescale(v: int, ein: int, aus: int) -> int:
    return verschiebe(v, ein - aus)


def clamp_i16(v: int) -> int:
    return max(-32768, min(32767, v))


def clamp_i32(v: int) -> int:
    return max(-2147483648, min(2147483647, v))


def lut_lookup(x: int, lut, shift: int, offset: int) -> int:
    idx = (x >> shift) + offset
    return lut[max(0, min(len(lut) - 1, idx))]


def softmax_ueber_vokabular(logits, logit_frac, exp_lut, exp_input_frac, frac_bits):
    """Nachbildung von `softmax::softmax_ueber_vokabular` (Fund 177).

    Unabhaengig geschrieben: die Summe in beliebiger Genauigkeit (Python
    rechnet ohnehin gross), die Rundung ueber `rshift_round`-Verwandtes
    ausgeschrieben, damit der Vergleich nicht dieselbe Zeile zweimal
    prueft.
    """
    schieben = logit_frac - exp_input_frac
    assert schieben >= 0
    lut_eins = exp_lut[0]
    m = max(logits)
    exps = []
    for z in logits:
        d = m - z
        if d <= 0:
            exps.append(lut_eins)
        else:
            idx = d >> schieben
            exps.append(exp_lut[idx] if idx < len(exp_lut) else 0)
    s = sum(exps)
    eins = 1 << frac_bits
    if s == 0:
        basis, rest = divmod(eins, len(logits))
        return [basis + (1 if i < rest else 0) for i in range(len(logits))]
    aus = []
    for e in exps:
        num = e * eins
        # Ganzzahldivision Richtung null, wie in Rust; die Zaehler sind
        # hier nie negativ, aber die Regel steht trotzdem da.
        q = abs(num) // abs(s) * (1 if (num >= 0) == (s > 0) else -1)
        r = num - q * s
        if abs(r) * 2 > abs(s) or (abs(r) * 2 == abs(s) and (q & 1) != 0):
            q += 1
        aus.append(q)
    return aus


def softmax_backward(g, p, frac):
    summe = rshift_round(sum(gi * pi for gi, pi in zip(g, p)), frac)
    return [clamp_i32(rshift_round((gi - summe) * pi, frac)) for gi, pi in zip(g, p)]


# --------------------------------------------------------------------------
# Die fuenf Rueckwaertskerne, die bisher kein Vektor deckte
# --------------------------------------------------------------------------

def attention_backward(g, q, k, v, p, maske, score_mult, score_mult_frac,
                       q_frac, k_frac, v_frac, prob_frac):
    """Nachbildung von `backward::attention_backward` (nach Fund 173).

    **Jede Groesse ist eine reale Groesse.** `dL/dp` traegt deshalb
    `v_frac` und nicht `prob_frac`, und die beiden Schiebeweiten fuer
    `dL/dq` und `dL/dk` sind **verschieden**: Im ersten ist der Faktor
    `k`, im zweiten `q`.
    """
    kv = len(k)
    hd = len(q)
    gv, gp = [], []
    for j in range(kv):
        if not maske[j]:
            gv.append([0] * hd)
            gp.append(0)
            continue
        gv.append([clamp_i32(rshift_round(g[d] * p[j], prob_frac)) for d in range(hd)])
        acc = sum(g[d] * v[j][d] for d in range(hd))
        gp.append(clamp_i32(rshift_round(acc, v_frac)))

    gs = softmax_backward(gp, p, prob_frac)

    gq_shift = k_frac + score_mult_frac
    gk_shift = q_frac + score_mult_frac
    gq_acc = [0] * hd
    gk = []
    for j in range(kv):
        if not maske[j] or gs[j] == 0:
            gk.append([0] * hd)
            continue
        zeile = []
        for d in range(hd):
            gq_acc[d] += gs[j] * k[j][d]
            zeile.append(clamp_i32(rshift_round(gs[j] * q[d] * score_mult, gk_shift)))
        gk.append(zeile)
    gq = [clamp_i32(rshift_round(a * score_mult, gq_shift)) for a in gq_acc]
    return gq, gk, gv


def silu_backward(g, x, grad_lut, x_frac, lut_in_frac, lut_offset,
                  lut_out_frac, g_frac, out_frac):
    aus = []
    for gi, xi in zip(g, x):
        dom = rescale(xi, x_frac, lut_in_frac)
        ableitung = lut_lookup(clamp_i16(dom), grad_lut, 0, lut_offset)
        aus.append(clamp_i32(rescale(gi * ableitung, g_frac + lut_out_frac, out_frac)))
    return aus


def rmsnorm_backward(g, x, x_shifts, gamma, gamma_shifts, r, norm_frac,
                     ref_shift, inv_n_q20, g_frac, gx_frac):
    """Nachbildung von `backward::rmsnorm_backward` (nach Fund 175).

    **Jede Groesse ist real.** `r` traegt `norm_frac − ref_shift`
    Bruchstellen gegenueber dem realen Wert, und `x` geht mit seiner
    Kanalskala ein, im ersten wie im zweiten Term.
    """
    schutz = 24
    n = len(x)
    rf = norm_frac - ref_shift
    ref_summe = max(gamma_shifts[i] + x_shifts[i] for i in range(n))
    s_acc = 0
    for i in range(n):
        eigen = gamma_shifts[i] + x_shifts[i]
        s_acc += (g[i] * gamma[i] * x[i]) << (ref_summe - eigen)
    summe_fein = verschiebe(s_acc, ref_summe - schutz)

    ggamma = []
    for i in range(n):
        ggamma.append(verschiebe(g[i] * x[i] * r, x_shifts[i] + rf))

    r2f = verschiebe(r * r, rf - schutz)
    r3f = verschiebe(r2f * r, rf)
    gx = []
    for j in range(n):
        t1 = verschiebe(g[j] * gamma[j] * r, gamma_shifts[j] + rf)
        mit_x = verschiebe(r3f * x[j], x_shifts[j])
        mit_n = (mit_x * inv_n_q20) >> 20
        t2 = verschiebe(mit_n * summe_fein, rf + 2 * schutz)
        gx.append(clamp_i32(rescale(t1 - t2, g_frac, gx_frac)))
    return gx, ggamma


def embedding_backward(ziel, hidden, token_id, g):
    start = token_id * hidden
    for d, gd in enumerate(g):
        ziel[start + d] += gd
    return ziel


# --------------------------------------------------------------------------
# Der Optimierer
# --------------------------------------------------------------------------

FEIN_BITS = 20


def splitmix64(state: int):
    state = (state + 0x9E3779B97F4A7C15) & MASK64
    z = state
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK64
    z ^= z >> 31
    return state, z & MASK64


def wuerfel(ebene: int, schritt: int, index: int) -> int:
    def rotl(v, n):
        return ((v << n) | (v >> (64 - n))) & MASK64
    s, _ = splitmix64(ebene)
    s, _ = splitmix64(s ^ rotl(schritt, 17))
    _, z = splitmix64(s ^ rotl(index, 41))
    return z


def runde_stochastisch(fein: int, wurf: int) -> int:
    stufe = 1 << FEIN_BITS
    ganz = fein // stufe          # Python floor-divide == div_euclid fuer positive Stufe
    rest = fein - ganz * stufe
    schwelle = wurf & (stufe - 1)
    return ganz + 1 if schwelle < rest else ganz


def schritt(master, grad, ebene, nr, versatz, lr_z, lr_n):
    aus = []
    for i, (w, g) in enumerate(zip(master, grad)):
        index = versatz + i
        # ⚑ **Rust trunkiert zur Null, Python rundet ab.** Bei einem
        # negativen Zaehler sind das verschiedene Zahlen, und der
        # Vertrag ist Rusts Trunkierung.
        zaehler = -g * lr_z * (1 << FEIN_BITS)
        betrag = abs(zaehler) // lr_n
        fein = betrag if zaehler >= 0 else -betrag
        stufen = runde_stochastisch(fein, wuerfel(ebene, nr, index))
        aus.append(max(-2147483648, min(2147483647, w + stufen)))
    return aus


# --------------------------------------------------------------------------

def schreiben(name: str, metadata: dict, inputs: dict, outputs: dict):
    gv = {
        "name": name,
        "level": "training",
        "theta_v_hash": theta_v_hash(),
        # ⚑ **Die Herkunft steht im Vektor, nicht im Kommentar.** Ein
        # selbsterzeugter Vektor belegt Determinismus, kein
        # unabhaengiger Vektor belegt Richtigkeit. Wer die beiden
        # verwechselt, haelt eine Selbstzertifizierung fuer einen Beleg.
        "herkunft": "unabhaengig",
        "metadata": metadata,
        "inputs": inputs,
        "outputs": outputs,
    }
    ZIEL.mkdir(parents=True, exist_ok=True)
    pfad = ZIEL / f"{name}.golden.json"
    pfad.write_text(json.dumps(gv, indent=2) + "\n", encoding="utf-8")
    print(f"  {pfad.relative_to(REPO)}")


def main() -> int:
    print("Golden Vectors des Trainingspfades:")
    geschrieben = 0

    # --- attention ------------------------------------------------------
    q_frac, k_frac, v_frac, prob_frac, smf = 8, 7, 10, 14, 15
    hd, kv = 4, 3
    q = [300, -180, 90, 240]
    k = [[120, 60, -40, 200], [-90, 150, 220, 30], [200, -20, 70, -110]]
    v = [[1000, -400, 250, 700], [-600, 900, -120, 340], [50, 80, -900, 120]]
    p = [8608, 1974, 5802]
    assert sum(p) == 1 << prob_frac, "p muss sich zu 2^prob_frac summieren"
    maske = [True, True, True]
    g = [3 << v_frac, -2 << v_frac, 5 << v_frac, 1 << v_frac]
    score_mult = round(math.isqrt((1 << 30) // hd))
    gq, gk, gv = attention_backward(
        g, q, k, v, p, maske, score_mult, smf, q_frac, k_frac, v_frac, prob_frac)
    schreiben(
        "backward_attention",
        {"head_dim": hd, "kv_len": kv, "score_mult": score_mult,
         "score_mult_frac": smf, "q_frac": q_frac, "k_frac": k_frac,
         "v_frac": v_frac, "prob_frac": prob_frac, "maske": maske},
        {"g": tensor(g, "int32"), "q": tensor(q, "int16"),
         "k": tensor([w for zeile in k for w in zeile], "int16"),
         "v": tensor([w for zeile in v for w in zeile], "int16"),
         "p": tensor(p, "int32")},
        {"gq": tensor(gq, "int32"),
         "gk": tensor([w for zeile in gk for w in zeile], "int32"),
         "gv": tensor([w for zeile in gv for w in zeile], "int32")},
    )
    geschrieben += 1

    # --- silu -----------------------------------------------------------
    lut_in, lut_out, offset = 6, 12, 256
    grad_lut = []
    for i in range(513):
        xr = (i - offset) / (1 << lut_in)
        sig = 1.0 / (1.0 + math.exp(-xr))
        ab = sig * (1.0 + xr * (1.0 - sig))
        grad_lut.append(max(-32768, min(32767, round(ab * (1 << lut_out)))))
    x = [-400, -120, -8, 0, 15, 96, 350, 900]
    g = [7, -3, 11, 5, -9, 2, 13, -6]
    aus = silu_backward(g, x, grad_lut, 8, lut_in, offset, lut_out, 8, 8)
    schreiben(
        "backward_silu",
        {"x_frac": 8, "lut_in_frac": lut_in, "lut_offset": offset,
         "lut_out_frac": lut_out, "g_frac": 8, "out_frac": 8},
        {"g": tensor(g, "int32"), "x": tensor(x, "int16"),
         "grad_lut": tensor(grad_lut, "int16")},
        {"gx": tensor(aus, "int32")},
    )
    geschrieben += 1

    # --- rmsnorm --------------------------------------------------------
    n = 8
    x = [300, -180, 90, 240, -60, 410, -320, 150]
    xs = [6, 6, 7, 6, 8, 6, 5, 7]
    gamma = [64, -40, 90, 32, -70, 55, 20, -100]
    gs = [5, 6, 7, 5, 6, 7, 5, 6]
    inv_n = round((1 << 20) / n)
    b = 12
    g = [c << b for c in (3, -2, 5, 1, -4, 2, -6, -1)]
    r, norm_frac, ref_shift = 106, 17, 8
    gx, ggamma = rmsnorm_backward(g, x, xs, gamma, gs, r, norm_frac, ref_shift, inv_n, b, b)
    schreiben(
        "backward_rmsnorm",
        {"r": r, "norm_frac": norm_frac, "ref_shift": ref_shift,
         "inv_n_q20": inv_n, "g_frac": b, "gx_frac": b},
        {"g": tensor(g, "int32"), "x": tensor(x, "int16"),
         "x_shifts": tensor(xs, "int32"), "gamma": tensor(gamma, "int8"),
         "gamma_shifts": tensor(gs, "int32")},
        {"gx": tensor(gx, "int32"), "ggamma": tensor(ggamma, "int64")},
    )
    geschrieben += 1

    # --- embedding ------------------------------------------------------
    hidden, vocab = 4, 5
    ziel = [0] * (hidden * vocab)
    # ⚑ Zweimal dasselbe Token: Der Gradient muss sich addieren.
    embedding_backward(ziel, hidden, 2, [7, -3, 11, 5])
    embedding_backward(ziel, hidden, 2, [1, 1, -1, -1])
    embedding_backward(ziel, hidden, 0, [4, 4, 4, 4])
    schreiben(
        "backward_embedding",
        {"hidden": hidden, "vocab_size": vocab, "token_ids": [2, 2, 0]},
        {"g0": tensor([7, -3, 11, 5], "int32"), "g1": tensor([1, 1, -1, -1], "int32"),
         "g2": tensor([4, 4, 4, 4], "int32")},
        {"ziel": tensor(ziel, "int64")},
    )
    geschrieben += 1

    # --- optimierer -----------------------------------------------------
    master = [1280, -640, 320, 0, -1280, 96, -32, 4096]
    grad = [1500, -700, 40, 900, -3, 1_000_000, 7, -250_000]
    ebene, nr, versatz = 3, 17, 64
    lr_z, lr_n = 1, 1000
    neu = schritt(master, grad, ebene, nr, versatz, lr_z, lr_n)
    wuerfe = [wuerfel(ebene, nr, versatz + i) for i in range(len(master))]
    schreiben(
        "optimierer_schritt",
        {"ebene": ebene, "schritt": nr, "index_versatz": versatz,
         "lr_zaehler": lr_z, "lr_nenner": lr_n, "fein_bits": FEIN_BITS},
        {"master": tensor(master, "int32"), "grad": tensor(grad, "int32")},
        # ⚑ Die Wuerfe stehen mit im Vektor. Wer nur das Ergebnis
        # vergleicht, sieht nicht, ob der Zufall oder die Rundung abwich.
        {"master_neu": tensor(neu, "int32"),
         "wuerfe": tensor([w - (1 << 64) if w >= (1 << 63) else w for w in wuerfe], "int64")},
    )
    geschrieben += 1

    # --- softmax ueber ein Vokabular (Fund 177) -------------------------
    # ⚑ Klein gehalten, aber jeder Zweig kommt vor: die
    # Maximumsubtraktion, die Verschiebung von der Logit- auf die
    # Tabellenskala, ein Index JENSEITS der Tabelle (der auf null faellt),
    # und die kaufmaennische Rundung der Division.
    exp_input_frac, logit_frac, frac_bits = 4, 10, 20
    exp_lut = [round(math.exp(-i / (1 << exp_input_frac)) * (1 << 14)) for i in range(64)]
    logits = [0, 512, -512, 1024, 2048, -4096, 3, 1023, -1, 256, -20000, 900]
    p = softmax_ueber_vokabular(logits, logit_frac, exp_lut, exp_input_frac, frac_bits)
    assert p[10] == 0, "der Eintrag jenseits der Tabelle muss auf null fallen"
    schreiben(
        "softmax_vokabular",
        {"logit_frac": logit_frac, "exp_input_frac": exp_input_frac,
         "frac_bits": frac_bits},
        {"logits": tensor(logits, "int32"), "exp_lut": tensor(exp_lut, "int16")},
        {"p": tensor(p, "int32")},
    )
    geschrieben += 1

    print(f"\n{geschrieben} Vektoren geschrieben. Pruefen mit:")
    print("    cd TESTCLIENT/myl-testclient && cargo run -- konformitaet")
    return 0


if __name__ == "__main__":
    sys.exit(main())
