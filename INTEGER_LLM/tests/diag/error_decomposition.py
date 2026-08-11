#!/usr/bin/env python3
"""
Fehlerzerlegung: Kommt der Quantisierungsfehler eines Linear-Layers primär
aus der Aktivierungsquantisierung (int16 Per-Layer) oder aus der
Gewichtsquantisierung (int8 Per-Channel)?

Misst für einen Layer vier Varianten gegen den Float-Layer:
  act_only    : nur Aktivierung quantisiert, Gewichte float
  weight_only : nur Gewichte quantisiert, Aktivierung float
  both        : beides quantisiert (entspricht der Pipeline)
Die dominante Komponente zeigt, wo eine Eskalation ansetzen müsste.

Gleitkomma erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python error_decomposition.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

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


def pow2_scale_from_absmax(absmax, bits):
    import math
    if absmax <= 0:
        return 0
    return int(math.floor(math.log2(((1 << (bits - 1)) - 1) / absmax)))


def main():
    import torch
    import numpy as np

    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    from transformers import AutoTokenizer
    tok = AutoTokenizer.from_pretrained(REPO / "models" / "Qwen2.5-0.5B")

    # Mehrere Layer testen (je ein q_proj und ein gate_proj verschiedener
    # Tiefen), um ein repräsentatives Bild zu bekommen.
    targets = [
        ("layers.0.self_attn.q_proj", model.model.layers[0].self_attn.q_proj,
         model.model.layers[0].input_layernorm),
        ("layers.3.mlp.gate_proj", model.model.layers[3].mlp.gate_proj,
         model.model.layers[3].post_attention_layernorm),
        ("layers.12.mlp.gate_proj", model.model.layers[12].mlp.gate_proj,
         model.model.layers[12].post_attention_layernorm),
        ("layers.23.mlp.gate_proj", model.model.layers[23].mlp.gate_proj,
         model.model.layers[23].post_attention_layernorm),
    ]

    for label, target, norm_src in targets:
        acts = []

        def hook(module, input, output):
            t = output if not isinstance(output, tuple) else output[0]
            acts.append(t.detach().float().cpu())

        h = norm_src.register_forward_hook(hook)
        with torch.no_grad():
            for prompt in PROMPTS:
                inputs = tok(prompt, return_tensors="pt").to(model.device)
                _ = model(**inputs)
        h.remove()

        x = torch.cat([a.reshape(-1, a.shape[-1]) for a in acts], dim=0).numpy()
        W = target.weight.detach().float().cpu().numpy()
        y_float = x @ W.T

        bits_act, bits_w = 16, 8

        def quant_act(x_):
            absmax = np.abs(x_).max()
            shift = pow2_scale_from_absmax(absmax, bits_act)
            q = np.clip(np.round(x_ * (2.0 ** shift)), -(1 << 15), (1 << 15) - 1)
            return q / (2.0 ** shift)

        def quant_w(W_):
            absmax = np.abs(W_).max(axis=1, keepdims=True)
            shift = np.floor(np.log2(127.0 / np.maximum(absmax, 1e-9)))
            shift = np.clip(shift, 0, 20)
            q = np.clip(np.round(W_ * (2.0 ** shift)), -128, 127)
            return q / (2.0 ** shift)

        n = np.linalg.norm(y_float)
        err_act = np.linalg.norm(quant_act(x) @ W.T - y_float) / n
        err_w = np.linalg.norm(x @ quant_w(W).T - y_float) / n
        err_both = np.linalg.norm(quant_act(x) @ quant_w(W).T - y_float) / n

        print(f"{label:28s} act_only={err_act:.5f} weight_only={err_w:.5f} "
              f"both={err_both:.5f}")

    print("\nInterpretation: die größere der beiden Einzel-Fehlerquellen dominiert.")
    print("act_only >> weight_only  => Aktivierungsquantisierung ist das Problem.")
    print("weight_only >> act_only  => Gewichtsquantisierung ist das Problem.")


if __name__ == "__main__":
    main()
