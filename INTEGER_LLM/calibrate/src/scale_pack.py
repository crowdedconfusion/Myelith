"""Versionierte Skalenpakete: der deterministische Teil des Artefaktbaus.

**Warum es das gibt (Fund 32, 2026-08-20).** Der Artefaktbau ist auf
derselben Maschine bitgleich reproduzierbar — zwei Laeufe von
Qwen2.5-0,5B lieferten 593 von 593 Dateien identisch. Er ist es aber
**nicht nachweislich ueber Maschinengrenzen hinweg**, denn die
Aktivierungsstatistik entsteht in Gleitkomma: Der Shift folgt aus
``floor(log2(32767 / absmax))``, und gemessen sitzen **3 von 314**
Skaleneintraegen innerhalb von 0,01 % einer Zweierpotenz-Grenze, der
knappste bei 0,003 %. Eine andere BLAS-Version oder CPU reicht, um sie
umzuwerfen — und ein gekippter Shift aendert die Artefaktbytes, also das
Modell, also die Bitgleichheit.

Fuer einen Cross-Hardware-Test waere das fatal: Er wuerde nicht die
Hardware messen, sondern die Kalibrierung. Genau die Fehlerklasse, die
dieses Projekt an anderer Stelle zehnmal getroffen hat.

**Der Rest des Baus ist dagegen exakt.** Die Gewichtsquantisierung ist
``round(W * 2**shift)`` mit ganzzahligem Shift: Die Multiplikation mit
einer Zweierpotenz ist in IEEE-Gleitkomma exakt (nur der Exponent
aendert sich), und ``round`` ist round-half-to-even. Bei **festen**
Skalen ist der Bau auf jeder Plattform bitgleich. Nichtdeterministisch
sind allein die Aktivierungsstatistik und die LUTs, die aus
``math.exp``/``math.log`` entstehen und damit von der libm abhaengen.

**Deshalb wird nicht das Modell verteilt, sondern die Skalen.** Ein Paket
enthaelt ``scales.json``, die LUT-Binaerdateien samt ``luts.json`` und
``theta_v.json``; zusammen unter 1,5 MB je Modell. Die Gewichte holt sich
jeder selbst von Hugging Face. Nebeneffekt: Der Bau wird deutlich
schneller, weil der gesamte Kalibrierkorpus-Durchlauf entfaellt.

**Gebunden an theta_v.** Ein Paket gilt genau fuer die Spec-Version, mit
der es erzeugt wurde. Passt sie nicht zur aktuellen ``spec.json``, wird
abgebrochen statt stillschweigend gemischt — eine LUT aus einer anderen
Spec ist kein Detail, sondern ein anderes Modell.
"""
from __future__ import annotations

import json
import os
import struct
from pathlib import Path
from typing import Dict, List, Optional

# Ablageort der versionierten Pakete im Repository.
SCALE_PACKS_DIR = Path(__file__).resolve().parent.parent.parent / "scale_packs"

#: ``INTEGER_LLM_SCALE_PACK=0`` erzwingt eine vollstaendige Neukalibrierung,
#: ein Pfad waehlt ein Paket ausserhalb des Standardorts.
SCALE_PACK_ENV = "INTEGER_LLM_SCALE_PACK"

PAKET_DATEIEN = ("scales.json", "luts.json", "theta_v.json")


def paket_pfad(model_name: str) -> Optional[Path]:
    """Pfad zum zu verwendenden Skalenpaket, oder ``None`` fuer Neukalibrierung."""
    gesetzt = os.environ.get(SCALE_PACK_ENV, "").strip()
    if gesetzt in ("0", "aus", "off"):
        return None
    kandidat = Path(gesetzt) if gesetzt else SCALE_PACKS_DIR / model_name
    return kandidat if (kandidat / "scales.json").is_file() else None


def _spec_version() -> str:
    from .luts import load_nonlinear_spec  # zirkelfrei: nur zur Laufzeit
    spec_pfad = Path(__file__).resolve().parent.parent.parent / "theta_v" / "spec.json"
    return json.loads(spec_pfad.read_text())["theta_v"]["version"]


def lade(paket: Path) -> tuple[Dict, Dict[str, List[int]]]:
    """Laedt Skalen und LUTs aus einem Paket.

    Bricht ab, wenn die theta_v-Version des Pakets nicht zur aktuellen
    ``spec.json`` passt, oder wenn eine LUT-Datei nicht zu ihrem im
    Manifest hinterlegten Hash passt. Beides waere ein stiller
    Modellwechsel.
    """
    for datei in PAKET_DATEIEN:
        if not (paket / datei).is_file():
            raise FileNotFoundError(f"Skalenpaket unvollstaendig: {paket / datei} fehlt")

    paket_version = json.loads((paket / "theta_v.json").read_text())["version"]
    aktuell = _spec_version()
    if paket_version != aktuell:
        raise ValueError(
            f"Skalenpaket {paket.name} traegt theta_v {paket_version}, "
            f"die spec.json aber {aktuell}. Ein Paket gilt nur fuer die Spec, "
            f"mit der es erzeugt wurde — LUTs und Skalen haengen daran. "
            f"Entweder das passende Paket neu erzeugen "
            f"(tools/skalenpaket_bauen.py) oder mit "
            f"{SCALE_PACK_ENV}=0 vollstaendig neu kalibrieren."
        )

    scales = json.loads((paket / "scales.json").read_text())
    manifest = json.loads((paket / "luts.json").read_text())

    from .export import hash_file  # gleiche Hash-Funktion wie beim Schreiben
    luts: Dict[str, List[int]] = {}
    for name, eintrag in manifest.items():
        bin_pfad = paket / eintrag["file"]
        if not bin_pfad.is_file():
            raise FileNotFoundError(f"LUT-Datei fehlt: {bin_pfad}")
        ist = hash_file(bin_pfad)
        if ist != eintrag["hash"]:
            raise ValueError(
                f"LUT {name} passt nicht zu ihrem Hash im Paketmanifest "
                f"({ist[:16]}… statt {eintrag['hash'][:16]}…). Paket beschaedigt."
            )
        roh = bin_pfad.read_bytes()
        luts[name] = list(struct.unpack(f"<{eintrag['length']}h", roh))

    return scales, luts
