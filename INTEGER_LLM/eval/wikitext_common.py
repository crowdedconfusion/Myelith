#!/usr/bin/env python3
"""
Gemeinsame WikiText-2-Aufbereitung für die Qualitätsmessung
(Fahrplan-Punkte 12.19–12.21).

Diese Modul ist die EINZIGE Quelle der Messsequenzen: Integer-E2E-Test
(tests/integration/test_end2end_real.py), Gleitkomma-Baseline
(eval/baseline.py) und Perplexitätsvergleich (eval/perplexity.py) verwenden
dieselbe Auswahl, denselben Tokenizer und dieselbe Sequenzlänge — nur so
ist der Perplexitätsvergleich am Entscheidungspunkt 12.21 aussagekräftig
(„identische Messmethode", Fahrplan 12.20).
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DATASETS = REPO / "eval" / "datasets"
WIKITEXT_CACHE = DATASETS / "wikitext2_test.txt"
MODEL_DIR = REPO / "models" / "Qwen2.5-0.5B"

MIN_LINE_CHARS = 160  # nur inhaltlich substantielle Zeilen
MIN_SEQ_TOKENS = 8    # kürzere Sequenzen sind nicht aussagekräftig


def load_wikitext2_test(verbose: bool = True) -> list:
    """Lädt den WikiText-2-Testsplit (mit lokalem Datei-Cache)."""
    if WIKITEXT_CACHE.exists():
        lines = WIKITEXT_CACHE.read_text(encoding="utf-8").splitlines()
        if verbose:
            print(f"[wikitext] Cache: {WIKITEXT_CACHE.name} ({len(lines)} Zeilen)")
        return lines

    try:
        from datasets import load_dataset
    except ImportError:
        print("[wikitext] FEHLT: 'datasets' nicht installiert (calibrate/requirements.txt).",
              file=sys.stderr)
        sys.exit(1)

    if verbose:
        print("[wikitext] Lade WikiText-2 von Hugging Face ...")
    ds = load_dataset("Salesforce/wikitext", "wikitext-2-raw-v1", split="test")
    lines = [str(x) for x in ds["text"]]
    DATASETS.mkdir(parents=True, exist_ok=True)
    WIKITEXT_CACHE.write_text("\n".join(lines), encoding="utf-8")
    if verbose:
        print(f"[wikitext] Geladen und gecacht: {len(lines)} Zeilen")
    return lines


def select_sequences(n_sequences: int, seq_len: int, verbose: bool = True) -> list:
    """
    Deterministische Sequenz-Auswahl: substantielle Zeilen mit festem Stride,
    tokenisiert mit dem Qwen2.5-0.5B-Tokenizer, auf seq_len begrenzt.
    """
    from transformers import AutoTokenizer
    tokenizer = AutoTokenizer.from_pretrained(str(MODEL_DIR))

    lines = load_wikitext2_test(verbose=verbose)
    candidates = [l.strip() for l in lines if len(l.strip()) >= MIN_LINE_CHARS]
    if not candidates:
        print("[wikitext] FEHLT: keine substantiellen WikiText-Zeilen gefunden.",
              file=sys.stderr)
        sys.exit(1)

    stride = max(1, len(candidates) // n_sequences)
    sequences = []
    for i in range(n_sequences):
        text = candidates[(i * stride) % len(candidates)]
        ids = tokenizer(text).input_ids[:seq_len]
        if len(ids) >= MIN_SEQ_TOKENS:
            sequences.append(ids)
    if verbose:
        total = sum(len(s) for s in sequences)
        print(f"[wikitext] {len(sequences)} Sequenzen, {total} Tokens")
    return sequences
