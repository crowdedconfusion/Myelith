#!/usr/bin/env python3
"""Hält Konformitätsläufe auf verschiedener Hardware fest (TESTCLIENT 2.3).

Aufruf:
  python3 BENCHMARKS/Inferenz/kreuzhardware.py aufnehmen <ausgabe von run.sh> \\
      --plattform linux-x86_64 --art emuliert --weg reference [--hinweis "..."]
  python3 BENCHMARKS/Inferenz/kreuzhardware.py uebersicht

# ⚑ Warum es diese Datei gibt

Laufprotokolle sind flüchtig: Sie liegen in einem Zwischenordner, und der
wird nach der Auswertung gelöscht. Ein Nachweis, dass derselbe Vektorsatz
auf zwei Architekturen bitgleich besteht, muss aber bleiben. Er steht
deshalb hier, je Lauf eine Zeile in `results/kreuzhardware.jsonl`, und die
Übersicht `results/kreuzhardware.md` wird daraus erzeugt.

# Was festgehalten wird

- **Der Vektorsatz** als SHA-256 über alle Dateien unter
  `INTEGER_LLM/conformance/vectors` (Pfad und Inhalt, sortiert). Zwei Läufe
  sind nur vergleichbar, wenn dieser Wert gleich ist.
- **Das Ergebnis** aus der Zeile `=== Ergebnis: a/b bestanden, …` von
  `run.sh`. Nur ein volles Ergebnis zählt.
- **Die Plattform** (Architektur und System) und **die Art**: `nativ` oder
  `emuliert`. ⚠️ Ein Lauf unter Rosetta führt x86_64-Befehle aus, aber auf
  derselben Maschine; er belegt die Übersetzung für x86_64, nicht fremde
  Hardware. Die Übersicht sagt das, statt es zu verschweigen.

# Wann ein Vektorsatz als bestätigt gilt

Wenn er auf mindestens zwei Architekturen vollständig besteht. Steht eine
davon nur emuliert da, sagt das Urteil es dazu.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
VEKTOREN = REPO / "INTEGER_LLM" / "conformance" / "vectors"
# `MYL_KREUZ_ZIEL` lenkt nur die Gegenproben in einen Zwischenordner.
ERGEBNISSE = pathlib.Path(os.environ.get("MYL_KREUZ_ZIEL") or REPO / "BENCHMARKS" / "Inferenz" / "results")
JSONL = ERGEBNISSE / "kreuzhardware.jsonl"
UEBERSICHT = ERGEBNISSE / "kreuzhardware.md"
ERGEBNIS = re.compile(r"=== Ergebnis: (\d+)/(\d+) bestanden, (\d+) fehlgeschlagen ===")


def vektorsatz() -> str:
    h = hashlib.sha256()
    for p in sorted(q for q in VEKTOREN.rglob("*") if q.is_file()):
        rel = p.relative_to(VEKTOREN).as_posix().encode()
        h.update(len(rel).to_bytes(4, "little") + rel)
        inhalt = p.read_bytes()
        h.update(len(inhalt).to_bytes(8, "little") + inhalt)
    return h.hexdigest()


def fassung() -> str:
    return (REPO / "VERSION.md").read_text(encoding="utf-8").splitlines()[0].strip()


def aufnehmen(args: argparse.Namespace) -> int:
    text = pathlib.Path(args.ausgabe).read_text(encoding="utf-8", errors="replace")
    treffer = ERGEBNIS.findall(text)
    if len(treffer) != 1:
        print(f"[kreuz] FEHLER: {len(treffer)} Ergebniszeilen in {args.ausgabe}, erwartet eine")
        return 1
    bestanden, gesamt, rot = (int(x) for x in treffer[0])
    eintrag = {
        "datum": datetime.date.today().isoformat(),
        "fassung": fassung(),
        "vektorsatz": vektorsatz(),
        "plattform": args.plattform,
        "art": args.art,
        "weg": args.weg,
        "bestanden": bestanden,
        "gesamt": gesamt,
        "voll": bestanden == gesamt and rot == 0,
        "hinweis": args.hinweis or "",
    }
    ERGEBNISSE.mkdir(parents=True, exist_ok=True)
    with JSONL.open("a", encoding="utf-8") as f:
        f.write(json.dumps(eintrag, ensure_ascii=False, sort_keys=True) + "\n")
    print(f"[kreuz] aufgenommen: {args.plattform} ({args.art}), {args.weg}, {bestanden}/{gesamt}")
    return uebersicht(args)


def architektur(plattform: str) -> str:
    return "x86_64" if "x86_64" in plattform else "aarch64" if "aarch64" in plattform else plattform


def uebersicht(_args: argparse.Namespace | None = None) -> int:
    eintraege = [json.loads(z) for z in JSONL.read_text(encoding="utf-8").splitlines() if z.strip()] if JSONL.is_file() else []
    zeilen = [
        "# Konformität über Hardware (erzeugt von `kreuzhardware.py`)",
        "",
        "Nicht von Hand ändern; Herkunft und Regeln im Kopf des Skripts.",
        "",
        "| Datum | Fassung | Vektorsatz | Plattform | Art | Rechenweg | Ergebnis |",
        "|---|---|---|---|---|---|---|",
    ]
    for e in eintraege:
        zeilen.append(
            f"| {e['datum']} | {e['fassung']} | `{e['vektorsatz'][:12]}` | {e['plattform']} | {e['art']} | "
            f"{e['weg']} | {e['bestanden']}/{e['gesamt']}{'' if e['voll'] else ' ⛔️'} |"
        )
    zeilen += ["", "## Bestätigt je Vektorsatz", ""]
    saetze: dict[str, list[dict]] = {}
    for e in eintraege:
        saetze.setdefault(e["vektorsatz"], []).append(e)
    for satz, liste in saetze.items():
        voll = [e for e in liste if e["voll"]]
        archs = {architektur(e["plattform"]) for e in voll}
        nur_emuliert = {
            a for a in archs if all(e["art"] == "emuliert" for e in voll if architektur(e["plattform"]) == a)
        }
        if len(archs) >= 2:
            zusatz = f" ({' und '.join(sorted(nur_emuliert))} nur emuliert)" if nur_emuliert else ""
            urteil = f"bestätigt über {' und '.join(sorted(archs))}{zusatz}"
        else:
            urteil = "noch nicht bestätigt (eine Architektur)"
        zeilen.append(f"- `{satz[:12]}`: {urteil}")
    UEBERSICHT.write_text("\n".join(zeilen) + "\n", encoding="utf-8")
    print(f"[kreuz] Übersicht: {len(eintraege)} Lauf/Läufe, {UEBERSICHT.name}")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    u = p.add_subparsers(dest="befehl", required=True)
    a = u.add_parser("aufnehmen")
    a.add_argument("ausgabe")
    a.add_argument("--plattform", required=True)
    a.add_argument("--art", choices=["nativ", "emuliert"], required=True)
    a.add_argument("--weg", required=True)
    a.add_argument("--hinweis")
    u.add_parser("uebersicht")
    args = p.parse_args()
    return aufnehmen(args) if args.befehl == "aufnehmen" else uebersicht(args)


if __name__ == "__main__":
    sys.exit(main())
