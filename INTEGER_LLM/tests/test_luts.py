#!/usr/bin/env python3
"""
Tests fuer calibrate/src/luts.py (Fahrplan-Punkt 12.17): LUT-Generierung
ausschliesslich aus den Parametern von theta_v/spec.json, inkl. der
input_shift-Semantik der rsqrt-LUT (Index x repraesentiert x * 2^-input_shift).

Eigenstaendiges Skript nach Projektkonvention (siehe test_fixed_point.py),
kein pytest, keine torch/numpy-Abhaengigkeit.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.luts import (generate_rsqrt_lut, generate_silu_lut, generate_exp_lut,
                      generate_sin_cos_lut, load_nonlinear_spec)


def silu_ref(x):
    return x / (1.0 + math.exp(-x))


def test_load_nonlinear_spec_structure():
    nl = load_nonlinear_spec()
    for key in ("rsqrt", "silu", "softmax", "rope"):
        assert key in nl, f"spec.json-Abchnitt 'nonlinear' ohne '{key}'"
    assert nl["rsqrt"]["input_range"] == [0, 32767]
    assert nl["rsqrt"]["input_shift"] == 7
    assert nl["rsqrt"]["output_frac_bits"] == 8
    assert nl["silu"]["input_range"] == [-128, 127]
    assert nl["silu"]["output_frac_bits"] == 6
    assert nl["softmax"]["exp_lut_range"] == 128
    assert nl["softmax"]["exp_lut_frac_bits"] == 8
    assert nl["rope"]["max_seq_len"] == 2048
    assert nl["rope"]["frac_bits"] == 8


def test_rsqrt_lut_input_shift_semantics():
    # input_shift=7: Index x steht fuer den Realwert x/128.
    lut = generate_rsqrt_lut(max_input=1024, input_shift=7, frac_bits=8)
    assert lut[0] == 256, "Sentinel fuer x=0 muss 1.0 (scale) sein"
    assert lut[128] == 256, "rsqrt(1.0) = 1.0"
    assert lut[512] == 128, "rsqrt(4.0) = 0.5"
    assert lut[32] == 512, "rsqrt(0.25) = 2.0"
    assert all(v > 0 for v in lut), "rsqrt ist ueberall positiv"
    assert all(lut[i] >= lut[i + 1] for i in range(1, len(lut) - 1)), \
        "rsqrt muss monoton fallend sein"


def test_rsqrt_lut_input_shift_zero_entspricht_alter_skala():
    # input_shift=0: Index x steht fuer den Realwert x (alte Interpretation).
    lut = generate_rsqrt_lut(max_input=16, input_shift=0, frac_bits=8)
    assert lut[1] == 256, "rsqrt(1) = 1.0"
    assert lut[4] == 128, "rsqrt(4) = 0.5"


def test_silu_lut_spot_values():
    lut = generate_silu_lut(input_min=-128, input_max=127, frac_bits=6)
    assert len(lut) == 256
    assert lut[128] == 0, "silu(0) = 0"
    assert lut[0] == round(silu_ref(-2.0) * 64), "silu(-2) bei frac 6"
    assert lut[255] == round(silu_ref(127 / 64) * 64), "silu(127/64) bei frac 6"


def test_exp_lut_spot_values():
    lut = generate_exp_lut(exp_range=128, frac_bits=8)
    assert len(lut) == 129
    assert lut[0] == 256, "exp(0) = 1.0"
    assert lut[128] == round(math.exp(-0.5) * 256), "exp(-0.5) bei frac 8"
    assert all(lut[i] >= lut[i + 1] for i in range(len(lut) - 1)), \
        "exp(-x) muss monoton fallend sein"


def test_sin_cos_lut_spot_values():
    sin_lut, cos_lut = generate_sin_cos_lut(n=2048, frac_bits=8)
    assert len(sin_lut) == 2048 and len(cos_lut) == 2048
    assert cos_lut[0] == 256, "cos(0) = 1.0"
    assert sin_lut[0] == 0, "sin(0) = 0"
    assert sin_lut[512] == 256, "sin(pi/2) = 1.0"
    assert cos_lut[1024] == -256, "cos(pi) = -1.0"
    assert sin_lut[1536] == -256, "sin(3pi/2) = -1.0"
    assert all(abs(v) <= 256 for v in sin_lut + cos_lut)


def test_spec_driven_generation_lengths():
    # Die in main.py verwendete spec-gesteuerte Erzeugung muss dieselben
    # Laengen liefern wie das von runtime/Loader erwartete Format.
    nl = load_nonlinear_spec()
    rsqrt = generate_rsqrt_lut(max_input=nl["rsqrt"]["input_range"][1],
                               input_shift=nl["rsqrt"]["input_shift"],
                               frac_bits=nl["rsqrt"]["output_frac_bits"])
    silu = generate_silu_lut(input_min=nl["silu"]["input_range"][0],
                             input_max=nl["silu"]["input_range"][1],
                             frac_bits=nl["silu"]["output_frac_bits"])
    exp = generate_exp_lut(exp_range=nl["softmax"]["exp_lut_range"],
                           frac_bits=nl["softmax"]["exp_lut_frac_bits"])
    sin, cos = generate_sin_cos_lut(n=nl["rope"]["max_seq_len"],
                                    frac_bits=nl["rope"]["frac_bits"])
    assert len(rsqrt) == 32768
    assert len(silu) == 256
    assert len(exp) == 129
    assert len(sin) == 2048 and len(cos) == 2048
    # Alle Werte muessen in int16 passen (LUT-Format der Runtime).
    for lut in (rsqrt, silu, exp, sin, cos):
        assert all(-32768 <= v <= 32767 for v in lut)


if __name__ == "__main__":
    test_load_nonlinear_spec_structure()
    print("[test] spec.json-nonlinear-Abschnitt hat erwartete Struktur: PASSED")
    test_rsqrt_lut_input_shift_semantics()
    print("[test] rsqrt-LUT input_shift-Semantik (x * 2^-7): PASSED")
    test_rsqrt_lut_input_shift_zero_entspricht_alter_skala()
    print("[test] rsqrt-LUT input_shift=0 (alte Skala): PASSED")
    test_silu_lut_spot_values()
    print("[test] SiLU-LUT Stuetzwerte: PASSED")
    test_exp_lut_spot_values()
    print("[test] exp-LUT Stuetzwerte: PASSED")
    test_sin_cos_lut_spot_values()
    print("[test] Sin/Cos-LUT Stuetzwerte: PASSED")
    test_spec_driven_generation_lengths()
    print("[test] spec-gesteuerte Erzeugung: Laengen und int16-Bereich: PASSED")
    print("Alle Tests bestanden.")
