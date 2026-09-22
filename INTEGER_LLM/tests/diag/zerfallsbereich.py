#!/usr/bin/env python3
"""Der Zerfallsbereich der Zustandsschicht, an echten Gewichten gemessen.

⚑ **Wofuer.** Die Zustandsschicht zerfaellt je Schritt um

    g = exp(-exp(A_log) * softplus(a + dt_bias))

und der Zustand wird ueber N Schritte mit g multipliziert. Wie viele
Nachkommastellen g braucht, haengt daran, **wie nah g an 1 liegt**: Ein
absoluter Fehler eps in g wird ueber N Schritte zu einem relativen
Fehler von rund N*eps/g. Diese Datei misst, wie nah g wirklich an 1
kommt, statt es anzunehmen.

⚠️ **Die Spanne von `a` kommt aus der Messung, nicht aus einer
Schaetzung.** `scales.json` des gebauten Artefakts haelt je Ebene den
beobachteten Betragsgrossstwert des AUSGANGS von `in_proj_a`, gesammelt
ueber den ganzen Kalibrierkorpus (stats.py hakt den Ausgang ein,
`take_input=False`).

Aufruf:
    python3 zerfallsbereich.py <artefakt-verzeichnis> [<quell-verzeichnis>]
"""
import json
import math
import os
import sys

# Die Auflösung der Werte im Zustand. Der Zerfall darf ueber die ganze
# Folge nicht mehr Fehler machen, als eine Wertstelle ohnehin traegt.
WERT_FRAC = 8


def lade_a_spannen(artefakt):
    """Je Ebene der gemessene Betragsgrosstwert des `in_proj_a`-Ausgangs."""
    with open(os.path.join(artefakt, "scales.json"), encoding="utf-8") as f:
        skalen = json.load(f)
    spannen = {}
    for name, eintrag in skalen.items():
        if name.endswith(".linear_attn.in_proj_a"):
            ebene = int(name.split(".layers.")[1].split(".")[0])
            spannen[ebene] = float(eintrag["absmax_observed"])
    return spannen


def lade_rekurrenzgewichte(quelle):
    """`A_log` und `dt_bias` je Ebene, aus den Quellgewichten."""
    from safetensors import safe_open
    with open(os.path.join(quelle, "model.safetensors.index.json"),
              encoding="utf-8") as f:
        karte = json.load(f)["weight_map"]

    nach_datei = {}
    for schluessel, datei in karte.items():
        if schluessel.endswith(".linear_attn.A_log") or \
                schluessel.endswith(".linear_attn.dt_bias"):
            nach_datei.setdefault(datei, []).append(schluessel)

    werte = {}
    for datei, schluessel in nach_datei.items():
        with safe_open(os.path.join(quelle, datei), framework="pt") as f:
            for s in schluessel:
                ebene = int(s.split(".layers.")[1].split(".")[0])
                werte.setdefault(ebene, {})[s.rsplit(".", 1)[1]] = \
                    f.get_tensor(s).float()
    return werte


def softplus(x):
    # Stabil fuer grosse x, wie die Referenz.
    return x if x > 30.0 else math.log1p(math.exp(x))


def main():
    artefakt = sys.argv[1]
    quelle = sys.argv[2] if len(sys.argv) > 2 else \
        os.path.join(os.path.dirname(os.path.abspath(__file__)),
                     "..", "..", "..", "MODELS", "llm", "Qwen3.6-35B-A3B")

    spannen = lade_a_spannen(artefakt)
    gewichte = lade_rekurrenzgewichte(quelle)
    ebenen = sorted(spannen)
    print(f"[zerfall] {len(ebenen)} Zustandsebenen gefunden\n")

    g_max_gesamt = 0.0        # das langsamste Zerfallen, also g nahe 1
    g_min_gesamt = 1.0
    ebene_max = None
    print(f"{'Ebene':>5} {'|a|max':>8} {'exp_A':>18} {'dt_bias':>16} "
          f"{'g_min':>10} {'g_max':>12}")
    for ebene in ebenen:
        a_max = spannen[ebene]
        exp_a = gewichte[ebene]["A_log"].exp()
        dt = gewichte[ebene]["dt_bias"]
        # Je Kopf: der kleinste und groesste Zerfall ueber a in [-a_max, a_max].
        g_min_e, g_max_e = 1.0, 0.0
        for i in range(exp_a.numel()):
            ea = float(exp_a.reshape(-1)[i])
            b = float(dt.reshape(-1)[i])
            # softplus ist monoton, also liegen die Extreme an den Raendern.
            d_klein = ea * softplus(-a_max + b)
            d_gross = ea * softplus(a_max + b)
            g_min_e = min(g_min_e, math.exp(-d_gross))
            g_max_e = max(g_max_e, math.exp(-d_klein))
        print(f"{ebene:>5} {a_max:>8.3f} "
              f"{float(exp_a.min()):>8.4f}..{float(exp_a.max()):<8.4f} "
              f"{float(dt.min()):>7.3f}..{float(dt.max()):<7.3f} "
              f"{g_min_e:>10.3e} {g_max_e:>12.9f}")
        if g_max_e > g_max_gesamt:
            g_max_gesamt, ebene_max = g_max_e, ebene
        g_min_gesamt = min(g_min_gesamt, g_min_e)

    print(f"\n[zerfall] ueber alle Ebenen: g in [{g_min_gesamt:.3e}, "
          f"{g_max_gesamt:.12f}]")
    print(f"[zerfall] das langsamste Zerfallen steht in Ebene {ebene_max}")

    # ⚑ Der Abstand von 1 ist die Groesse, auf die es ankommt.
    luecke = 1.0 - g_max_gesamt
    print(f"[zerfall] 1 - g_max = {luecke:.3e}")
    if luecke > 0:
        print(f"[zerfall] das sind {-math.log2(luecke):.1f} Bit, nur um "
              f"g von 1 zu unterscheiden")

    print("\n[zerfall] noetige Nachkommastellen fuer den Zerfall,")
    print("          damit der Fehler nach N Schritten unter 2^-%d bleibt:"
          % WERT_FRAC)
    print(f"{'N':>10} {'Bit':>6}")
    for n in (128, 1024, 4096, 32768, 262144):
        # eps * min(N, 1/(1-g)) < 2^-WERT_FRAC  mit  eps = 2^-(frac+1).
        #
        # ⚑ **Das Minimum und nicht das Produkt.** Eine Folge kann nur so
        # weit zurueckwirken, wie sie lang ist ODER wie weit das
        # Gedaechtnis reicht. Im engsten Punkt (g = 1 - 1/N) sind beide
        # Faktoren gleich N, also zaehlt N.
        #
        # ⚠️ **Der Term -log2(g) steht bewusst NICHT hier.** Er ist bei
        # g nahe eins rund 1e-9 gross, aendert also nichts, hebt aber
        # `ceil` ueber die glatte Zahl: 25,0000000015 wird zu 26. Eine
        # Stelle, die aus einem Gleitkommarest kommt, ist keine Stelle.
        bit = math.log2(n) + WERT_FRAC - 1
        print(f"{n:>10} {math.ceil(bit):>6}")


if __name__ == "__main__":
    main()
