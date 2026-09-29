#!/usr/bin/env python3
"""
Proben fuer den ternaeren Export eines gedrehten Pakets
(`calibrate/src/export_weights.py`, `calibrate/src/gedrehtes_paket.py`).

⚑ **Das Format steht an zwei Orten**, in der Laufzeit
(`kernels/src/ternaer.rs`) und hier im Export. Beide Seiten halten es an
denselben festen Bytes fest (`das_format_ist_verschraenkt_nach_position` in
Rust, `test_format_wie_die_laufzeit` hier); aendert sich eine Seite, wird die
Probe der anderen rot.

Eigenstaendiges Skript nach Projektkonvention, kein pytest. Braucht numpy;
die Drehproben brauchen torch und springen sonst ausdruecklich.
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent.parent / "calibrate"))
from src.export_weights import ternaer_betraege, ternaer_muster  # noqa: E402


def test_format_wie_die_laufzeit():
    w = np.zeros((1, 128), dtype=np.int8)
    w[0, 17] = 5            # Block 0, s = 1, j = 1: Byte 1, Bits 2..3, Code 2
    w[0, 64 + 48 + 15] = -5  # Block 1, s = 3, j = 15: Byte 31, Bits 6..7, Code 0
    codes = (np.sign(w) + 1).astype(np.uint8)
    m = ternaer_muster(codes)[0]
    assert m[1] == 0b0101_1001, bin(m[1])
    assert m[31] == 0b0001_0101, bin(m[31])
    assert all(b == 0b0101_0101 for i, b in enumerate(m) if i not in (1, 31))


def test_codes_kommen_zurueck():
    rng = np.random.default_rng(3)
    codes = rng.integers(0, 3, size=(5, 512)).astype(np.uint8)
    m = ternaer_muster(codes)
    for z in range(5):
        for i in range(512):
            g, r = divmod(i, 128)
            block, rest = divmod(r, 64)
            s, j = divmod(rest, 16)
            byte = m[z, g * 32 + block * 16 + j]
            assert (byte >> (2 * s)) & 3 == codes[z, i], (z, i)


def test_skalen_werden_exakt_zerlegt():
    rng = np.random.default_rng(7)
    # Spanne der Exponenten je Zeile bis 3, wie gemessen, dazu Nullen und
    # subnormale Zahlen.
    # Basis innerhalb eines Binaerexponenten, dazu Faktor 1 bis 8: genau
    # die gemessene Spanne. Eine breitere Zeile ist nicht exakt darstellbar
    # und wird abgewiesen (naechste Probe).
    basis = rng.uniform(0.0625, 0.1249, size=(64, 40)).astype(np.float16)
    faktor = np.float16(2.0) ** rng.integers(0, 4, size=(64, 40)).astype(np.float16)
    s = (basis * faktor).astype(np.float16)
    s[0, 0] = 0
    s[1, :] = np.float16(6e-8)
    betrag, shift = ternaer_betraege(s)
    assert betrag.dtype == np.dtype("<i2") and shift.dtype == np.int8
    wieder = betrag.astype(np.float64) / np.exp2(shift[:, None].astype(np.float64))
    assert np.array_equal(wieder, s.astype(np.float64))
    assert betrag.max() <= 32767 and shift.min() >= 0


def test_eine_zu_weite_zeile_faellt_laut_aus():
    s = np.array([[1.0, 2.0 ** -20]], dtype=np.float16)
    try:
        ternaer_betraege(s)
    except ValueError:
        return
    raise AssertionError("eine nicht exakt darstellbare Zeile wurde gerundet statt abgewiesen")


def test_drehen_und_zurueck():
    try:
        import torch
    except ImportError:
        print("[test] Drehung: UEBERSPRUNGEN (torch fehlt)")
        return False
    from src.gedrehtes_paket import drehen, hadamard, rueckdrehen
    h = hadamard(1024)
    assert torch.allclose(h @ h, torch.eye(1024), atol=1e-5), "H ist nicht orthogonal und symmetrisch"
    x = torch.randn(3, 5120)
    vz = torch.where(torch.rand(5120) < 0.5, -1.0, 1.0)
    assert torch.allclose(rueckdrehen(drehen(x, vz), vz), x, atol=1e-4)
    # Die Sylvester-Ordnung: erste Zeile lauter Einsen, zweite abwechselnd.
    assert torch.all(h[0] > 0) and torch.all(h[1, ::2] > 0) and torch.all(h[1, 1::2] < 0)
    return True


if __name__ == "__main__":
    test_format_wie_die_laufzeit()
    print("[test] Format wie die Laufzeit: PASSED")
    test_codes_kommen_zurueck()
    print("[test] Codes kommen zurueck: PASSED")
    test_skalen_werden_exakt_zerlegt()
    print("[test] Skalen exakt zerlegt: PASSED")
    test_eine_zu_weite_zeile_faellt_laut_aus()
    print("[test] Zu weite Zeile faellt laut aus: PASSED")
    if test_drehen_und_zurueck():
        print("[test] Drehen und zurueck: PASSED")
    print("[test] Alle Tests bestanden.")
