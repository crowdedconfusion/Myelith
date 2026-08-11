#!/usr/bin/env python3
"""
int16-Gewichts-Simulation: Schließt int16-Gewichtsquantisierung die
Perplexitäts-Lücke zu FP?

Lädt das HF-Referenzmodell, quantisiert alle Linearen Projektionen
(q/k/v/o/gate/up/down) auf die angegebene Bit-Breite (per-Channel,
Zweierpotenz-Skala) und misst die Perplexität im Teacher-Forcing auf den
WikiText-2-Messsequenzen. Arithmetik bleibt float (BF16) — isoliert wird
allein der Gewichts-Quantisierungs-Effekt (Aktivierungen/LUTs sind laut
Fehlerzerlegung vernachlässigbar).

Bit-Breiten: 8 (Validierung: sollte ~73 ergeben, wie der Integer-Pfad) und
16 (Ziel: zeigt, ob int16 die Lücke zu FP 14,95 schließt).

Gleitkomma erlaubt (Simulation). Kein Teil des Auslieferungspfads.

Usage: python int16_simulation.py [bits]   (bits = 8 | 16, Standard 16)
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import select_sequences  # noqa: E402

PROJ_NAMES = ("q_proj", "k_proj", "v_proj", "o_proj",
              "gate_proj", "up_proj", "down_proj")


def quantize_weight_per_channel(W, bits):
    """Per-Channel (pro Ausgabe-Zeile), Zweierpotenz-Skala, int<bits>."""
    import torch
    max_int = (1 << (bits - 1)) - 1
    absmax = W.abs().amax(dim=1, keepdim=True)
    shift = torch.floor(torch.log2(max_int / absmax.clamp(min=1e-9)))
    shift = torch.clamp(shift, 0, 30)
    scale = 2.0 ** shift
    q = torch.clamp(torch.round(W * scale), -max_int - 1, max_int)
    return q / scale


def set_quantized_weights(model, bits):
    n = 0
    for layer in model.model.layers:
        for pn in PROJ_NAMES:
            mod = getattr(layer.self_attn, pn, None) or getattr(layer.mlp, pn, None)
            mod.weight.data = quantize_weight_per_channel(
                mod.weight.data.float(), bits).to(mod.weight.dtype)
            n += 1
    return n


def measure_perplexity(model, tok, sequences, device):
    import torch
    total_logp = 0.0
    total_tokens = 0
    with torch.no_grad():
        for ids in sequences:
            input_ids = torch.tensor([ids], device=device)
            logits = model(input_ids=input_ids).logits[0]  # [seq, vocab]
            # Teacher-Forcing: Position t sagt Token t+1 voraus
            shift_logits = logits[:-1, :]
            targets = input_ids[0, 1:]
            log_probs = torch.log_softmax(shift_logits.float(), dim=-1)
            tok_logp = log_probs.gather(1, targets.unsqueeze(1)).squeeze(1)
            total_logp += tok_logp.sum().item()
            total_tokens += targets.numel()
    import math
    return math.exp(-total_logp / total_tokens), total_tokens


def main():
    bits = int(sys.argv[1]) if len(sys.argv) > 1 else 16
    import torch

    model, tok = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    device = model.device

    sequences = select_sequences(4, 128)
    print(f"[sim] {len(sequences)} Sequenzen, "
          f"{sum(len(s) for s in sequences)} Tokens")

    # Referenz (unquantisiert)
    ppl_fp, ntok = measure_perplexity(model, tok, sequences, device)
    print(f"[sim] FP (unquantisiert):      Perplexitaet {ppl_fp:.2f} "
          f"({ntok} Positionen)")

    # Gewichte quantisieren
    n = set_quantized_weights(model, bits)
    print(f"[sim] {n} Lineare Projektionen auf int{bits} quantisiert")

    ppl_q, ntok = measure_perplexity(model, tok, sequences, device)
    print(f"[sim] int{bits}-Gewichte:             Perplexitaet {ppl_q:.2f} "
          f"({ntok} Positionen)")

    print(f"\n[sim] FP-Baseline:            {ppl_fp:.2f}")
    print(f"[sim] int{bits}-Gewichte:        {ppl_q:.2f}")
    print(f"[sim] Integer-Pfad (int8+GPTQ): 73.15")
    print(f"[sim] Relativer Anstieg int{bits} vs FP: "
          f"{(ppl_q/ppl_fp - 1)*100:.1f} %")


if __name__ == "__main__":
    main()
