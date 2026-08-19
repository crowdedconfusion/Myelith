#!/usr/bin/env python3
"""
Misst die Dynamik EINES Kanals ueber die Positionen hinweg.

Die Kernfrage (2026-08-19), die bisher nie gestellt wurde: Per-Kanal-Skalen
(Fund 20) helfen genau dann, wenn jeder EINZELNE Kanal einen schmalen
Wertebereich hat und die Kanaele sich nur untereinander unterscheiden.
Sie helfen NICHT, wenn derselbe Kanal ueber die Positionen hinweg eine
riesige Spanne hat — dann braucht dieser Kanal allein mehr Bits, als int16
hergibt, und keine Skalenplatzierung rettet ihn.

Genau das ist die praezise Form der "Massive Activations": nicht
"verschiedene Kanaele haben verschiedene Groessen", sondern "derselbe Kanal
ist an Position 0 um Groessenordnungen groesser als an allen anderen".

Gemessen wird je Kanal: max ueber Positionen / Median ueber Positionen.
Das noetige Bitbudget ist log2(dieses Verhaeltnisses) plus die Bits fuer
die gewuenschte Aufloesung des Median-Werts.

Gleitkomma erlaubt - Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage: INTEGER_LLM_MODEL=qwen2.5-7b python tests/diag/channel_dynamic_range.py
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

    seq_len = int(os.environ.get("E2E_SEQ_LEN", "128"))
    sequences = select_sequences(2, seq_len, verbose=False)
    model, _ = load_reference_model(MODEL_DIR)

    gesammelt = {}

    def mach_hook(name):
        def hook(module, inputs, output):
            x = inputs[0]
            if not isinstance(x, torch.Tensor):
                return
            # [positionen, kanaele]
            t = x.detach().float().reshape(-1, x.shape[-1]).abs().cpu()
            gesammelt.setdefault(name, []).append(t)
        return hook

    handles = []
    ziel = ["model.layers.4.input_layernorm", "model.layers.10.input_layernorm",
            "model.layers.16.input_layernorm"]
    for name, module in model.named_modules():
        if name in ziel:
            handles.append(module.register_forward_hook(mach_hook(name)))

    with torch.no_grad():
        for ids in sequences:
            model(input_ids=torch.tensor([ids], device=model.device))
    for h in handles:
        h.remove()

    print("Dynamik EINES Kanals ueber die Positionen (max / Median):")
    print("Noetige Bits = log2(max/median) + Bits fuer die Aufloesung des Medians")
    print()
    for name in ziel:
        if name not in gesammelt:
            continue
        t = torch.cat(gesammelt[name], dim=0)     # [positionen, kanaele]
        kanal_max = t.amax(dim=0)
        kanal_med = t.median(dim=0).values.clamp(min=1e-9)
        verhaeltnis = (kanal_max / kanal_med)
        bits = torch.log2(verhaeltnis.clamp(min=1.0))
        print(f"{name}:")
        print(f"   Kanal-Dynamik (max/median): Median ueber Kanaele {verhaeltnis.median():.1f}x, "
              f"schlimmster Kanal {verhaeltnis.max():.0f}x")
        print(f"   Noetige Bits allein fuer die Dynamik: Median {bits.median():.1f}, "
              f"schlimmster {bits.max():.1f}")
        # Wie viele Kanaele sprengen int16 (15 Bit + Vorzeichen), wenn man
        # dem Median nur 4 Bit Aufloesung zugesteht?
        sprengt = (bits > 15 - 4).sum().item()
        print(f"   Kanaele, die int16 sprengen (Dynamik > 11 Bit): "
              f"{sprengt}/{len(bits)} ({sprengt/len(bits):.1%})")
        print()


if __name__ == "__main__":
    main()
