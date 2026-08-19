#!/usr/bin/env python3
"""
HF-Referenz zum positionsweisen Perplexitaetsvergleich.

Gegenstueck zu `runtime/src/bin/perplexity_probe.rs --per-token`: spielt
dieselben Mess-Sequenzen (eval/wikitext_common.py) durch das BF16-Modell
und gibt JEDE Positions-Log-Probability im selben Format aus
(`POS <seq> <pos> <log_softmax_target>`).

Zweck (2026-08-19): Die aggregierte 7B-Perplexitaet weicht stark von der
FP-Baseline ab, ohne dass Fund 19 (Attention-Skalierung), Fund 20
(Per-Kanal-Residualskalen) oder die GPTQ-Fehlerkompensation es erklaeren.
Ein aggregierter Wert sagt aber nicht, WO die Abweichung sitzt: gleichmaessig
ueber alle Positionen verteilt (Rauschen) oder auf wenige Positionen
konzentriert (Strukturfehler). Genau diese Unterscheidung hat bei 0,5B die
Funde 15/16 aufgebrochen.

Gleitkomma ist hier erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage: INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/perplexity_probe_hf.py
"""
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import MODEL_DIR, select_sequences  # noqa: E402


def main():
    import torch

    n_sequences = int(os.environ.get("E2E_SEQUENCES", "4"))
    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    sequences = select_sequences(n_sequences, seq_len, verbose=False)

    model, _ = load_reference_model(MODEL_DIR)

    for seq_idx, ids in enumerate(sequences):
        input_ids = torch.tensor([ids], device=model.device)
        with torch.no_grad():
            logits = model(input_ids=input_ids).logits[0].float()
        # Position i sagt Token i+1 voraus - identische Konvention wie
        # perplexity_probe.rs und eval/baseline.py.
        log_probs = torch.log_softmax(logits[:-1], dim=-1)
        targets = torch.tensor(ids[1:], device=log_probs.device)
        token_logps = log_probs.gather(1, targets.unsqueeze(1)).squeeze(1)
        for pos, lp in enumerate(token_logps.tolist()):
            print(f"POS {seq_idx} {pos} {lp:.6f}")


if __name__ == "__main__":
    main()
