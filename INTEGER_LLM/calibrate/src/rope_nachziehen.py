"""Zieht die RoPE-Tabellen eines vorhandenen Artefakts auf seine volle
Kontextlaenge nach, ohne Gewichte und Skalen anzufassen.

    python3 INTEGER_LLM/calibrate/src/rope_nachziehen.py <artefakt> [--zeilen N] [--trocken] [--berichtigen]

⚑ **Warum es das braucht.** Die Kontextgrenze eines Artefakts ist das
Kleinere aus `max_context` und den Zeilen seiner Drehtabellen (siehe
`Model::kontextgrenze`). Fruehere Ausfuhren schrieben 40 960 Zeilen, gleich
was das Modell kann, und ein damals gebautes Artefakt traegt sie noch: Das
Qwen3.6-35B-A3B kann 262 144 Positionen und rechnete nur 40 960 (gemessen am
Artefakt vom 2026-09-22). Die Ausfuhr nimmt die Zeilenzahl seither aus dem Modell
(`rope_masse`). Ein Artefakt deshalb neu zu kalibrieren hiesse Stunden fuer
zwei Tabellen von wenigen Megabyte.

⚑ **Dieselbe Herleitung, derselbe Erzeuger, dasselbe Format** wie die
Ausfuhr: `rope_masse` und `generate_rope_luts` aus `luts.py`, geschrieben wie
`export.py` (`<h`, `luts.json` und `theta_v.json` mit sortierten Schluesseln).
Ein zweiter Weg zu denselben Tabellen waere ein zweiter Ort, der
auseinanderlaeuft.

⛔️ **Die Gegenprobe, ohne die nichts geschrieben wird:** Die ersten Zeilen
der neuen Tabelle muessen bytegleich zur alten sein. Weichen sie ab, wurde
das Artefakt mit anderen Massen gebaut (Basis, Drehbreite, Skalierung), und
eine laengere Tabelle mit den heutigen Massen drehte jede Position anders als
die, mit der es kalibriert ist.

⚑ **`--berichtigen` schreibt trotzdem**, und nur dafuer gibt es den Schalter:
wenn die alte Tabelle nachweislich falsch ist. 📌 Anlass (2026-10-13): Das
35B-Artefakt vom 2026-09-22 drehte mit der Basis 1e6, das Modell verlangt
1e7 (`rope_theta` in seiner `model_config.json`, bei Qwen in `rope_scaling`
versteckt). Die Tabelle stammt aus einer Ausfuhr vor der Korrektur in
`rope_masse`. Ausgegeben wird dann die Basis, die die alte Tabelle
tatsaechlich hatte, aus ihrer Zeile 1 zurueckgerechnet.

⚠️ **Neue Dateien, dann umbenannt**, nie an Ort und Stelle beschrieben, auch
`luts.json` und `theta_v.json`: Artefakte teilen Dateien als harte Verweise
(das ternaere 35B mit dem int8-35B), und ein Schreiben in die Datei aenderte
beide.

⚠️ **Die Kennung des Artefakts aendert sich** (`luts_hash` in
`theta_v.json`): Es ist danach ein anderes Artefakt, und ein Knoten, der
gegen den alten Hash rechnet, rechnet gegen ein anderes.
"""

import argparse
import json
import os
import struct
import sys
from pathlib import Path

# Ohne Bytecode: Python legte sonst `__pycache__` in `calibrate/src`.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).parent))
from export import export_json, hash_file  # noqa: E402
from luts import generate_rope_luts, load_nonlinear_spec, rope_masse  # noqa: E402


def lies_tabelle(pfad: Path) -> bytes:
    return pfad.read_bytes() if pfad.exists() else b""


def schreiben_ersetzen(pfad: Path, daten: bytes):
    """Neue Datei unter eigenem Namen, dann an die Stelle: bricht harte
    Verweise auf, statt die geteilte Datei zu aendern."""
    neu = pfad.with_name("." + pfad.name + ".neu")
    neu.write_bytes(daten)
    os.replace(neu, pfad)


def basis_der_tabelle(sin: bytes, cos: bytes, paare: int) -> float:
    """Die Basis, mit der eine Tabelle gerechnet ist, aus Zeile 1 und dem
    Paar 1 zurueckgerechnet (Winkel = Basis^(-1/paare)). Grob, denn die
    Tabelle hat nur wenige Nachkommabits; sie unterscheidet 1e6 von 1e7."""
    import math
    s = struct.unpack_from("<h", sin, 2 * (paare + 1))[0]
    c = struct.unpack_from("<h", cos, 2 * (paare + 1))[0]
    winkel = math.atan2(s, c)
    return winkel ** (-paare) if winkel > 0 else float("inf")


def json_ersetzen(obj: dict, pfad: Path):
    """Wie `export_json`, aber als neue Datei an die Stelle.
    📌 `luts.json` und `theta_v.json` sind ebenso hart verknuepft wie die
    Tabellen (gemessen am 35B, 2026-10-13); in die Datei geschrieben,
    naennte das andere Artefakt danach Hashes, die zu seinen Tabellen nicht
    passen, und luede nicht mehr."""
    neu = pfad.with_name("." + pfad.name + ".neu")
    export_json(obj, neu)
    os.replace(neu, pfad)


def nachziehen(artefakt: Path, zeilen: int = None, trocken: bool = False, berichtigen: bool = False) -> dict:
    konfig = json.loads((artefakt / "model_config.json").read_text(encoding="utf-8"))
    masse = rope_masse(konfig, load_nonlinear_spec())
    soll = zeilen or masse["zeilen"]
    if soll > konfig["max_context"]:
        raise ValueError(f"{soll} Zeilen, aber das Modell kann {konfig['max_context']} Positionen")
    paare = masse["paare"]
    luts_pfad = artefakt / "luts.json"
    luts = json.loads(luts_pfad.read_text(encoding="utf-8"))
    alt_zeilen = luts["cos"]["length"] // paare
    bericht = {"alt": alt_zeilen, "neu": soll, "geschrieben": False, "berichtigt": None}
    if soll <= alt_zeilen and not berichtigen:
        return bericht

    sin_lut, cos_lut = generate_rope_luts(
        max_seq_len=soll,
        head_dim=masse["drehbreite"],
        rope_theta=masse["rope_theta"],
        frac_bits=masse["frac_bits"],
        frequenzen=masse["frequenzen"],
        amplitude=masse["amplitude"],
    )
    neu = {
        "sin": struct.pack(f"<{len(sin_lut)}h", *sin_lut),
        "cos": struct.pack(f"<{len(cos_lut)}h", *cos_lut),
    }
    for name, daten in neu.items():
        if luts[name]["dtype"] != "int16":
            raise ValueError(f"{name}: dtype {luts[name]['dtype']}, erwartet int16")
        alt = lies_tabelle(artefakt / luts[name]["file"])
        if len(alt) != luts[name]["length"] * 2:
            raise ValueError(f"{name}: {len(alt)} Bytes, luts.json sagt {luts[name]['length']} Eintraege")
        if daten[: len(alt)] != alt:
            erste = next(i for i in range(len(alt)) if daten[i] != alt[i]) // 2
            if berichtigen:
                bericht["berichtigt"] = {
                    "zeile": erste // paare,
                    "alte_basis": basis_der_tabelle(lies_tabelle(artefakt / luts["sin"]["file"]), lies_tabelle(artefakt / luts["cos"]["file"]), paare),
                    "neue_basis": masse["rope_theta"],
                }
                continue
            raise ValueError(
                f"{name}: die neue Tabelle weicht ab Eintrag {erste} (Zeile {erste // paare}) von der alten ab. "
                "Das Artefakt ist mit anderen Massen gebaut; nichts geschrieben."
            )
    if trocken:
        return bericht

    for name, daten in neu.items():
        schreiben_ersetzen(artefakt / luts[name]["file"], daten)
        luts[name]["hash"] = hash_file(artefakt / luts[name]["file"])
        luts[name]["length"] = len(daten) // 2
    json_ersetzen(luts, luts_pfad)
    theta = json.loads((artefakt / "theta_v.json").read_text(encoding="utf-8"))
    theta["luts_hash"] = hash_file(luts_pfad)
    json_ersetzen(theta, artefakt / "theta_v.json")
    bericht["geschrieben"] = True
    return bericht


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("artefakt", type=Path)
    p.add_argument("--zeilen", type=int, default=None, help="Zeilen der Tabellen; ohne Angabe max_context")
    p.add_argument("--trocken", action="store_true", help="nur pruefen, nichts schreiben")
    p.add_argument("--berichtigen", action="store_true", help="auch schreiben, wenn die alte Tabelle abweicht (nachweislich falsch)")
    a = p.parse_args()
    b = nachziehen(a.artefakt, a.zeilen, a.trocken, a.berichtigen)
    if b["berichtigt"]:
        r = b["berichtigt"]
        was = "geschrieben" if b["geschrieben"] else "trocken"
        print(f"[rope] {a.artefakt.name}: alte Tabelle mit Basis ~{r['alte_basis']:.3g} ab Zeile {r['zeile']}, "
              f"neu mit {r['neue_basis']:g}; {b['alt']} -> {b['neu']} Zeilen ({was})")
    elif b["neu"] <= b["alt"]:
        print(f"[rope] {a.artefakt.name}: {b['alt']} Zeilen, nichts zu tun")
    elif b["geschrieben"]:
        print(f"[rope] {a.artefakt.name}: {b['alt']} -> {b['neu']} Zeilen, die ersten {b['alt']} bytegleich; geschrieben")
    else:
        print(f"[rope] {a.artefakt.name}: {b['alt']} -> {b['neu']} Zeilen moeglich, die ersten {b['alt']} bytegleich (trocken)")


if __name__ == "__main__":
    main()
