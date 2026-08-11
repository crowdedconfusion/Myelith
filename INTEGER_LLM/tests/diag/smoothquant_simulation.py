#!/usr/bin/env python3
"""
SmoothQuant-Simulation: Quantisierungsfehler eines Linear-Layers mit und ohne
per-Kanal-Skalierung der Aktivierung.

Für einen besonders auswreißer-behafteten Linear-Layer (Eingang = RMSNorm-
Ausgabe mit hohem max/median-Verhältnis) wird der quantisierte Ausgang
simuliert:
  (a) IST: Aktivierung int16 mit Per-Layer-Skala, Gewicht int8 per-Channel.
  (b) SmoothQuant: per-Kanal-Skalierung s der Aktivierung (und invers in die
      Gewichtsspalten), dann dieselbe Quantisierung.
Gemessen wird der relative Ausgangsfehler gegenüber dem Float-Layer. Zeigt
(b) deutlich weniger Fehler, lohnt sich die Umsetzung.

Gleitkomma erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python smoothquant_simulation.py
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


def pow2_scale_from_absmax(absmax, bits):
    """Zweierpotenz-Skala: shift = floor(log2((2^(bits-1)-1)/absmax))."""
    import math
    if absmax <= 0:
        return 0
    return int(math.floor(math.log2(((1 << (bits - 1)) - 1) / absmax)))


def main():
    import torch
    import numpy as np

    model, _ = load_reference_model(REPO / "models" / "Qwen2.5-0.5B")
    model.eval()
    from transformers import AutoTokenizer
    tok = AutoTokenizer.from_pretrained(REPO / "models" / "Qwen2.5-0.5B")

    # Ziel-Layer: gate_proj von Block 3 (Eingang = post_attention_layernorm
    # Block 3, dort max/median ~126).
    target = model.model.layers[3].mlp.gate_proj
    norm_src = model.model.layers[3].post_attention_layernorm

    acts = []

    def hook(module, input, output):
        t = output if not isinstance(output, tuple) else output[0]
        acts.append(t.detach().float().cpu())

    h = norm_src.register_forward_hook(hook)
    with torch.no_grad():
        for prompt in PROMPTS:
            inputs = tok(prompt, return_tensors="pt").to(model.device)
            _ = model(**inputs)
    h.remove()

    x = torch.cat([a.reshape(-1, a.shape[-1]) for a in acts], dim=0).numpy()  # [N, in]
    W = target.weight.detach().float().cpu().numpy()  # [out, in]
    y_float = x @ W.T  # [N, out]

    in_dim = x.shape[1]
    bits_act, bits_w = 16, 8

    def quantize_linear(x_, W_, s_shift=None):
        """Quantisierter Layer. s_shift: None = IST (Per-Layer-Aktivierungs-
        skala). Sonst echtes SmoothQuant: Aktivierungs-Kanal j wird mit
        2^-s_shift[j] skaliert, Gewichtsspalte j mit 2^+s_shift[j] (wird von
        der per-Channel-Gewichtsquantisierung absorbiert). Das Produkt bleibt
        vor der Quantisierung identisch, nur die Quantisierung ändert sich."""
        xs = x_.copy()
        Ws = W_.copy()
        if s_shift is not None:
            s = 2.0 ** s_shift.astype(np.float64)      # [in]
            xs = xs / s[None, :]                        # Aktivierung runter
            Ws = Ws * s[None, :]                        # Gewichtsspalten rauf
        # Aktivierung: Per-Layer-Zweierpotenz-Skala, int16
        absmax_x = np.abs(xs).max()
        shift_x = pow2_scale_from_absmax(absmax_x, bits_act)
        xq = np.clip(np.round(xs * (2.0 ** shift_x)), -(1 << 15), (1 << 15) - 1)
        xdq = xq / (2.0 ** shift_x)
        # Gewicht: per-Channel (pro Ausgabe-Zeile), int8
        absmax_w = np.abs(Ws).max(axis=1, keepdims=True)
        shift_w = np.floor(np.log2(127.0 / np.maximum(absmax_w, 1e-9)))
        shift_w = np.clip(shift_w, 0, 20)
        Wq = np.clip(np.round(Ws * (2.0 ** shift_w)), -128, 127)
        Wdq = Wq / (2.0 ** shift_w)
        return xdq @ Wdq.T

    # (a) IST: Per-Layer-Aktivierungsskala
    y_ist = quantize_linear(x, W, s_shift=None)
    err_ist = np.linalg.norm(y_ist - y_float) / np.linalg.norm(y_float)

    # (b) SmoothQuant: per-Kanal-Shift aus Aktivierungs-AbsMax pro Kanal,
    #     balanciert mit Gewichtsspaltmax (alpha=0.5, geglättet)
    chan_absmax_x = np.maximum(np.abs(x).max(axis=0), 1e-9)   # [in]
    chan_absmax_w = np.maximum(np.abs(W).max(axis=0), 1e-9)   # [in]
    alpha = 0.5
    s_cont = (chan_absmax_x ** alpha) / (chan_absmax_w ** (1.0 - alpha))
    s_shift = np.clip(np.round(np.log2(s_cont / np.median(s_cont))), -8, 8)
    y_sq = quantize_linear(x, W, s_shift=s_shift)
    err_sq = np.linalg.norm(y_sq - y_float) / np.linalg.norm(y_float)

    print(f"Ziel: layers.3.mlp.gate_proj  (in_dim={in_dim})")
    print(f"Kanal-AbsMax Aktivierung: max={chan_absmax_x.max():.2f} "
          f"median={np.median(chan_absmax_x):.2f} "
          f"max/median={chan_absmax_x.max()/np.median(chan_absmax_x):.1f}")
    print(f"Rel. Ausgangsfehler IST    (Per-Layer-Skala):      {err_ist:.5f}")
    print(f"Rel. Ausgangsfehler SmoothQuant (alpha=0.5):       {err_sq:.5f}")
    if err_sq < err_ist:
        print(f"=> SmoothQuant reduziert den Fehler um Faktor {err_ist/err_sq:.2f}")
    else:
        print("=> SmoothQuant bringt hier keine Verbesserung.")


if __name__ == "__main__":
    main()
