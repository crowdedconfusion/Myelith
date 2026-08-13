#!/usr/bin/env python3
"""tau_sim.py — Trennschaerfe-Analyse fuer die Toleranzschwelle TAU
(Whitepaper v0.2, Kapitel 6.3/6.7, Meilenstein M0, Kapitel 10 Punkt 1/2).

WAS DIESE SIMULATION IST UND WAS NICHT
--------------------------------------
Sie ist KEINE Kalibrierung von TAU. Der reale Wert folgt aus Messungen echter
Hardware (M0). Sie ist ein Modell der Entscheidungsstatistik: Gegeben zwei
Verteilungen von Commitment-Abstaenden — ehrliche Hardware vs. Manipulation —
beantwortet sie:

  (1) Existiert ueberhaupt ein TAU-Korridor, der beide Fehlerarten klein haelt?
  (2) Wie gross muss die Trennung sein, damit das Design traegt? (Zielgroesse fuer M0)
  (3) Was kostet Pipeline-Akkumulation ueber k Shards?
  (4) Wie schnell entlarvt die Vorzeichenstatistik (Kap. 6.9) gerichtete Drift,
      die einzeln unterhalb von TAU bleibt?

Modellannahmen (explizit, damit widerlegbar):
  - Ehrliches Rechenrauschen: quadratisch akkumulierende Beitraege je Shard,
    d. h. der Gesamtabstand folgt einer Chi-Verteilung mit k Freiheitsgraden.
    Begruendung: Summe vieler kleiner, vorzeichenloser Rundungsdifferenzen.
  - Manipulation: Abstand skaliert mit der Eingriffstiefe, Skala s_att = SEP * s_hw.
    SEP ("separation") ist der zu messende Parameter aus M0.
  - Commitment-Abstaende ueber k Shards akkumulieren wie unabhaengige Zufallsgroessen
    (Wurzel-k-Gesetz), NICHT linear — Aktivierungen werden je Shard neu normalisiert.

Nur Standardbibliothek. Aufruf: python3 tau_sim.py
"""

import math
import random

# ─── Parameter ────────────────────────────────────────────────────────────────
S_HW = 1.0          # Skala des ehrlichen Rauschens (normiert; nur Verhaeltnisse zaehlen)
K_SHARDS = 8        # Shards pro Pod (Whitepaper-Standard)
P_SAMPLE = 0.02     # Stichprobenrate der Checker (Kap. 3.4 Stufe 2)
SEED = 20260803


def _lower_gamma_reg(s, x, terms=400):
    """Regularisierte untere unvollstaendige Gammafunktion P(s, x), Reihenentwicklung."""
    if x <= 0:
        return 0.0
    total, term = 1.0 / s, 1.0 / s
    for n in range(1, terms):
        term *= x / (s + n)
        total += term
        if term < total * 1e-15:
            break
    return total * math.exp(-x + s * math.log(x) - math.lgamma(s))


def cdf_accumulated(tau, scale, k):
    """P(Abstand <= tau) fuer die Summe von k unabhaengigen quadratischen Beitraegen.

    Der akkumulierte Abstand folgt einer Chi-Verteilung mit k Freiheitsgraden,
    skaliert mit `scale`. Analytisch statt Monte-Carlo: exakt und schnell.
    """
    if tau <= 0:
        return 0.0
    return _lower_gamma_reg(k / 2.0, (tau / scale) ** 2 / 2.0)


def error_rates(tau, sep, k, n=None, seed=None):
    """Falsch-Positiv (ehrlich > tau) und Falsch-Negativ (Angriff <= tau)."""
    fp = 1.0 - cdf_accumulated(tau, S_HW, k)
    fn = cdf_accumulated(tau, S_HW * sep, k)
    return fp, fn


def tau_corridor(sep, k, max_fp=1e-4, max_fn=0.01):
    """Bereich zulaessiger TAU: FP unter max_fp UND FN unter max_fn.

    max_fp streng: Falsch-Positive slashen ehrliche Miner — inakzeptabel.
    max_fn lockerer: Ein durchgerutschtes Segment traegt weiterhin das
    Stichproben- und Redundanzrisiko (Kap. 6.9), ist also nicht fatal.
    """
    lo = hi = None
    base = math.sqrt(k) * S_HW
    for i in range(1, 1201):
        tau = base * i / 100.0
        fp, fn = error_rates(tau, sep, k)
        ok = fp <= max_fp and fn <= max_fn
        if ok and lo is None:
            lo = tau
        if ok:
            hi = tau
    return lo, hi


def drift_detection(bias_fraction, n_audits, seed=SEED):
    """Kap. 6.9: Vorzeichenstatistik gegen gerichtete Drift.

    bias_fraction = Anteil der Segmente, die der Angreifer in dieselbe Richtung
    verzerrt (jeweils unterhalb TAU, also einzeln unauffaellig).
    Rueckgabe: p-Wert des Binomialtests auf Vorzeichensymmetrie.
    """
    rng = random.Random(seed)
    pos = 0
    for _ in range(n_audits):
        if rng.random() < bias_fraction:
            pos += 1                      # manipuliert: immer dieselbe Richtung
        elif rng.random() < 0.5:
            pos += 1                      # ehrlich: vorzeichenlos
    # Normalapproximation des Binomialtests, H0: p = 0.5
    mu, sigma = n_audits / 2, math.sqrt(n_audits) / 2
    z = (pos - mu) / sigma if sigma else 0.0
    p_value = 0.5 * math.erfc(z / math.sqrt(2))     # einseitig
    return z, p_value


def main():
    print("=" * 72)
    print("TAU-Trennschaerfe-Analyse  —  Myelith v0.2, Kap. 6 / M0")
    print("=" * 72)
    print(f"Modell: k={K_SHARDS} Shards, ehrliches Rauschen ~ HalfNormal(s_hw),")
    print(f"        Manipulation ~ HalfNormal(SEP * s_hw), Akkumulation ueber sqrt(k).\n")

    # ── 1. Welche Trennung SEP wird gebraucht? ───────────────────────────────
    print("[1] Existiert ein zulaessiger TAU-Korridor?")
    print("    Kriterium: Falsch-Positiv <= 1e-4 (ehrliche Miner nie slashen),")
    print("               Falsch-Negativ <= 1 % (Rest faengt Redundanz + Stichprobe).\n")
    print("    SEP   TAU-Korridor (in Einheiten von sqrt(k)*s_hw)      Bewertung")
    print("    " + "-" * 64)
    verdicts = {}
    for sep in (2, 3, 5, 8, 12, 20):
        lo, hi = tau_corridor(sep, K_SHARDS)
        base = math.sqrt(K_SHARDS) * S_HW
        if lo is None:
            verdicts[sep] = False
            print(f"    {sep:>3}x   —  kein zulaessiges TAU  —                    NICHT TRAGFAEHIG")
        else:
            verdicts[sep] = True
            breite = (hi - lo) / base
            print(f"    {sep:>3}x   [{lo/base:.2f} .. {hi/base:.2f}]  Breite {breite:.2f}"
                  f"{'':>10}{'komfortabel' if breite > 0.5 else 'knapp'}")
    min_sep = min([s for s, ok in verdicts.items() if ok], default=None)
    print(f"\n    ==> Mindest-Trennung fuer Tragfaehigkeit: SEP >= {min_sep}x")
    print(f"    ==> ZIELGROESSE FUER M0: Manipulationen muessen mindestens das")
    print(f"        {min_sep}-fache des Hardware-Rauschens erzeugen.\n")

    # ── 2. Einfluss der Pipeline-Laenge ──────────────────────────────────────
    print("[2] Einfluss der Shard-Anzahl k (Akkumulation des Rauschens)")
    print("      k    zulaessiges TAU-Fenster bei SEP=5x        Bewertung")
    print("    " + "-" * 60)
    for k in (4, 8, 16, 32):
        lo, hi = tau_corridor(5, k)
        base = math.sqrt(k) * S_HW
        if lo is None:
            print(f"    {k:>3}    kein Fenster                              KRITISCH")
        else:
            print(f"    {k:>3}    [{lo/base:.2f} .. {hi/base:.2f}] * sqrt(k)*s_hw"
                  f"{'':>8}{'ok' if (hi-lo)/base > 0.3 else 'eng'}")
    print("    ==> BEFUND: Groesseres k VERBESSERT die Trennschaerfe. Der relative")
    print("        Streuungsanteil der Chi-Verteilung faellt mit 1/sqrt(2k), das")
    print("        zulaessige TAU-Fenster waechst also mit der Pipeline-Laenge.")
    print("        Bei k=4 existiert bei SEP=5x KEIN zulaessiges TAU.")
    print("        Damit wirkt k in dieselbe Richtung wie die Kollusionsschranke")
    print("        (Kap. 4.1): klein-k ist nicht nur sicherheits-, sondern auch")
    print("        verifikationsseitig teuer.\n")

    # ── 3. Restschaden unterhalb TAU (Kap. 10 Punkt 2) ──────────────────────
    print("[3] Wie viel Manipulation passt unbemerkt unter TAU?")
    lo, _ = tau_corridor(5, K_SHARDS)
    base = math.sqrt(K_SHARDS) * S_HW
    print(f"    Bei TAU = {lo/base:.2f}*sqrt(k)*s_hw liegt die maximale unentdeckte")
    print(f"    Abweichung je Segment in derselben Groessenordnung wie das")
    print(f"    Hardware-Rauschen selbst (per Konstruktion von TAU).")
    print(f"    ==> Einzelschaden ist damit nach oben beschraenkt. Kritisch ist")
    print(f"        allein die WIEDERHOLUNG in dieselbe Richtung — siehe [4].\n")

    # ── 4. Vorzeichenstatistik gegen gerichtete Drift ───────────────────────
    print("[4] Entlarvung gerichteter Drift (Kap. 6.9)")
    print("    Angreifer verzerrt einen Anteil seiner Segmente stets gleichsinnig.")
    print("    Geprueft wird die Vorzeichensymmetrie der Checker-Stichproben.\n")
    print("    Drift-Anteil   Audits bis p < 0.001   entspricht Segmenten (p=2%)")
    print("    " + "-" * 62)
    for bias in (0.5, 0.2, 0.1, 0.05):
        need = None
        for n_aud in range(20, 20001, 20):
            z, pv = drift_detection(bias, n_aud)
            if pv < 0.001:
                need = n_aud
                break
        if need:
            print(f"    {bias:>10.0%}   {need:>18}   {int(need / P_SAMPLE):>22,}")
        else:
            print(f"    {bias:>10.0%}   {'> 20 000':>18}   {'—':>22}")
    print("\n    ==> Selbst schwache Drift (5 %) wird nach wenigen tausend Audits")
    print("        signifikant. Die adaptive Erhoehung der Stichprobenrate greift,")
    print("        lange bevor sich kleine Verzerrungen zu echtem Schaden summieren.\n")

    print("=" * 72)
    print("FAZIT (fuer Whitepaper Kap. 10, Punkt 1 und 2):")
    print(f"  - Das Verifikationsmodell ist tragfaehig, SOFERN Manipulationen")
    print(f"    mindestens das {min_sep}-fache Hardware-Rauschen erzeugen.")
    print( "  - Diese Trennung ist in M0 empirisch zu belegen. Sie ist die eine")
    print( "    Messgroesse, an der Kapitel 6 steht oder faellt.")
    print( "  - Pipeline-Laenge k wirkt GUENSTIG auf die Trennschaerfe: das")
    print( "    TAU-Fenster waechst mit k. Zusammen mit der Kollusionsschranke")
    print( "    beta^(2k) spricht das gegen kleine k — entgegen der Latenzintuition.")
    print( "  - Gerichtete Drift wird durch die Vorzeichenstatistik zuverlaessig")
    print( "    erfasst; sie schliesst die Restluecke unterhalb von TAU.")
    print("=" * 72)


if __name__ == "__main__":
    main()
