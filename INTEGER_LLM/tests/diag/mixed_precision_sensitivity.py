#!/usr/bin/env python3
"""
Mischpräzisions-Empfindlichkeits-Analyse: Welche Layer reagieren am
empfindlichsten auf Gewichts-Quantisierungsfehler (Fortpflanzung Richtung
Logits)?

Für jeden Block werden ISOLIERT dessen Lineare Projektionen auf int8
(per-Channel, RNE) quantisiert, während alle anderen Layer float bleiben.
Gemessen wird die Änderung der Logits gegenüber der reinen Float-Referenz.
Layer mit großer Logit-Änderung sind empfindlich und Kandidaten für
int16-Mischpräzision; Layer mit kleiner Änderung kommen mit int8 aus.

Gleitkomma erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python mixed_precision_sensitivity.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

PROMPT = ("Decentralized consensus networks coordinate independent nodes by "
          "verifying identical computation, and deterministic integer "
          "arithmetic enables dispute resolution through bisection.")


def quantize_int8_per_channel(W):
    """RNE per-Channel (pro Ausgabe-Zeile), Zweierpotenz-Skala."""
    import torch
    absmax = W.abs().amax(dim=1, keepdim=True)
    shift = torch.floor(torch.log2(127.0 / absmax.clamp(min=1e-9)))
    shift = torch.clamp(shift, 0, 20)
    scale = 2.0 ** shift
    q = torch.clamp(torch.round(W * scale), -128, 127)
    return q / scale


def main():
    import torch

    model, tok = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    dev = model.device
    inputs = tok(PROMPT, return_tensors="pt").to(dev)

    with torch.no_grad():
        ref_logits = model(**inputs).logits[0, -1].float().cpu()
    ref_norm = ref_logits.norm().item()

    layers = model.model.layers
    proj_names = ("q_proj", "k_proj", "v_proj", "o_proj",
                  "gate_proj", "up_proj", "down_proj")

    results = []
    for li, layer in enumerate(layers):
        # Gewichte sichern und quantisieren
        originals = {}
        for pn in proj_names:
            mod = getattr(layer.self_attn, pn, None) or getattr(layer.mlp, pn, None)
            originals[pn] = mod.weight.data.clone()
            mod.weight.data = quantize_int8_per_channel(mod.weight.data.float()).to(mod.weight.dtype)

        with torch.no_grad():
            q_logits = model(**inputs).logits[0, -1].float().cpu()

        # wiederherstellen
        for pn in proj_names:
            mod = getattr(layer.self_attn, pn, None) or getattr(layer.mlp, pn, None)
            mod.weight.data = originals[pn]

        diff = (q_logits - ref_logits).norm().item() / ref_norm
        results.append((li, diff))

    results.sort(key=lambda r: -r[1])
    print(f"{'Rang':>4} {'Layer':>6} {'Logit-Änderung (rel.)':>22}")
    for rank, (li, diff) in enumerate(results, 1):
        print(f"{rank:>4} {li:>6} {diff:>22.5f}")

    print("\nInterpretation: Layer mit hoher relativer Logit-Änderung sind die")
    print("Kandidaten für int16-Mischpräzision. Die unteren Ränge kommen mit int8 aus.")


if __name__ == "__main__":
    main()
