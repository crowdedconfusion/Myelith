#!/usr/bin/env python3
"""Packt die Gewichte eines ONNX-Modells auf wenige Bit je Zahl, fuer `myl-scan`.

Aufruf: packen.py <ein.onnx> <aus.onnx> <bit>

Jede groessere Gleitkomma-Tabelle wird je Kanal linear zwischen Kleinst- und
Groesstwert abgestuft, denn eine Skala fuer alle Kanaele zerstoert diese Netze.
`doc_string` nennt q:bit,achse,gruppen; `Erkenner` packt beim Laden aus. Nur die
Standardbibliothek.
"""

from __future__ import annotations

import struct
import sys

MINDESTENS = 64


def varint(n: int) -> bytes:
    aus = bytearray()
    while True:
        b = n & 127
        n >>= 7
        aus.append(b | (128 if n else 0))
        if not n:
            return bytes(aus)


def lies_varint(b: bytes, i: int):
    s = n = 0
    while True:
        x = b[i]
        i += 1
        n |= (x & 127) << s
        s += 7
        if x < 128:
            return n, i


def felder(b: bytes):
    """Liefert (feld, draht, wert, roh) je Feld; `wert` ist Zahl oder Bytes."""
    i = 0
    while i < len(b):
        a = i
        k, i = lies_varint(b, i)
        f, w = k >> 3, k & 7
        if w == 0:
            v, i = lies_varint(b, i)
        elif w == 1:
            v, i = b[i : i + 8], i + 8
        elif w == 5:
            v, i = b[i : i + 4], i + 4
        elif w == 2:
            n, i = lies_varint(b, i)
            v, i = b[i : i + n], i + n
        else:
            raise ValueError(f"Draht {w}")
        yield f, w, v, b[a:i]


def laenge(feld: int, inhalt: bytes) -> bytes:
    return varint(feld << 3 | 2) + varint(len(inhalt)) + inhalt


def stufen(gruppen, bit: int) -> bytes:
    """Je Gruppe eine eigene Skala: Sie steht vorn, die Bitfolge danach."""
    grenze = (1 << bit) - 1
    kopf = bytearray()
    aus = bytearray()
    speicher = belegt = 0
    for werte in gruppen:
        tief, hoch = min(werte), max(werte)
        kopf += struct.pack("<ff", tief, hoch)
        breite = (hoch - tief) or 1.0
        for v in werte:
            speicher |= round((v - tief) / breite * grenze) << belegt
            belegt += bit
            while belegt >= 8:
                aus.append(speicher & 255)
                speicher >>= 8
                belegt -= 8
    if belegt:
        aus.append(speicher & 255)
    return bytes(kopf + aus)


def gruppieren(werte, formen):
    """Faltungen je Filter (erste Achse), Matrizen je Spalte (letzte Achse)."""
    if len(formen) >= 3:
        je = len(werte) // formen[0]
        return 0, [werte[i * je : (i + 1) * je] for i in range(formen[0])]
    if len(formen) == 2 and formen[1] > 1:
        return 1, [werte[i :: formen[1]] for i in range(formen[1])]
    return -1, [werte]


def tabelle(b: bytes, bit: int) -> bytes:
    roh = typ = None
    formen = []
    for f, w, v, _r in felder(b):
        if f == 2:
            typ = v
        elif f == 9:
            roh = v
        elif f == 1 and w == 0:
            formen.append(v)
        elif f == 1 and w == 2:
            i = 0
            while i < len(v):
                n, i = lies_varint(v, i)
                formen.append(n)
    if typ != 1 or roh is None or len(roh) < 4 * MINDESTENS:
        return b
    werte = struct.unpack(f"<{len(roh) // 4}f", roh)
    achse, gruppen = gruppieren(werte, formen)
    aus = bytearray()
    for f, _w, _v, r in felder(b):
        if f not in (9, 12):
            aus += r
    aus += laenge(12, f"q:{bit},{achse},{len(gruppen)}".encode()) + laenge(9, stufen(gruppen, bit))
    return bytes(aus)


def main() -> int:
    ein, aus, bit = sys.argv[1], sys.argv[2], int(sys.argv[3])
    b = open(ein, "rb").read()
    neu = bytearray()
    for f, _w, v, r in felder(b):
        if f != 7:
            neu += r
            continue
        g = bytearray()
        for h, _x, u, s in felder(v):
            g += laenge(5, tabelle(u, bit)) if h == 5 else s
        neu += laenge(7, bytes(g))
    open(aus, "wb").write(neu)
    print(f"{ein}: {len(b) / 1048576:.1f} -> {len(neu) / 1048576:.1f} MB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
