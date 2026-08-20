#!/usr/bin/env python3
"""Was kostet die STATISCHE Skala gegenueber einer dynamischen? (Fund 31)

**Anlass.** Fund 31 hat gezeigt: Der massive Kanal 62 traegt an Position 0
rund 1714, an jeder anderen Position rund 5. Die kalibrierte Skala je
Kanal muss beides bedienen und ist damit fuer alle Nicht-Senken-Positionen
um Groessenordnungen zu grob. Die Recherche (I-LLM, 2024) nennt genau
diesen Fall als Grund fuer DYNAMISCHE Quantisierung: Skalen werden zur
Laufzeit aus dem Maximum abgeleitet statt aus einem Kalibrierlauf, weil
statische Skalen "bei Eingaben ausserhalb des Kalibriersatzes versagen".

**Was hier gemessen wird — und was nicht.** Dies ist ein *Vorabschirm*,
kein Fund: Gemessen wird allein der DARSTELLUNGSFEHLER des
Residualstroms, also wieviel Genauigkeit die Skalenwahl kostet, wenn man
denselben echten Gleitkommawert einmal statisch und einmal dynamisch
quantisiert. Die Perplexitaetswirkung sagt das nicht voraus — genau
diese Verwechslung hat die Suche schon einmal in die Irre gefuehrt.

Referenz ist float32, NICHT bfloat16: bei einem Wert von 1704 ist die
bf16-ULP 8, die Referenz koennte den Unterschied gar nicht aufloesen
(zehnter Instrumentenfehler, 2026-08-20).

Gleitkomma erlaubt — Referenzmessung, nicht Inferenzpfad.

Usage:
    INTEGER_LLM_MODEL=qwen2.5-0.5b python -u tests/diag/token_scale_simulation.py <tok...>
"""
import sys
from pathlib import Path

import numpy as np
import torch
from transformers import AutoModelForCausalLM

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "eval"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from wikitext_common import MODEL_DIR  # noqa: E402
from fortschritt import Fortschritt  # noqa: E402

MAX_SHIFT = 20
INT16 = 32767


def quantisiere(werte: np.ndarray, shifts: np.ndarray) -> np.ndarray:
    """int16-Rundtrip mit gegebenen Zweierpotenz-Shifts je Kanal."""
    roh = np.clip(np.round(werte * (2.0 ** shifts)), -INT16 - 1, INT16)
    return roh / (2.0 ** shifts)


def shifts_aus(absmax: np.ndarray) -> np.ndarray:
    """Groesster Shift, bei dem absmax noch in int16 passt."""
    a = np.maximum(absmax, 1e-9)
    return np.clip(np.floor(np.log2(INT16 / a)), 0, MAX_SHIFT)


def rel(a: np.ndarray, b: np.ndarray) -> float:
    d = a - b
    return 100.0 * float(np.sqrt((d * d).sum() / max((b * b).sum(), 1e-30)))


def main() -> None:
    toks = [int(t) for t in sys.argv[1:]]
    m = AutoModelForCausalLM.from_pretrained(str(MODEL_DIR), dtype=torch.float32,
                                             local_files_only=True)
    m.eval()
    with torch.no_grad():
        hs = [h[0].float().numpy().astype(np.float64)
              for h in m(torch.tensor([toks]), output_hidden_states=True).hidden_states]
    n_layer = m.config.num_hidden_layers
    del m

    print(f"{len(toks)} Positionen, {n_layer} Ebenen, Referenz float32\n")
    print("Darstellungsfehler des Residualstroms (relativer L2, in Prozent)")
    print()
    kopf = (f"{'Ebene':>5} | {'statisch/Kanal':>15} | {'dynamisch/Kanal':>16} | "
            f"{'Faktor':>7} | {'nur Pos 0':>10} | {'ohne Pos 0':>11}")
    print(kopf)
    print("-" * len(kopf))

    zeilen = []
    with Fortschritt(n_layer, "Ebenen") as fort:
        for e in range(n_layer):
            x = hs[e + 1]                       # [pos, kanal], Ausgabe Ebene e
            # (a) statisch je Kanal: ein Shift je Kanal, aus dem Maximum
            #     ueber ALLE Positionen — so kalibriert unser Pfad.
            s_stat = shifts_aus(np.abs(x).max(axis=0))
            q_stat = quantisiere(x, s_stat[None, :])
            # (b) dynamisch je Kanal UND Position: der Shift folgt dem
            #     tatsaechlichen Wert an dieser Position.
            s_dyn = shifts_aus(np.abs(x))
            q_dyn = quantisiere(x, s_dyn)

            zeilen.append((
                e,
                rel(q_stat, x), rel(q_dyn, x),
                rel(q_stat[0:1], x[0:1]), rel(q_stat[1:], x[1:]),
            ))
            fort.schritt()

    for e, a, b, p0, ohne in zeilen:
        faktor = a / max(b, 1e-9)
        print(f"{e:>5} | {a:14.3f} % | {b:15.3f} % | {faktor:6.1f}× | "
              f"{p0:9.3f} % | {ohne:10.3f} %")

    a = np.mean([z[1] for z in zeilen]); b = np.mean([z[2] for z in zeilen])
    print()
    print(f"Mittel: statisch {a:.3f} %, dynamisch {b:.3f} %  ->  Faktor {a/max(b,1e-9):.1f}×")


if __name__ == "__main__":
    main()
