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

import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DATASETS = REPO / "eval" / "datasets"
WIKITEXT_CACHE = DATASETS / "wikitext2_test.txt"

sys.path.insert(0, str(REPO / "calibrate"))
from src.model_configs import get_export_model_config  # noqa: E402

MODEL_ENV = "INTEGER_LLM_MODEL"
DEFAULT_MODEL = "qwen2.5-0.5b"

# Modellwahl identisch zu calibrate/src/main.py: dieselbe Umgebungsvariable,
# dieselbe Vorgabe, dieselbe verifizierte Konfigurationsquelle. Zwei
# Mechanismen fuer dieselbe Entscheidung waeren zwei Wahrheiten - und ein
# Perplexitaetsvergleich, bei dem Kalibrierung und Messung auf verschiedene
# Modelle zeigen, faellt nicht auf, sondern liefert stillschweigend Unsinn.
MODEL_NAME = os.environ.get(MODEL_ENV, "").strip() or DEFAULT_MODEL
_CONFIG = get_export_model_config(MODEL_NAME)
HF_MODEL_ID = _CONFIG["hf_model_id"]
MODEL_DIR = REPO / "models" / HF_MODEL_ID.split("/")[-1]
ARTIFACTS_DIR = REPO / "artifacts" / MODEL_NAME

# Ergebnisdateien tragen den Modellnamen, damit ein 7B-Lauf die
# 0.5B-Messung nicht ueberschreibt (die belegt den Entscheidungspunkt
# 12.21 und muss reproduzierbar bleiben).
_SUFFIX = "" if MODEL_NAME == DEFAULT_MODEL else f"_{MODEL_NAME.replace('.', '')}"


def ergebnis_pfad(basis: str, endung: str = ".json") -> Path:
    """Pfad einer Ergebnisdatei, je Modell getrennt.

    0.5B behaelt die historischen Dateinamen (baseline_wikitext2.json),
    weil sie in Fahrplan, Whitepaper-Vorarbeit und Changelog zitiert sind.
    """
    return REPO / "eval" / "results" / f"{basis}{_SUFFIX}{endung}"


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
    tokenisiert mit dem Tokenizer des gewaehlten Modells, auf seq_len begrenzt.

    **Die ausgewaehlten Zeilen sind modellunabhaengig, die Token nicht.**
    Stride und Quelltext sind fuer jede Variante gleich; 0.5B und 7B haben
    aber verschiedene Vokabulare (151 936 vs. 152 064), also verschiedene
    Tokenisierungen derselben Zeilen. Das ist richtig so: verglichen wird je
    Modell Integer gegen Gleitkomma auf identischer Tokenisierung. Der
    Vergleich *zwischen* Modellen ist einer der relativen Abstaende, nicht
    der absoluten Perplexitaeten.
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
