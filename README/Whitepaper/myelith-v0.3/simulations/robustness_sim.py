#!/usr/bin/env python3
"""robustness_sim.py — Robustheit der TAU-Anforderung gegen verletzte Modellannahmen
(Whitepaper v0.2, Anhang B.5; Meilenstein M0, Kap. 10 Punkte 1 und 2).

ZWECK
-----
tau_sim.py berechnet die erforderliche Trennung SEP unter zwei Idealannahmen:
  (A1) Die Beitraege der k Shards akkumulieren UNABHAENGIG.
  (A2) Die Beitraege sind normalverteilt (Gesamtabstand: Chi-Verteilung).

Beide sind unbewiesen. Dieses Skript beantwortet daher nicht "stimmen sie?",
sondern die praktisch wichtigere Frage: "Wie sehr haengt das Ergebnis davon ab?"

Geprueft werden vier Abweichungen von der Idealwelt:
  1. Korrelierte Fehlerfortpflanzung   (A1 verletzt)
  2. Schwere Verteilungsraender        (A2 verletzt)
  3. Heterogene Hardware-Rauschpegel   (eine Skala s_hw fuer alle ist unrealistisch)
  4. Ausreisser-Knoten                 (einzelne Knoten deutlich verrauschter)

Methodik: Monte-Carlo, da Korrelation und schwere Raender analytisch unhandlich sind.
Nur Standardbibliothek. Aufruf: python3 robustness_sim.py
"""

import math
import random

K = 8                # Shards pro Pod
N = 60_000           # Monte-Carlo-Stichproben je Verteilung
MAX_FP = 1e-3        # Falsch-Positiv-Schranke (gelockert ggue. B.5, s. Hinweis unten)
MAX_FN = 0.01        # Falsch-Negativ-Schranke
SEED = 20260803

# Hinweis zur FP-Schranke: B.5 nutzt 1e-4. Bei N=60_000 Monte-Carlo-Stichproben
# ist 1e-4 nicht mehr zuverlaessig aufloesbar (6 erwartete Ereignisse). Wir
# arbeiten hier mit 1e-3 und vergleichen ausschliesslich RELATIV zwischen den
# Szenarien — die Aussage ist die Verschiebung, nicht der Absolutwert.


def draw_chain(rng, k, scale, rho, tail, hetero):
    """Ein akkumulierter Commitment-Abstand ueber k Shard-Uebergaenge.

    rho    : Korrelation aufeinanderfolgender Shard-Beitraege (AR(1)-Kopplung).
             rho=0 entspricht der Unabhaengigkeitsannahme A1.
    tail   : 'normal' | 'heavy'. 'heavy' nutzt Student-t (df=3) — Modell fuer
             Ausreisser-Dimensionen in Aktivierungen, die in der Praxis auftreten.
    hetero : Streuung der knotenindividuellen Rauschskala (0 = alle gleich).
    """
    # knotenindividuelle Skalen (Hardware-Heterogenitaet)
    scales = [scale * math.exp(rng.gauss(0, hetero)) for _ in range(k)]
    prev = 0.0
    total = 0.0
    for i in range(k):
        if tail == 'heavy':
            # Student-t(3) via Normal/sqrt(Chi2/df); auf Einheitsvarianz normiert
            z = rng.gauss(0, 1) / math.sqrt(sum(rng.gauss(0, 1) ** 2 for _ in range(3)) / 3)
            z /= math.sqrt(3.0)          # Var(t_3) = 3 -> normieren
        else:
            z = rng.gauss(0, 1)
        # AR(1): Beitrag haengt teilweise vom vorherigen ab (Fehlerfortpflanzung)
        cur = rho * prev + math.sqrt(1 - rho ** 2) * z
        prev = cur
        total += (scales[i] * cur) ** 2
    return math.sqrt(total)


def _quantile(sorted_vals, q):
    """Empirisches Quantil aus sortierter Liste."""
    if q <= 0:
        return sorted_vals[0]
    if q >= 1:
        return sorted_vals[-1]
    idx = min(len(sorted_vals) - 1, int(q * len(sorted_vals)))
    return sorted_vals[idx]


def _sample(scale, rho, tail, hetero, outlier, seed):
    """N Ziehungen des akkumulierten Abstands, sortiert."""
    rng = random.Random(seed)
    out = []
    for _ in range(N):
        s = scale
        if outlier and rng.random() < outlier:
            s = scale * 3.0               # Ausreisser-Knoten: 3x verrauschter
        out.append(draw_chain(rng, K, s, rho, tail, hetero))
    out.sort()
    return out


def required_sep(rho=0.0, tail='normal', hetero=0.0, outlier=0.0):
    """Kleinstes SEP, fuer das ein zulaessiges TAU existiert.

    Ein zulaessiges TAU existiert genau dann, wenn das (1-MAX_FP)-Quantil der
    ehrlichen Verteilung unter dem MAX_FN-Quantil der Angriffsverteilung liegt:
    dann laesst sich TAU dazwischen legen und erfuellt beide Schranken.
    """
    honest = _sample(1.0, rho, tail, hetero, outlier, SEED)
    tau_min = _quantile(honest, 1 - MAX_FP)          # darueber: zu viele Falsch-Positive
    for sep in (2, 3, 5, 8, 12, 20, 35, 60, 100):
        attack = _sample(sep, rho, tail, hetero, 0.0, SEED + 1)
        tau_max = _quantile(attack, MAX_FN)          # darunter: zu viele Falsch-Negative
        if tau_min <= tau_max:
            return sep
    return None


def main():
    print("=" * 74)
    print("ROBUSTHEIT DER TAU-ANFORDERUNG  —  Myelith v0.2, Anhang B.5 / M0")
    print("=" * 74)
    print(f"Referenz (Idealannahmen A1+A2): unabhaengig, normalverteilt, homogen\n")

    base = required_sep()
    print(f"  [Referenz]  erforderliche Trennung SEP >= {base}x\n")

    print("[1] Korrelierte Fehlerfortpflanzung (Annahme A1 verletzt)")
    print("    rho = Kopplung aufeinanderfolgender Shard-Beitraege")
    print("    rho     erforderliches SEP     Verschiebung")
    print("    " + "-" * 50)
    for rho in (0.0, 0.3, 0.6, 0.9):
        s = required_sep(rho=rho)
        shift = "Referenz" if rho == 0 else (f"{s/base:.1f}x strenger" if s and base and s > base else "unveraendert")
        print(f"    {rho:>4.1f}    {str(s)+'x':>16}     {shift}")
    print()

    print("[2] Schwere Verteilungsraender (Annahme A2 verletzt)")
    print("    Student-t(3) statt Normal — Modell fuer Aktivierungs-Ausreisser")
    for tail in ('normal', 'heavy'):
        s = required_sep(tail=tail)
        print(f"    {tail:>8}: SEP >= {s}x")
    print()

    print("[3] Heterogene Hardware (unterschiedliche Rauschpegel je Knoten)")
    print("    sigma = Streuung der log-Rauschskala ueber Knoten")
    for h in (0.0, 0.3, 0.6):
        s = required_sep(hetero=h)
        print(f"    sigma={h:>3.1f}: SEP >= {s}x")
    print()

    print("[4] Ausreisser-Knoten (Anteil mit 3-fachem Rauschen)")
    for o in (0.0, 0.02, 0.10):
        s = required_sep(outlier=o)
        print(f"    Anteil={o:>4.0%}: SEP >= {s}x")
    print()

    print("[5] Kombinierter Ungunstfall (alle Abweichungen gleichzeitig)")
    worst = required_sep(rho=0.6, tail='heavy', hetero=0.3, outlier=0.02)
    print(f"    rho=0.6, heavy tails, hetero=0.3, 2 % Ausreisser: SEP >= {worst}x")
    print()
    print("=" * 74)
    print("LESART DES ERGEBNISSES")
    print("  Entscheidend ist nicht der Absolutwert, sondern der FAKTOR zwischen")
    print("  Referenz und Ungunstfall. Er sagt, wie viel Sicherheitsreserve die")
    print("  in M0 gemessene Trennung ueber die 5x hinaus haben muss, damit das")
    print("  Modell auch bei verletzten Annahmen traegt.")
    print("=" * 74)


if __name__ == "__main__":
    main()
