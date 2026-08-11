#!/usr/bin/env python3
"""
Gleitkomma-Baseline: Qwen2.5-0.5B in BF16 auf denselben WikiText-2-
Sequenzen wie der Integer-E2E-Test (Fahrplan-Punkt 12.20).

Identische Messmethode (Fahrplan-Vorgabe): dieselbe Sequenz-Auswahl
(eval/wikitext_common.py), derselbe Tokenizer, dieselbe Sequenzlänge.
Einziger Unterschied ist das Zahlenformat des Modells — damit ist der
Perplexitätsvergleich am Entscheidungspunkt 12.21 aussagekräftig.

Messpfad: Teacher-Forcing in einem einzigen Forward pro Sequenz
(Gleitkomma ist hier erlaubt — dies ist die Referenzmessung, nicht der
Integer-Inferenzpfad).

Ergebnis wird zusätzlich als JSON gesichert:
  eval/results/baseline_wikitext2.json

Steuerung über Umgebungsvariablen (dieselben wie der E2E-Test!):
  E2E_SEQUENCES  Anzahl Sequenzen (Standard: 4)
  E2E_SEQ_LEN    Tokens je Sequenz (Standard: 128)
"""

import json
import math
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from wikitext_common import MODEL_DIR, select_sequences


def main():
    n_sequences = int(os.environ.get("E2E_SEQUENCES", "4"))
    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))

    import torch
    from transformers import AutoModelForCausalLM

    sequences = select_sequences(n_sequences, seq_len)

    print(f"[baseline] Lade HF-Modell aus {MODEL_DIR} (BF16) ...")
    model = AutoModelForCausalLM.from_pretrained(
        str(MODEL_DIR), dtype=torch.bfloat16, device_map="auto")
    model.eval()

    n_eval = 0
    sum_logp = 0.0
    per_seq = []

    with torch.no_grad():
        for i, ids in enumerate(sequences):
            input_ids = torch.tensor([ids], device=model.device)
            out = model(input_ids=input_ids)
            # Logits an Position t sagen Token t+1 vorher.
            logits = out.logits[0, :-1, :].float()
            targets = torch.tensor(ids[1:], device=model.device)
            log_probs = torch.log_softmax(logits, dim=-1)
            token_logps = log_probs.gather(1, targets.unsqueeze(1)).squeeze(1)

            slp = token_logps.sum().item()
            count = token_logps.numel()
            ppl = math.exp(-slp / count)
            n_eval += count
            sum_logp += slp
            per_seq.append({"tokens": len(ids), "evaluated": count,
                            "sum_logp": slp, "perplexity": ppl})
            print(f"[baseline] Sequenz {i}: {count} Positionen, "
                  f"Perplexitaet {ppl:.2f}")

    overall_ppl = math.exp(-sum_logp / n_eval)
    result = {
        "model": "Qwen/Qwen2.5-0.5B (HF, BF16)",
        "dataset": "wikitext-2-raw-v1 (Testsplit)",
        "n_sequences": len(sequences),
        "seq_len": seq_len,
        "evaluated_tokens": n_eval,
        "sum_logp": sum_logp,
        "perplexity": overall_ppl,
        "sequences": per_seq,
    }

    results_dir = Path(__file__).resolve().parent / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    out_path = results_dir / "baseline_wikitext2.json"
    out_path.write_text(json.dumps(result, indent=2, ensure_ascii=False),
                        encoding="utf-8")

    print(f"[baseline] Gesamt-Perplexitaet (FP-Baseline): {overall_ppl:.2f} "
          f"({n_eval} Positionen)")
    print(f"[baseline] Ergebnis gesichert: {out_path}")


if __name__ == "__main__":
    main()
