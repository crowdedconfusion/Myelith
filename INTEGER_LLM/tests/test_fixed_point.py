#!/usr/bin/env python3
"""
Unit-Tests fuer Fixed-Point-Operationen.
"""


def rshift_round(value: int, shift: int) -> int:
    if shift == 0:
        return value
    mask = (1 << shift) - 1
    half = 1 << (shift - 1)
    quotient = value >> shift
    remainder = value & mask
    if remainder > half or (remainder == half and (quotient & 1)):
        return quotient + 1
    return quotient


def test_rshift_round_positive():
    assert rshift_round(4, 1) == 2
    assert rshift_round(3, 1) == 2
    assert rshift_round(5, 1) == 2
    assert rshift_round(7, 1) == 4


def test_rshift_round_negative():
    assert rshift_round(-5, 1) == -2
    assert rshift_round(-4, 1) == -2
    assert rshift_round(-3, 1) == -2


def test_rescale():
    acc = 127 * 127
    assert rshift_round(acc, 7) == 126


if __name__ == "__main__":
    test_rshift_round_positive()
    test_rshift_round_negative()
    test_rescale()
    print("[fixed_point] Alle Tests bestanden.")
