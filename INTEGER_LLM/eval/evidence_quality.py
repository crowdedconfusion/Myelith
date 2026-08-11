#!/usr/bin/env python3
"""
Evidenz-Lauf 2: Qualität der Integer-Inferenz, plastisch gemacht.

Zwei Nachweise:
  A) Parallelgenerierung: dieselben Prompts (DE/EN) werden einmal mit
     dem Integer-Modell (greedy, Runtime-CLI) und einmal mit der
     HF-Gleitkomma-Referenz (BF16, greedy) generiert und nebeneinander
     gelegt — sichtbar wird, wie nah die Ausgaben inhaltlich liegen.
  B) Top-1-Agreement: auf denselben WikiText-2-Sequenzen wie am
     Entscheidungspunkt 12.21 (eval/wikitext_common.py) wird an jeder
     Position die Top-1-Vorhersage des Integer-Modells
     (runtime/src/bin/seq_logits_sweep.rs) mit der Top-1-Vorhersage der
     HF-Referenz verglichen — ein quantitatives Qualitätsmaß neben der
     Perplexität.

Gleitkomma ist nur im HF-Referenzpfad im Einsatz (Referenzmessung,
nicht der Integerpfad).

Ergebnis: eval/results/evidence/quality.json
"""

import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ARTIFACTS = REPO / "artifacts" / "qwen2.5-0.5b"
CLI = REPO / "runtime" / "target" / "release" / "integer-llm-runtime"
SEQ_SWEEP = REPO / "runtime" / "target" / "release" / "seq_logits_sweep"
RESULTS_DIR = REPO / "eval" / "results" / "evidence"

sys.path.insert(0, str(REPO / "eval"))
sys.path.insert(0, str(REPO / "calibrate"))
from wikitext_common import select_sequences, MODEL_DIR  # noqa: E402
from src.loader import load_reference_model  # noqa: E402

PROMPTS = [
    "The capital of France is",
    "In quantum mechanics, the wave function describes",
    "Die Hauptstadt von Frankreich ist",
    "In der Quantenmechanik beschreibt die Wellenfunktion",
    "The result of 17 times 23 is",
]
MAX_TOKENS = 40

TOKEN_RE = re.compile(r"\[runtime\] Generierte Token: \[(.*)\]")
SWEEP_RE = re.compile(r"pos (\d+): input=(\d+) -> top1=(\d+)")


def generate_integer(prompt: str) -> list:
    result = subprocess.run(
        [str(CLI), str(ARTIFACTS), prompt, str(MAX_TOKENS)],
        capture_output=True, text=True, timeout=7200,
    )
    if result.returncode != 0:
        print(f"[quality] FEHLT: Integer-Lauf fehlgeschlagen: {result.stderr}",
              file=sys.stderr)
        sys.exit(1)
    for line in result.stdout.splitlines():
        m = TOKEN_RE.search(line)
        if m:
            raw = m.group(1).strip()
            return [int(t) for t in raw.split(",")] if raw else []
    print(f"[quality] FEHLT: keine Token-Ausgabe:\n{result.stdout}", file=sys.stderr)
    sys.exit(1)


def sweep_integer(ids: list) -> list:
    """Top-1-Vorhersage des Integer-Modells an jeder Position (Teacher-Forcing)."""
    result = subprocess.run(
        [str(SEQ_SWEEP), str(ARTIFACTS)] + [str(t) for t in ids],
        capture_output=True, text=True, timeout=7200,
    )
    if result.returncode != 0:
        print(f"[quality] FEHLT: Sweep fehlgeschlagen: {result.stderr}",
              file=sys.stderr)
        sys.exit(1)
    top1 = {}
    for line in result.stdout.splitlines():
        m = SWEEP_RE.search(line)
        if m:
            top1[int(m.group(1))] = int(m.group(3))
    return [top1[p] for p in range(len(ids))]


def main():
    if not CLI.exists() or not SEQ_SWEEP.exists():
        print("[quality] FEHLT: Binaries — zuerst 'cargo build --release --bins'.",
              file=sys.stderr)
        sys.exit(1)

    print("[quality] Lade HF-Referenz (BF16) ...")
    import torch
    model, tokenizer = load_reference_model(MODEL_DIR)
    model.eval()

    # --- A) Parallelgenerierung -------------------------------------------
    generations = []
    with torch.no_grad():
        for prompt in PROMPTS:
            int_ids = generate_integer(prompt)
            prompt_ids = tokenizer(prompt).input_ids
            hf_out = model.generate(
                torch.tensor([prompt_ids], device=model.device),
                max_new_tokens=MAX_TOKENS, do_sample=False,
            )
            hf_ids = hf_out[0][len(prompt_ids):].tolist()
            generations.append({
                "prompt": prompt,
                "integer_text": tokenizer.decode(int_ids),
                "fp_text": tokenizer.decode(hf_ids),
                "integer_tokens": int_ids,
                "fp_tokens": hf_ids,
            })
            print(f"[quality] Prompt: {prompt!r}")
            print(f"          Integer: {tokenizer.decode(int_ids)[:120]}")
            print(f"          FP:      {tokenizer.decode(hf_ids)[:120]}")

    # --- B) Top-1-Agreement auf den 12.21-Messsequenzen ------------------
    sequences = select_sequences(4, 128)
    agreement_records = []
    total = 0
    agree = 0
    with torch.no_grad():
        for i, ids in enumerate(sequences):
            int_top1 = sweep_integer(ids)
            input_ids = torch.tensor([ids], device=model.device)
            out = model(input_ids=input_ids)
            hf_top1 = out.logits[0].argmax(dim=-1).tolist()
            matches = [a == b for a, b in zip(int_top1, hf_top1)]
            first_div = next((p for p, m in enumerate(matches) if not m), None)
            agreement_records.append({
                "sequence": i,
                "positions": len(ids),
                "agreement": sum(matches) / len(matches),
                "first_divergence": first_div,
            })
            total += len(matches)
            agree += sum(matches)
            print(f"[quality] Sequenz {i}: Top-1-Agreement "
                  f"{sum(matches)}/{len(matches)} "
                  f"({sum(matches) / len(matches):.1%}), erste Abweichung: "
                  f"{'keine' if first_div is None else 'Position ' + str(first_div)}")

    overall_agreement = agree / total
    print(f"[quality] Top-1-Agreement gesamt: {agree}/{total} "
          f"({overall_agreement:.1%})")

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    out = RESULTS_DIR / "quality.json"
    out.write_text(json.dumps({
        "parallel_generation": {
            "decoding": "greedy, max_new_tokens = " + str(MAX_TOKENS),
            "hf_reference": "Qwen/Qwen2.5-0.5B, BF16, HF generate()",
            "generations": generations,
        },
        "top1_agreement": {
            "dataset": "WikiText-2, Testsplit; identische Sequenzen wie 12.21",
            "n_sequences": len(sequences),
            "seq_len": 128,
            "overall_agreement": overall_agreement,
            "sequences": agreement_records,
        },
    }, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"[quality] Gesichert: {out}")


if __name__ == "__main__":
    main()
