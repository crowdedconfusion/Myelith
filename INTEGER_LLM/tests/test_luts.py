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
    """Die **Regeln** der Spec, nicht ihre Zahlen.

    ⚑ **Hier standen bis zum 2026-08-25 getippte Werte**, und zwar die
    von theta_v 0.14.0: silu-Eingangsbereich [-1024, 1023],
    `exp_lut_frac_bits` 8. Die Spec steht seit 0.15.0 und 0.16.0 auf
    [-8192, 8191] und 14, beides aus benannten Gruenden (SiLU-Raster
    verfeinert, weil der ganze MLP-Fehler dort entstand;
    Softmax-Aufloesung angehoben, weil bei 512 Positionen das
    Schwanzgewicht auf null rundete).

    **Der Test war seitdem rot und ist niemandem aufgefallen**, weil die
    CI nur die vier Audit-Skripte startet. Dieselbe Klasse wie Fund 44.

    Und er war doppelt wertlos: Ein Test, der prueft, dass in der Spec
    steht, was in der Spec steht, ist eine Tautologie. Er faellt bei
    **jeder** richtigen Aenderung um und erzeugt Druck, sie
    zurueckzunehmen - genau das, was AGENTS.md unter „Tests gegen
    Literale" beschreibt.

    Geprueft wird jetzt, was gelten **muss**, damit die LUTs ueberhaupt
    funktionieren. Die Grenzen stehen in den `note`-Feldern der Spec
    selbst; sie sind hier ausgerechnet statt abgeschrieben.
    """
    nl = load_nonlinear_spec()
    for key in ("rsqrt", "silu", "softmax", "rope"):
        assert key in nl, f"spec.json-Abschnitt 'nonlinear' ohne '{key}'"

    # rsqrt: die Indexnormierung ist eine Festlegung, kein Messwert, und
    # `rmsnorm_i16` verlangt einen geraden Shift (Halb-Bit-Faktor).
    assert nl["rsqrt"]["index_normalization"] == "dynamic_even_shift"
    assert nl["rsqrt"]["input_shift"] % 2 == 0, "Halb-Bit-Faktor braucht geraden Shift"
    assert nl["rsqrt"]["input_range"][0] == 0, "rsqrt ist auf nichtnegativen Werten definiert"

    # silu: Der Eingangsbereich muss zum Raster passen, sonst deckt die
    # LUT eine andere reale Domaene ab als die Runtime annimmt.
    silu = nl["silu"]
    lo, hi = silu["input_range"]
    assert hi + 1 == -lo, f"silu-Bereich muss symmetrisch sein, ist [{lo}, {hi}]"
    assert (hi + 1) % 2 == 0
    # Der Index wird in integer_math::lut_lookup als i16 gerechnet;
    # `lut.len() as i16 - 1` laeuft ab 32768 Eintraegen ueber.
    assert hi - lo + 1 <= 32767, "silu-LUT sprengt den i16-Index"
    # Groesster Ausgabewert = silu(hi_real) ~ hi_real; er muss in i16
    # passen, sonst saettigen die LUT-Eintraege selbst.
    hi_real = hi / (2 ** silu["input_frac_bits"])
    assert hi_real * (2 ** silu["output_frac_bits"]) <= 32767, (
        f"silu-LUT-Eintraege sprengen i16: {hi_real} * 2^{silu['output_frac_bits']}"
    )

    # softmax: exp(0) ist der groesste Eintrag und muss in i16 passen.
    sm = nl["softmax"]
    assert 2 ** sm["exp_lut_frac_bits"] <= 32767, (
        f"exp(0) = 2^{sm['exp_lut_frac_bits']} sprengt die i16-LUT"
    )
    # Die Tabelle deckt den Eingangsbereich beim gewaehlten Raster ab.
    assert sm["exp_lut_range"] >= 2 ** sm["exp_input_frac_bits"], (
        "exp-LUT kuerzer als ein einziger Einheitsschritt des Eingangsrasters"
    )

    # rope: Die Paarung ist eine Protokollfestlegung (Fund 15), und die
    # LUT-Zeilenzahl haengt an max_seq_len.
    assert nl["rope"]["pairing"] == "half_split", "Fund 15: Qwen2-Schema"
    assert nl["rope"]["max_seq_len"] > 0
    assert nl["rope"]["rope_theta"] > 1.0


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
    # ⚑ **Aus der Spec gerechnet, nicht getippt** (2026-08-25). Hier
    # standen 32768 / 2048 / 1025, die Laengen von theta_v 0.14.0. Seit
    # 0.15.0 ist die silu-LUT 16384 Eintraege lang und seit 0.16.0 die
    # exp-LUT 16385. Der Test war rot und niemandem aufgefallen, weil die
    # CI ihn nicht startet.
    #
    # Eine getippte Laenge prueft nur, dass sich die Spec nicht geaendert
    # hat. Geprueft gehoert, dass der **Erzeuger der Spec folgt**: Genau
    # das faengt einen Erzeuger, der einen Eintrag zu wenig oder zu viel
    # anlegt, und genau das ist der Fehler, der die Runtime am Rand der
    # Domaene ins Leere greifen liesse.
    rope_len = nl["rope"]["max_seq_len"] * (head_dim // 2)
    silu_len = nl["silu"]["input_range"][1] - nl["silu"]["input_range"][0] + 1
    assert len(rsqrt) == nl["rsqrt"]["input_range"][1] + 1, (
        f"rsqrt: {len(rsqrt)} Eintraege, Spec sagt "
        f"{nl['rsqrt']['input_range'][1] + 1}"
    )
    assert len(silu) == silu_len, f"silu: {len(silu)} Eintraege, Spec sagt {silu_len}"
    # exp deckt [0, exp_lut_range] **einschliesslich** ab, daher +1.
    assert len(exp) == nl["softmax"]["exp_lut_range"] + 1, (
        f"exp: {len(exp)} Eintraege, Spec sagt "
        f"{nl['softmax']['exp_lut_range'] + 1}"
    )
    assert len(sin) == rope_len and len(cos) == rope_len
    # Alle Werte muessen in int16 passen (LUT-Format der Runtime).
    for lut in (rsqrt, silu, exp, sin, cos):
        assert all(-32768 <= v <= 32767 for v in lut)


if __name__ == "__main__":
    test_load_nonlinear_spec_structure()
    print("[test] spec.json-nonlinear-Abschnitt haelt seine eigenen Regeln: PASSED")
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
