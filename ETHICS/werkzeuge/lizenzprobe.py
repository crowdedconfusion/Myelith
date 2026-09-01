#!/usr/bin/env python3
"""Lizenz-Prüfliste Basismodell (Punkt 1.3, Whitepaper Kap. 10.1).

Liest die Lizenzdatei jedes Modellverzeichnisses und prüft sie gegen die
Kriterien aus Kap. 10.1: **Apache 2.0 oder MIT, ohne Nutzerzahl- und
ohne Geo-Beschränkung.**

## ⚑ Warum die Kriterien so und nicht anders lauten

Sie sind keine Geschmacksfrage. Ein Netz, in dem Fremde für Rechenarbeit
bezahlt werden, **kennt seine Nutzerzahl nicht und kann sie nicht
begrenzen**; eine Lizenz mit Nutzerzahl-Obergrenze wäre also von Anfang
an gebrochen. Dasselbe gilt für Geo-Beschränkungen: Wer Miner in aller
Welt zulässt, kann nicht zusichern, wo gerechnet wird.

## ⚑ Was diese Prüfung nicht leistet

Sie liest **Dateien**, nicht Recht. Ein Verzeichnis ohne Lizenzdatei
fällt auf; eine Lizenz, die anders heißt als ihr Inhalt, fällt nicht
auf. Die variantenscharfe Bewertung steht in
`ETHICS/Lizenzlage.md` und stammt von Menschen.

**Und sie prüft nur, was lokal liegt.** Ein Modell, das nie
heruntergeladen wurde, hat hier kein Verzeichnis und erzeugt keinen
Treffer. Ein leerer Lauf ist deshalb kein Beleg für eine saubere
Modellfamilie.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
MODELLE = REPO / "INTEGER_LLM" / "models"

# Erkennungsmerkmale im Lizenztext, nicht im Dateinamen.
ERKENNUNG = [
    ("Apache 2.0", re.compile(r"Apache License.{0,40}Version 2\.0", re.S)),
    ("MIT", re.compile(r"MIT License|Permission is hereby granted, free of charge")),
]

# ⚑ Muster, die eine sonst passende Lizenz unbrauchbar machen.
SPERREN = [
    ("Nutzerzahl-Obergrenze", re.compile(r"monthly active users|MAU\b", re.I)),
    ("nur nicht-kommerziell", re.compile(r"NON-?COMMERCIAL", re.I)),
    ("Geo-Beschränkung", re.compile(r"not (?:be )?(?:used|available) in|export restriction", re.I)),
]


def lizenzdatei(verzeichnis: Path) -> Path | None:
    for kandidat in sorted(verzeichnis.glob("LICENSE*")):
        if kandidat.is_file():
            return kandidat
    return None


def beurteile(text: str) -> tuple[str | None, list[str]]:
    art = next((name for name, m in ERKENNUNG if m.search(text)), None)
    sperren = [name for name, m in SPERREN if m.search(text)]
    return art, sperren


def main() -> int:
    print("[lizenz] Lizenz-Prüfliste Basismodell (Kap. 10.1)")
    if not MODELLE.is_dir():
        print(f"[lizenz] kein Modellverzeichnis unter {MODELLE.relative_to(REPO)}, nichts zu prüfen")
        return 0

    verzeichnisse = sorted(p for p in MODELLE.iterdir() if p.is_dir())
    print(f"[lizenz] {len(verzeichnisse)} Modellverzeichnis(se)")

    fehler = 0
    for v in verzeichnisse:
        name = v.name
        datei = lizenzdatei(v)
        if datei is None:
            print(f"[lizenz] FEHLT: {name} hat keine Lizenzdatei")
            fehler += 1
            continue
        art, sperren = beurteile(datei.read_text(encoding="utf-8", errors="replace"))
        if art is None:
            print(f"[lizenz] UNBEKANNT: {name} — weder Apache 2.0 noch MIT erkennbar")
            fehler += 1
            continue
        if sperren:
            print(f"[lizenz] UNBRAUCHBAR: {name} ist {art}, aber: {', '.join(sperren)}")
            fehler += 1
            continue
        print(f"[lizenz] ok      {name}: {art}")

    if fehler:
        print(f"[lizenz] FEHLGESCHLAGEN: {fehler} Modell(e) erfüllen Kap. 10.1 nicht")
        return 1
    print("[lizenz] PASSED: alle lokal vorliegenden Modelle erfüllen Kap. 10.1")
    return 0


if __name__ == "__main__":
    sys.exit(main())
