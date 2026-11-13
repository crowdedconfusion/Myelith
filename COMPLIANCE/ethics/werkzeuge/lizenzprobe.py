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
`COMPLIANCE/ethics/Lizenzlage.md` und stammt von Menschen.

**Und sie prüft nur, was lokal liegt.** Ein Modell, das nie
heruntergeladen wurde, hat hier kein Verzeichnis und erzeugt keinen
Treffer. Ein leerer Lauf ist deshalb kein Beleg für eine saubere
Modellfamilie.

## ⚑ Private Modelle und Ordner ohne Katalogeintrag (seit 2026-10-19)

Kap. 10.1 gilt für Modelle, die **im Netz** rechnen. Ein Eintrag in
`MODELS/llm/KATALOG.json` mit `betrieb: lokal` ist privat und nicht für den
Netzbetrieb zugelassen (der Knoten lädt ihn nicht); seine Lizenz wird gelesen
und mit ihren Sperren genannt, aber nicht an Kap. 10.1 gemessen. Ein Ordner,
den der Katalog gar nicht kennt, wird geprüft, wenn er eine Lizenzdatei hat
(etwa die Ternary-Bonsai-Quellen); fehlt sie, ist das ein Hinweis und kein
Fehler (etwa die eben erkundete Quelle eines selbst gebauten Modells, von der
nur die Konfiguration liegt). Ein Katalogeintrag ohne Lizenzdatei bleibt ein
Fehler.

📌 **Fund 575:** Mit `vulture-126b-a7b-int8` (Qwen3.8-Flash-Next, Qwen Community
License) kam das erste private Modell in den Katalog, und diese Prüfung,
die keine privaten Modelle kannte, machte die CI zu `81a3fb0` rot. Die
sieben Proben der Kurzanleitung fahren sie nicht.

## ⚑ Sehen, Hören und Sprechen (seit 2026-11-11)

Die Gewichte unter `MODELS/audio` und `MODELS/vision` kommen als einzelne
Dateien und werden beim Ablegen umbenannt; ein Verzeichnis für eine
Lizenzdatei gibt es nicht. Ihre Angabe steht deshalb je Rubrik in
`LIZENZEN.json` (Quelle, Revision, Lizenz, Lizenzdatei), die Lizenztexte
liegen versioniert daneben. Geprüft wird: Jede Datei, die in der Rubrik
liegt, steht im Katalog; jeder Eintrag hat seine Lizenzdatei; deren Text
ist die genannte Lizenz und trägt keine Sperre. Ein `hinweis` im Eintrag
wird genannt, aber nicht als Fehler gewertet.

📌 Bis dahin war das eine benannte Lücke: Der Abschlusssatz sagte, dass
`audio` und `vision` nicht geprüft sind.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]

# ⚑ **Die Rubrik der Quellmodelle, nicht `MODELS/` selbst** (Umzug
# 2026-09-21). Ein Lauf ueber `MODELS/` naehme `llm`, `audio` und
# `vision` fuer Modellverzeichnisse und meldete drei fehlende
# Lizenzdateien: Eine Pruefung, die ihre eigenen Rubriken anklagt, ist
# keine.
MODELLE = REPO / "MODELS" / "llm"
KATALOG = MODELLE / "KATALOG.json"


def katalog_nach_ordner() -> dict[str, dict]:
    """Die Katalogeintraege nach ihrem Ordner unter `MODELS/llm`
    (`hf_verzeichnis`)."""
    if not KATALOG.is_file():
        return {}
    katalog = json.loads(KATALOG.read_text(encoding="utf-8"))
    return {
        v["hf_verzeichnis"]: {"schluessel": k, **v}
        for k, v in katalog.items()
        if not k.startswith("_") and isinstance(v, dict) and v.get("hf_verzeichnis")
    }

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


SINNE = [REPO / "MODELS" / "audio", REPO / "MODELS" / "vision"]
# Was in einer Rubrik der Sinne liegen darf, ohne ein Gewicht zu sein.
KEIN_GEWICHT = re.compile(r"^(\.gitkeep|LIZENZEN\.json|LIZENZ-.*\.txt|\.DS_Store)$")


def sinne_pruefen() -> int:
    """Die Rubriken der Sinne gegen ihre `LIZENZEN.json`; gibt die Zahl
    der Fehler zurueck."""
    fehler = 0
    for rubrik in SINNE:
        name = rubrik.name
        katalog_datei = rubrik / "LIZENZEN.json"
        if not katalog_datei.is_file():
            print(f"[lizenz] FEHLT: MODELS/{name}/LIZENZEN.json")
            fehler += 1
            continue
        katalog = {k: v for k, v in json.loads(katalog_datei.read_text(encoding="utf-8")).items() if not k.startswith("_")}
        # Jede liegende Datei steht im Katalog; Ordner (etwa Qwen3-TTS) ueber
        # ihren Eintrag mit Unterpfad.
        for f in sorted(rubrik.iterdir()):
            if KEIN_GEWICHT.match(f.name):
                continue
            if f.is_file() and f.name not in katalog:
                print(f"[lizenz] FEHLT: MODELS/{name}/{f.name} liegt da und steht nicht in LIZENZEN.json")
                fehler += 1
        for datei, e in katalog.items():
            text_datei = rubrik / e.get("lizenzdatei", "")
            if not e.get("lizenzdatei") or not text_datei.is_file():
                print(f"[lizenz] FEHLT: {name}/{datei} hat keine Lizenzdatei")
                fehler += 1
                continue
            art, sperren = beurteile(text_datei.read_text(encoding="utf-8", errors="replace"))
            if art is None or art != e.get("lizenz"):
                print(f"[lizenz] UNBEKANNT: {name}/{datei} nennt {e.get('lizenz')}, der Text ist {art or 'weder Apache 2.0 noch MIT'}")
                fehler += 1
                continue
            if sperren:
                print(f"[lizenz] UNBRAUCHBAR: {name}/{datei} ist {art}, aber: {', '.join(sperren)}")
                fehler += 1
                continue
            da = "liegt da" if (rubrik / datei).exists() else "nicht geladen"
            print(f"[lizenz] ok      {name}/{datei}: {art} ({da})")
            if e.get("hinweis"):
                print(f"[lizenz] ⚠️ Hinweis {name}/{datei}: {e['hinweis']}")
    return fehler


def main() -> int:
    print("[lizenz] Lizenz-Prüfliste Basismodell (Kap. 10.1)")
    if not MODELLE.is_dir():
        print(f"[lizenz] kein Modellverzeichnis unter {MODELLE.relative_to(REPO)}, nichts zu prüfen")
        return 0

    verzeichnisse = sorted(p for p in MODELLE.iterdir() if p.is_dir())
    print(f"[lizenz] {len(verzeichnisse)} Modellverzeichnis(se)")

    katalog = katalog_nach_ordner()
    fehler = 0
    for v in verzeichnisse:
        name = v.name
        eintrag = katalog.get(name)
        privat = eintrag is not None and eintrag.get("betrieb") == "lokal"
        datei = lizenzdatei(v)
        if datei is None and eintrag is None:
            print(f"[lizenz] Hinweis: {name} steht nicht im Katalog und hat keine Lizenzdatei, nicht geprüft")
            continue
        if datei is None:
            print(f"[lizenz] FEHLT: {name} hat keine Lizenzdatei")
            fehler += 1
            continue
        art, sperren = beurteile(datei.read_text(encoding="utf-8", errors="replace"))
        if privat:
            # ⚑ Gelesen und genannt, aber nicht an Kap. 10.1 gemessen.
            was = art or "weder Apache 2.0 noch MIT"
            print(
                f"[lizenz] privat  {name}: {was}"
                + (f", mit {', '.join(sperren)}" if sperren else "")
                + f" ({eintrag['schluessel']}, betrieb lokal, nicht fürs Netz)"
            )
            continue
        if art is None:
            print(f"[lizenz] UNBEKANNT: {name} — weder Apache 2.0 noch MIT erkennbar")
            fehler += 1
            continue
        if sperren:
            print(f"[lizenz] UNBRAUCHBAR: {name} ist {art}, aber: {', '.join(sperren)}")
            fehler += 1
            continue
        print(f"[lizenz] ok      {name}: {art}")

    fehler += sinne_pruefen()
    if fehler:
        print(f"[lizenz] FEHLGESCHLAGEN: {fehler} Modell(e) erfüllen Kap. 10.1 nicht")
        return 1
    # ⚑ **Der Satz nennt den Umfang.** Private Modelle sind gelesen, aber
    # nicht an Kap. 10.1 gemessen.
    print(
        "[lizenz] PASSED: alle lokal vorliegenden Quellmodelle des Netzes unter "
        "MODELS/llm und die Gewichte der Sinne unter MODELS/audio und MODELS/vision "
        "erfüllen Kap. 10.1 (private Modelle nicht geprüft)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
