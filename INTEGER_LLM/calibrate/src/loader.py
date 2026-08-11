"""
Laedt das HF-Referenzmodell (BF16/FP16) fuer die Offline-Kalibrierung.
Float ist hier erlaubt – der Output wird in Integer-Artefakte ueberfuehrt.

Das Modell wird ausschliesslich aus dem lokalen Snapshot unter models/
geladen (reproduzierbare Herkunft, siehe models/README.md), nie ueber die
HF-ID aus dem impliziten Hugging-Face-Cache.
"""

import torch
from pathlib import Path
from typing import Union
from transformers import AutoModelForCausalLM, AutoTokenizer


def load_reference_model(model_path: Union[str, Path]):
    """Laedt das Modell in BF16 fuer die Kalibrierung (lokaler Snapshot)."""
    path = Path(model_path)
    model = AutoModelForCausalLM.from_pretrained(
        str(path),
        torch_dtype=torch.bfloat16,
        device_map="auto",
    )
    tokenizer = AutoTokenizer.from_pretrained(str(path))
    return model, tokenizer
