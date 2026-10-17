#!/usr/bin/env python3
"""**Ein Gemisch mit gemischter Genauigkeit**: die haeufigen Experten int8,
die seltenen ternaer.

# ⚑ Wozu

Passt ein Gemisch nicht in den Arbeitsspeicher, kommen vor allem die
**seltenen** Experten von der Platte, denn die haeufigen haelt der
Dateicache. Ternaer gepackt ist ein Experte rund viermal kleiner. Werden nur
die seltenen ternaer, sinkt das Lesen bei einem Fehlzugriff auf ein Viertel,
und es passen mehr Experten in denselben Speicher. Die Qualitaet leidet
weniger als bei einem voll ternaeren Modell, weil ein seltener Experte
selten rechnet.

# Was es tut

Aus einem int8-Artefakt und einem ternaeren Artefakt **desselben Modells**
(gleiche `model_config.json`, gleiche Skalen und Tabellen) entsteht ein
drittes: Die Experten, die zusammen hoechstens `--anteil` Promille der
Aufrufe tragen (nach einem Expertenprofil, die seltensten zuerst), kommen
aus dem ternaeren, alles andere aus dem int8. Dateien werden **hart
verlinkt**, nicht kopiert; das neue Artefakt kostet fast keinen Platz.
Geschrieben wird nach `<ziel>.unfertig` und erst am Ende umbenannt.

⚠️ **Ein Expertenprofil ist eine Zaehlung an einem Text.** Welche Experten
selten sind, gilt fuer die Art Arbeit, an der gezaehlt wurde.

Aufruf, aus der Wurzel:

    python3 INTEGER_LLM/tools/experten_mischen.py <int8> <ternaer> <profil.json> <ziel> --anteil <promille>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
from pathlib import Path

GLEICH = ("model_config.json", "scales.json", "luts.json")
# Wie `pruefstand::MARKENNAME` der Laufzeit.
PRUEFMARKE = ".geprueft"
EXPERTE = re.compile(r"^model_layers_(\d+)_mlp_experts_(\d+)_(gate|up|down)_proj_weight$")


def rangfolge(profil: dict, ebenen: int, je: int) -> list[tuple[int, int]]:
    """Alle Experten, die **seltensten** zuerst, bei Gleichstand nach Lage."""
    z = profil["zaehler"]

    def zahl(e: int, x: int) -> int:
        return z[e][x] if e < len(z) and x < len(z[e]) else 0

    alle = [(e, x) for e in range(ebenen) for x in range(je)]
    alle.sort(key=lambda p: (zahl(*p), p))
    return alle


def seltene(profil: dict, ebenen: int, je: int, promille: int) -> set[tuple[int, int]]:
    """Die seltensten Experten, die zusammen hoechstens `promille` der
    Aufrufe tragen."""
    z = profil["zaehler"]
    gesamt = sum(sum(r) for r in z)
    wahl: set[tuple[int, int]] = set()
    summe = 0
    for e, x in rangfolge(profil, ebenen, je):
        n = z[e][x] if e < len(z) and x < len(z[e]) else 0
        if (summe + n) * 1000 > gesamt * promille:
            break
        summe += n
        wahl.add((e, x))
    return wahl


def dateien(eintrag: dict) -> list[str]:
    return [eintrag[k] for k in ("file", "shifts_file", "betraege_file") if k in eintrag]


def main() -> int:
    a = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    a.add_argument("int8", type=Path)
    a.add_argument("ternaer", type=Path)
    a.add_argument("profil", type=Path)
    a.add_argument("ziel", type=Path)
    a.add_argument("--anteil", type=int, required=True, help="Promille der Aufrufe, die ternaer werden duerfen")
    x = a.parse_args()

    for f in GLEICH:
        if (x.int8 / f).read_bytes() != (x.ternaer / f).read_bytes():
            print(f"FEHLER: {f} unterscheidet sich; die Artefakte sind nicht dasselbe Modell", file=sys.stderr)
            return 1
    konfig = json.loads((x.int8 / "model_config.json").read_text())
    ebenen, je = konfig["num_layers"], konfig["num_experts"]
    profil = json.loads(x.profil.read_text())
    wahl = seltene(profil, ebenen, je, x.anteil)

    m8 = json.loads((x.int8 / "weights_manifest.json").read_text())
    mt = json.loads((x.ternaer / "weights_manifest.json").read_text())
    if set(m8) != set(mt):
        print("FEHLER: die Manifeste nennen verschiedene Tensoren", file=sys.stderr)
        return 1

    unfertig = x.ziel.with_name(x.ziel.name + ".unfertig")
    if unfertig.exists():
        shutil.rmtree(unfertig)
    unfertig.mkdir(parents=True)
    neu: dict = {}
    herkunft: dict[str, Path] = {}
    for name, eintrag in m8.items():
        t = EXPERTE.match(name)
        ternaer = t is not None and (int(t.group(1)), int(t.group(2))) in wahl
        quelle, e = (x.ternaer, mt[name]) if ternaer else (x.int8, eintrag)
        neu[name] = e
        for d in dateien(e):
            if d in herkunft and herkunft[d] != quelle:
                print(f"FEHLER: {d} kaeme aus beiden Artefakten", file=sys.stderr)
                return 1
            herkunft[d] = quelle
    # Alles, was keine Datei irgendeines Manifesteintrags ist (Tabellen,
    # Wortschatz, Beschreibung), kommt aus dem int8. 📌 Erst stand hier
    # „was nicht schon belegt ist“, und dann kamen die int8-Dateien der
    # ternaer gewordenen Experten zusaetzlich mit: Laden ging, aber der
    # Bedarf des Dispatchers zaehlt Dateien und haette beide gerechnet.
    gewichtsdateien = {d for m in (m8, mt) for e in m.values() for d in dateien(e)}
    for f in os.listdir(x.int8):
        # Die Pruefmarke des Laders gilt nur fuer ihr eigenes Artefakt;
        # verlinkt schrieb das Laden des neuen in die Marke der Quelle
        # (Fund 551).
        if f.startswith(PRUEFMARKE):
            continue
        if f not in gewichtsdateien and f not in ("weights_manifest.json", "theta_v.json") and (x.int8 / f).is_file():
            herkunft[f] = x.int8
    for f, quelle in herkunft.items():
        os.link(quelle / f, unfertig / f)
    text = json.dumps(neu, indent=2, sort_keys=True).encode()
    (unfertig / "weights_manifest.json").write_bytes(text)
    theta = json.loads((x.int8 / "theta_v.json").read_text())
    theta["weights_hash"] = hashlib.sha256(text).hexdigest()
    (unfertig / "theta_v.json").write_text(json.dumps(theta, indent=2, sort_keys=True))
    if x.ziel.exists():
        shutil.rmtree(x.ziel)
    unfertig.rename(x.ziel)

    gesamt = sum(sum(r) for r in profil["zaehler"])
    getragen = sum(profil["zaehler"][e][i] for e, i in wahl if e < len(profil["zaehler"]) and i < len(profil["zaehler"][e]))
    groesse = sum((x.ziel / f).stat().st_size for f in herkunft)
    print(
        f"{x.ziel.name}: {len(wahl)} von {ebenen * je} Experten ternaer, sie tragen "
        f"{1000 * getragen // max(gesamt, 1)} Promille der Aufrufe; Gewichte {groesse / 1e9:.2f} GB"
    )
    print("Fertig")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
