#!/usr/bin/env python3
"""Vergleich der Modelle an kurzen Aufgaben in vier Stufen, mit mehreren Saaten.

⚑ **Warum kurze Aufgaben statt des Loop-Szenarios** (Festlegung des
Projektinhabers, 2026-09-29): Ein langer Lauf scheitert an einer fruehen
Stelle und sagt dann nichts ueber den Rest. Vier Stufen (einfach, mittel,
komplex, overkill) zeigen, **wo** ein Modell abfaellt; mehrere Saaten zeigen,
wie weit es dabei streut. Jede Zelle ist Aufgabe mal Saat, und **alle Modelle
bekommen dieselben Saaten**, damit die Zellen vergleichbar sind.

⚑ **Alles, was die Messung bestimmt, steht im Aufruf und in `lauf.json`**:
Saaten, Antwortlaenge, Denkmodus, Werkzeugsatz. Die Einstellungen der
Maschine gelten nicht (`--ab-werk`); die des Projektinhabers schalten Denken
an und setzten Top-k auf 2.

    python3 -u BENCHMARKS/Agent/stufen_vergleich.py starten \\
        --modelle myelith-35b-a3b,myelith-30b-a3b --mitschriften <ordner>
    python3 BENCHMARKS/Agent/stufen_vergleich.py auswerten <ergebnisordner>

Der Bericht landet als `results/stufen-<datum>.md` neben dem Ordner mit den
Rohdaten je Modell (`results/stufen-<datum>/<modell>.json`).
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HIER = Path(__file__).resolve().parent
WURZEL = HIER.parents[1]
ARTEFAKTE = WURZEL / "INTEGER_LLM" / "artifacts"
STUFEN = ["einfach", "mittel", "komplex", "overkill"]

#: ⚑ **Das langsamste zuletzt**: Das ternaere 27B bereitet mit rund 12 Token/s
#: vor und haelt sonst alle anderen auf.
VORGABE_MODELLE = "myelith-35b-a3b,myelith-30b-a3b,myelith-8b,myelith-4b,myelith-27b-ternaer"


def saaten_ziehen(n: int) -> list[int]:
    """Neue Saaten unter 2^53, wie der Client sie zieht."""
    return [int.from_bytes(os.urandom(8), "little") & ((1 << 53) - 1) for _ in range(n)]


def starten(a: argparse.Namespace) -> int:
    datum = datetime.date.today().isoformat()
    ziel = a.ausgabe or HIER / "results" / f"stufen-{datum}"
    ziel.mkdir(parents=True, exist_ok=True)
    saaten = [int(s) for s in a.saaten.split(",") if s.strip()] if a.saaten else saaten_ziehen(a.anzahl)
    modelle = [m.strip() for m in a.modelle.split(",") if m.strip()]
    fehlend = [m for m in modelle if not (ARTEFAKTE / m).is_dir()]
    if fehlend:
        print(f"[stufen] Artefakte fehlen: {', '.join(fehlend)}")
        return 2
    lauf = {
        "begonnen": datetime.datetime.now().isoformat(timespec="seconds"),
        "modelle": modelle,
        "saaten": saaten,
        "token": a.token,
        "denken": a.denken,
        "werkzeuge": "voll",
        "einstellungen": "ab Werk",
        "satz": "stufen.json",
    }
    (ziel / "lauf.json").write_text(json.dumps(lauf, indent=2) + "\n", encoding="utf-8")
    print(f"[stufen] Ergebnisse: {ziel}")
    print(f"[stufen] Saaten: {', '.join(map(str, saaten))}")
    print(f"[stufen] Antwortlaenge {a.token}, Denken {a.denken}, Modelle: {', '.join(modelle)}", flush=True)
    for m in modelle:
        anfang = time.monotonic()
        print(f"[stufen] === {m} ===", flush=True)
        befehl = [
            sys.executable, "-u", str(HIER / "agentenprobe.py"), str(ARTEFAKTE / m),
            "--auftraege", str(HIER / "stufen.json"), "--werkzeuge", "voll",
            "--saaten", ",".join(map(str, saaten)), "--token", str(a.token), "--denken", a.denken,
            "--json", str(ziel / f"{m}.json"), "--ab-werk",
        ]
        if a.mitschriften:
            befehl += ["--mitschrift", str(a.mitschriften / m)]
        r = subprocess.run(befehl)
        print(f"[stufen] {m} fertig nach {(time.monotonic() - anfang) / 60:.1f} min (Rueckgabe {r.returncode})", flush=True)
        # Nach jedem Modell der Zwischenstand: Ein langer Vergleich soll nicht
        # erst am Ende etwas sagen.
        auswerten_ordner(ziel)
    print("[stufen] Fertig", flush=True)
    return 0


def auswerten_ordner(ziel: Path) -> Path:
    lauf = json.loads((ziel / "lauf.json").read_text(encoding="utf-8"))
    satz = {x["name"]: x for x in json.loads((HIER / "stufen.json").read_text(encoding="utf-8"))["auftrag"]}
    zeilen = [
        f"# Kurze Aufgaben in vier Stufen, {ziel.name.removeprefix('stufen-')}",
        "",
        f"Saaten: {', '.join(map(str, lauf['saaten']))}. Antwortlänge {lauf['token']}, Denken {lauf['denken']}, "
        f"Werkzeugsatz {lauf['werkzeuge']}. Je Stufe drei Aufgaben, je Aufgabe eine Zelle je Saat; "
        "bestanden heißt: Die Abnahme von außen endet mit 0.",
        "",
        "| Modell | " + " | ".join(STUFEN) + " | gesamt | Minuten |",
        "|---|" + "---|" * (len(STUFEN) + 2),
    ]
    einzeln = []
    for m in lauf["modelle"]:
        p = ziel / f"{m}.json"
        if not p.is_file():
            zeilen.append(f"| {m} | " + " | ".join("…" for _ in STUFEN) + " | läuft oder steht aus | |")
            continue
        laeufe = json.loads(p.read_text(encoding="utf-8"))["laeufe"]
        zellen = []
        for i, _ in enumerate(STUFEN, start=1):
            w = [x["bestanden"] for x in laeufe if x["stufe"] == i]
            zellen.append(f"{sum(w)}/{len(w)}" if w else "–")
        alle = [x["bestanden"] for x in laeufe]
        minuten = sum(x["sekunden"] for x in laeufe) / 60
        zeilen.append(f"| {m} | " + " | ".join(zellen) + f" | **{sum(alle)}/{len(alle)}** | {minuten:.0f} |")
        einzeln.append((m, laeufe))
    zeilen += ["", "## Je Aufgabe", ""]
    for m, laeufe in einzeln:
        zeilen += [f"### {m}", "", "| Aufgabe | Stufe | Saaten | Sekunden | Grund, falls nicht |", "|---|---|---|---|---|"]
        for name in satz:
            w = [x for x in laeufe if x["name"] == name]
            if not w:
                continue
            marken = " ".join("✓" if x["bestanden"] else "✗" for x in w)
            sek = " / ".join(f"{x['sekunden']:.0f}" for x in w)
            gruende = "; ".join(sorted({x["grund"][:120] for x in w if x["grund"]}))
            zeilen.append(f"| `{name}` | {satz[name].get('stufenname', '')} | {marken} | {sek} | {gruende} |")
        zeilen.append("")
    bericht = ziel.parent / f"{ziel.name}.md"
    bericht.write_text("\n".join(zeilen) + "\n", encoding="utf-8")
    return bericht


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    u = p.add_subparsers(dest="was", required=True)
    s = u.add_parser("starten")
    s.add_argument("--modelle", default=VORGABE_MODELLE)
    s.add_argument("--saaten", default="", help="feste Saaten durch Komma; sonst neu gezogen")
    s.add_argument("--anzahl", type=int, default=3, help="wie viele Saaten gezogen werden")
    s.add_argument("--token", type=int, default=1600)
    s.add_argument("--denken", choices=["an", "aus"], default="aus")
    s.add_argument("--ausgabe", type=Path)
    s.add_argument("--mitschriften", type=Path, help="Ordner fuer die ganze Ausgabe je Lauf (nicht ins Repositorium)")
    w = u.add_parser("auswerten")
    w.add_argument("ordner", type=Path)
    a = p.parse_args()
    if a.was == "starten":
        return starten(a)
    print(auswerten_ordner(a.ordner))
    return 0


if __name__ == "__main__":
    sys.exit(main())
