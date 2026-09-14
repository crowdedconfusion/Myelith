#!/usr/bin/env python3
"""Erzeugt eine Leiter von Referenzaufgaben, die kein Modell schon kann.

⚑ **Warum selbst erzeugt und nicht heruntergeladen.** Bei jedem
oeffentlichen Benchmark weiss niemand, wie viel davon im Vortraining
stand; ein Rueckgang der Perplexitaet kann dann Erinnerung sein statt
Lernen. Diese Aufgaben entstehen aus einem Startwert, es gibt sie
nirgends sonst, und frische Haltefolgen sind unbegrenzt verfuegbar.

⚑ **Warum eine Leiter und nicht eine Aufgabe.** Ein einzelnes Set
liefert ja oder nein. Eine Leiter sagt, **wo** die Faehigkeit endet:
Zeigt Stufe 1 nichts, liegt es am Optimierer oder am Massstab; zeigt
Stufe 1 etwas und Stufe 4 nichts, ist es eine Aussage ueber
Schwierigkeit. Das ist der Unterschied zwischen einem Befund und einer
Diagnose.

Jede Stufe liefert zwei Dateien mit **derselben Regel** und
**disjunkten** Beispielen:

  <name>_lern.txt    worauf trainiert wird
  <name>_halte.txt   frische Beispiele derselben Regel

⚑ **Die Haltemenge teilt die Regel, nicht den Inhalt.** Nur so trennt
die Messung Lernen von Auswendiglernen: Wer die Beispiele merkt, wird
auf der Haltemenge nicht besser.

⚑ **Und jede Messung braucht ihren Rauschnullpunkt** (Fund 196).
Reines Rauschen auf ein quantisiertes Modell senkt die Perplexitaet auf
gewoehnlichem Text um bis zu 0,67 Prozent, ganz ohne Gradient. Auf
diesen Aufgaben sollte es das nicht koennen, denn Dithering bringt ein
Modell naeher an sein Gleitkomma-Selbst, und das kann die Regeln hier
auch nicht. **Genau deshalb sind sie als Referenz geeignet.**

Aufruf:  python3 BENCHMARKS/Training/referenzleiter.py [--ziel VERZEICHNIS]
"""

from __future__ import annotations

import argparse
import pathlib
import random
import string

STARTWERT = 20260907

# 📌 **Die erste Fassung hatte zwei Silbenlisten zu je zehn Eintraegen,
# also hundert moegliche Woerter.** Damit blieb die Haltemenge der
# ersten beiden Stufen **leer**, und eine Referenzleiter ohne
# Haltemenge misst Erinnerung statt Lernen. Drei Listen ergeben 1000
# Staemme, mit dem Endbuchstaben 10 000 Woerter.
SILBEN_A = ["bar", "kel", "mor", "tan", "vus", "zir", "pol", "hen", "dral", "quin"]
SILBEN_B = ["an", "ex", "ol", "ur", "iss", "ath", "ent", "ork", "yl", "ume"]
SILBEN_C = ["bo", "fi", "gra", "ju", "lem", "nix", "pra", "sod", "tur", "wel"]

# Der letzte Buchstabe traegt in Stufe 2 die Regel und wird deshalb
# ausdruecklich angehaengt statt dem Zufall der Silbe ueberlassen.
ENDUNGEN = "NXLRSHTKME"


def kunstwort(r: random.Random, endung: str | None = None) -> str:
    stamm = (r.choice(SILBEN_A) + r.choice(SILBEN_B) + r.choice(SILBEN_C)).upper()
    return stamm + (endung if endung is not None else r.choice(ENDUNGEN))


def stufe1_kopie(r: random.Random, n: int) -> list:
    """Trivial: die Regel ist Wiederholung.

    ⚑ **Kein Strohmann, sondern die Untergrenze.** Zeigt selbst diese
    Stufe keinen Rueckgang, dann erreicht der Schritt das Modell nicht,
    und jede schwerere Aufgabe zu messen waere Zeitverschwendung.
    """
    return [f"{(w := kunstwort(r))} ist {w} ." for _ in range(n)]


def stufe2_regel(r: random.Random, n: int) -> list:
    """Eine Oberflaechenregel: der letzte Buchstabe bestimmt die Farbe.

    Generalisiert, weil die Haltemenge andere Woerter mit derselben
    Endung enthaelt.
    """
    farbe = dict(zip(ENDUNGEN, ["rot", "blau", "gruen", "gelb", "weiss",
                                "schwarz", "grau", "braun", "silber", "golden"]))
    aus = []
    for _ in range(n):
        e = r.choice(ENDUNGEN)
        aus.append(f"{kunstwort(r, e)} ist {farbe[e]} .")
    return aus


def stufe3_umkehr(r: random.Random, n: int) -> list:
    """Eine Abbildung ueber die Zeichenfolge: rueckwaerts."""
    aus = []
    for _ in range(n):
        w = "".join(r.choice(string.ascii_uppercase[:8]) for _ in range(5))
        aus.append(f"{w} rueckwaerts ist {w[::-1]} .")
    return aus


def stufe4_addition(r: random.Random, n: int, stellen: int) -> list:
    """Zweistellige und dreistellige Addition.

    ⚑ **Die schwerste Stufe, und die ehrlichste.** Sie braucht
    Uebertragslogik. Ein 0,5B kann sie nicht zuverlaessig, und dass sie
    aus wenigen tausend Token entsteht, ist nicht zu erwarten. Sie steht
    hier als **obere** Schranke der Leiter, nicht als Erwartung.
    """
    lo, hi = 10 ** (stellen - 1), 10 ** stellen - 1
    aus = []
    for _ in range(n):
        a, b = r.randint(lo, hi), r.randint(lo, hi)
        aus.append(f"{a} + {b} = {a + b} .")
    return aus


def schreibe(ziel: pathlib.Path, name: str, lern: list, halte: list) -> None:
    ueberschneidung = set(lern) & set(halte)
    assert not ueberschneidung, (
        f"{name}: {len(ueberschneidung)} Beispiele stehen in beiden Mengen. "
        "Eine Haltemenge, die Lernbeispiele enthaelt, misst Erinnerung."
    )
    (ziel / f"{name}_lern.txt").write_text("\n".join(lern) + "\n", encoding="utf-8")
    (ziel / f"{name}_halte.txt").write_text("\n".join(halte) + "\n", encoding="utf-8")
    zeichen = sum(len(z) for z in lern)
    print(f"  {name:<12} {len(lern):5d} Lernzeilen ({zeichen:6d} Zeichen), "
          f"{len(halte):4d} Haltezeilen")


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--ziel", default="BENCHMARKS/Training/datasets")
    p.add_argument("--lern", type=int, default=4000, help="Zeilen zum Trainieren")
    p.add_argument("--halte", type=int, default=800, help="frische Zeilen zum Messen")
    a = p.parse_args()

    ziel = pathlib.Path(a.ziel)
    ziel.mkdir(parents=True, exist_ok=True)
    print(f"[referenzleiter] Startwert {STARTWERT}, Ziel {ziel}")

    # ⚑ Getrennte Zufallsstroeme je Stufe: Wer eine Stufe umbaut,
    # veraendert die anderen nicht, und ein Vergleich ueber die Zeit
    # bleibt gueltig.
    for name, bau in [
        ("leiter1_kopie", lambda r, n: stufe1_kopie(r, n)),
        ("leiter2_regel", lambda r, n: stufe2_regel(r, n)),
        ("leiter3_umkehr", lambda r, n: stufe3_umkehr(r, n)),
        ("leiter4_add2", lambda r, n: stufe4_addition(r, n, 2)),
        ("leiter5_add3", lambda r, n: stufe4_addition(r, n, 3)),
    ]:
        r = random.Random(f"{STARTWERT}-{name}")
        # Mit Reserve erzeugen und entdoppeln, dann disjunkt schneiden.
        roh = list(dict.fromkeys(bau(r, (a.lern + a.halte) * 3)))
        if len(roh) < a.lern + a.halte:
            print(f"  {name:<12} nur {len(roh)} verschiedene Beispiele moeglich")
        schreibe(ziel, name, roh[: a.lern], roh[a.lern : a.lern + a.halte])

    print("[referenzleiter] fertig. Jede Messung braucht ihren Rauschnullpunkt.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
