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

    "shift" bezeichnet hier frac_bits in der Laufzeit-Konvention (Kap. 6.2 /
    Anhang B.5.4 des Whitepapers): Quantisierung ist eine Linksverschiebung
    um `shift` Bit (quantized = round(t * 2^shift)), Dequantisierung der
    entsprechende arithmetische Rechtsshift (real ≈ quantized >> shift).

    Fruehere Fassung dieser Funktion berechnete "shift" nur fuer den Fall
    absmax > 127 (Wert muss VERKLEINERT werden, um in int8 zu passen) und
    gab sonst unbedingt shift=0 zurueck. Fuer reale Modellgewichte
    (absmax typischerweise deutlich unter 1) bedeutete das:
    quantized = round(t), also praktisch durchgaengig 0. Diese Fassung
    waehlt stattdessen die groesstmoegliche Zweierpotenz-Praezision, die
    absmax noch verlustfrei in [-127, 127] abbildet.
    """
    t = tensor.detach().float().cpu()
    absmax = t.abs().max().item()
    if absmax < 1e-9:
        return np.zeros(t.shape, dtype=np.int8), 1.0, 0

    # Groesstmoegliche Praezision, sodass absmax * 2^shift <= 127. floor() ist
    # zwingend: ceil() koennte den Wertebereich von int8 verletzen. Bei sehr
    # grossem absmax (> 127) wird shift <= 0, geklammert auf 0 - Werte
    # ausserhalb des Bereichs saettigen dann beim Runden/Clamping.
    shift = int(np.floor(np.log2(127.0 / absmax)))
    shift = max(0, min(shift, MAX_FRAC_BITS))
    scale_pow2 = 2.0 ** (-shift)

    # Quantisiere: Linksverschiebung um shift Bit statt Division.
    quantized = torch.clamp(torch.round(t * (2.0 ** shift)), -128, 127).to(torch.int8)
    return quantized.numpy(), scale_pow2, shift


def quantize_model_weights(model) -> Dict[str, dict]:
    """
    Quantisert alle relevanten Gewichte eines HF-Modells.
    Returns: Dict[tensor_name -> {int8_data, scale, shift, shape}]
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
            q_data, scale, shift = quantize_symmetric_int8(param)
            quantized[name] = {
                "int8": q_data,
                "scale": scale,
                "shift": shift,
                "shape": list(q_data.shape),
                "original_dtype": str(param.dtype),
            }

    return quantized
