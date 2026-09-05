#!/usr/bin/env python3
"""Erzeugt die Golden Vectors des MoE-Routingpfades (Umfang `moe`).

## ⚑ Warum es diese Datei gibt (Fund 180, 2026-09-05)

`route_top_k` und `mische_experten` waren **nirgends** gegen ein festes
Soll geprueft: nicht unter `op`, nicht unter `training`, nicht unter
`layer` (die Layer-Vektoren gehoeren zum dichten 0,5B-Modell).

**Das ist der Pfad, der entscheidet, WELCHE Experten rechnen.** Zwei
Knoten, die verschieden routen, rechnen verschiedene Netze; der
Redundanzvergleich meldete beide als fehlerhaft, ohne dass einer gelogen
haette. Dieselbe Klasse wie die drei Rueckwaertskerne ohne Aufrufer
(Funde 173 bis 175): tragend und unbelegt.

Der Anlass ist eine Entscheidung ueber das Primaermodell: Wird es ein
Expertengemisch, ist dieser Pfad der Konsenspfad.

## Warum ein eigener Umfang

Aus demselben Grund wie bei `training`: `894d8357ae92b5c1` steht an
sechs Stellen des Repositoriums fest, darunter beide CI-Laeufe. Neue
Vektoren unter `op` haetten ihn geaendert, also eine bestehende Zusage
gebrochen, um eine neue aufzustellen.

## Warum in Python

**Ein Vektor, den der zu pruefende Code selbst erzeugt, prueft nichts.**
Die Sollwerte entstehen hier aus einer UNABHAENGIGEN Nachbildung.

Usage:
    cd INTEGER_LLM
    python tools/golden_moe.py
"""
import hashlib
import json
import math
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ZIEL = REPO / "conformance" / "vectors" / "moe"
SPEC = REPO / "theta_v" / "spec.json"


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


def schreiben(name: str, metadata: dict, inputs: dict, outputs: dict):
    gv = {
        "name": name,
        "level": "moe",
        "theta_v_hash": theta_v_hash(),
        "herkunft": "unabhaengig",
        "metadata": metadata,
        "inputs": inputs,
        "outputs": outputs,
    }
    ZIEL.mkdir(parents=True, exist_ok=True)
    pfad = ZIEL / f"{name}.golden.json"
    pfad.write_text(json.dumps(gv, indent=2) + "\n", encoding="utf-8")
    print(f"  {pfad.relative_to(REPO)}")


# --------------------------------------------------------------------------
# Die Nachbildung
# --------------------------------------------------------------------------

def softmax_int(logits, exp_lut, lut_shift, frac_bits):
    """Nachbildung von `softmax::softmax_int`."""
    eins = 1 << frac_bits
    lut_eins = exp_lut[0]
    m = max(logits)
    exps = []
    for z in logits:
        d = m - z
        if d <= 0:
            exps.append(lut_eins)
        else:
            i = d >> lut_shift
            exps.append(exp_lut[i] if i < len(exp_lut) else 0)
    s = sum(exps)
    if s == 0:
        basis, rest = divmod(eins, len(exps))
        return [basis + (1 if i < rest else 0) for i in range(len(exps))]
    aus = []
    for e in exps:
        num = e * eins
        q = abs(num) // abs(s) * (1 if (num >= 0) == (s > 0) else -1)
        r = num - q * s
        if abs(r) * 2 > abs(s) or (abs(r) * 2 == abs(s) and (q & 1) != 0):
            q += 1
        aus.append(q)
    return aus


def waehle_top_k(logits, k):
    """Die k besten nach (Logit absteigend, Index aufsteigend).

    ⚑ **Bewusst k-mal das Maximum und keine Sortierung.** Ein
    `sort` ueber einen Vergleich, der nur den Logit ansieht, ordnet
    gleiche Elemente bibliotheksabhaengig an, und auf einer einzelnen
    Maschine faellt das nie auf.
    """
    genommen = [False] * len(logits)
    aus = []
    for _ in range(min(k, len(logits))):
        best = -1
        for i, z in enumerate(logits):
            if genommen[i]:
                continue
            if best < 0 or z > logits[best] or (z == logits[best] and i < best):
                best = i
        genommen[best] = True
        aus.append(best)
    return aus


def route_top_k(logits, k, exp_lut, lut_shift, frac_bits, normieren):
    """Nachbildung von `moe::route_top_k`, samt Boden (Fund 79)."""
    experten = waehle_top_k(logits, k)
    if normieren:
        gewaehlte = [logits[e] for e in experten]
        w = softmax_int(gewaehlte, exp_lut, lut_shift, frac_bits)
        # ⚑ Der Boden: kein Gewicht auf null, keines traegt alles.
        w = [max(v, 1) for v in w]
        # Summenkorrektur: der Rest geht an das groesste, bei Gleichstand
        # an den kleineren Index.
        soll = 1 << frac_bits
        rest = soll - sum(w)
        if rest != 0:
            g = 0
            for i, v in enumerate(w):
                if v > w[g]:
                    g = i
            w[g] += rest
    else:
        alle = softmax_int(logits, exp_lut, lut_shift, frac_bits)
        w = [alle[e] for e in experten]
    return experten, w


def mische_experten(ausgaben, gewichte, frac_bits):
    """Nachbildung von `moe::mische_experten`: i64-Summe, einmal geschoben."""
    n = len(ausgaben[0])
    aus = []
    for j in range(n):
        summe = 0
        for a, w in zip(ausgaben, gewichte):
            summe += a[j] * w
        # Kaufmaennisch zur geraden Zahl, wie `rshift_round_i64`.
        s = frac_bits
        mask = (1 << s) - 1
        half = 1 << (s - 1)
        q = summe >> s
        r = summe & mask
        if r > half or (r == half and (q & 1) != 0):
            q += 1
        aus.append(max(-32768, min(32767, q)))
    return aus


def main() -> int:
    print("Golden Vectors des MoE-Routingpfades:")
    geschrieben = 0
    frac = 8
    # Dieselbe kleine Tabelle wie die Tests des Kerns: exp(-i) auf 2^8.
    exp_lut = [round(math.exp(-i) * 256) for i in range(65)]

    # --- Routing mit Normierung, samt Boden ---------------------------
    # ⚑ Der Aufbau ist so gewaehlt, dass er OHNE Boden saettigen wuerde:
    # Rang 2 liegt 70 Einheiten unter Rang 1, die Tabelle liefert dort
    # null. Mit Boden steht dort eine Eins, und der Router lebt.
    logits = [0, -70, -200, -5, -400, -12]
    e, w = route_top_k(logits, 3, exp_lut, 0, frac, True)
    assert min(w) >= 1, w
    assert sum(w) == 1 << frac, w
    schreiben(
        "routing_normiert",
        {"k": 3, "frac_bits": frac, "lut_shift": 0, "normieren": True},
        {"logits": tensor(logits, "int32"), "exp_lut": tensor(exp_lut, "int16")},
        {"experten": tensor(e, "int32"), "gewichte": tensor(w, "int32")},
    )
    geschrieben += 1

    # --- Routing ohne Normierung --------------------------------------
    e2, w2 = route_top_k(logits, 3, exp_lut, 0, frac, False)
    schreiben(
        "routing_unnormiert",
        {"k": 3, "frac_bits": frac, "lut_shift": 0, "normieren": False},
        {"logits": tensor(logits, "int32"), "exp_lut": tensor(exp_lut, "int16")},
        {"experten": tensor(e2, "int32"), "gewichte": tensor(w2, "int32")},
    )
    geschrieben += 1

    # --- Gleichstand an der Auswahlgrenze -----------------------------
    # ⚑ Genau der Fall, fuer den die Tie-Break-Regel da ist: Rang k und
    # k+1 tragen denselben Logit, und nur die Regel entscheidet, welcher
    # rechnet.
    gleich = [100, 50, 50, 10]
    e3, w3 = route_top_k(gleich, 2, exp_lut, 0, frac, True)
    assert e3 == [0, 1], e3
    schreiben(
        "routing_gleichstand",
        {"k": 2, "frac_bits": frac, "lut_shift": 0, "normieren": True},
        {"logits": tensor(gleich, "int32"), "exp_lut": tensor(exp_lut, "int16")},
        {"experten": tensor(e3, "int32"), "gewichte": tensor(w3, "int32")},
    )
    geschrieben += 1

    # --- Die Mischung -------------------------------------------------
    ausgaben = [[1000, -400, 250, 700], [-600, 900, -120, 340], [50, 80, -900, 120]]
    gemischt = mische_experten(ausgaben, w, frac)
    schreiben(
        "mischung",
        {"frac_bits": frac, "experten": 3, "breite": 4},
        {
            "a0": tensor(ausgaben[0], "int16"),
            "a1": tensor(ausgaben[1], "int16"),
            "a2": tensor(ausgaben[2], "int16"),
            "gewichte": tensor(w, "int32"),
        },
        {"y": tensor(gemischt, "int16")},
    )
    geschrieben += 1

    print(f"\n{geschrieben} Vektoren geschrieben. Pruefen mit:")
    print("    cd TESTCLIENT/myl-testclient && cargo run -- konformitaet")
    return 0


if __name__ == "__main__":
    sys.exit(main())
