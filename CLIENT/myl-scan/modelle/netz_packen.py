#!/usr/bin/env python3
"""Schneidet aus einer `.traineddata` (tessdata_fast) die drei Teile fuers Lesen heraus.

Aufruf: netz_packen.py <ein.traineddata> <aus.bin>

Woerterbuecher bleiben draussen: sie sind das Zehnfache des Netzes, und ohne sie
wird nichts geraten. Ablage: je Teil vier Byte Laenge, dann der Inhalt
(Netz, Codes, Zeichensatz).
"""

import struct
import sys

NETZ, ZEICHENSATZ, CODES = 17, 21, 22


def main() -> int:
    b = open(sys.argv[1], "rb").read()
    n = struct.unpack("<i", b[:4])[0]
    orte = struct.unpack(f"<{n}q", b[4 : 4 + 8 * n])
    sortiert = sorted(o for o in orte if o >= 0) + [len(b)]

    def teil(i):
        return b[orte[i] : sortiert[sortiert.index(orte[i]) + 1]]

    aus = b"".join(struct.pack("<I", len(t)) + t for t in (teil(NETZ), teil(CODES), teil(ZEICHENSATZ)))
    open(sys.argv[2], "wb").write(aus)
    print(f"{sys.argv[1]}: {len(b) / 1048576:.1f} -> {len(aus) / 1048576:.2f} MB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
