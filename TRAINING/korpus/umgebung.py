#!/usr/bin/env python3
"""Was die Buchkette an Umgebung braucht, an einer Stelle.

# ⚑ Die Kette braucht nichts zwingend, und das ist eine Entscheidung

`buch_zu_md.py`, `buchkorpus.py` und `md_zu_mappe.py` kommen mit der
**Standardbibliothek** aus. Zwei Dinge machen sie besser, und beide sind
freiwillig:

| Baustein | Wofür | Ohne ihn |
|---|---|---|
| `pypdf` (oder `pdftotext`) | PDF auslesen | PDF wird **abgelehnt**, mit Ansage |
| `tokenizers` | Token **zählen** statt schätzen | Es wird geschätzt, und der Bericht sagt es |

⚑ **Deshalb läuft ein frischer Klon sofort**, für EPUB, HTML und Text
ohne jede Einrichtung. Wer PDF braucht, ruft `einrichten.sh`; das Rad
dafür liegt im Repositorium, also **ohne Netz**.

# ⛔️ Warum hier kein Python mitgeliefert wird

Ein Interpreter im Repositorium wäre je Plattform ein eigenes Paket von
zig Megabyte, das niemand prüft und das bei jeder Sicherheitslücke
nachgezogen werden müsste. **Ein Repositorium, das eine Laufzeit
mitliefert, liefert auch deren Lücken mit.**

⚑ **Stattdessen liegt die Anforderung so tief, dass sie überall erfüllt
ist:** Python **3.9**, die Fassung, die macOS selbst mitbringt und die
in jeder gepflegten Linux-Verteilung seit Jahren steht. Und sie wird
**geprüft**, statt an einer unverständlichen Fehlermeldung aufzufallen.
"""

from __future__ import annotations

import sys

# ⚑ **Die Untergrenze ist gemessen und nicht geschätzt**: Sie kommt vom
# jüngsten benutzten Sprachmittel, der Vereinigung zweier Wörterbücher
# mit `|` (Python 3.9). Wer sie senken will, muss diese Stelle ändern.
MINDESTENS = (3, 9)


def pruefen() -> None:
    """Hält an, wenn der Interpreter zu alt ist, und sagt warum."""
    if sys.version_info < MINDESTENS:
        gefordert = ".".join(str(z) for z in MINDESTENS)
        haben = ".".join(str(z) for z in sys.version_info[:3])
        raise SystemExit(
            f"[buchkette] Python {gefordert} oder neuer wird gebraucht, hier laeuft "
            f"{haben} ({sys.executable}).\n"
            "Auf macOS bringt das System eine passende Fassung mit; sonst hilft die "
            "Paketverwaltung der Verteilung."
        )


def pdf_weg() -> str:
    """Womit sich hier ein PDF auslesen liesse, oder 'nichts'."""
    import shutil

    if shutil.which("pdftotext"):
        return "pdftotext"
    try:
        import pypdf  # noqa: F401

        return "pypdf"
    except ImportError:
        return "nichts"


def zaehlweg() -> str:
    """Ob Token gezaehlt oder geschaetzt werden."""
    try:
        import tokenizers  # noqa: F401

        return "tokenizers"
    except ImportError:
        return "Schaetzung"


def bericht() -> str:
    """Eine Zeile, die sagt, was hier geht."""
    return (
        f"Python {'.'.join(str(z) for z in sys.version_info[:3])}, "
        f"PDF ueber {pdf_weg()}, Token ueber {zaehlweg()}"
    )


if __name__ == "__main__":
    pruefen()
    print(f"[buchkette] {bericht()}")
