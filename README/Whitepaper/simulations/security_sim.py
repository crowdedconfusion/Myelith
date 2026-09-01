#!/usr/bin/env python3
"""security_sim.py — Stichprobenrate p gegen Betrugsrate (Anhang B.1, Kap. 6.8).

WAS DIESE SIMULATION IST UND WAS NICHT
--------------------------------------
Sie ist KEINE Kalibrierung von p oder S. Beide sind
Governance-Parameter. Sie ist eine **Gegenprobe der Herleitung** in
Anhang B.1: Dort steht eine Schranke, die mit einer Naeherung hergeleitet
wurde, und diese Datei prueft, wo die Naeherung traegt und wo nicht.

Sie beantwortet vier Fragen:

  (1) Haelt `S_min = g/p^2` gegen einen Hit-and-Run-Miner, der ein
      Betrugsfenster ausnutzt und danach aussteigt?
  (2) Bis zu welcher **Kapazitaet** haelt sie? Die Herleitung benutzt
      `P_d ~= q*n*p`, und dieser Ausdruck wird groesser als 1, sobald
      `q*n > 1/p`. Genau dort liegt der Fall, den die schaerfere
      Schranke abdecken soll.
  (3) Was traegt Stufe 1 (Redundanzvergleich) wirklich bei? Anhang B.1
      laesst sie **bewusst weg** und nennt das konservativ. Diese Datei
      zeigt, ab welchem Angreiferanteil das nicht mehr Vorsicht ist,
      sondern die einzige richtige Rechnung.
  (4) Stimmt die multiplikative Gesamtrechnung aus Kap. 6.8 gegen eine
      Ziehung?

MODELLANNAHMEN (ausdrueklich, damit widerlegbar)
------------------------------------------------
  - Ein Segment wird von zwei disjunkten Pods gerechnet (Stufe 1),
    unabhaengig davon mit Wahrscheinlichkeit p von einem Checker
    nachgerechnet (Stufe 2) und mit Wahrscheinlichkeit gamma als
    Kontrollsegment eingeschleust (Stufe 3).
  - Ein Pod gilt als angreifend, wenn **alle** seine Shard-Positionen
    dem Angreifer gehoeren. Das ist dieselbe Definition wie im
    Kollusionstest von VERIFICATION.
  - Der Angreifer haelt einen Anteil alpha der Pods. Welche zwei Pods
    ein Paar bilden, kann er nicht waehlen: Die Paarung folgt dem Seed.
  - Slashing nimmt den ganzen Stake. Ein erwischter Miner verliert S,
    ein nicht erwischter behaelt seinen Gewinn.

Nur Standardbibliothek. Aufruf: python3 security_sim.py
"""

import math
import random

# ─── Parameter aus dem Papier ────────────────────────────────────────────────
P_SAMPLE = 0.02      # Stichprobenrate der Checker (Kap. 3.4, Stufe 2)
GAMMA = 0.01         # Anteil Kontrollsegmente (Kap. 6.7, Stufe 3)
G_SEGMENT = 0.5      # Gewinn je betrogenem Segment, in MYL (Anhang B.1)
SEED = 20260901
ZIEHUNGEN = 200_000


def s_min(g: float, p: float) -> float:
    """Die Schranke des Papiers: `S_min = g/p^2` (Anhang B.1)."""
    return g / (p * p)


def entdeckungswahrscheinlichkeit(n: int, p: float) -> float:
    """`P_d = 1 - (1-p)^n`, **exakt**, nicht genaehert.

    Anhang B.1 rechnet mit `P_d ~= n*p`. Das ist die Naeherung, deren
    Grenze diese Datei sucht.
    """
    return 1.0 - (1.0 - p) ** n


def haelt_die_schranke(n: int, g: float, p: float, s: float) -> bool:
    """Dominiert Ehrlichkeit, wenn auf `n` Segmenten betrogen wird?

    Erwarteter Gewinn `n*g` gegen erwartete Strafe `P_d(n) * s`.
    """
    return entdeckungswahrscheinlichkeit(n, p) * s > n * g


def groesste_tragende_kapazitaet(g: float, p: float, s: float, deckel: int = 10_000_000) -> int:
    """Das groesste `n`, bei dem die Schranke noch haelt.

    Monoton: `n*g` waechst linear, `P_d(n)*s` laeuft gegen `s`. Es gibt
    also genau einen Wechsel, und binaere Suche findet ihn.
    """
    if not haelt_die_schranke(1, g, p, s):
        return 0
    lo, hi = 1, deckel
    if haelt_die_schranke(hi, g, p, s):
        return hi
    while lo + 1 < hi:
        mitte = (lo + hi) // 2
        if haelt_die_schranke(mitte, g, p, s):
            lo = mitte
        else:
            hi = mitte
    return lo


def ueberlebenswahrscheinlichkeit(alpha: float, p: float, gamma: float) -> float:
    """Kap. 6.8: Ein falsches Segment ueberlebt alle drei Stufen.

    Stufe 1 faellt nur aus, wenn **beide** Pods des Paares dem Angreifer
    gehoeren; sonst widersprechen sie sich. Stufe 2 und 3 sind davon
    unabhaengig.
    """
    return alpha * alpha * (1.0 - p) * (1.0 - gamma)


def ziehe_ueberlebensrate(alpha: float, p: float, gamma: float, n: int, rng: random.Random) -> float:
    """Dieselbe Groesse durch Ziehen statt durch Formel."""
    ueberlebt = 0
    for _ in range(n):
        beide_boese = rng.random() < alpha and rng.random() < alpha
        if not beide_boese:
            continue
        if rng.random() < p:
            continue
        if rng.random() < gamma:
            continue
        ueberlebt += 1
    return ueberlebt / n


def main() -> int:
    rng = random.Random(SEED)
    p, g, gamma = P_SAMPLE, G_SEGMENT, GAMMA
    s = s_min(g, p)

    print("security_sim — Stichprobenrate gegen Betrugsrate (Anhang B.1, Kap. 6.8)")
    print(f"  p = {p}, gamma = {gamma}, g = {g} MYL")
    print(f"  S_min = g/p^2 = {s:.0f} MYL  ({s / g:.0f} Segment-Rewards)")
    print()

    # ── (1) Haelt die Schranke fuer ein einzelnes Segment? ───────────────────
    assert haelt_die_schranke(1, g, p, s), "S_min haelt nicht einmal fuer ein Segment"
    print("(1) Ein Segment: erwartete Strafe "
          f"{entdeckungswahrscheinlichkeit(1, p) * s:.1f} gegen Gewinn {g} MYL — haelt.")

    # ── (2) Das Hit-and-Run-Fenster, fuer das die Schranke gemacht ist ───────
    fenster = round(1 / p)
    gewinn = fenster * g
    strafe = entdeckungswahrscheinlichkeit(fenster, p) * s
    assert haelt_die_schranke(fenster, g, p, s), "S_min haelt nicht im Fenster 1/p"
    print(f"(2) Fenster 1/p = {fenster} Segmente: Gewinn {gewinn:.1f} gegen "
          f"erwartete Strafe {strafe:.1f} MYL — haelt mit Faktor {strafe / gewinn:.1f}.")

    # ── (3) Wo die Naeherung des Anhangs ihre Gueltigkeit verliert ───────────
    #
    # ⚑ Anhang B.1 schreibt `P_d ~= q*n*p`. Dieser Ausdruck wird groesser
    # als **1**, sobald `q*n > 1/p`, also genau im Hit-and-Run-Fall, den
    # die schaerfere Schranke abdecken soll. Die Schranke ist trotzdem
    # richtig — aber sie ist es aus einem anderen Grund als dem
    # angegebenen, und das gehoert gesagt.
    genaehert = fenster * p
    exakt = entdeckungswahrscheinlichkeit(fenster, p)
    assert genaehert > exakt, "die Naeherung muesste hier ueber dem exakten Wert liegen"
    print(f"(3) Naeherung P_d ~= n*p ergibt bei n = {fenster}: {genaehert:.3f}, "
          f"exakt sind es {exakt:.3f}.")
    ueber_eins = math.ceil(1 / p)
    assert ueber_eins * p >= 1.0, "n*p muesste hier mindestens 1 erreichen"
    print(f"    Ab n = {ueber_eins} ueberschreitet die Naeherung die 1 und ist "
          "keine Wahrscheinlichkeit mehr.")

    # ── (4) Bis zu welcher Kapazitaet traegt die Schranke? ───────────────────
    #
    # Exakt gerechnet verlangt Dominanz `P_d(n)*S > n*g`. Da `P_d` gegen 1
    # laeuft, wird die Bedingung fuer grosses n zu `S > n*g`: **Der Stake
    # muss den Gesamtgewinn uebersteigen**, und der waechst mit der
    # Kapazitaet. Die Schranke `g/p^2` ist konstant, traegt also nur bis
    # zu einer bestimmten Zahl von Segmenten je Ausstieg.
    kapazitaet = groesste_tragende_kapazitaet(g, p, s)
    assert kapazitaet > fenster, "die Schranke muss ueber das Fenster 1/p hinaus tragen"
    print(f"(4) Sie traegt bis {kapazitaet} betrogene Segmente je Ausstieg "
          f"({kapazitaet / fenster:.0f}-faches Fenster).")
    assert not haelt_die_schranke(kapazitaet + 1, g, p, s), \
        "die Suche haette die Grenze nicht gefunden"
    print(f"    Bei {kapazitaet + 1} Segmenten kippt sie: Gewinn "
          f"{(kapazitaet + 1) * g:.0f} gegen Strafe "
          f"{entdeckungswahrscheinlichkeit(kapazitaet + 1, p) * s:.0f} MYL.")
    print("    ⚑ Das ist kein Fehler der Schranke, sondern ihr Geltungsbereich:")
    print("      Wer in einer Epoche mehr Segmente betruegen kann als das, braucht")
    print("      Stake nach Kapazitaet und nicht nach Segment.")

    # ── (5) Was Stufe 1 beitraegt, und ab wann nichts mehr ───────────────────
    #
    # ⚑ Anhang B.1 laesst Stufe 1 weg und nennt das konservativ. Das ist
    # es, solange der Angreifer wenige Pods haelt: Dann faellt sein
    # Betrug fast immer schon am Redundanzvergleich auf. Es hoert auf,
    # Vorsicht zu sein, wenn er beide Seiten besetzen kann.
    print()
    print("(5) Beitrag von Stufe 1 (Redundanzvergleich) je Angreiferanteil:")
    vorher = None
    for alpha in (0.05, 0.10, 0.25, 0.50, 0.90):
        ohne_stufe1 = (1.0 - p) * (1.0 - gamma)
        mit_stufe1 = ueberlebenswahrscheinlichkeit(alpha, p, gamma)
        faktor = ohne_stufe1 / mit_stufe1
        assert mit_stufe1 <= ohne_stufe1, "Stufe 1 darf das Risiko nie erhoehen"
        if vorher is not None:
            assert faktor < vorher, "mehr Pods des Angreifers muessen den Beitrag senken"
        vorher = faktor
        print(f"    alpha = {alpha:4.2f}: Ueberlebensrate {mit_stufe1:.4f}, "
              f"Stufe 1 senkt sie um Faktor {faktor:6.1f}")
    print("    ⚑ Bei alpha = 0,90 bleibt fast nichts uebrig: Wer fast alle Pods haelt,")
    print("      vergleicht sich mit sich selbst. Die Annahme des Anhangs, Stufe 1")
    print("      wegzulassen, ist genau dort die richtige und nicht bloss die sichere.")

    # ── (6) Formel gegen Ziehung ─────────────────────────────────────────────
    print()
    alpha = 0.25
    formel = ueberlebenswahrscheinlichkeit(alpha, p, gamma)
    gezogen = ziehe_ueberlebensrate(alpha, p, gamma, ZIEHUNGEN, rng)
    abweichung = abs(gezogen - formel) / formel
    assert abweichung < 0.05, (
        f"Ziehung {gezogen:.5f} weicht um {abweichung:.1%} von der Formel "
        f"{formel:.5f} ab; Kap. 6.8 unterstellt Unabhaengigkeit"
    )
    print(f"(6) Kap. 6.8 gegen Ziehung bei alpha = {alpha}: Formel {formel:.5f}, "
          f"gezogen {gezogen:.5f} ({ZIEHUNGEN} Ziehungen, {abweichung:.1%} Abweichung).")
    print("    Die multiplikative Rechnung des Papiers haelt.")

    print()
    print("security_sim: alle Behauptungen bestanden.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
