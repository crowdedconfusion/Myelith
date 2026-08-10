"""
Laedt das HF-Referenzmodell (BF16/FP16) fuer die Offline-Kalibrierung.
Float ist hier erlaubt – der Output wird in Integer-Artefakte ueberfuehrt.
"""

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer


def load_reference_model(model_name: str = "Qwen/Qwen2.5-0.5B-Instruct"):
    """Laedt das Modell in BF16 fuer die Kalibrierung."""
    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        torch_dtype=torch.bfloat16,
        device_map="auto",
    )
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    return model, tokenizer
