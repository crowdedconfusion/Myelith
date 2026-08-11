#!/usr/bin/env python3
"""Logit-Sweep über Positionen, HF-Referenz (Mehrpositions-Divergenzsuche,
Fund 14 Kandidat iii). Gibt an jeder Position der Sequenz den Top-1-Logit
(id + Wert) aus — Gegenstück zu `runtime/src/bin/seq_logits_sweep.rs`.

Gleitkomma ist hier erlaubt — Referenzmessung, nicht der Integerpfad.
Kein Teil des Auslieferungspfads.

Usage: python seq_logits_sweep_hf.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

# Identisch zu seq_logits_sweep.rs: erste 8 Tokens der ersten Mess-Sequenz.
TOKENS = [34532, 425, 10965, 465, 374, 458, 6364, 4531]


def main():
    import torch
    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()

    input_ids = torch.tensor([TOKENS], device=model.device)
    with torch.no_grad():
        out = model(input_ids=input_ids)

    logits = out.logits[0].float()  # [seq, vocab]
    for pos in range(len(TOKENS)):
        top = torch.topk(logits[pos], 1)
        print(f"pos {pos}: input={TOKENS[pos]} -> top1={top.indices[0].item()} "
              f"(wert {top.values[0].item():.4f})")


if __name__ == "__main__":
    main()
