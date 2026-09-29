#!/usr/bin/env python3
"""Prueft die Abnahmen von `mehrstufig.json` in beide Richtungen.

Jede Abnahme muss am Ausgangsstand scheitern und an der Musterloesung
bestehen. Eine Abnahme, die beides nicht trennt, misst nichts.

    python3 BENCHMARKS/Agent/mehrstufig_pruefen.py
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HIER = Path(__file__).resolve().parent


def abnahme(ordner: Path, befehl: str) -> bool:
    return subprocess.run(["sh", "-c", befehl], cwd=ordner, capture_output=True).returncode == 0


def main() -> int:
    auftraege = json.loads((HIER / "mehrstufig.json").read_text(encoding="utf-8"))["auftrag"]
    fehler = 0
    for a in auftraege:
        with tempfile.TemporaryDirectory() as t:
            ordner = Path(t)
            for name, inhalt in a.get("dateien", {}).items():
                (ordner / name).parent.mkdir(parents=True, exist_ok=True)
                (ordner / name).write_text(inhalt, encoding="utf-8")
            vorher = abnahme(ordner, a["abnahme"])
            for name, inhalt in a["_muster"].items():
                (ordner / name).parent.mkdir(parents=True, exist_ok=True)
                (ordner / name).write_text(inhalt, encoding="utf-8")
            nachher = abnahme(ordner, a["abnahme"])
        gut = (not vorher) and nachher
        fehler += not gut
        print(f"{'ok  ' if gut else 'FEHL'} {a['name']:28} Ausgangsstand {'besteht' if vorher else 'scheitert'}, Musterloesung {'besteht' if nachher else 'scheitert'}")
    print("Fertig" if not fehler else f"FEHLER: {fehler} Abnahmen trennen nicht")
    return 1 if fehler else 0


if __name__ == "__main__":
    sys.exit(main())
