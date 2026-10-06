#!/usr/bin/env python3
"""Erzeugt die Konformitaetsvektoren der Gruppe `ternaer`, unabhaengig.

Die Sollwerte rechnet dieses Skript selbst, in reiner Ganzzahlarithmetik
und ohne eine Zeile aus den Rust-Kernen: eigene Rundung, eigene Packung,
eigene Walsh-Hadamard-Transformation. Eine Uebereinstimmung mit den Kernen
belegt deshalb **Richtigkeit** und nicht nur Determinismus (Feld
`herkunft: unabhaengig`).

Gedeckt werden die vier Bausteine, ueber die ein ternaeres Modell rechnet
und trainiert:

| Vektor | Baustein |
|---|---|
| `linear_ternaer` | lineare Schicht mit gepackten ternaeren Gewichten, Code 3 (+2), leere Gruppe, Betrag bis 32 767 |
| `packen_ternaer` | int8/int16-Werte zu 2-Bit-Codes und Gruppenbetraegen |
| `ternaer_aus_master` | Master (i32, 20 Bruchstellen) zu ternaerem Gewicht und Zeilenshift |
| `drehen` | blockweise Hadamard-Drehung mit Vorzeichen, Rechtsshift |
| `drehen_saettigung` | dieselbe Drehung mit Linksshift und Saettigung |

Aufruf aus der Wurzel des Repositoriums:

    python3 INTEGER_LLM/conformance/ternaer_erzeugen.py

Ueberschreibt die fuenf Dateien unter `conformance/vectors/ternaer/`. Die
Eingaben kommen aus einem festen linearen Kongruenzgenerator, also ergibt
jeder Lauf dieselben Bytes.
"""

from __future__ import annotations

import hashlib
import json
import struct
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
ZIEL = HIER / "vectors" / "ternaer"
SPEC = HIER.parent / "theta_v" / "spec.json"

GRUPPE = 128
BYTES_JE_GRUPPE = GRUPPE // 4
DREHBLOCK = 1024
MASTER_FRAC = 20


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


# --- Ternaeres Format -----------------------------------------------------


def bitstelle(i: int) -> tuple[int, int]:
    """Byte und Bitversatz des Gewichts `i` einer Gruppe: je 64 Gewichte ein
    Block von 16 Byte, Byte `j` traegt in den Bits `2s, 2s+1` das Gewicht
    `16 s + j` des Blocks."""
    block, p = divmod(i, 64)
    s, j = divmod(p, 16)
    return block * 16 + j, 2 * s


def packen(werte: list[int], spalten: int) -> tuple[list[int], list[int]]:
    muster: list[int] = []
    betraege: list[int] = []
    for g in range(len(werte) // GRUPPE):
        gruppe = werte[g * GRUPPE:(g + 1) * GRUPPE]
        nicht_null = {abs(v) for v in gruppe if v != 0}
        assert len(nicht_null) <= 1, f"Gruppe {g} ist nicht ternaer"
        betrag = nicht_null.pop() if nicht_null else 0
        assert betrag <= 32767
        bytes_ = [0] * BYTES_JE_GRUPPE
        for i, v in enumerate(gruppe):
            code = 2 if v > 0 else (0 if v < 0 else 1)
            b, s = bitstelle(i)
            bytes_[b] |= code << s
        muster.extend(bytes_)
        betraege.append(betrag)
    return muster, betraege


def entpacken(muster: list[int], betraege: list[int]) -> list[int]:
    werte: list[int] = []
    for g, betrag in enumerate(betraege):
        bytes_ = muster[g * BYTES_JE_GRUPPE:(g + 1) * BYTES_JE_GRUPPE]
        for i in range(GRUPPE):
            b, s = bitstelle(i)
            code = (bytes_[b] >> s) & 3
            werte.append(betrag * (code - 1))
    return werte


def linear(x: list[int], w: list[int], spalten: int, w_shifts: list[int],
           act_frac: int, out_frac: int) -> list[int]:
    aus = []
    for z, shift in enumerate(w_shifts):
        zeile = w[z * spalten:(z + 1) * spalten]
        summe = sum(a * b for a, b in zip(zeile, x))
        aus.append(klemmen(umskalieren(summe, shift + act_frac, out_frac), -32768, 32767))
    return aus


def ternaer_aus_master(master: list[int], spalten: int) -> tuple[list[int], list[int]]:
    """Je Gruppe `a = Mittel der Betraege` (gerundet), Gewicht `sign(m) a`, wenn
    `2|m| > a`, sonst 0; danach je Zeile in int8 mit Zeilenshift."""
    werte: list[int] = []
    for g in range(len(master) // GRUPPE):
        gruppe = master[g * GRUPPE:(g + 1) * GRUPPE]
        a = rechts_gerade(sum(abs(m) for m in gruppe), 7)
        for m in gruppe:
            t = (1 if m > 0 else -1) if 2 * abs(m) > a else 0
            werte.append(t * a)
    gewichte: list[int] = []
    shifts: list[int] = []
    for z in range(len(werte) // spalten):
        zeile = werte[z * spalten:(z + 1) * spalten]
        groesster = max(abs(v) for v in zeile)
        if groesster == 0:
            shifts.append(MASTER_FRAC)
            gewichte.extend([0] * spalten)
            continue
        s = 0
        while (groesster >> s) > 127:
            s += 1
        shifts.append(MASTER_FRAC - s)
        gewichte.extend(klemmen(rechts_gerade(v, s), -127, 127) for v in zeile)
    return gewichte, shifts


def hadamard(block: list[int]) -> list[int]:
    """Sylvester-Hadamard ausmultipliziert: H[i][k] = (-1)^popcount(i & k)."""
    n = len(block)
    return [sum(v if bin(i & k).count("1") % 2 == 0 else -v for k, v in enumerate(block))
            for i in range(n)]


def drehen(x: list[int], x_frac: int, vorzeichen: list[int], ziel_frac: int) -> list[int]:
    t = [a * s for a, s in zip(x, vorzeichen)]
    gedreht: list[int] = []
    for b in range(len(t) // DREHBLOCK):
        gedreht.extend(hadamard(t[b * DREHBLOCK:(b + 1) * DREHBLOCK]))
    shift = x_frac + 5 - ziel_frac
    return [klemmen(rechts_gerade(v, shift) if shift >= 0 else v << (-shift), -32768, 32767)
            for v in gedreht]


# --- Vektoren -------------------------------------------------------------

PACKFORM = {"int8": "b", "int16": "h", "int32": "i"}


def tensor(daten: list[int], dtype: str, form: list[int]) -> dict:
    roh = struct.pack("<" + str(len(daten)) + PACKFORM[dtype], *daten)
    return {"dtype": dtype, "shape": form, "hash": hashlib.sha256(roh).hexdigest(), "data": daten}


def vektor(name: str, metadata: dict, eingaben: dict, ausgaben: dict) -> dict:
    return {
        "name": name,
        "level": "ternaer",
        "theta_v_hash": "sha256:" + hashlib.sha256(SPEC.read_bytes()).hexdigest(),
        "herkunft": "unabhaengig",
        "metadata": metadata,
        "inputs": eingaben,
        "outputs": ausgaben,
    }


def v_linear() -> dict:
    z = Kongruenz(1)
    zeilen, spalten = 4, 256
    werte: list[int] = []
    for zeile in range(zeilen):
        for g in range(spalten // GRUPPE):
            # Zeile 3, Gruppe 1 bleibt leer; Zeile 2 traegt einen Kopfbetrag.
            if (zeile, g) == (3, 1):
                werte.extend([0] * GRUPPE)
                continue
            betrag = 32767 if zeile == 2 and g == 0 else z.zahl(1, 127)
            werte.extend(betrag * z.zahl(-1, 1) for _ in range(GRUPPE))
    muster, betraege = packen(werte, spalten)
    # ⚑ Code 3 heisst +2. Der Packer erzeugt ihn nie; festgelegt ist er
    #   trotzdem, also steht er im Vektor: Gewicht 5 der Zeile 0, Gruppe 0.
    b, s = bitstelle(5)
    muster[b] = (muster[b] & ~(3 << s)) | (3 << s)
    w = entpacken(muster, betraege)
    assert w[5] == 2 * betraege[0]
    # Klein genug, dass nur Zeile 3 (Shift 0) bewusst saettigt: Eine
    # gesaettigte Ausgabe verdeckt jeden Fehler in der Summe davor.
    x = [z.zahl(-400, 400) for _ in range(spalten)]
    w_shifts = [7, 7, 20, 0]
    act_frac, out_frac = 8, 8
    y = linear(x, w, spalten, w_shifts, act_frac, out_frac)
    return vektor(
        "linear_ternaer",
        {"zeilen": zeilen, "spalten": spalten, "w_shifts": w_shifts,
         "act_frac": act_frac, "out_frac": out_frac},
        {"x": tensor(x, "int16", [spalten]),
         "muster": tensor(muster, "int32", [len(muster)]),
         "betraege": tensor(betraege, "int16", [len(betraege)])},
        {"y": tensor(y, "int16", [zeilen])},
    )


def v_packen() -> dict:
    z = Kongruenz(2)
    zeilen, spalten = 2, 256
    werte: list[int] = []
    for zeile in range(zeilen):
        for g in range(spalten // GRUPPE):
            betrag = 30000 if (zeile, g) == (1, 1) else z.zahl(1, 127)
            werte.extend(betrag * z.zahl(-1, 1) for _ in range(GRUPPE))
    muster, betraege = packen(werte, spalten)
    return vektor(
        "packen_ternaer",
        {"zeilen": zeilen, "spalten": spalten},
        {"werte": tensor(werte, "int16", [zeilen, spalten])},
        {"muster": tensor(muster, "int32", [len(muster)]),
         "betraege": tensor(betraege, "int16", [len(betraege)])},
    )


def v_aus_master() -> dict:
    z = Kongruenz(3)
    zeilen, spalten = 3, 256
    master: list[int] = []
    for zeile in range(zeilen):
        if zeile == 2:
            # Eine Zeile aus Nullen: Shift ist die Skala des Masters.
            master.extend([0] * spalten)
            continue
        # Zeile 0 klein (dort steht die Schwellenprobe, und ihr Shift muss
        # sie sichtbar lassen), Zeile 1 gross.
        spanne = 1500 if zeile == 0 else 1 << 22
        master.extend(z.zahl(-spanne, spanne) for _ in range(spalten))
    # ⚑ Die Schwelle genau treffen: Gruppe 0 der Zeile 0 so setzen, dass
    #   ein Eintrag `2|m| == a` hat (bleibt null) und der naechste knapp
    #   darueber liegt (wird `-a`). Beide gehen selbst ins Mittel `a` ein,
    #   also wird der Wert gesucht, bei dem es aufgeht.
    for x in range(1, 2000):
        gruppe = [1000] * 126 + [x, -(x + 1)]
        a = rechts_gerade(sum(abs(m) for m in gruppe), 7)
        if 2 * x == a:
            break
    else:
        raise SystemExit("[ternaer] FEHLER: keine Gruppe trifft die Schwelle")
    assert 2 * (x + 1) > a
    master[0:GRUPPE] = gruppe
    sicht, _ = ternaer_aus_master(master, spalten)
    assert sicht[126] == 0 and sicht[127] < 0, "die Schwelle ist nach dem Zeilenshift nicht sichtbar"
    gewichte, shifts = ternaer_aus_master(master, spalten)
    return vektor(
        "ternaer_aus_master",
        {"zeilen": zeilen, "spalten": spalten, "master_frac": MASTER_FRAC},
        {"master": tensor(master, "int32", [zeilen, spalten])},
        {"gewichte": tensor(gewichte, "int8", [zeilen, spalten]),
         "shifts": tensor(shifts, "int32", [zeilen])},
    )


def v_drehen(name: str, saat: int, x_frac: int, ziel_frac: int, spanne: int) -> dict:
    z = Kongruenz(saat)
    n = 2 * DREHBLOCK
    x = [z.zahl(-spanne, spanne) for _ in range(n)]
    vorzeichen = [1 if z.zahl(0, 1) else -1 for _ in range(n)]
    y = drehen(x, x_frac, vorzeichen, ziel_frac)
    return vektor(
        name,
        {"x_frac": x_frac, "ziel_frac": ziel_frac},
        {"x": tensor(x, "int16", [n]), "vorzeichen": tensor(vorzeichen, "int8", [n])},
        {"y": tensor(y, "int16", [n])},
    )


def main() -> int:
    vektoren = [
        v_linear(),
        v_packen(),
        v_aus_master(),
        v_drehen("drehen", 4, 10, 8, 2000),
        v_drehen("drehen_saettigung", 5, 2, 9, 300),
    ]
    ZIEL.mkdir(parents=True, exist_ok=True)
    for v in vektoren:
        pfad = ZIEL / f"{v['name']}.golden.json"
        pfad.write_text(json.dumps(v, indent=1) + "\n", encoding="utf-8")
        print(f"[ternaer] {pfad.relative_to(HIER.parent.parent)}")
    y_lin = vektoren[0]["outputs"]["y"]["data"]
    if sum(1 for y in y_lin if -32768 < y < 32767) < 3:
        print("[ternaer] FEHLER: linear_ternaer saettigt zu oft", file=sys.stderr)
        return 1
    # Linksshift UND Saettigung muessen beide reichlich vorkommen, sonst
    # prueft der Vektor nur eines von beiden.
    y_satt = vektoren[4]["outputs"]["y"]["data"]
    sattes = sum(1 for y in y_satt if y in (32767, -32768))
    if sattes < 100 or len(y_satt) - sattes < 100:
        print(f"[ternaer] FEHLER: drehen_saettigung saettigt {sattes} von {len(y_satt)}", file=sys.stderr)
        return 1
    print(f"[ternaer] {len(vektoren)} Vektoren, drehen_saettigung saettigt {sattes} Werte")
    return 0


if __name__ == "__main__":
    sys.exit(main())
