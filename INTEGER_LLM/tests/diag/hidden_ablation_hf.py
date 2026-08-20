#!/usr/bin/env python3
"""
Ablation: trennt den Fehlerbeitrag des HIDDEN-STATE vom LM-HEAD.

Hintergrund (2026-08-19): Die 7B-Integer-Perplexitaet liegt bei 40,5
gegen eine FP-Baseline von 8,7, ohne dass ein Strukturfehler gefunden
wurde — Fund 19 (Attention-Skalierung) und Fund 20 (Per-Kanal-
Residualskalen) sind behoben, GPTQ ist als Ursache ausgeschlossen, und
der positionsweise Vergleich zeigt breit verteiltes Rauschen (83,7 % aller
Positionen schlechter) statt einer lokalisierten Divergenz.

Diese Ablation beantwortet die verbleibende Frage: Wie viel des Fehlers
entsteht VOR dem LM-Head (im Residualstrom ueber 28 Ebenen), und wie viel
IM LM-Head (int16-Quantisierung von 152064 x 3584)?

Verfahren: Der Integer-Hidden-State (Ausgang der finalen RMSNorm, gedumpt
von runtime/src/bin/final_hidden_dump.rs) wird durch HFs FLOAT-LM-Head
geschickt. Drei Perplexitaeten:

    A  Integer-Hidden + Integer-LM-Head  = der echte Integer-Wert
    B  Integer-Hidden + Float-LM-Head    = wird hier gemessen
    C  Float-Hidden   + Float-LM-Head    = die FP-Baseline (8,7)

Liegt B nahe A, ist der Hidden-State die Fehlerquelle und der LM-Head
unschuldig. Liegt B nahe C, ist es umgekehrt.

Gleitkomma ist hier erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage:
  INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/hidden_ablation_hf.py <hidden_dump.txt>
"""
import math
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

    if len(sys.argv) < 2:
        print("Usage: hidden_ablation_hf.py <hidden_dump.txt>", file=sys.stderr)
        sys.exit(1)
    dump_path = Path(sys.argv[1])

    n_sequences = int(os.environ.get("E2E_SEQUENCES", "4"))
    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    sequences = select_sequences(n_sequences, seq_len, verbose=False)

    # Integer-Hidden-States einlesen: {(seq, pos): [float, ...]}
    integer_hidden = {}
    with open(dump_path) as f:
        for line in f:
            teile = line.split()
            if len(teile) < 3:
                continue
            s, p = int(teile[0]), int(teile[1])
            integer_hidden[(s, p)] = [float(x) for x in teile[2:]]
    print(f"[ablation] {len(integer_hidden)} Integer-Hidden-States geladen")

    model, _ = load_reference_model(MODEL_DIR)
    lm_head = model.get_output_embeddings().weight.detach().float()
    print(f"[ablation] HF-LM-Head: {tuple(lm_head.shape)}")

    sum_logp_b = 0.0
    n = 0
    for seq_idx, ids in enumerate(sequences):
        for pos in range(len(ids) - 1):
            key = (seq_idx, pos)
            if key not in integer_hidden:
                continue
            h = torch.tensor(integer_hidden[key], dtype=torch.float32)
            logits = lm_head @ h                      # [vocab]
            logp = torch.log_softmax(logits, dim=-1)
            sum_logp_b += logp[ids[pos + 1]].item()
            n += 1

    ppl_b = math.exp(-sum_logp_b / n)
    print()
    print(f"[ablation] B  Integer-Hidden + Float-LM-Head : Perplexitaet {ppl_b:.2f} "
          f"({n} Positionen)")
    print(f"[ablation] Zum Vergleich:")
    # Der Vergleichswert war bis 2026-08-20 fest auf 40.48 verdrahtet —
    # dem Stand VOR den Funden 23/24. Nach deren Behebung liegt der
    # Integer-Pfad bei 9,40, und die daraus berechnete Prozentaufteilung
    # war entsprechend unsinnig (sie meldete "98 % im LM-Head", waehrend
    # der LM-Head tatsaechlich nichts beitraegt). Jetzt ueber die
    # Umgebung setzbar, damit die Zahl mit dem Messstand mitwandert.
    ppl_a = float(os.environ.get("INTEGER_PPL", "9.40"))
    print(f"[ablation] A  Integer-Hidden + Integer-LM-Head: {ppl_a:.2f} (gemessen)")
    print(f"[ablation] C  Float-Hidden   + Float-LM-Head  :  8.68 (FP-Baseline)")
    print()
    spanne = ppl_a - 8.68
    anteil_lm = ((ppl_a - ppl_b) / spanne) if spanne > 1e-9 else 0.0
    print(f"[ablation] Deutung: {1 - anteil_lm:.0%} des Perplexitaets-Abstands "
          f"entsteht VOR dem LM-Head (Hidden-State),")
    print(f"[ablation]          {anteil_lm:.0%} IM LM-Head.")


if __name__ == "__main__":
    main()
