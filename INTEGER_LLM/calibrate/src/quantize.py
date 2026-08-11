"""
Symmetrische INT8-Quantisierung von HF-Modell-Gewichten.
"""

import torch
import numpy as np
from pathlib import Path
from typing import Dict, Tuple

# Obergrenze fuer "shift" (= frac_bits), damit act_frac_bits + weight_frac_bits
# in der Laufzeit (runtime/src/kernels/fixed_point.rs::rescale) sicher unter
# 128 bleibt - dort wird die Summe nach i8 gecastet, ein Ueberlauf wuerde
# stillschweigend falsche Vorzeichen erzeugen. 20 laesst dafuer reichlich
# Reserve (siehe Anhang B.5.2 des Whitepapers zur Akkumulator-Reserve).
MAX_FRAC_BITS = 20


def quantize_symmetric_int8(tensor: torch.Tensor) -> Tuple[np.ndarray, float, int]:
    """
    Symmetrische per-tensor Quantisierung nach INT8, Zweierpotenz-Skala.
    Returns: (quantized_array, scale, shift)

    Hinweis (theta_v 0.7.0): fuer Gewichte wird diese Funktion nicht mehr
    verwendet — Per-Tensor-Skalen zerstoeren bei Matrizen mit Ausreisser-
    AbsMax 10–17 % der Eintraege (sie werden zu 0 gerundet). Gewichte werden
    per-channel quantisiert (quantize_symmetric_int8_per_channel). Diese
    Funktion bleibt fuer Spezialfaelle und Tests erhalten.
    """
    t = tensor.detach().float().cpu()
    absmax = t.abs().max().item()
    if absmax < 1e-9:
        return np.zeros(t.shape, dtype=np.int8), 1.0, 0

    shift = int(np.floor(np.log2(127.0 / absmax)))
    shift = max(0, min(shift, MAX_FRAC_BITS))
    scale_pow2 = 2.0 ** (-shift)

    quantized = torch.clamp(torch.round(t * (2.0 ** shift)), -128, 127).to(torch.int8)
    return quantized.numpy(), scale_pow2, shift


def quantize_symmetric_int8_per_channel(tensor: torch.Tensor) -> dict:
    """
    Symmetrische Per-Channel-Quantisierung nach INT8 mit eigener
    Zweierpotenz-Skala je Zeile (Achse 0; bei 1D-Tensoren je Element).

    Standard seit theta_v 0.7.0 (Eskalation nach dem Entscheidungspunkt
    12.21): Per-Tensor-Skalen zerstoerten bei Projektionsmatrizen 10–17 %
    der Eintraege, weil der AbsMax 17–34x ueber der typischen Groesse liegt;
    per-channel sind es 0,0 %. Determinismus bleibt unberuehrt: alle Skalen
    sind Zweierpotenzen, alle Runtime-Operationen bleiben ganzzahlig.

    Returns: {"int8": np.ndarray[int8], "shifts": np.ndarray[int8] je Zeile,
              "shape": [...]}
    """
    t = tensor.detach().float().cpu()
    # 1D-Tensoren (Bias, Gamma) werden als Spaltenvektor behandelt, damit
    # die Broadcasting-Multiplikation t * 2^shifts nicht zu einer [n,n]-
    # Matrix aufblaest (Fund 11: q_proj.bias wurde zu 896x896 expandiert).
    was_1d = t.dim() == 1
    if was_1d:
        t = t.unsqueeze(1)

    absmax = t.abs().amax(dim=tuple(range(1, t.dim())), keepdim=True)

    shifts = torch.where(
        absmax < 1e-9,
        torch.zeros_like(absmax),
        torch.floor(torch.log2(127.0 / absmax.clamp(min=1e-9))),
    )
    shifts = torch.clamp(shifts, 0, MAX_FRAC_BITS)
    quantized = torch.clamp(torch.round(t * (2.0 ** shifts)), -128, 127).to(torch.int8)
    if was_1d:
        quantized = quantized.squeeze(1)
    return {
        "int8": quantized.numpy(),
        "shifts": shifts.squeeze(1).round().to(torch.int8).numpy(),
        "shape": list(quantized.shape),
    }


def quantize_symmetric_int16_per_channel(tensor: torch.Tensor) -> dict:
    """
    Symmetrische Per-Channel-Quantisierung nach INT16 mit eigener
    Zweierpotenz-Skala je Zeile (Achse 0).

    Benannte Ausnahme der spec (theta_v 0.6.0, Eskalation nach dem
    Entscheidungspunkt 12.21): der LM-Head entscheidet bei Spannweiten von
    wenigen Einheiten direkt über die Token-Rangfolge; die int8-Per-Tensor-
    Quantisierung der geteilten Embedding-Tabelle hatte dort ~20 % relativen
    Fehler pro Eintrag und die Logits unbrauchbar gemacht. Per-Channel-int16
    senkt den Fehler um ~4 Größenordnungen und bleibt vollständig
    ganzzahlig und deterministisch (Zweierpotenz-Skalen, arithmetische
    Shifts in der Runtime).

    Returns: {"int16": np.ndarray[int16], "shifts": np.ndarray[int8] je Zeile,
              "shape": [...]}
    """
    t = tensor.detach().float().cpu()
    absmax = t.abs().amax(dim=1, keepdim=True)  # [rows, 1]

    shifts = torch.where(
        absmax < 1e-9,
        torch.zeros_like(absmax),
        torch.floor(torch.log2(32767.0 / absmax.clamp(min=1e-9))),
    )
    shifts = torch.clamp(shifts, 0, MAX_FRAC_BITS)
    scale = 2.0 ** shifts

    quantized = torch.clamp(torch.round(t * scale), -32768, 32767).to(torch.int16)
    return {
        "int16": quantized.numpy(),
        "shifts": shifts.squeeze(1).round().to(torch.int8).numpy(),
        "shape": list(quantized.shape),
    }


def quantize_model_weights(model) -> Dict[str, dict]:
    """
    Quantisiert alle relevanten Gewichte eines HF-Modells per-channel
    (theta_v 0.7.0: Zweierpotenz-Skala je Ausgabe-Zeile; bei 1D-Tensoren
    wie LayerNorm-Gammas je Element).
    Returns: Dict[tensor_name -> {int8_data, shifts, shape}]
    """
    quantized = {}
    target_keys = [
        "embed_tokens", "lm_head",
        "self_attn.q_proj", "self_attn.k_proj", "self_attn.v_proj", "self_attn.o_proj",
        "mlp.gate_proj", "mlp.up_proj", "mlp.down_proj",
        "input_layernorm", "post_attention_layernorm", "norm",
    ]

    for name, param in model.named_parameters():
        if any(key in name for key in target_keys):
            quantized[name] = quantize_symmetric_int8_per_channel(param)

    return quantized
