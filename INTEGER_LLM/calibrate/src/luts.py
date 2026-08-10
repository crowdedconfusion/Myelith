"""
Generiert Lookup-Tables fuer Nichtlinearitaeten.
Float ist hier erlaubt (Offline-Phase).
"""

import math
from typing import List


def generate_rsqrt_lut(max_input: int, frac_bits: int) -> List[int]:
    scale = 1 << frac_bits
    lut = []
    for x in range(max_input + 1):
        if x == 0:
            lut.append(scale)
        else:
            val = 1.0 / math.sqrt(x)
            lut.append(int(round(val * scale)))
    return lut


def generate_silu_lut(input_min: int, input_max: int, frac_bits: int) -> List[int]:
    scale = 1 << frac_bits
    lut = []
    for x in range(input_min, input_max + 1):
        xf = x / scale
        val = xf * (1.0 / (1.0 + math.exp(-xf)))
        lut.append(int(round(val * scale)))
    return lut


def generate_exp_lut(exp_range: int, frac_bits: int) -> List[int]:
    scale = 1 << frac_bits
    lut = []
    for i in range(exp_range + 1):
        x = i / scale
        val = math.exp(-x)
        lut.append(int(round(val * scale)))
    return lut


def generate_sin_cos_lut(n: int, frac_bits: int):
    scale = 1 << frac_bits
    sin_lut = []
    cos_lut = []
    for i in range(n):
        angle = 2.0 * math.pi * i / n
        sin_lut.append(int(round(math.sin(angle) * scale)))
        cos_lut.append(int(round(math.cos(angle) * scale)))
    return sin_lut, cos_lut
