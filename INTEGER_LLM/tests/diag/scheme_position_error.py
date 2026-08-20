#!/usr/bin/env python3
"""Kontrolle: Springt das SCHEMA an Ebene 2 genauso wie unser Pfad?

**Warum diese Messung (2026-08-20).** Der Reihe nach ausgeschlossen sind
inzwischen: Attention-Arithmetik (0,04-0,16 %, `attn_probe --ebene`),
Residualstrom-Skalen (0,005 %, `token_scale_simulation.py`),
Aktivierungsskalen (0,01-0,05 %, `activation_scale_simulation.py`), alle
Matrixmultiplikationen bei identischen Gewichten (0,01-0,02 %) und die
Softmax-Aufloesung (theta_v 0.16.0). Trotzdem liegt der Ebenenfehler ab
Ebene 2 bei 7-11 %.

Damit bleibt genau eine Erklaerung uebrig, und sie ist die einzige, die
nie POSITIONSWEISE geprueft wurde: die **Gewichtsquantisierung selbst**.
`scheme_layer_error.py` hat den Schema-Boden gemessen, aber nur an einer
Position. Wenn das Schema an Ebene 2 denselben Sprung zeigt, ist der
Befund "unser Pfad springt an Ebene 2" gar kein Implementierungsfehler,
sondern eine Eigenschaft des Modells unter W8.

Hier: dieselbe Kennzahl, dieselbe Referenz, alle Positionen — nur mit
Gewichten, die in Gleitkomma durch den int8-Rundtrip geschickt wurden.
Kein Aktivierungspfad, keine LUT, keine Skalen. Was hier auftaucht, ist
allein das Schema.

Referenz float32 (nicht bfloat16, zehnter Instrumentenfehler).

Usage:
    INTEGER_LLM_MODEL=qwen2.5-0.5b python -u tests/diag/scheme_position_error.py <tok...>
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
LIN = ("q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj")
WEIT = ("embed_tokens", "lm_head", "input_layernorm", "post_attention_layernorm", "norm")


def q8(W: torch.Tensor) -> torch.Tensor:
    """int8-Rundtrip mit Per-Kanal-Zweierpotenzskala — wie `calibrate`."""
    a = W.abs().clamp(min=1e-9) if W.dim() == 1 else W.abs().amax(dim=1, keepdim=True).clamp(min=1e-9)
    sc = torch.pow(2.0, torch.floor(torch.log2(127.0 / a)).clamp(0, MAX_SHIFT))
    return torch.clamp(torch.round(W * sc), -128, 127) / sc


def hidden(toks, quantisiert: bool):
    m = AutoModelForCausalLM.from_pretrained(str(MODEL_DIR), dtype=torch.float32,
                                             local_files_only=True)
    m.eval()
    if quantisiert:
        with torch.no_grad():
            for n, mod in m.named_modules():
                w = getattr(mod, "weight", None)
                if w is not None and (n.endswith(LIN) or any(k in n for k in WEIT)):
                    mod.weight.data = q8(w.data.float())
    with torch.no_grad():
        hs = [h[0].float().numpy().astype(np.float64)
              for h in m(torch.tensor([toks]), output_hidden_states=True).hidden_states]
    n_layer = m.config.num_hidden_layers
    del m
    return hs, n_layer


def rel(a, b):
    d = a - b
    return 100.0 * float(np.sqrt((d * d).sum() / max((b * b).sum(), 1e-30)))


def main():
    toks = [int(t) for t in sys.argv[1:]]
    with Fortschritt(2, "Modell-Durchlaeufe") as fort:
        exakt, n_layer = hidden(toks, False)
        fort.schritt()
        schema, _ = hidden(toks, True)
        fort.schritt()

    zeigen = [p for p in (0, 1, 2, 8, 16, len(toks) - 1)]
    print(f"\nSchema allein (W8 per Kanal, sonst float32), {len(toks)} Positionen\n")
    kopf = "Ebene | " + " | ".join(f"Pos {p:>2}" for p in zeigen)
    print(kopf); print("-" * len(kopf))
    for e in range(n_layer - 1):
        print(f"{e:>5} | " + " | ".join(f"{rel(schema[e + 1][p], exakt[e + 1][p]):6.2f}"
                                        for p in zeigen))


if __name__ == "__main__":
    main()
