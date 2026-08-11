#!/usr/bin/env python3
"""
Aktivierungs-Quantisierungs-Simulation: Verursacht die int16-Aktivierungs-
quantisierung (zwischen den Layern) den Perplexitäts-Blow-up?

Registriert einen Forward-Hook auf jedem Transformer-Block, der den
Residualstrom (Hidden-Zustand) nach dem Block auf int16 quantisiert
(Zweierpotenz-Skala aus dem laufenden AbsMax, wie im Integer-Pfad). Alles
andere bleibt float. Zeigt diese Simulation ~73, ist die
Aktivierungsquantisierung der Flaschenhals; zeigt sie ~15, liegt es an
LUTs/anderer Integer-Arithmetik.

Gleitkomma erlaubt (Simulation). Kein Teil des Auslieferungspfads.

Usage: python activation_quant_simulation.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
sys.path.insert(0, str(REPO / "eval"))
from src.loader import load_reference_model  # noqa: E402
from wikitext_common import select_sequences  # noqa: E402


def quantize_int16_pow2(x):
    import torch
    absmax = x.abs().max()
    if absmax < 1e-9:
        return x
    shift = torch.floor(torch.log2(32767.0 / absmax))
    scale = 2.0 ** shift
    q = torch.clamp(torch.round(x * scale), -32768, 32767)
    return q / scale


def main():
    import torch
    import math

    model, tok = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    device = model.device
    sequences = select_sequences(4, 128)
    print(f"[sim] {len(sequences)} Sequenzen")

    layers = model.model.layers

    def make_hook():
        def hook(module, input, output):
            # Der Decoder-Block liefert in transformers 5.x nur den
            # Hidden-Zustand (Tensor), kein Tupel.
            if isinstance(output, tuple):
                hidden = output[0]
                hq = quantize_int16_pow2(hidden.float()).to(hidden.dtype)
                return (hq,) + output[1:]
            return quantize_int16_pow2(output.float()).to(output.dtype)
        return hook

    def run(quantize_activations):
        handles = []
        if quantize_activations:
            for layer in layers:
                handles.append(layer.register_forward_hook(make_hook()))
        total_logp = 0.0
        total_tokens = 0
        try:
            with torch.no_grad():
                for ids in sequences:
                    input_ids = torch.tensor([ids], device=device)
                    logits = model(input_ids=input_ids).logits
                    shift_logits = logits[0, :-1, :]
                    targets = input_ids[0, 1:]
                    log_probs = torch.log_softmax(shift_logits.float(), dim=-1)
                    tok_logp = log_probs.gather(1, targets.unsqueeze(1)).squeeze(1)
                    total_logp += tok_logp.sum().item()
                    total_tokens += targets.numel()
        finally:
            for h in handles:
                h.remove()
        return math.exp(-total_logp / total_tokens), total_tokens

    ppl_ref, ntok = run(quantize_activations=False)
    print(f"[sim] Referenz (float, keine Akt-Quant.):  {ppl_ref:.2f} ({ntok} Pos.)")

    ppl_q, ntok = run(quantize_activations=True)
    print(f"[sim] mit int16-Aktivierungsquantisierung: {ppl_q:.2f} ({ntok} Pos.)")

    print(f"\n[sim] Integer-Pfad (real): 73.15")
    print(f"[sim] -> Wenn ~73: Aktivierungsquantisierung ist der Flaschenhals.")
    print(f"[sim] -> Wenn ~15: es liegt an LUTs / anderer Integer-Arithmetik.")


if __name__ == "__main__":
    main()
