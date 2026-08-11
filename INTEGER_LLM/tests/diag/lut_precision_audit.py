#!/usr/bin/env python3
"""
LUT-Präzisions-Audit: misst den realen Fehler jeder LUT (exp, SiLU, rsqrt,
RoPE-sin/cos) über ihren Wertebereich gegen die exakte Float-Funktion.

Zeigt den maximalen und mittleren absoluten/relativen Fehler. Eine LUT mit
großem Fehler (v. a. in dem Bereich, der real durchlaufen wird) ist ein
Kandidat für den Perplexitäts-Blow-up.

Gleitkomma erlaubt (reine Diagnose). Kein Teil des Auslieferungspfads.

Usage: python lut_precision_audit.py
"""
import json
import math
import sys
from pathlib import Path

REPO = Path(__file__).parent.parent.parent
sys.path.insert(0, str(REPO / "calibrate"))


def load_lut(name):
    import struct
    art = REPO / "artifacts" / "qwen2.5-0.5b"
    luts = json.loads((art / "luts.json").read_text())
    e = luts[name]
    data = (art / e["file"]).read_bytes()
    return list(struct.unpack(f"<{len(data)//2}h", data))


def main():
    spec = json.loads((REPO / "theta_v" / "spec.json").read_text())["theta_v"]
    nl = spec["nonlinear"]

    # --- exp-LUT: exp(-x), Eingang frac 4, Ausgang frac 8 ---
    ef_in = nl["softmax"]["exp_input_frac_bits"]
    ef_out = nl["softmax"]["exp_lut_frac_bits"]
    exp_lut = load_lut("exp")
    max_abs, max_rel = 0.0, 0.0
    for i in range(len(exp_lut)):
        x = i / (2.0 ** ef_in)
        ref = math.exp(-x)
        got = exp_lut[i] / (2.0 ** ef_out)
        max_abs = max(max_abs, abs(got - ref))
        if ref > 1e-6:
            max_rel = max(max_rel, abs(got - ref) / ref)
    print(f"exp-LUT   ({len(exp_lut)} Einträge, in_frac={ef_in}, out_frac={ef_out}):")
    print(f"   max abs Fehler = {max_abs:.6f}, max rel Fehler = {max_rel:.4%}")

    # --- SiLU-LUT: Eingang frac, Ausgang frac ---
    s_in = nl["silu"]["input_frac_bits"]
    s_out = nl["silu"]["output_frac_bits"]
    s_min, s_max = nl["silu"]["input_range"]
    silu_lut = load_lut("silu")
    max_abs, max_rel = 0.0, 0.0
    for idx, val in enumerate(silu_lut):
        i = s_min + idx
        x = i / (2.0 ** s_in)
        ref = x / (1.0 + math.exp(-x))
        got = val / (2.0 ** s_out)
        max_abs = max(max_abs, abs(got - ref))
        if abs(ref) > 1e-3:
            max_rel = max(max_rel, abs(got - ref) / abs(ref))
    print(f"SiLU-LUT  ({len(silu_lut)} Einträge, in_frac={s_in}, out_frac={s_out}):")
    print(f"   max abs Fehler = {max_abs:.6f}, max rel Fehler = {max_rel:.4%}")

    # --- rsqrt-LUT: Eingang shift, Ausgang frac ---
    r_shift = nl["rsqrt"]["input_shift"]
    r_out = nl["rsqrt"]["output_frac_bits"]
    rsqrt_lut = load_lut("rsqrt")
    max_rel = 0.0
    for x in range(1, len(rsqrt_lut)):
        real = x / (2.0 ** r_shift)
        ref = 1.0 / math.sqrt(real)
        got = rsqrt_lut[x] / (2.0 ** r_out)
        if ref > 1e-9:
            max_rel = max(max_rel, abs(got - ref) / ref)
    print(f"rsqrt-LUT ({len(rsqrt_lut)} Einträge, in_shift={r_shift}, out_frac={r_out}):")
    print(f"   max rel Fehler = {max_rel:.4%}")

    # --- RoPE sin/cos ---
    rope_frac = nl["rope"]["frac_bits"]
    sin_lut = load_lut("sin")
    cos_lut = load_lut("cos")
    # RoPE-LUTs sind [max_seq_len, head_dim/2]; hier nur die erste Position
    # und ein paar Winkel prüfen. Wir rekonstruieren den Winkel nicht exakt,
    # sondern prüfen die Wertebereiche und eine Stichprobe.
    import numpy as np
    half = 32  # head_dim/2 für 0.5B
    sin_arr = np.array(sin_lut).reshape(-1, half)
    cos_arr = np.array(cos_lut).reshape(-1, half)
    print(f"RoPE-LUT  (sin {sin_arr.shape}, cos {cos_arr.shape}, frac={rope_frac}):")
    print(f"   sin Bereich [{sin_arr.min()}, {sin_arr.max()}] (erwartet ~±{2**rope_frac})")
    print(f"   cos Bereich [{cos_arr.min()}, {cos_arr.max()}] (erwartet ~±{2**rope_frac})")
    # Position 0 muss cos=+1 (2^frac), sin=0 sein
    print(f"   Position 0: cos[0,:3]={cos_arr[0,:3].tolist()} (erwartet {2**rope_frac}), "
          f"sin[0,:3]={sin_arr[0,:3].tolist()} (erwartet 0)")


if __name__ == "__main__":
    main()
