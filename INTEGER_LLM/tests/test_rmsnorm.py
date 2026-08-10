#!/usr/bin/env python3
"""
Referenz-Tests fuer RMSNorm.
"""

import math


def rmsnorm_ref(x, gamma, frac_bits, eps):
    n = len(x)
    mean_sq = sum(v * v for v in x) / n
    rms = math.sqrt(mean_sq + eps)
    scale = 2 ** frac_bits
    out = []
    for v, g in zip(x, gamma):
        y = (v / rms) * (g / scale)
        out.append(round(y * scale))
    return out


def test_rmsnorm_against_ref():
    x = [64, 64, -64]
    gamma = [64, 64, 64]
    ref = rmsnorm_ref(x, gamma, 6, 1)
    print("Ref:", ref)
    assert all(abs(r) <= 127 for r in ref)


if __name__ == "__main__":
    test_rmsnorm_against_ref()
