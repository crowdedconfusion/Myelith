#!/usr/bin/env python3
"""
Aktivierungs-Ausreißer-Analyse: Sind die Aktivierungs-Ausreißer an den
Linear-Layer-Eingängen (den RMSNorm-Ausgaben) kanal-konzentriert?

Nur dann lohnt sich eine SmoothQuant-artige Skalen-Umverteilung
(per-Kanal-Skalierung der Aktivierung, invers in die Gewichte). Gemessen
wird pro Kanal das AbsMax über die Kalibrier-Sequenzen sowie
Konzentrations-Kennzahlen (max/median, Anteil der Kanäle die >50% der
Energie tragen).

Gleitkomma erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python activation_outlier_analysis.py
"""
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))
from src.loader import load_reference_model  # noqa: E402

PROMPTS = [
    "Die numerische Stabilitaet von Fixed-Point-Inferenz ist entscheidend "
    "fuer die Bitgleichheit ueber unabhaengige Knoten hinweg.",
    "Decentralized consensus networks coordinate independent nodes by "
    "verifying identical computation, and deterministic integer "
    "arithmetic enables dispute resolution through bisection.",
    "Ein Agent plant mehrere Schritte, ruft Werkzeuge auf und beachtet "
    "dabei Budgetgrenzen, bevor er eine Transaktion signiert.",
    "Quantization maps floating point weights to int8 with calibrated "
    "power-of-two scales; lookup tables approximate nonlinear functions "
    "such as silu, exp, rsqrt and the rotary position embeddings.",
]


def main():
    import torch

    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()

    # Per-Kanal-AbsMax für die RMSNorm-Ausgaben (= Linear-Layer-Eingänge).
    chan_absmax = {}

    def make_hook(name):
        def hook(module, input, output):
            t = output
            if isinstance(t, tuple):
                t = t[0]
            # t: [1, seq, hidden] -> pro Kanal max über seq
            per_chan = t.detach().float().abs().amax(dim=(0, 1))  # [hidden]
            if name not in chan_absmax:
                chan_absmax[name] = torch.zeros_like(per_chan)
            chan_absmax[name] = torch.maximum(chan_absmax[name], per_chan)
        return hook

    handles = []
    for name, module in model.named_modules():
        if name.endswith("input_layernorm") or name.endswith("post_attention_layernorm") \
                or name == "model.norm":
            handles.append(module.register_forward_hook(make_hook(name)))

    # Tokenizer laden und Kalibrier-Prompts durch das Modell spielen.
    from transformers import AutoTokenizer
    tok = AutoTokenizer.from_pretrained(REPO / "models" / "Qwen2.5-0.5B")
    with torch.no_grad():
        for prompt in PROMPTS:
            inputs = tok(prompt, return_tensors="pt").to(model.device)
            _ = model(**inputs)

    for h in handles:
        h.remove()

    import numpy as np
    print(f"{'Modul':42s} {'Kanäle':>6} {'max':>9} {'median':>9} "
          f"{'max/med':>8} {'Energie% top-1%':>16}")
    for name, ca in sorted(chan_absmax.items()):
        v = ca.cpu().numpy()
        n = len(v)
        mx = v.max()
        med = np.median(v)
        # Energie-Anteil der obersten 1% Kanäle
        order = np.sort(v)[::-1]
        k = max(1, n // 100)
        energy_top = (order[:k] ** 2).sum() / (v ** 2).sum() * 100
        print(f"{name:42s} {n:>6} {mx:>9.3f} {med:>9.3f} "
              f"{mx/med:>8.1f} {energy_top:>15.1f}%")

    print("\nInterpretation: max/med >> 1 und hohe Energie-Konzentration in")
    print("wenigen Kanälen => Ausreißer sind kanal-konzentriert => SmoothQuant")
    print("(per-Kanal-Skalierung) ist anwendbar und vielversprechend.")


if __name__ == "__main__":
    main()
