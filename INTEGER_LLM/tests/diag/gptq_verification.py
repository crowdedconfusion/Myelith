#!/usr/bin/env python3
"""
GPTQ-Verifikation: Reduziert GPTQ den Ausgangsfehler eines echten
Modell-Layers gegenüber Round-to-Nearest (RNE)?

Für einen Linearen Layer wird der Hessian H = Σ x·xᵀ aus echten
Aktivierungen gesammelt, dann werden die Gewichte einmal per RNE und einmal
per GPTQ quantisiert und der jeweilige Ausgangsfehler gegen den Float-Layer
gemessen. Ist der GPTQ-Fehler NICHT kleiner als der RNE-Fehler, ist die
GPTQ-Implementierung oder ihre Anwendung das Problem.

Gleitkomma erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python gptq_verification.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402
from src.gptq import gptq_quantize, per_channel_shifts  # noqa: E402

PROMPTS = [
    "Die numerische Stabilitaet von Fixed-Point-Inferenz ist entscheidend "
    "fuer die Bitgleichheit ueber unabhaengige Knoten hinweg.",
    "Decentralized consensus networks coordinate independent nodes by "
    "verifying identical computation, and deterministic integer "
    "arithmetic enables dispute resolution through bisection.",
    "Ein Agent plant mehrere Schritte, ruft Werkzeuge auf und beachtet "
    "dabei Budgetgrenzen, bevor er eine Transaktion signiert.",
    "Quantization maps floating point weights to int8 with calibrated "
    "power-of-two scales; lookup tables approximate nonlinear functions "
    "such as silu, exp, rsqrt and the rotary position embeddings.",
]


def main():
    import torch
    import numpy as np

    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    from transformers import AutoTokenizer
    tok = AutoTokenizer.from_pretrained(REPO / "models" / "Qwen2.5-0.5B")

    targets = [
        ("layers.3.mlp.gate_proj", model.model.layers[3].mlp.gate_proj),
        ("layers.12.mlp.gate_proj", model.model.layers[12].mlp.gate_proj),
    ]

    for label, target in targets:
        acts = []
        gram_acc = None

        def hook(module, inputs, output):
            nonlocal gram_acc
            x = inputs[0]
            xf = x.detach().reshape(-1, x.shape[-1]).float()
            acts.append(xf.cpu())
            g = (xf.T @ xf).detach().cpu().numpy()
            gram_acc = g if gram_acc is None else gram_acc + g

        h = target.register_forward_hook(hook)
        with torch.no_grad():
            for prompt in PROMPTS:
                inputs = tok(prompt, return_tensors="pt").to(model.device)
                _ = model(**inputs)
        h.remove()

        x = torch.cat(acts, dim=0).numpy().astype(np.float64)  # [N, in]
        H = gram_acc.astype(np.float64)
        W = target.weight.detach().float().cpu().numpy().astype(np.float64)
        y_float = x @ W.T

        # RNE per-Channel
        shifts = per_channel_shifts(target.weight).astype(np.float64)
        scale = np.power(2.0, shifts)
        Wq_rne = np.clip(np.round(W * scale[:, None]), -128, 127) / scale[:, None]
        err_rne = np.linalg.norm(x @ Wq_rne.T - y_float) / np.linalg.norm(y_float)

        # GPTQ
        gptq = gptq_quantize(target.weight, H)
        s_g = gptq["shifts"].astype(np.float64)
        sc_g = np.power(2.0, s_g)
        Wq_gptq = gptq["int8"].astype(np.float64) / sc_g[:, None]
        err_gptq = np.linalg.norm(x @ Wq_gptq.T - y_float) / np.linalg.norm(y_float)

        print(f"{label:28s} RNE={err_rne:.5f}  GPTQ={err_gptq:.5f}  "
              f"{'GPTQ besser' if err_gptq < err_rne else 'GPTQ NICHT besser'} "
              f"(Faktor {err_rne/max(err_gptq,1e-12):.2f})")


if __name__ == "__main__":
    main()
