#!/usr/bin/env python3
"""Die Zustandsschicht ganzzahlig, gegen die echte Fremdschicht gemessen.

## ⚑ Was hier isoliert wird, und warum

Die Schicht besteht aus sieben Stuecken, von denen sechs im Projekt
schon vorkommen (Projektionen, Faltung, SiLU, Sigmoid, Softplus,
torgesteuerte Norm). **Neu ist allein die Rekurrenz**, und neu ist die
Frage, ob ein ganzzahliger Zustand ihr ueber eine Folge folgen kann.

⚑ **Deshalb laeuft Stufe 1 mit Gleitkomma-Projektionen und
ganzzahliger Rekurrenz.** Sie misst den Fehler, der aus dem Zustand
kommt, und nur diesen. Wer alles auf einmal quantisiert und einen
Abstand sieht, weiss nicht, woher er kommt.

⚠️ **Das ist kein Rechenpfad, sondern eine Messung.** Der Rechenpfad ist
ganzzahlig; diese Datei darf Gleitkomma benutzen, weil sie die
**Referenz** stellt, gegen die er geprueft wird.

## ⛔️ Was die Vorlage tut und der erste Entwurf nicht hatte

`q` und `k` werden je Kopf auf Einheitslaenge gebracht (`l2norm`), und
`q` wird zusaetzlich mit `1/sqrt(kopf_dim)` skaliert. Beides steht hier
drin, weil ohne die Normierung eine andere Schicht gemessen wuerde.

Aufruf:
    python3 zustandsschicht_referenz.py [--laenge 64] [--ebene 0]
"""
import argparse
import json
import math
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
QUELLE = REPO / "MODELS/llm/Qwen3.6-35B-A3B"

def breiten_aus_dem_kernel():
    """Die Bruchbits, **aus dem Kernel gelesen** statt hier wiederholt.

    ⛔️ **Hier standen sie bis zum 2026-09-21 als eigene Zahlen**, und
    damit stand dieselbe Angabe an zwei Orten. Wer eine Breite im Kernel
    aendert, aendert sie nicht hier, und dann misst diese Datei
    stillschweigend etwas anderes, als der Rechenpfad rechnet.
    📌 **Was an zwei Orten steht, laeuft auseinander, und der zweite Ort
    meldet sich nicht.**
    """
    quelle = REPO / "INTEGER_LLM/kernels/src/zustandsschicht.rs"
    text = quelle.read_text(encoding="utf-8")
    werte = {}
    for name in ("WERT_FRAC", "NORM_FRAC", "ZUSTAND_FRAC", "ZERFALL_FRAC"):
        marke = f"pub const {name}: u32 = "
        i = text.index(marke) + len(marke)
        werte[name] = int(text[i:text.index(";", i)])
    return werte


_B = breiten_aus_dem_kernel()
WERT_FRAC = _B["WERT_FRAC"]
NORM_FRAC = _B["NORM_FRAC"]
ZUSTAND_FRAC = _B["ZUSTAND_FRAC"]
ZERFALL_FRAC = _B["ZERFALL_FRAC"]


def rshift_round(wert: int, schiebung: int) -> int:
    """Rechtsshift mit Runden zur naechsten GERADEN Zahl (Regel 6)."""
    if schiebung <= 0:
        return wert << (-schiebung)
    rest = wert & ((1 << schiebung) - 1)
    ab = wert >> schiebung
    halb = 1 << (schiebung - 1)
    if rest > halb or (rest == halb and (ab & 1)):
        ab += 1
    return ab


def isqrt_round(n: int) -> int:
    """Ganzzahlige Wurzel, zur naechsten ganzen Zahl gerundet.

    ⚑ Dasselbe Verfahren wie `fixed_point::isqrt_round` in den Kernen,
    damit die Messung nicht gegen eine andere Wurzel prueft als der
    Rechenpfad spaeter rechnet.
    """
    if n <= 0:
        return 0
    r = math.isqrt(n)
    return r + 1 if (r + 1) * (r + 1) - n < n - r * r else r


def l2norm_ganzzahlig(x_int, aus_frac):
    """`x / ||x||`, ganzzahlig, Ergebnis in Einheiten von `2^-aus_frac`.

    ⚑ **Ohne Division.** `||x||` kommt als ganze Zahl in denselben
    Einheiten wie `x`; der Quotient ist dimensionslos und wird als
    `(x << aus_frac + s) / norm` gebildet, wobei die Division hier
    **einmal** als ganzzahlige Division steht, weil sie der Kernel
    spaeter ueber eine Kehrwurzeltabelle ersetzt.

    ⚠️ **Diese Datei misst die Rekurrenz, nicht die Tabelle.** Die
    Tabellenfassung kommt, wenn der Kernel sie bekommt; ein Unterschied
    zwischen beiden waere dann ein eigener Befund.
    """
    summe = sum(v * v for v in x_int)
    if summe == 0:
        return [0] * len(x_int)
    norm = isqrt_round(summe)
    if norm == 0:
        return [0] * len(x_int)
    return [int(round((v << aus_frac) / norm)) for v in x_int]


def rekurrenz_ganzzahlig(q, k, v, g, beta, s_frac, zerfall_frac, wert_frac,
                         norm_frac):
    """Die Rekurrenz, ganzzahlig, wie der Kernel sie rechnet.

    Alle Eingaben sind Listen je Schritt; `q` und `k` sind bereits
    normiert und liegen in `2^-wert_frac`, `g` in `2^-zerfall_frac`,
    `beta` in `2^-wert_frac`.
    """
    n = len(q)
    k_dim = len(k[0])
    v_dim = len(v[0])
    # Zeilenweise: S[a][b], a ueber die Schluessel, b ueber die Werte.
    S = [[0] * v_dim for _ in range(k_dim)]
    aus = []
    links = s_frac - norm_frac - wert_frac
    assert links >= 0, "s_frac unter norm_frac+wert_frac: Schritt 4 wuerde runden"
    # ⚑ `q` und `k` liegen in `2^-norm_frac`, der Zustand in `2^-s_frac`;
    #   das Produkt landet also `norm_frac` Stellen zu tief.
    lesen = s_frac + norm_frac - wert_frac
    for t in range(n):
        # 1. Zerfall.
        gt = g[t]
        for a in range(k_dim):
            zeile = S[a]
            for b in range(v_dim):
                zeile[b] = rshift_round(zeile[b] * gt, zerfall_frac)
        # 2. Lesen mit dem Schluessel.
        kt = k[t]
        kv = [0] * v_dim
        for b in range(v_dim):
            akku = 0
            for a in range(k_dim):
                akku += S[a][b] * kt[a]
            kv[b] = rshift_round(akku, lesen)
        # 3. Korrektur.
        bt = beta[t]
        delta = [rshift_round((v[t][b] - kv[b]) * bt, wert_frac) for b in range(v_dim)]
        # 4. Rang-1-Zuschlag.
        for a in range(k_dim):
            ka = kt[a]
            if ka == 0:
                continue
            zeile = S[a]
            for b in range(v_dim):
                zeile[b] += (ka * delta[b]) << links
        # 5. Lesen mit der Abfrage.
        qt = q[t]
        zeile_aus = [0] * v_dim
        for b in range(v_dim):
            akku = 0
            for a in range(k_dim):
                akku += S[a][b] * qt[a]
            zeile_aus[b] = rshift_round(akku, lesen)
        aus.append(zeile_aus)
    return aus


def vom_vollausschlag(abstand: float, groesste_ausgabe: float) -> float:
    """Der Abstand als Anteil der groessten Ausgabe, in Prozent.

    ⛔️ **Eine Zahl in „letzten Stellen" ist ohne den Vollausschlag keine
    Aussage.** Am 2026-09-21 sah die torgesteuerte Norm mit 4,50 Stellen
    achtmal schlechter aus als die Rekurrenz mit 0,55. Ihre Ausgabe ist
    aber auch zehnmal groesser: 0,18 Prozent gegen 0,21 Prozent, also
    **minimal besser**.

    📌 **Ein Massstab, der die Groesse des Gemessenen nicht kennt,
    vergleicht Aepfel mit Birnen und sieht dabei streng aus.**
    """
    if groesste_ausgabe <= 0:
        return 0.0
    return 100.0 * abstand / groesste_ausgabe


def pruefe_faltung(quelle, ebene, schritte=24, kanaele=64, artefakt=None):
    """Die kausale Faltung gegen die Fremdimplementierung.

    ⚑ **Tiefenweise, kausal, danach SiLU**, genau wie die Vorlage:
    `conv1d` mit `groups = kanaele` und `padding = KERN - 1`.

    ⚠️ **Geprueft wird die Arithmetik, nicht der Rust-Kern.** Dass der
    Rust-Kern dieselbe Rechnung macht, sichern seine eigenen Proben samt
    Mutationen; diese Datei sichert, dass **diese Rechnung** die richtige
    ist. Zwei verschiedene Fragen, zwei verschiedene Belege.
    """
    import torch
    import torch.nn.functional as F
    from safetensors import safe_open

    karte = json.loads((quelle / "model.safetensors.index.json")
                       .read_text(encoding="utf-8"))["weight_map"]
    treffer = [s for s in karte
               if s.endswith(f"layers.{ebene}.linear_attn.conv1d.weight")]
    assert treffer, f"Ebene {ebene} hat keine Faltung"
    with safe_open(str(quelle / karte[treffer[0]]), framework="pt") as f:
        w = f.get_tensor(treffer[0]).to(torch.float64)
    c_all, _, kern = w.shape
    kanaele = min(kanaele, c_all)
    print(f"[zref] Faltung: {c_all} Kanaele, Kern {kern}; geprueft werden "
          f"{kanaele} ueber {schritte} Schritte")

    # ⛔️ **Der Pruefeingang hatte bis zum 2026-09-21 die falsche
    # Groessenordnung.** Er kam aus `randn`, lag also bei etwa drei; der
    # echte Eingang ist der Ausgang von `in_proj_qkv`, und die
    # Aktivierungsstatistik misst dort **39,0** in Ebene 0. Die Probe lief
    # damit auf einem Zehntel der Amplitude, und ein relativer Massstab
    # sah deshalb schlecht aus, obwohl die Faltung nichts dafuer konnte.
    #
    # 📌 **Ein Pruefeingang ohne die gemessene Amplitude prueft eine
    # andere Schicht.**
    torch.manual_seed(20260921)
    spanne = 1.0
    if artefakt is not None:
        skalen_datei = artefakt / "scales.json"
        if skalen_datei.is_file():
            skalen = json.loads(skalen_datei.read_text(encoding="utf-8"))
            schluessel = f"model.layers.{ebene}.linear_attn.in_proj_qkv"
            if schluessel in skalen:
                spanne = float(skalen[schluessel]["absmax_observed"])
                print(f"[zref] Faltung: Eingangsamplitude aus der Messung: "
                      f"{spanne:.1f}")
    if spanne == 1.0:
        print("[zref] ⚠️ Faltung: keine gemessene Amplitude, es wird mit 1.0 "
              "gerechnet; das Urteil gilt dann nur fuer diese Amplitude")
    # Gleichverteilt bis zur gemessenen Amplitude, damit auch der Rand
    # vorkommt und nicht nur die Mitte einer Glocke.
    x = (torch.rand(schritte, kanaele, dtype=torch.float64) * 2 - 1) * spanne
    xt = x.T.unsqueeze(0)
    wk = w[:kanaele, 0, :].unsqueeze(1)
    ref = F.silu(F.conv1d(F.pad(xt, (kern - 1, 0)), wk, groups=kanaele))[0].T

    versatz = 1 << (WERT_FRAC + 4)
    lut = [max(-32768, min(32767,
               round((lambda v: v / (1 + math.exp(-v)))((i - versatz) / (1 << WERT_FRAC))
                     * (1 << WERT_FRAC))))
           for i in range(1 << 13)]

    def silu_nach(xi):
        idx = xi + versatz
        if idx < 0:
            return 0
        # ⚑ Oberhalb der Tabelle die Identitaet, wie `integer_math`.
        return lut[idx] if idx < len(lut) else xi

    w_int, w_schiebung = [], []
    for c in range(kanaele):
        m = float(w[c, 0].abs().max())
        s = 0 if m == 0 else max(0, min(31, int(math.floor(math.log2(127.0 / m)))))
        w_int.append([max(-128, min(127, int(round(float(v) * (1 << s)))))
                      for v in w[c, 0]])
        w_schiebung.append(s)

    # ⚑ **Der Boden: nur die Gewichte quantisiert, sonst alles exakt.**
    #
    # ⛔️ Eine erfundene Schwelle („unter einem Prozent") ist keine
    # Herleitung. Der Fehler dieser Stufe kommt fast ganz aus der
    # int8-Quantisierung der Gewichte, und die ist die Grundlage, auf der
    # dieses ganze Projekt rechnet. Die Frage ist deshalb nicht, ob der
    # Abstand klein ist, sondern ob **die Umsetzung etwas ueber das
    # Unvermeidliche hinaus hinzufuegt**.
    w_zurueck = torch.tensor(
        [[w_int[c][j] / (1 << w_schiebung[c]) for j in range(kern)]
         for c in range(kanaele)], dtype=torch.float64)
    boden_ref = F.silu(F.conv1d(F.pad(xt, (kern - 1, 0)),
                                w_zurueck.unsqueeze(1), groups=kanaele))[0].T
    boden = float((boden_ref - ref).abs().max()) * (1 << WERT_FRAC)

    fenster = [[0] * (kern - 1) for _ in range(kanaele)]
    schlimm = 0.0
    for t in range(schritte):
        ein = [int(round(float(x[t, c]) * (1 << WERT_FRAC))) for c in range(kanaele)]
        for c in range(kanaele):
            akku = sum(w_int[c][j] * fenster[c][j] for j in range(kern - 1))
            akku += w_int[c][kern - 1] * ein[c]
            roh = max(-32768, min(32767,
                                  rshift_round(akku, w_schiebung[c] + WERT_FRAC - WERT_FRAC)))
            schlimm = max(schlimm,
                          abs(silu_nach(roh) - float(ref[t, c]) * (1 << WERT_FRAC)))
            fenster[c] = fenster[c][1:] + [ein[c]]

    voll = float(ref.abs().max()) * (1 << WERT_FRAC)
    print(f"[zref] Faltung: Ausgabe bis {voll:.0f} Stellen von 2^-{WERT_FRAC}")
    print(f"[zref] Faltung: Boden aus der int8-Quantisierung der Gewichte: "
          f"{boden:.2f} Stellen ({vom_vollausschlag(boden, voll):.3f} Prozent)")
    print(f"[zref] Faltung: voller Ganzzahlpfad: {schlimm:.2f} Stellen "
          f"({vom_vollausschlag(schlimm, voll):.3f} Prozent)")

    # ⚑ Das Urteil vergleicht gegen den Boden, nicht gegen eine Zahl,
    #   die sich jemand ausgedacht hat.
    if boden <= 0:
        print("[zref] ⚠️ BEFUND: kein Boden messbar, das Urteil traegt nicht.")
        return 1
    ueber = schlimm / boden
    print(f"[zref] Faltung: das {ueber:.2f}-fache des Bodens")
    if ueber <= 1.5:
        print("[zref] ⚑ BEFUND: die Faltung fuegt nichts Nennenswertes ueber")
        print("        die int8-Gewichte hinaus hinzu. Sie traegt.")
        return 0
    print("[zref] ⚠️ BEFUND: die Umsetzung kostet deutlich mehr als die")
    print("        Gewichtsquantisierung allein. Da ist noch eine Stelle,")
    print("        die unnoetig rundet.")
    return 1


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--laengen", type=str, default="32,128,256",
                   help="Folgenlaengen, durch Komma getrennt. ⚑ MEHRERE, denn\n                        eine einzelne Laenge kann Wachstum nicht zeigen")
    p.add_argument("--ebene", type=int, default=0)
    p.add_argument("--artefakt", type=Path, default=None,
                   help="gebautes Artefakt; liefert die GEMESSENEN "
                        "Aktivierungsamplituden aus scales.json")
    p.add_argument("--teil", choices=("faltung", "rekurrenz", "alle"),
                   default="alle", help="welches Stueck der Schicht geprueft wird")
    p.add_argument("--koepfe", type=int, default=4,
                   help="wieviele Koepfe geprueft werden (32 waere langsam)")
    args = p.parse_args()
    laengen = sorted(int(x) for x in args.laengen.split(",") if x.strip())
    assert len(laengen) >= 2, "eine einzelne Laenge zeigt kein Wachstum"

    if args.teil in ("faltung", "alle"):
        rc = pruefe_faltung(QUELLE, args.ebene, artefakt=args.artefakt)
        if rc != 0 or args.teil == "faltung":
            return rc
        print()

    import torch
    import torch.nn.functional as F
    from safetensors import safe_open

    torch.manual_seed(20260921)
    cfg = json.loads((QUELLE / "config.json").read_text(encoding="utf-8"))
    tc = cfg.get("text_config", cfg)
    k_dim = tc["linear_key_head_dim"]
    v_dim = tc["linear_value_head_dim"]
    n_k = tc["linear_num_key_heads"]
    n_v = tc["linear_num_value_heads"]
    print(f"[zref] Ebene {args.ebene}, Laengen {laengen}, "
          f"{args.koepfe} von {n_v} Koepfen")
    print(f"[zref] k_dim={k_dim} v_dim={v_dim} n_k={n_k} n_v={n_v}")
    print(f"[zref] Breiten aus dem Kernel: wert={WERT_FRAC} norm={NORM_FRAC} "
          f"zustand={ZUSTAND_FRAC} zerfall={ZERFALL_FRAC}")

    # Die Rekurrenzparameter der Ebene, aus den echten Gewichten.
    karte = json.loads((QUELLE / "model.safetensors.index.json")
                       .read_text(encoding="utf-8"))["weight_map"]
    praefix = [s for s in karte if s.endswith(f"layers.{args.ebene}.linear_attn.A_log")]
    assert praefix, f"Ebene {args.ebene} hat keine Zustandsschicht"
    basis = praefix[0][: -len("A_log")]

    def hole(name):
        s = basis + name
        with safe_open(str(QUELLE / karte[s]), framework="pt") as f:
            return f.get_tensor(s).to(torch.float64)

    A_log = hole("A_log")
    dt_bias = hole("dt_bias")

    # ⚑ q, k, v, a, b als Zufallsgroessen in der GEMESSENEN Spanne.
    #   Die echten Aktivierungen braeuchten den ganzen Vorwaertspass;
    #   was hier zaehlt, ist die Rekurrenz bei realistischen Eingaengen.
    n = max(laengen)
    H = args.koepfe
    q_f = torch.randn(n, H, k_dim, dtype=torch.float64)
    k_f = torch.randn(n, H, k_dim, dtype=torch.float64)
    v_f = torch.randn(n, H, v_dim, dtype=torch.float64) * 0.5
    # `a` in der gemessenen Spanne dieser Ebene, `b` aehnlich.
    a_f = torch.randn(n, H, dtype=torch.float64) * 3.0
    b_f = torch.randn(n, H, dtype=torch.float64) * 2.0

    # --- Die Referenz, genau wie die Vorlage sie rechnet ---
    q_n = F.normalize(q_f, p=2.0, dim=-1, eps=1e-6)
    k_n = F.normalize(k_f, p=2.0, dim=-1, eps=1e-6)
    q_n = q_n / math.sqrt(k_dim)
    beta_f = torch.sigmoid(b_f)
    g_log = -torch.exp(A_log[:H].double()) * F.softplus(a_f + dt_bias[:H].double())
    g_f = torch.exp(g_log)
    print(f"[zref] g in [{float(g_f.min()):.6f}, {float(g_f.max()):.9f}], "
          f"beta in [{float(beta_f.min()):.4f}, {float(beta_f.max()):.4f}]")

    ref = torch.zeros(n, H, v_dim, dtype=torch.float64)
    for h in range(H):
        S = torch.zeros(k_dim, v_dim, dtype=torch.float64)
        for t in range(n):
            S = S * float(g_f[t, h])
            kv = (S * k_n[t, h].unsqueeze(-1)).sum(dim=-2)
            d = (v_f[t, h] - kv) * float(beta_f[t, h])
            S = S + k_n[t, h].unsqueeze(-1) * d.unsqueeze(-2)
            ref[t, h] = (S * q_n[t, h].unsqueeze(-1)).sum(dim=-2)

    # --- Der Ganzzahlpfad ---
    eins_w = 1 << WERT_FRAC
    eins_g = 1 << ZERFALL_FRAC
    je_laenge = {l: 0.0 for l in laengen}
    for h in range(H):
        # Normieren geschieht GANZZAHLIG, aus den rohen q und k.
        q_i, k_i = [], []
        for t in range(n):
            q_roh = [int(round(float(x) * eins_w)) for x in q_f[t, h]]
            k_roh = [int(round(float(x) * eins_w)) for x in k_f[t, h]]
            # ⚑ `q` traegt zusaetzlich 1/sqrt(k_dim); das steckt in der
            #   Ausgangsauflösung, damit kein zweiter Rundungsschritt
            #   entsteht.
            qn = l2norm_ganzzahlig(q_roh, NORM_FRAC)
            skala = isqrt_round((1 << (2 * NORM_FRAC)) // k_dim)
            qn = [rshift_round(x * skala, NORM_FRAC) for x in qn]
            q_i.append(qn)
            k_i.append(l2norm_ganzzahlig(k_roh, NORM_FRAC))
        v_i = [[int(round(float(x) * eins_w)) for x in v_f[t, h]] for t in range(n)]
        g_i = [int(round(float(g_f[t, h]) * eins_g)) for t in range(n)]
        b_i = [int(round(float(beta_f[t, h]) * eins_w)) for t in range(n)]

        aus = rekurrenz_ganzzahlig(q_i, k_i, v_i, g_i, b_i,
                                   ZUSTAND_FRAC, ZERFALL_FRAC, WERT_FRAC,
                                   NORM_FRAC)
        # ⚑ Je Laenge der groesste Abstand ueber die ersten `l` Schritte.
        #   Ein Lauf, viele Messpunkte: Der Zustand bei Schritt l haengt
        #   nur an den Schritten davor.
        for l in laengen:
            for t in range(l):
                for b in range(v_dim):
                    soll = float(ref[t, h, b]) * eins_w
                    je_laenge[l] = max(je_laenge[l], abs(aus[t][b] - soll))

    print()
    print(f"{'Laenge':>8} {'groesster Abstand':>20}")
    print("-" * 29)
    for l in laengen:
        print(f"{l:>8} {je_laenge[l]:>20.2f}")
    voll = float(ref.abs().max()) * eins_w
    print(f"\nin Stellen von 2^-{WERT_FRAC}, bei einer Ausgabe bis "
          f"{voll:.0f} Stellen.")
    print(f"Der groesste Abstand sind {vom_vollausschlag(je_laenge[laengen[-1]], voll):.3f} "
          f"Prozent vom Vollausschlag.")
    print()

    # ⛔️ **Das Urteil liest die Tabelle**, statt eine einzelne Zahl gegen
    # eine Schwelle zu halten. Eine Schwellenprobe bei einer Laenge kann
    # Wachstum nicht sehen, und Wachstum ist hier die ganze Frage.
    erster, letzter = je_laenge[laengen[0]], je_laenge[laengen[-1]]
    faktor_laenge = laengen[-1] / laengen[0]

    # ⛔️ **Der beste Fall darf nicht als der schlimmste herauskommen.**
    # Ein erster Wert von null ergibt beim Teilen `inf`, und damit meldete
    # diese Stelle am 2026-09-21 fuer einen **bitgleichen** Lauf
    # „waechst mit der Laenge". 📌 **Ein Verhaeltnis braucht einen
    # Nenner**, und wo keiner ist, gehoert der Fall vorher abgefangen.
    if letzter == 0:
        print(f"[zref] ⚑ BEFUND: bitgleich zur breiten Fassung bei jeder "
              f"gemessenen Laenge (bis {laengen[-1]}).")
        return 0
    if erster == 0:
        print(f"[zref] ⚠️ BEFUND: bei Laenge {laengen[0]} bitgleich, bei "
              f"{laengen[-1]} nicht mehr ({letzter:.2f} Stellen).")
        print("        Ein Wachstum aus der Null heraus laesst sich nicht als")
        print("        Verhaeltnis ausdruecken; die Reihe gehoert verlaengert.")
        return 1

    faktor_fehler = letzter / erster
    print(f"[zref] {faktor_laenge:.0f}-fache Laenge kostet das "
          f"{faktor_fehler:.2f}-fache an Abstand.")
    if faktor_fehler >= faktor_laenge * 0.5:
        print("[zref] ⛔️ BEFUND: der Abstand waechst etwa MIT der Laenge.")
        print("        Damit saettigt er nicht, und die Breiten tragen keine")
        print("        lange Folge. Der Zerfall daempft aeltere Fehler nicht")
        print("        genug, oder eine Stelle rundet in eine Richtung.")
        return 1
    grenze = 2.0
    if letzter > grenze:
        print(f"[zref] ⚠️ BEFUND: der Abstand saettigt, liegt aber mit "
              f"{letzter:.2f} ueber {grenze} Stellen.")
        return 1
    print(f"[zref] ⚑ BEFUND: der Abstand waechst deutlich UNTERlinear und")
    print(f"        bleibt mit {letzter:.2f} unter {grenze} Stellen.")
    print("        ⚠️ **Hochgerechnet und nicht gemessen:** Bei diesem")
    print("        Wachstum bliebe er auch ueber das volle Fenster unter")
    print("        einer Stelle. Die volle Laenge ist hier nicht zu")
    print("        rechnen; sie gehoert in den Rust-Pfad.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
