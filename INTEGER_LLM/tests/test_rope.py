#!/usr/bin/env python3
"""
Regressionstest RoPE (Fund-15-Fix, theta_v 0.10.0): Die Integer-RoPE
(Multi-Frequenz, half-split-Paarung) muss mit HF's apply_rotary_pos_emb
übereinstimmen (q*cos + rotate_half(q)*sin, pro Dimensions-Paar eigene
Frequenz theta_j = 1/rope_theta^(j/half)).

Reproduziert die Kernel-Arithmetik (rotate_half_split_i16: rshift_round mit
Round-to-nearest-even) in Python und vergleicht gegen die Float-Referenz.
Kein pytest, eigenständiges Skript nach Projektkonvention; kein torch/numpy
nötig.
"""

import math
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.luts import generate_rope_luts, load_nonlinear_spec  # noqa: E402
from src.model_configs import get_model_config  # noqa: E402


def _rshift_round(v: int, n: int) -> int:
    """Arithmetischer Rechtsshift mit Round-to-nearest-even (Kernels)."""
    q = v >> n
    rem = v - (q << n)
    half = 1 << (n - 1)
    if rem > half or (rem == half and (q & 1)):
        q += 1
    return q


def _int_rotate(x, cos_row, sin_row, frac):
    head_dim = len(x)
    half = head_dim // 2
    out = [0] * head_dim
    for j in range(half):
        c, s = cos_row[j], sin_row[j]
        x0, x1 = x[j], x[j + half]
        out[j] = _rshift_round(x0 * c - x1 * s, frac)
        out[j + half] = _rshift_round(x1 * c + x0 * s, frac)
    return out


def _hf_rotate(reals, pos, rope_theta, half):
    out = [0.0] * (2 * half)
    for j in range(half):
        ang = pos * (1.0 / (rope_theta ** (j / half)))
        c, s = math.cos(ang), math.sin(ang)
        out[j] = reals[j] * c - reals[j + half] * s
        out[j + half] = reals[j + half] * c + reals[j] * s
    return out


def test_rope_matches_hf_at_positions():
    nl = load_nonlinear_spec()
    head_dim = get_model_config("qwen2.5-0.5b")["head_dim"]
    rope_theta = nl["rope"]["rope_theta"]
    frac = nl["rope"]["frac_bits"]
    half = head_dim // 2
    max_seq = nl["rope"]["max_seq_len"]

    sin_lut, cos_lut = generate_rope_luts(max_seq_len=max_seq, head_dim=head_dim,
                                          rope_theta=rope_theta, frac_bits=frac)

    random.seed(1234)
    frac_q = 8  # Aktivierungsskala der Q/K-Werte
    for pos in (0, 1, 2, 7, 63, 2047):
        cos_row = cos_lut[pos * half:(pos + 1) * half]
        sin_row = sin_lut[pos * half:(pos + 1) * half]
        reals = [random.uniform(-5, 5) for _ in range(head_dim)]
        x = [round(r * (1 << frac_q)) for r in reals]
        iq = _int_rotate(x, cos_row, sin_row, frac)
        hf = _hf_rotate(reals, pos, rope_theta, half)
        max_err = max(abs(iq[j] / (1 << frac_q) - hf[j]) for j in range(head_dim))
        # Wenige Rundungen à 1/256 pro Operation; Toleranz deutlich darüber,
        # aber weit unter jedem Strukturfehler (alte RoPE: Fehler ~ O(1)).
        assert max_err < 0.03, f"Position {pos}: max_err {max_err:.4f} zu groß"


def test_rope_position_zero_identity():
    nl = load_nonlinear_spec()
    head_dim = get_model_config("qwen2.5-0.5b")["head_dim"]
    half = head_dim // 2
    sin_lut, cos_lut = generate_rope_luts(
        max_seq_len=nl["rope"]["max_seq_len"], head_dim=head_dim,
        rope_theta=nl["rope"]["rope_theta"], frac_bits=nl["rope"]["frac_bits"])
    # Position 0: alle Winkel 0 -> cos = 1.0 (256), sin = 0 -> Identität.
    assert all(v == 256 for v in cos_lut[:half])
    assert all(v == 0 for v in sin_lut[:half])


if __name__ == "__main__":
    test_rope_matches_hf_at_positions()
    print("[test] Integer-RoPE stimmt mit HF überein (Pos 0/1/2/7/63/2047): PASSED")
    test_rope_position_zero_identity()
    print("[test] RoPE Position 0 ist Identität: PASSED")
    print("Alle Tests bestanden.")
