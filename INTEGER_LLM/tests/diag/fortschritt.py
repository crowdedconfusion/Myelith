#!/usr/bin/env python3
"""Fortschrittsbalken und Laufzeitschätzung für lange Diagnoseläufe.

Bewusst ohne Fremdbibliothek: Die Diagnosen laufen teils in der
Kalibrier-Umgebung, teils im System-Python, und eine zusätzliche
Abhängigkeit nur für einen Balken wäre der falsche Tausch.

**Wichtig:** Der Balken erscheint nur dann laufend, wenn die Ausgabe
ungepuffert ist. Bei Umleitung in eine Datei puffert Python blockweise —
deshalb `python -u` verwenden. Das ist hier schon einmal aufgefallen
(2026-08-20): Ein 25-Minuten-Lauf zeigte bis zum Ende keine Zeile, und
es war nicht erkennbar, ob er arbeitete oder hing.

Benutzung:

    from fortschritt import Fortschritt
    with Fortschritt(len(sequenzen), "Perplexität") as f:
        for s in sequenzen:
            ...
            f.schritt()
"""
import sys
import time


def zeit(sekunden: float) -> str:
    if sekunden < 90:
        return f"{sekunden:.0f}s"
    if sekunden < 5400:
        return f"{sekunden / 60:.1f}min"
    return f"{sekunden / 3600:.1f}h"


class Fortschritt:
    """Balken mit laufender Restzeitschätzung aus der gemessenen Rate."""

    def __init__(self, gesamt: int, name: str = "", breite: int = 32,
                 strom=sys.stderr):
        self.gesamt = max(gesamt, 1)
        self.name = name
        self.breite = breite
        self.strom = strom
        self.erledigt = 0
        self.start = time.time()

    def __enter__(self):
        self._zeichne()
        return self

    def schritt(self, n: int = 1):
        self.erledigt = min(self.erledigt + n, self.gesamt)
        self._zeichne()

    def _zeichne(self):
        anteil = self.erledigt / self.gesamt
        voll = int(self.breite * anteil)
        balken = "█" * voll + "·" * (self.breite - voll)
        vergangen = time.time() - self.start
        if self.erledigt > 0:
            rest = vergangen / self.erledigt * (self.gesamt - self.erledigt)
            schaetzung = f"noch {zeit(rest)}"
        else:
            schaetzung = "schätze …"
        self.strom.write(
            f"\r{self.name} [{balken}] {self.erledigt}/{self.gesamt}  "
            f"{100 * anteil:5.1f}%  {zeit(vergangen)} vergangen, {schaetzung}   "
        )
        self.strom.flush()

    def __exit__(self, *exc):
        vergangen = time.time() - self.start
        self.strom.write(f"\r{' ' * (self.breite + 70)}\r")
        self.strom.write(f"{self.name}: {self.gesamt} Einheiten in {zeit(vergangen)}\n")
        self.strom.flush()
        return False


def schaetze_inferenz(tokens: int, modell: str = "0.5b") -> str:
    """Grobe Vorabschätzung für einen Inferenzlauf.

    Raten aus `bench/README.md` (arm64, cpu-simd). Bewusst konservativ:
    Eine zu optimistische Schätzung ist schlimmer als gar keine.
    """
    rate = 2.0 if "7" in modell else 24.0
    laden = 60.0 if "7" in modell else 10.0
    return zeit(tokens / rate + laden)


if __name__ == "__main__":
    with Fortschritt(20, "Selbsttest") as f:
        for _ in range(20):
            time.sleep(0.05)
            f.schritt()
    print("7B, 4 Sequenzen à 128 Token:", schaetze_inferenz(512, "7b"))
    print("0,5B, 16 Sequenzen à 128 Token:", schaetze_inferenz(2048, "0.5b"))
