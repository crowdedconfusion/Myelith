#!/usr/bin/env python3
"""
Funktionserhaltende Expansion, ganzzahlig und exakt (Kap. 7.5).

## Die Frage

Ein Modell soll wachsen, ohne dabei schlechter zu werden. Die Literatur
(Net2Net, bert2BERT) verdoppelt dafuer eine Einheit und halbiert ihre
ausgehenden Gewichte. In Gleitkomma ist das nur NAEHERUNGSWEISE
funktionserhaltend, und die beiden Kopien bekommen danach identische
Gradienten: Sie bleiben fuer immer gleich, die neue Kapazitaet ist tot.
Die Literatur behilft sich mit kuenstlichem Rauschen, das nicht
deterministisch ist und deshalb hier nicht in Frage kommt.

## Was diese Probe zeigt

Ganzzahlig geht beides besser.

**Funktionserhaltung durch AUFTEILUNG statt Halbierung:**

    a = m // 2      b = m - a       a + b = m

gilt fuer jede ganze Zahl, gerade wie ungerade. Es wird nichts gerundet,
also entsteht kein Fehler. Die Ausgabe nach der Expansion ist BITGLEICH
zur Ausgabe davor, und damit ist das Akzeptanzkriterium aus Phase 4 ein
Digestvergleich statt eines Toleranzvergleichs.

**Symmetriebrechung zweifach, davon einmal ohne Zufall:** Die Aufteilung
trennt `a` und `b` bei jedem ungeraden Eintrag um 1 LSB. Zusaetzlich
trennt das stochastische Runden die eingehenden Zeilen, weil der Wuerfel
am Index haengt und zwei Kopien verschiedene Indizes haben.

## Warum ein kleines Netz und kein Sprachmodell

Die Frage ist strukturell, nicht modellspezifisch: Es geht um die
Arithmetik der Expansion, nicht um die Qualitaet eines bestimmten
Modells. An einem kleinen Netz ist sie sauber stellbar und in Sekunden
beantwortet.

**Ein erster Versuch war falsch** und ist hier festgehalten, weil er die
Art Fehler zeigt, gegen die diese Probe gebaut ist: Er halbierte die
ausgehende Spalte ueber die SKALA statt ueber die Werte und lag um
1,24e-03 daneben. Ein zweiter verdoppelte nur die eingehende Zeile und
liess die ausgehende unveraendert; dann bekommen die Einheiten von
vornherein verschiedene Gradienten und laufen auseinander, ganz gleich
wie gerundet wird. Beide Male haette die Probe etwas bestaetigt, das sie
nicht gemessen hat.

Gleitkomma erlaubt: Referenzmessung, nicht Inferenzpfad.
Kein Teil des Auslieferungspfads.

Usage:
    cd INTEGER_LLM/calibrate
    .venv/bin/python ../../TRAINING/tests/diag/expansion_simulation.py
"""
import sys
from pathlib import Path

WURZEL = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

import torch  # noqa: E402
import integer_master_simulation as im  # noqa: E402

F = im.ZUSATZBITS
INT8_MAX = 127


def w8(master, shift, ebene, schritt, stochastisch, keim=1):
    """Das wirksame Gewicht, wie im Trainingsschritt aus 0.2."""
    roh = master / (1 << F)
    if stochastisch:
        idx = torch.arange(master.numel(), dtype=torch.int64).reshape(master.shape)
        wu = im.wuerfel(idx, ebene, schritt, keim)
        q = torch.floor(roh) + (wu < (roh - torch.floor(roh))).to(roh.dtype)
    else:
        q = torch.round(roh)
    return torch.clamp(q, -INT8_MAX - 1, INT8_MAX) / torch.pow(2.0, shift)


def aufteilen(spalte: torch.Tensor):
    """Die ganzzahlige Aufteilung: a + b = m, ohne Rundung.

    Das Herzstueck. Eine Halbierung waere bei ungeraden Werten ungenau
    und muesste runden; eine Aufteilung ist exakt, und der Rest von einem
    LSB landet in einer der beiden Haelften statt im Fehler.
    """
    a = torch.floor(spalte / 2)
    return a, spalte - a


def expandieren(m1, s1, m2, s2, j: int):
    """Verdoppelt Einheit `j`: Zeile kopieren, Spalte aufteilen."""
    m1e = torch.cat([m1, m1[j : j + 1]], dim=0)
    s1e = torch.cat([s1, s1[j : j + 1]], dim=0)
    a, b = aufteilen(m2[:, j])
    m2e = torch.cat([m2, b.unsqueeze(1)], dim=1)
    m2e[:, j] = a
    return m1e, s1e, m2e, s2.clone()


def main():
    torch.manual_seed(0)
    d, h = 16, 8
    m1 = torch.round(torch.randn(h, d) * 40 * (1 << F) / 128)
    m2 = torch.round(torch.randn(d, h) * 40 * (1 << F) / 128)
    s1 = torch.full((h, 1), 5.0)   # Zweierpotenz-Skalen, eingefroren
    s2 = torch.full((d, 1), 5.0)
    x = torch.randn(4, d)

    def vorwaerts(m1, s1, m2, s2, schritt, stoch):
        a = torch.nn.functional.silu(x @ w8(m1, s1, 0, schritt, stoch).t())
        return a @ w8(m2, s2, 1, schritt, stoch).t()

    j = 0
    m1e, s1e, m2e, s2e = expandieren(m1, s1, m2, s2, j)

    print("Expansion, ganzzahlig")
    print(f"  Breite {h} -> {m1e.shape[0]}, Einheit {j} verdoppelt\n")

    a, b = aufteilen(m2[:, j])
    exakt = bool(torch.equal(a + b, m2[:, j]))
    print(f"1. Aufteilung a + b = m exakt fuer alle Eintraege: {exakt}")
    assert exakt

    # Ohne Zufall, weil hier die FUNKTION geprueft wird und nicht die
    # Rundung: Zwei Laeufe mit verschiedenen Wuerfeln waeren nicht
    # vergleichbar, und die Frage waere eine andere.
    vor = vorwaerts(m1, s1, m2, s2, 0, False)
    nach = vorwaerts(m1e, s1e, m2e, s2e, 0, False)
    bitgleich = bool(torch.equal(vor, nach))
    abweichung = float((vor - nach).abs().max())
    print(f"2. Ausgabe vor und nach der Expansion bitgleich: {bitgleich}"
          f"   (max. Abweichung {abweichung:.2e})")
    assert bitgleich

    print("\n3. Symmetrie der beiden Kopien ueber 20 Schritte")
    for stoch, name in [(False, "Rundung zur naechsten Stufe"),
                        (True, "stochastisch (Zaehlerwuerfel)")]:
        gleich = sum(
            bool(torch.equal(w8(m1e, s1e, 0, s, stoch)[j],
                             w8(m1e, s1e, 0, s, stoch)[h]))
            for s in range(20)
        )
        urteil = "NICHT gebrochen" if gleich == 20 else "gebrochen"
        print(f"   eingehende Zeilen, {name:30} {gleich:2d}/20 identisch  -> {urteil}")

    unterschied = int((m2e[:, j] != m2e[:, h]).sum().item())
    print(f"   ausgehende Spalten, ganzzahlige Aufteilung      "
          f"{unterschied:2d}/{d} verschieden  -> gebrochen, ohne Zufall")

    print("\n4. Reproduzierbarkeit des Wuerfels")
    p = im.wuerfel(torch.arange(64, dtype=torch.int64), 0, 5, 1)
    q = im.wuerfel(torch.arange(64, dtype=torch.int64), 0, 5, 1)
    print(f"   zweimal derselbe Aufruf identisch: {bool(torch.equal(p, q))}")

    print("\nAlle Zusicherungen gehalten.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
