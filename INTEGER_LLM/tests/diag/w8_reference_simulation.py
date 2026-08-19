#!/usr/bin/env python3
"""
Trennt "Quantisierungsschema unzureichend" von "Implementierungsfehler".

Die entscheidende Frage (2026-08-19), nachdem sieben Kandidaten gemessen
ausgeschlossen sind: Liegt der 7B-Perplexitaetsverlust am SCHEMA (W8A16
reicht fuer 7B nicht) oder an unserer IMPLEMENTIERUNG (irgendwo im
Integer-Pfad steckt ein Fehler)?

Das laesst sich sauber trennen: Wir bauen exakt dasselbe
Gewichtsquantisierungs-Schema in PyTorch nach (int8, symmetrisch,
Per-Channel-Zweierpotenz-Skalen — identisch zu
calibrate/src/quantize.py::quantize_symmetric_int8_per_channel), lassen
alles andere in float, und messen die Perplexitaet.

    Ergebnis ~8,7  -> W8 ist unschuldig, der Fehler liegt bei uns
    Ergebnis ~40   -> das Schema selbst traegt bei 7B nicht

Das ist der Test, der die Frage "andere Quantisierungen funktionieren
doch auch" direkt beantwortet.

Gleitkomma erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage: INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/w8_reference_simulation.py
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

MAX_FRAC_BITS = 20


def quantisiere_int8_per_channel(W):
    """Identisch zu calibrate/src/quantize.py: symmetrisch, int8,
    eine Zweierpotenz-Skala je Ausgabe-Zeile."""
    import torch
    absmax = W.abs().amax(dim=1, keepdim=True).clamp(min=1e-9)
    shift = torch.floor(torch.log2(127.0 / absmax)).clamp(0, MAX_FRAC_BITS)
    skala = torch.pow(2.0, shift)
    q = torch.clamp(torch.round(W * skala), -128, 127)
    return q / skala


def main():
    import torch

    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    sequences = select_sequences(4, seq_len, verbose=False)
    model, _ = load_reference_model(MODEL_DIR)

    ziel = ("q_proj", "k_proj", "v_proj", "o_proj",
            "gate_proj", "up_proj", "down_proj")
    n = 0
    with torch.no_grad():
        for name, module in model.named_modules():
            if name.endswith(ziel) and hasattr(module, "weight"):
                W = module.weight.data.float()
                module.weight.data = quantisiere_int8_per_channel(W).to(module.weight.dtype)
                n += 1
    print(f"[w8] {n} lineare Projektionen auf int8 per-channel quantisiert")

    sum_logp, count = 0.0, 0
    with torch.no_grad():
        for ids in sequences:
            input_ids = torch.tensor([ids], device=model.device)
            logits = model(input_ids=input_ids).logits[0].float()
            logp = torch.log_softmax(logits[:-1], dim=-1)
            ziel_ids = torch.tensor(ids[1:], device=logp.device)
            tl = logp.gather(1, ziel_ids.unsqueeze(1)).squeeze(1)
            sum_logp += tl.sum().item()
            count += tl.numel()

    ppl = math.exp(-sum_logp / count)
    print()
    print(f"[w8] Perplexitaet mit int8-Per-Channel-Gewichten, Rest float: "
          f"{ppl:.2f} ({count} Positionen)")
    print(f"[w8] FP-Baseline: 8.68   |   unser Integer-Pfad: 41.42")
    print()
    if ppl < 12:
        print("[w8] -> Das SCHEMA traegt. Der Fehler liegt in unserer Implementierung.")
    else:
        print("[w8] -> Das Schema selbst verliert bereits deutlich.")


if __name__ == "__main__":
    main()
