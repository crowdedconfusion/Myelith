#!/usr/bin/env python3
"""
Rausch-Ablation der Nichtlinearitäten/LUTs und der Aktivierungsquantisierung
(Fund-14-Folge nach dem RoPE/Attention-Fix, v0.12.30).

Misst den intrinsischen Fehler jeder LUT gegen eine float64-Referenz über
ihre gesamte Domäne (Obergrenze des Beitrags) und identifiziert so, welche
Nichtlinearität das meiste Quantisierungsrauschen in die Perplexität trägt.

Reine Diagnose (Float ist hier erlaubt). Kein Teil des Auslieferungspfads.

Usage: python noise_ablation.py
"""

import json
import math
import struct
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
ART = REPO / "artifacts" / "qwen2.5-0.5b"


def load_lut(name):
    luts = json.load(open(ART / "luts.json"))
    e = luts[name]
    data = (ART / e["file"]).read_bytes()
    return list(struct.unpack(f"<{len(data)//2}h", data))


def silu(x):
    return x / (1.0 + math.exp(-x))


def report(name, errs_abs, errs_rel):
    n = len(errs_abs)
    mx = max(errs_abs)
    mean = sum(errs_abs) / n
    # 99.9-Perzentil (Ausreißer an Domaenenrändern ausblenden)
    srt = sorted(errs_abs)
    p999 = srt[int(0.999 * n)]
    print(f"{name:28s} max={mx:.5f}  mean={mean:.6f}  p99.9={p999:.6f}")


def main():
    nl = json.load(open(REPO / "theta_v" / "spec.json"))["theta_v"]["nonlinear"]

    # --- exp-LUT (Softmax) ---
    # Index i repraesentiert x = i * 2^-exp_input_frac_bits; Eintrag ist
    # round(exp(-x) * 2^exp_lut_frac_bits).
    exp_lut = load_lut("exp")
    ef_in = nl["softmax"]["exp_input_frac_bits"]
    ef_out = nl["softmax"]["exp_lut_frac_bits"]
    errs = []
    for i in range(len(exp_lut)):
        x = i / (1 << ef_in)
        ref = math.exp(-x)
        got = exp_lut[i] / (1 << ef_out)
        errs.append(abs(got - ref))
    report("exp-LUT (abs. Fehler)", errs, None)

    # --- SiLU-LUT ---
    # Index i in [input_min, input_max] repraesentiert x = i * 2^-input_frac;
    # Eintrag round(silu(x) * 2^output_frac).
    silu_lut = load_lut("silu")
    s_min = nl["silu"]["input_range"][0]
    s_in = nl["silu"]["input_frac_bits"]
    s_out = nl["silu"]["output_frac_bits"]
    errs = []
    for idx, val in enumerate(silu_lut):
        i = s_min + idx
        x = i / (1 << s_in)
        ref = silu(x)
        got = val / (1 << s_out)
        errs.append(abs(got - ref))
    report("SiLU-LUT (abs. Fehler)", errs, None)

    # --- RoPE cos/sin ---
    # 2D-LUT [max_seq_len, head_dim/2]; frac rope.frac_bits. Wert ist
    # round(cos(p*theta_j) * 2^frac). Fehler ist die Rundung auf 2^-frac.
    rope_frac = nl["rope"]["frac_bits"]
    cos_lut = load_lut("cos")
    # Der intrinsische Rundungsfehler ist <= 0.5 * 2^-rope_frac.
    half_step = 0.5 / (1 << rope_frac)
    print(f"RoPE cos/sin (frac {rope_frac}): max. Rundungsfehler <= {half_step:.6f} "
          f"(relativ zu cos/sin in [-1,1])")

    # --- rsqrt-LUT ---
    # rsqrt mit dynamic even index shift; misst den Fehler ueber die Indizes.
    # Index x repraesentiert x * 2^-input_shift; Eintrag round(1/sqrt(real)*2^out).
    rsqrt_lut = load_lut("rsqrt")
    r_shift = nl["rsqrt"]["input_shift"]
    r_out = nl["rsqrt"]["output_frac_bits"]
    errs = []
    for x in range(1, min(len(rsqrt_lut), 4096)):
        real = x / (1 << r_shift)
        ref = 1.0 / math.sqrt(real)
        got = rsqrt_lut[x] / (1 << r_out)
        errs.append(abs(got - ref) / ref)  # relativer Fehler (rsqrt wird multiplikativ genutzt)
    report("rsqrt-LUT (rel. Fehler)", errs, None)


if __name__ == "__main__":
    main()
