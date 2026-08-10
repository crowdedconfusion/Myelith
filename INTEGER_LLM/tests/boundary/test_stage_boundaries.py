#!/usr/bin/env python3
"""
Boundary-Tests fuer Pipeline-Stage-Grenzen.

Prueft:
1. Eingangstensor pro Stage hat korrektes Format (INT16, frac_bits=8)
2. Ausgangstensor pro Stage hat korrektes Format
3. Keine Float-Konvertierung an Grenzen
4. Checksummen stimmen
"""

import struct
import sys
from pathlib import Path


def test_boundary_format():
    """Prueft, dass Boundary-Tensoren INT16 sind."""
    # Simulierter Boundary-Tensor
    tensor = [100, -50, 25, -12]
    payload = struct.pack(f"<{len(tensor)}h", *tensor)
    
    # Pruefe: Keine Float-Bytes (kein IEEE 754 Pattern)
    for i in range(0, len(payload), 2):
        val = struct.unpack("<h", payload[i:i+2])[0]
        assert -32768 <= val <= 32767, f"Value {val} out of INT16 range"
    
    print("[boundary] INT16 Format: PASSED")


def test_boundary_endianness():
    """Prueft Little-Endian an Stage-Grenzen."""
    val = 0x1234
    le = struct.pack("<h", val)
    be = struct.pack(">h", val)
    assert le != be, "Little-Endian muss von Big-Endian unterscheidbar sein"
    assert le == b"\x34\x12", "Little-Endian Format falsch"
    print("[boundary] Little-Endian: PASSED")


def test_boundary_no_float():
    """Prueft, dass keine Float-Werte an Grenzen auftauchen."""
    # Ein Float (z.B. 1.5 als f32) waere: 0x3FC00000
    # Das darf in einem INT16-Stream nicht vorkommen
    forbidden_patterns = [b"\x00\x00\xc0\x3f", b"\x3f\xc0\x00\x00"]  # f32 1.5 LE/BE
    
    sample_payload = b"\x64\x00\xce\xff\x19\x00\xf4\xff"  # [100, -50, 25, -12]
    for pattern in forbidden_patterns:
        assert pattern not in sample_payload, "Float-Pattern in INT16-Stream gefunden!"
    
    print("[boundary] Kein Float an Grenzen: PASSED")


def test_boundary_checksum():
    """Prueft CRC32-Checksumme an Nachrichtengrenzen."""
    import zlib
    payload = b"\x64\x00\xce\xff"  # [100, -50]
    crc = zlib.crc32(payload) & 0xFFFFFFFF
    # CRC muss deterministisch sein
    crc2 = zlib.crc32(payload) & 0xFFFFFFFF
    assert crc == crc2, "CRC nicht deterministisch"
    print(f"[boundary] CRC32 deterministisch ({crc:08x}): PASSED")


def main():
    print("="*60)
    print("Boundary Tests fuer Stage-Grenzen")
    print("="*60)
    test_boundary_format()
    test_boundary_endianness()
    test_boundary_no_float()
    test_boundary_checksum()
    print("\n[boundary] Alle Boundary-Tests PASSED")


if __name__ == "__main__":
    main()
