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
    skaliert = torch.round(t * (2.0 ** shifts))
    # **Fund 23 (2026-08-19): stilles Clipping wird laut.**
    # Fuer absmax > 127 braeuchte es einen NEGATIVEN Shift; das
    # clamp(shifts, 0, ...) oben verbietet den, und torch.clamp unten
    # schnitt den Wert dann kommentarlos auf 127 ab. Bei Qwen2.5-7B traf
    # das den k_proj-Bias in den Ebenen 0 und 27 (Spitzenwert 414 -> 127,
    # also 69 % Verlust) und verfaelschte die Attention ab der ERSTEN
    # Ebene. Gewichtszeilen waren nie betroffen (0 von 1 694 720), nur
    # Biases (16 von 129 024) — deshalb tragen Biases seit theta_v 0.13.0
    # int16 (siehe quantize_bias_int16_per_element).
    #
    # Diese Pruefung ist die eigentliche Lehre: ein Quantisierer darf
    # nicht stillschweigend saettigen. Lieber ein lauter Abbruch als ein
    # Artefakt, das monatelang wie Quantisierungsrauschen aussieht.
    ueberlauf = int((skaliert.abs() > 127).sum())
    if ueberlauf:
        schlimmster = float(t.abs().max())
        raise ValueError(
            f"int8-Quantisierung wuerde {ueberlauf} Werte saettigen "
            f"(groesster Betrag {schlimmster:.2f} > 127). Das waere stiller "
            f"Genauigkeitsverlust. Fuer Tensoren mit Betraegen ueber 127 "
            f"int16 verwenden (quantize_symmetric_int16_per_channel bzw. "
            f"quantize_bias_int16_per_element)."
        )
    quantized = skaliert.to(torch.int8)
    if was_1d:
        quantized = quantized.squeeze(1)
    return {
        "int8": quantized.numpy(),
        "shifts": shifts.squeeze(1).round().to(torch.int8).numpy(),
        "shape": list(quantized.shape),
    }


def quantize_bias_int16_per_element(tensor: torch.Tensor) -> dict:
    """
    Bias-Quantisierung nach INT16 mit eigener Zweierpotenz-Skala je Element.

    **Fund 23 (theta_v 0.13.0).** Biases lagen bis 0.12.0 in int8 und
    saettigten dadurch still bei Betraegen ueber 127. Qwen2.5-7B traf das
    hart: der k_proj-Bias erreicht in Ebene 27 den Wert 414 und in Ebene 0
    den Wert 171 — beide wurden auf 127 abgeschnitten (69 % bzw. 26 %
    Verlust). Da der Fehler in Ebene 0 sitzt, verfaelschte er die Attention
    ab dem ersten Layer und propagierte durch alle 28.

    int16 ist hier praktisch gratis: Biases sind 1D und winzig (129 024
    Werte bei 7B gegen 1,7 Mio. Gewichtszeilen), die Artefaktgroesse waechst
    um ~0,25 MB. Der Wertebereich waechst um Faktor 256 und deckt die
    beobachteten Betraege muehelos.

    Warum je ELEMENT und nicht je Tensor: identisch zur Begruendung von
    quantize_symmetric_int8_per_channel — ein einzelner grosser Eintrag
    wuerde sonst die Aufloesung aller uebrigen zerstoeren. Bei 1D-Tensoren
    ist "je Zeile" gleichbedeutend mit "je Element".

    Returns: {"int16": np.ndarray[int16], "shifts": np.ndarray[int8] je
              Element, "shape": [...]}
    """
    t = tensor.detach().float().cpu()
    assert t.dim() == 1, "quantize_bias_int16_per_element erwartet einen 1D-Tensor"

    absmax = t.abs()
    shifts = torch.where(
        absmax < 1e-9,
        torch.zeros_like(absmax),
        torch.floor(torch.log2(32767.0 / absmax.clamp(min=1e-9))),
    )
    shifts = torch.clamp(shifts, 0, MAX_FRAC_BITS)
    skaliert = torch.round(t * (2.0 ** shifts))

    # Dieselbe laute Pruefung wie im int8-Pfad: saettigt es hier trotzdem,
    # ist der Wert jenseits von 32767 und braucht eine eigene Entscheidung.
    ueberlauf = int((skaliert.abs() > 32767).sum())
    if ueberlauf:
        raise ValueError(
            f"int16-Bias-Quantisierung wuerde {ueberlauf} Werte saettigen "
            f"(groesster Betrag {float(t.abs().max()):.2f}). Das braucht "
            f"eine eigene Design-Entscheidung, kein stilles Abschneiden."
        )

    return {
        "int16": skaliert.to(torch.int16).numpy(),
        "shifts": shifts.round().to(torch.int8).numpy(),
        "shape": list(t.shape),
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

    **Attention-Biases liegen seit theta_v 0.13.0 in int16** (Fund 23):
    int8 saettigte still bei Betraegen ueber 127, und Qwen2.5-7B hat in
    k_proj.bias Werte bis 414 — in Ebene 0 und 27, also unter anderem an
    der ersten Ebene, deren Fehler durch alle folgenden propagiert.
    Betroffen waren ausschliesslich Biases (16 von 129 024 Elementen),
    keine einzige Gewichtszeile (0 von 1 694 720). Biases sind 1D und
    winzig; int16 kostet ~0,25 MB und beseitigt das Problem vollstaendig.

    Returns: Dict[tensor_name -> {int8|int16, shifts, shape}]
    """
    quantized = {}
    target_keys = [
        "embed_tokens", "lm_head",
        "self_attn.q_proj", "self_attn.k_proj", "self_attn.v_proj", "self_attn.o_proj",
        "mlp.gate_proj", "mlp.up_proj", "mlp.down_proj",
        "input_layernorm", "post_attention_layernorm", "norm",
    ]

    for name, param in model.named_parameters():
        if not any(key in name for key in target_keys):
            continue
        if name.endswith(".bias"):
            quantized[name] = quantize_bias_int16_per_element(param)
        else:
            quantized[name] = quantize_symmetric_int8_per_channel(param)

    return quantized
