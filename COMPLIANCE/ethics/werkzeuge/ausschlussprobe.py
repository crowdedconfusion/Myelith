#!/usr/bin/env python3
"""Prüft den Ausschlusskatalog gegen die Anforderungen aus G1 und G9.

Der Katalog sagt, was das Netz nicht lernt und nicht bedient. Er ist die
einzige Quelle dafür; Antrag, Gateways und Client binden ihn ein, statt
den Text abzuschreiben.

## ⚑ Warum eine Klasse ohne Abgrenzung durchfällt

G1 verlangt objektive Entscheidbarkeit und begründet das scharf: „Wer
entscheidet, welcher Text schädlich ist, entscheidet auch, welcher Text
unbequem ist." Ein Stichwort wie „Waffen" verschluckt Geschichte,
Chemie, Metallurgie, Rüstungskontrolle und den halben Journalismus. Wer
es ohne Abgrenzung stehen lässt, hat keine Regel aufgeschrieben, sondern
einen Ermessensspielraum, und genau den soll es nicht geben.

**Deshalb ist `abgrenzung` Pflicht und nicht Beiwerk.** Der Maßstab ist
Befähigung, nicht Thema.

## Was dieses Skript nicht prüft

Ob die Klassen die richtigen sind. Das ist eine Festlegung des
Projektinhabers und keine Eigenschaft der Datei. Geprüft wird die Form,
die eine Anwendung ohne Ermessen überhaupt erst möglich macht.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

KATALOG = Path(__file__).resolve().parent.parent / "Ausschluss.json"

PFLICHTFELDER = ("kennung", "name", "beispiele", "abgrenzung")


def pruefe(d: dict) -> list[str]:
    m: list[str] = []

    for feld in ("fassung", "quelle", "klasse"):
        if not d.get(feld):
            m.append(f"{feld} fehlt")
    klassen = d.get("klasse") or []
    if not klassen:
        return m

    gesehen: set[str] = set()
    for i, k in enumerate(klassen):
        wo = k.get("kennung") or f"klasse[{i}]"
        for feld in PFLICHTFELDER:
            if not k.get(feld):
                m.append(f"{wo}.{feld} fehlt")
        # ⚑ Der Kern: eine Klasse ohne Abgrenzung ist keine Regel.
        if not k.get("abgrenzung"):
            m.append(
                f"{wo}: ohne Abgrenzung nicht objektiv entscheidbar (G1). "
                "Was faellt ausdruecklich NICHT darunter?"
            )
        if k.get("kennung") in gesehen:
            m.append(f"{wo}: Kennung doppelt vergeben")
        gesehen.add(k.get("kennung"))
        # Eine Klasse, die nirgends greift, ist ein Merkzettel.
        if not k.get("bei_aufnahme") and not k.get("bei_abfrage"):
            m.append(f"{wo}: greift weder bei Aufnahme noch bei Abfrage")

    return m


def main() -> int:
    p = Path(sys.argv[1]) if len(sys.argv) > 1 else KATALOG
    if not p.is_file():
        print(f"[ausschluss] FEHLGESCHLAGEN: {p} gibt es nicht")
        return 1
    d = json.loads(p.read_text(encoding="utf-8"))
    maengel = pruefe(d)
    n = len(d.get("klasse") or [])
    if maengel:
        print(f"[ausschluss] {len(maengel)} Beanstandung(en) in {n} Klassen")
        for x in maengel:
            print(f"[ausschluss]   {x}")
        print("[ausschluss] FEHLGESCHLAGEN")
        return 1
    print(f"[ausschluss] PASSED: {n} Klassen, jede mit Abgrenzung und Wirkung")
    print("[ausschluss] ⚑ Das ist keine Aussage darueber, ob es die richtigen sind.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
