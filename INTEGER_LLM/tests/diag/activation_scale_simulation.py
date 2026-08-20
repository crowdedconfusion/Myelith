#!/usr/bin/env python3
"""Was kostet EINE Skala je Ebene fuer die Zwischenaktivierungen?

**Anlass (2026-08-20).** Die Positionsanalyse zeigt: Der Ebenenfehler
springt an Ebene 2 von 3 % auf 7-11 %, aber nur an den Positionen >= 1;
Position 0 selbst bleibt flach bei 1,4-2,5 %. Ebene 2 ist genau die
Ebene, an der die *massive activation* entsteht (Kanal 62: 0,75 -> 796).

Die Attention-Arithmetik scheidet als Ursache aus: `attn_probe --ebene`
misst sie auf den Ebenen 0/2/3/5/11 mit 0,04-0,16 % gegen Gleitkomma aus
identischen q/k/v. Auch der Residualstrom scheidet aus: seine
Per-Kanal-Skalen kosten 0,005 % (`token_scale_simulation.py`).

Bleibt eine Asymmetrie im Code: Der Residualstrom traegt seit Fund 20
eine Skala JE KANAL (`Vec<u8>`), jede Zwischenaktivierung dagegen nur
EINE Skala je Ebene (`u8`) — geteilt ueber alle Kanaele UND alle
Positionen. Ab Ebene 2 ist eine dieser Positionen die Senke mit
Aktivierungen um Groessenordnungen ueber allen anderen. Traegt sie die
Skala, verlieren alle uebrigen Positionen Aufloesung.

**Was hier gemessen wird — und was nicht.** Nur der DARSTELLUNGSFEHLER
der Aktivierung, also der Preis der Skalenwahl bei identischem
Gleitkommawert. Die Perplexitaetswirkung sagt das nicht voraus.

Referenz float32 (nicht bfloat16, siehe zehnter Instrumentenfehler).

Usage:
    INTEGER_LLM_MODEL=qwen2.5-0.5b python -u tests/diag/activation_scale_simulation.py <tok...>
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

MAX_SHIFT, INT16 = 20, 32767


def shifts_aus(absmax):
    return np.clip(np.floor(np.log2(INT16 / np.maximum(absmax, 1e-9))), 0, MAX_SHIFT)


def quant(x, shifts):
    return np.clip(np.round(x * (2.0 ** shifts)), -INT16 - 1, INT16) / (2.0 ** shifts)


def rel(a, b):
    d = a - b
    return 100.0 * float(np.sqrt((d * d).sum() / max((b * b).sum(), 1e-30)))


def main():
    toks = [int(t) for t in sys.argv[1:]]
    m = AutoModelForCausalLM.from_pretrained(str(MODEL_DIR), dtype=torch.float32,
                                             local_files_only=True)
    m.eval()

    gefangen = {}

    def haken(name):
        def f(_mod, _ein, aus):
            gefangen[name] = aus.detach()[0].numpy().astype(np.float64)
        return f

    griffe = []
    n_layer = m.config.num_hidden_layers
    for i, lay in enumerate(m.model.layers):
        for name, mod in (("norm_attn", lay.input_layernorm),
                          ("q", lay.self_attn.q_proj),
                          ("v", lay.self_attn.v_proj),
                          ("norm_mlp", lay.post_attention_layernorm),
                          ("gate", lay.mlp.gate_proj)):
            griffe.append(mod.register_forward_hook(haken(f"{i}.{name}")))
    with torch.no_grad():
        m(torch.tensor([toks]))
    for g in griffe:
        g.remove()
    del m

    stufen = ("norm_attn", "q", "v", "norm_mlp", "gate")
    print(f"{len(toks)} Positionen, {n_layer} Ebenen, Referenz float32\n")
    print("Darstellungsfehler der Zwischenaktivierungen (relativer L2, Prozent)")
    print("  A = eine Skala je Ebene (unser Pfad)   B = je Position   C = je Kanal")
    print()
    kopf = f"{'Ebene':>5} | " + " | ".join(f"{s:>22}" for s in stufen)
    print(kopf); print("-" * len(kopf))

    zeilen = []
    with Fortschritt(n_layer, "Ebenen") as fort:
        for i in range(n_layer):
            spalten = []
            for s in stufen:
                x = gefangen[f"{i}.{s}"]                 # [pos, kanal]
                a = rel(quant(x, shifts_aus(np.abs(x).max())), x)
                b = rel(quant(x, shifts_aus(np.abs(x).max(axis=1, keepdims=True))), x)
                c = rel(quant(x, shifts_aus(np.abs(x).max(axis=0, keepdims=True))), x)
                spalten.append((a, b, c))
            zeilen.append((i, spalten))
            fort.schritt()

    for i, spalten in zeilen:
        print(f"{i:>5} | " + " | ".join(f"{a:6.2f} {b:6.2f} {c:6.2f}" for a, b, c in spalten))

    print()
    for k, s in enumerate(stufen):
        a = np.mean([z[1][k][0] for z in zeilen])
        b = np.mean([z[1][k][1] for z in zeilen])
        c = np.mean([z[1][k][2] for z in zeilen])
        print(f"  {s:>10}: je Ebene {a:6.2f} %   je Position {b:6.2f} %   "
              f"je Kanal {c:6.2f} %   -> Gewinn {a / max(b, 1e-9):5.1f}× / {a / max(c, 1e-9):5.1f}×")


if __name__ == "__main__":
    main()
