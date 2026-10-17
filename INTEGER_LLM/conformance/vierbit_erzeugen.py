#!/usr/bin/env python3
"""Erzeugt die Konformitaetsvektoren der Gruppe `vierbit`, unabhaengig.

Die Sollwerte rechnet dieses Skript selbst, in reiner Ganzzahlarithmetik
und ohne eine Zeile aus den Rust-Kernen: eigene Packung, eigene Rundung.
Eine Uebereinstimmung mit den Kernen belegt deshalb **Richtigkeit** und
nicht nur Determinismus (Feld `herkunft: unabhaengig`).

Das Format: je Gewicht ein Code q von 0 bis 15, je Gruppe von 128 ein
Schritt a (0 bis 255) und ein Versatz b (-128 bis 127); der Wert ist
a * q + b. Je Gruppe 64 Byte in vier Bloecken zu 16, Byte j eines Blocks
traegt in den Bits 0 bis 3 das Gewicht j und in den Bits 4 bis 7 das
Gewicht 16 + j des Blocks.

| Vektor | Baustein |
|---|---|
| `linear_vierbit` | lineare Schicht mit gepackten Gewichten, auch Schritt 255 und Versatz -128 (aus keinem int8, aber festgelegt), Schritt 0, Code 15 |
| `packen_vierbit` | int8-Werte zu Codes, Schritt und Versatz: kleinster Wert als Versatz, groesster gemeinsamer Teiler als Schritt |

Aufruf aus der Wurzel des Repositoriums:

    python3 INTEGER_LLM/conformance/vierbit_erzeugen.py

Ueberschreibt die zwei Dateien unter `conformance/vectors/vierbit/`. Die
Eingaben kommen aus einem festen linearen Kongruenzgenerator, also ergibt
jeder Lauf dieselben Bytes.
"""

from __future__ import annotations

import hashlib
import json
import struct
import sys
from math import gcd
from pathlib import Path

HIER = Path(__file__).resolve().parent
ZIEL = HIER / "vectors" / "vierbit"
SPEC = HIER.parent / "theta_v" / "spec.json"

GRUPPE = 128
BYTES_JE_GRUPPE = GRUPPE // 2


# --- Arithmetik -----------------------------------------------------------


def rechts_gerade(wert: int, shift: int) -> int:
    """Arithmetischer Rechtsshift, Rundung zur naechsten geraden Zahl."""
    if shift == 0:
        return wert
    quotient = wert >> shift
    rest = wert - (quotient << shift)
    halb = 1 << (shift - 1)
    if rest > halb or (rest == halb and quotient % 2 != 0):
        quotient += 1
    return quotient


def umskalieren(wert: int, ein_frac: int, aus_frac: int) -> int:
    d = ein_frac - aus_frac
    return rechts_gerade(wert, d) if d >= 0 else wert << (-d)


def klemmen(wert: int, unten: int, oben: int) -> int:
    return max(unten, min(oben, wert))


class Kongruenz:
    """Fester Zufall: x' = (a x + c) mod 2^64, obere 32 Bit als Ausgabe."""

    def __init__(self, saat: int) -> None:
        self.x = saat & ((1 << 64) - 1)

    def zahl(self, unten: int, oben: int) -> int:
        self.x = (6364136223846793005 * self.x + 1442695040888963407) & ((1 << 64) - 1)
        return unten + (self.x >> 32) % (oben - unten + 1)


# --- Vierbitformat --------------------------------------------------------


def bitstelle(i: int) -> tuple[int, int]:
    """Byte und Bitversatz des Gewichts `i` einer Gruppe."""
    block, p = divmod(i, 32)
    halb, j = divmod(p, 16)
    return block * 16 + j, 4 * halb


def packen(werte: list[int]) -> tuple[list[int], list[int]]:
    codes: list[int] = []
    gitter: list[int] = []
    for g in range(len(werte) // GRUPPE):
        gruppe = werte[g * GRUPPE:(g + 1) * GRUPPE]
        b = min(gruppe)
        a = 0
        for v in gruppe:
            a = gcd(a, v - b)
        assert a == 0 or (max(gruppe) - b) // a <= 15, f"Gruppe {g} ist nicht vierbittig"
        bytes_ = [0] * BYTES_JE_GRUPPE
        for i, v in enumerate(gruppe):
            q = 0 if a == 0 else (v - b) // a
            byte, s = bitstelle(i)
            bytes_[byte] |= q << s
        codes.extend(bytes_)
        gitter.extend([a, b & 0xFF])
    return codes, gitter


def entpacken(codes: list[int], gitter: list[int]) -> list[int]:
    werte: list[int] = []
    for g in range(len(gitter) // 2):
        a = gitter[2 * g]
        b = gitter[2 * g + 1] - 256 if gitter[2 * g + 1] > 127 else gitter[2 * g + 1]
        bytes_ = codes[g * BYTES_JE_GRUPPE:(g + 1) * BYTES_JE_GRUPPE]
        for i in range(GRUPPE):
            byte, s = bitstelle(i)
            werte.append(a * ((bytes_[byte] >> s) & 15) + b)
    return werte


def linear(x: list[int], w: list[int], spalten: int, w_shifts: list[int],
           act_frac: int, out_frac: int) -> list[int]:
    aus = []
    for z, shift in enumerate(w_shifts):
        zeile = w[z * spalten:(z + 1) * spalten]
        summe = sum(a * b for a, b in zip(zeile, x))
        aus.append(klemmen(umskalieren(summe, shift + act_frac, out_frac), -32768, 32767))
    return aus


# --- Vektoren -------------------------------------------------------------

PACKFORM = {"int8": "b", "int16": "h", "int32": "i"}


def tensor(daten: list[int], dtype: str, form: list[int]) -> dict:
    roh = struct.pack("<" + str(len(daten)) + PACKFORM[dtype], *daten)
    return {"dtype": dtype, "shape": form, "hash": hashlib.sha256(roh).hexdigest(), "data": daten}


def vektor(name: str, metadata: dict, eingaben: dict, ausgaben: dict) -> dict:
    return {
        "name": name,
        "level": "vierbit",
        "theta_v_hash": "sha256:" + hashlib.sha256(SPEC.read_bytes()).hexdigest(),
        "herkunft": "unabhaengig",
        "metadata": metadata,
        "inputs": eingaben,
        "outputs": ausgaben,
    }


def zufallsgruppe(z: Kongruenz, a: int) -> list[int]:
    b = z.zahl(-127, 127 - 15 * a)
    return [a * z.zahl(0, 15) + b for _ in range(GRUPPE)]


def v_linear() -> dict:
    z = Kongruenz(11)
    zeilen, spalten = 4, 256
    werte: list[int] = []
    for zeile in range(zeilen):
        for g in range(spalten // GRUPPE):
            if (zeile, g) == (3, 1):
                # Ein einziger Wert: Schritt 0.
                werte.extend([-9] * GRUPPE)
                continue
            werte.extend(zufallsgruppe(z, z.zahl(1, 16)))
    codes, gitter = packen(werte)
    # ⚑ Schritt 255 und Versatz -128 kommen aus keinem int8, sind aber
    #   festgelegt; also stehen sie im Vektor: Zeile 2, Gruppe 0, und dort
    #   ein Code 15 an Gewicht 7.
    gitter[2 * 4] = 255
    gitter[2 * 4 + 1] = 0x80
    byte, s = bitstelle(7)
    o = 4 * BYTES_JE_GRUPPE
    codes[o + byte] = (codes[o + byte] & ~(15 << s)) | (15 << s)
    w = entpacken(codes, gitter)
    assert w[2 * spalten + 7] == 255 * 15 - 128
    # Klein genug, dass keine Zeile saettigt: Eine gesaettigte Ausgabe
    # verdeckt jeden Fehler in der Summe davor. Zeile 2 mit Shift 14, damit
    # das eine grosse Gewicht in ihrer Ausgabe sichtbar bleibt.
    x = [z.zahl(-400, 400) for _ in range(spalten)]
    w_shifts = [7, 9, 14, 4]
    act_frac, out_frac = 8, 8
    y = linear(x, w, spalten, w_shifts, act_frac, out_frac)
    return vektor(
        "linear_vierbit",
        {"zeilen": zeilen, "spalten": spalten, "w_shifts": w_shifts,
         "act_frac": act_frac, "out_frac": out_frac},
        {"x": tensor(x, "int16", [spalten]),
         "codes": tensor(codes, "int32", [len(codes)]),
         "gitter": tensor(gitter, "int32", [len(gitter)])},
        {"y": tensor(y, "int16", [zeilen])},
    )


def v_packen() -> dict:
    z = Kongruenz(12)
    zeilen, spalten = 2, 384
    werte: list[int] = []
    for zeile in range(zeilen):
        for g in range(spalten // GRUPPE):
            if (zeile, g) == (0, 1):
                # Nur gerade Abstaende ab einem Wert, der nicht der kleinste
                # moegliche ist: Schritt 6, Versatz der kleinste Wert.
                werte.extend(6 * z.zahl(0, 15) - 50 for _ in range(GRUPPE))
                continue
            if (zeile, g) == (1, 2):
                werte.extend([127] * GRUPPE)
                continue
            werte.extend(zufallsgruppe(z, z.zahl(1, 16)))
    codes, gitter = packen(werte)
    assert entpacken(codes, gitter) == werte
    return vektor(
        "packen_vierbit",
        {"zeilen": zeilen, "spalten": spalten},
        {"werte": tensor(werte, "int8", [zeilen, spalten])},
        {"codes": tensor(codes, "int32", [len(codes)]),
         "gitter": tensor(gitter, "int32", [len(gitter)])},
    )


def main() -> int:
    vektoren = [v_linear(), v_packen()]
    ZIEL.mkdir(parents=True, exist_ok=True)
    for v in vektoren:
        pfad = ZIEL / f"{v['name']}.golden.json"
        pfad.write_text(json.dumps(v, indent=1) + "\n", encoding="utf-8")
        print(f"[vierbit] {pfad.relative_to(HIER.parent.parent)}")
    y_lin = vektoren[0]["outputs"]["y"]["data"]
    if sum(1 for y in y_lin if -32768 < y < 32767) < 3:
        print("[vierbit] FEHLER: linear_vierbit saettigt zu oft", file=sys.stderr)
        return 1
    print(f"[vierbit] {len(vektoren)} Vektoren, linear_vierbit y = {y_lin}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
