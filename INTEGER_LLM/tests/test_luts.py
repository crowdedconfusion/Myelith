#!/usr/bin/env python3
"""
Tests fuer calibrate/src/luts.py (Fahrplan-Punkt 12.17, aktualisiert fuer
theta_v 0.5.0 / v0.12.20): LUT-Generierung ausschliesslich aus den
Parametern von theta_v/spec.json, inkl. der input_shift-Semantik der
rsqrt-LUT (Index x repraesentiert x * 2^-input_shift) und der getrennten
Ein-/Ausgangs-Fraktionierung der SiLU-LUT.

Eigenstaendiges Skript nach Projektkonvention (siehe test_fixed_point.py),
kein pytest, keine torch/numpy-Abhaengigkeit.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.luts import (generate_rsqrt_lut, generate_silu_lut, generate_exp_lut,
                      generate_rope_luts, load_nonlinear_spec)
from src.model_configs import get_model_config


def silu_ref(x):
    return x / (1.0 + math.exp(-x))


def test_load_nonlinear_spec_structure():
    nl = load_nonlinear_spec()
    for key in ("rsqrt", "silu", "softmax", "rope"):
        assert key in nl, f"spec.json-Abchnitt 'nonlinear' ohne '{key}'"
    assert nl["rsqrt"]["input_range"] == [0, 32767]
    assert nl["rsqrt"]["input_shift"] == 8
    assert nl["rsqrt"]["output_frac_bits"] == 8
    assert nl["rsqrt"]["index_normalization"] == "dynamic_even_shift"
    assert nl["silu"]["input_range"] == [-1024, 1023]
    assert nl["silu"]["input_frac_bits"] == 3
    assert nl["silu"]["output_frac_bits"] == 6
    assert nl["softmax"]["exp_lut_range"] == 1024
    assert nl["softmax"]["exp_input_frac_bits"] == 4
    assert nl["softmax"]["exp_lut_frac_bits"] == 8
    assert nl["rope"]["max_seq_len"] == 2048
    assert nl["rope"]["frac_bits"] == 8
    assert nl["rope"]["rope_theta"] == 1000000.0
    assert nl["rope"]["pairing"] == "half_split"


def test_rsqrt_lut_input_shift_semantics():
    # input_shift=8 (spec 0.5.0): Index x steht fuer den Realwert x/256.
    lut = generate_rsqrt_lut(max_input=2048, input_shift=8, frac_bits=8)
    assert lut[0] == 256, "Sentinel fuer x=0 muss 1.0 (scale) sein"
    assert lut[256] == 256, "rsqrt(1.0) = 1.0"
    assert lut[1024] == 128, "rsqrt(4.0) = 0.5"
    assert lut[64] == 512, "rsqrt(0.25) = 2.0"
    assert all(v > 0 for v in lut), "rsqrt ist ueberall positiv"
    assert all(lut[i] >= lut[i + 1] for i in range(1, len(lut) - 1)), \
        "rsqrt muss monoton fallend sein"


def test_rsqrt_lut_input_shift_zero_entspricht_alter_skala():
    # input_shift=0: Index x steht fuer den Realwert x (triviale Skala).
    lut = generate_rsqrt_lut(max_input=16, input_shift=0, frac_bits=8)
    assert lut[1] == 256, "rsqrt(1) = 1.0"
    assert lut[4] == 128, "rsqrt(4) = 0.5"


def test_silu_lut_spot_values():
    # Spec 0.9.0: Indexbereich [-1024, 1023], Eingang frac 3 (Realwert idx/8),
    # Ausgang frac 6. Nullpunkt bei Index 1024 (Offset = -input_min).
    lut = generate_silu_lut(input_min=-1024, input_max=1023,
                            input_frac_bits=3, output_frac_bits=6)
    assert len(lut) == 2048
    assert lut[1024] == 0, "silu(0) = 0"
    assert lut[1024 + 8] == round(silu_ref(1.0) * 64), "silu(1) bei frac 6"
    assert lut[1024 - 16] == round(silu_ref(-2.0) * 64), "silu(-2) bei frac 6"
    assert lut[1024 + 32] == round(silu_ref(4.0) * 64), "silu(4) bei frac 6"
    # Domaeenenrand (Realwert -128 bzw. +127.875) wird abgedeckt.
    assert lut[0] == round(silu_ref(-128.0) * 64)
    assert lut[2047] == round(silu_ref(127.875) * 64)


def test_exp_lut_spot_values():
    # Spec 0.5.2: Domaene [0, 64) mit Eingang frac 4, Ausgang frac 8.
    lut = generate_exp_lut(exp_range=1024, input_frac_bits=4, output_frac_bits=8)
    assert len(lut) == 1025
    assert lut[0] == 256, "exp(0) = 1.0"
    assert lut[16] == round(math.exp(-1.0) * 256), "exp(-1) bei Eingang frac 4"
    assert lut[32] == round(math.exp(-2.0) * 256), "exp(-2)"
    # Am Domaenenrand ist exp(-64) praktisch 0.
    assert lut[1024] == 0
    assert all(lut[i] >= lut[i + 1] for i in range(len(lut) - 1)), \
        "exp(-x) muss monoton fallend sein"


def test_rope_lut_spot_values():
    # Multi-Frequenz-RoPE (theta_v 0.10.0): Index p*half + j, Winkel
    # p * theta_j mit theta_j = 1/rope_theta^(j/half). Kleine Parameter fuer
    # handrechnbare Stuetzwerte: max_seq_len=4, head_dim=4 (half=2).
    half = 2
    sin_lut, cos_lut = generate_rope_luts(max_seq_len=4, head_dim=4,
                                          rope_theta=1000000.0, frac_bits=8)
    assert len(sin_lut) == 4 * half and len(cos_lut) == 4 * half
    # Position 0: alle Winkel 0 -> cos=1.0 (256), sin=0.
    assert cos_lut[0] == 256 and cos_lut[1] == 256, "pos 0: cos = 1.0"
    assert sin_lut[0] == 0 and sin_lut[1] == 0, "pos 0: sin = 0"
    # Position 1, Paar j=0: theta_0 = 1 -> Winkel 1 rad.
    assert cos_lut[1 * half + 0] == round(math.cos(1.0) * 256)
    assert sin_lut[1 * half + 0] == round(math.sin(1.0) * 256)
    # Position 1, Paar j=1: theta_1 = 1/1e6^(1/2) = 1e-3 -> Winkel 1e-3 rad.
    assert cos_lut[1 * half + 1] == round(math.cos(1e-3) * 256)
    assert sin_lut[1 * half + 1] == round(math.sin(1e-3) * 256)
    # hoeheres Paar -> kleinere Frequenz: j=1 rotiert kaum (sin ~ 0).
    assert abs(sin_lut[1 * half + 1]) <= 1
    assert all(abs(v) <= 256 for v in sin_lut + cos_lut)


def test_rope_lut_full_spec_parameters():
    # Mit den echten spec-Parametern (0.5B: head_dim 64, max_seq_len 2048)
    # muss die LUT die erwartete Groesse haben und wohlgeformt sein.
    nl = load_nonlinear_spec()
    head_dim = get_model_config("qwen2.5-0.5b")["head_dim"]
    sin_lut, cos_lut = generate_rope_luts(
        max_seq_len=nl["rope"]["max_seq_len"], head_dim=head_dim,
        rope_theta=nl["rope"]["rope_theta"], frac_bits=nl["rope"]["frac_bits"])
    half = head_dim // 2
    assert len(sin_lut) == nl["rope"]["max_seq_len"] * half
    assert len(cos_lut) == nl["rope"]["max_seq_len"] * half
    # Position 0 ist die Identitaet (alle Paare cos=1.0, sin=0).
    assert all(cos_lut[j] == 256 for j in range(half))
    assert all(sin_lut[j] == 0 for j in range(half))


def test_spec_driven_generation_lengths():
    # Die in main.py verwendete spec-gesteuerte Erzeugung muss dieselben
    # Laengen liefern wie das von runtime/Loader erwartete Format.
    nl = load_nonlinear_spec()
    rsqrt = generate_rsqrt_lut(max_input=nl["rsqrt"]["input_range"][1],
                               input_shift=nl["rsqrt"]["input_shift"],
                               frac_bits=nl["rsqrt"]["output_frac_bits"])
    silu = generate_silu_lut(input_min=nl["silu"]["input_range"][0],
                             input_max=nl["silu"]["input_range"][1],
                             input_frac_bits=nl["silu"]["input_frac_bits"],
                             output_frac_bits=nl["silu"]["output_frac_bits"])
    exp = generate_exp_lut(exp_range=nl["softmax"]["exp_lut_range"],
                           input_frac_bits=nl["softmax"]["exp_input_frac_bits"],
                           output_frac_bits=nl["softmax"]["exp_lut_frac_bits"])
    head_dim = get_model_config("qwen2.5-0.5b")["head_dim"]
    sin, cos = generate_rope_luts(max_seq_len=nl["rope"]["max_seq_len"],
                                  head_dim=head_dim,
                                  rope_theta=nl["rope"]["rope_theta"],
                                  frac_bits=nl["rope"]["frac_bits"])
    rope_len = nl["rope"]["max_seq_len"] * (head_dim // 2)
    assert len(rsqrt) == 32768
    assert len(silu) == 2048
    assert len(exp) == 1025
    assert len(sin) == rope_len and len(cos) == rope_len
    # Alle Werte muessen in int16 passen (LUT-Format der Runtime).
    for lut in (rsqrt, silu, exp, sin, cos):
        assert all(-32768 <= v <= 32767 for v in lut)


if __name__ == "__main__":
    test_load_nonlinear_spec_structure()
    print("[test] spec.json-nonlinear-Abschnitt hat erwartete Struktur (0.5.0): PASSED")
    test_rsqrt_lut_input_shift_semantics()
    print("[test] rsqrt-LUT input_shift-Semantik (x * 2^-8): PASSED")
    test_rsqrt_lut_input_shift_zero_entspricht_alter_skala()
    print("[test] rsqrt-LUT input_shift=0 (triviale Skala): PASSED")
    test_silu_lut_spot_values()
    print("[test] SiLU-LUT Stuetzwerte (Domäne +/-128): PASSED")
    test_exp_lut_spot_values()
    print("[test] exp-LUT Stuetzwerte: PASSED")
    test_rope_lut_spot_values()
    print("[test] RoPE-LUT Stuetzwerte (Multi-Frequenz, half-split): PASSED")
    test_rope_lut_full_spec_parameters()
    print("[test] RoPE-LUT mit spec-Parametern (Groesse/Identitaet): PASSED")
    test_spec_driven_generation_lengths()
    print("[test] spec-gesteuerte Erzeugung: Laengen und int16-Bereich: PASSED")
    print("Alle Tests bestanden.")
