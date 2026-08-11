#!/usr/bin/env python3
"""Sequenz-Layer-Dump, HF-Referenz (Mehrpositions-Divergenzsuche, Fund 14
Kandidat iii).

Spielt dieselbe Token-Sequenz wie `runtime/src/bin/seq_layer_dump.rs` durch
das HF-Referenzmodell (BF16, `output_hidden_states=True`) und dumppt die
Spannweiten (AbsMax + erste vier Werte) der Hidden-Zustände nach jedem Layer
an der letzten Position sowie die Top-Logits. Gleicht man die Ausgabe gegen
den Integer-Dump ab, sieht man die erste divergierende Stufe im
Mehrpositions-Pfad.

Gleitkomma ist hier erlaubt — dies ist die Referenzmessung, nicht der
Integer-Inferenzpfad. Kein Teil des Auslieferungspfads.

Usage: python seq_layer_dump_hf.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

# Erste Tokens der ersten Mess-Sequenz (eval/wikitext_common.py,
# select_sequences(4,128)[0]) — identisch zu seq_layer_dump.rs.
ALL_TOKENS = [34532, 425, 10965, 465, 374, 458, 6364, 4531]


def summary(name, t):
    t = t.detach().float().flatten()
    absmax = t.abs().max().item()
    head = [round(v, 4) for v in t[:4].tolist()]
    print(f"{name}: absmax={absmax:9.4f} first4=[{head[0]:9.4f}, "
          f"{head[1]:9.4f}, {head[2]:9.4f}, {head[3]:9.4f}]")


def main():
    import torch
    # Optionales Argument: Anzahl Tokens (Default: alle). Dump an der
    # letzten Position der gewaehlten Praefix-Laenge.
    n = int(sys.argv[1]) if len(sys.argv) > 1 else len(ALL_TOKENS)
    tokens = ALL_TOKENS[:n]

    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()

    input_ids = torch.tensor([tokens], device=model.device)
    with torch.no_grad():
        out = model(input_ids=input_ids, output_hidden_states=True)

    print(f"Sequenz: {tokens} (Dump an Position {len(tokens) - 1})")
    # hidden_states[0] = Embedding, hidden_states[i+1] = nach Layer i.
    hs = out.hidden_states
    for i in range(len(hs) - 1):
        summary(f"layer {i:2}", hs[i + 1][0, -1, :])

    logits = out.logits[0, -1, :].float()
    top = torch.topk(logits, 5)
    print("Top-5 Logits (id: wert):")
    for i, v in zip(top.indices.tolist(), top.values.tolist()):
        print(f"  {i}: {v:.4f}")


if __name__ == "__main__":
    main()
