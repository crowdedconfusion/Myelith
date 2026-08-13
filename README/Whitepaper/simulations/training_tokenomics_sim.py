#!/usr/bin/env python3
"""training_tokenomics_sim.py — Vertraegt sich Training mit dem Burn-and-Mint-Kreislauf?
(Prueft Kapitel 5 gegen Kapitel 7; Grundlage fuer eine moegliche Anpassung.)

AUSGANGSFRAGE
-------------
Kapitel 5 beschreibt einen geschlossenen Kreislauf: Nutzer verbrennen MYL,
Miner erhalten Praegung fuer verifizierte Inferenzarbeit. Training passt dort
nicht hinein: Es erzeugt keinen Burn, liefert aber Arbeit, die verguetet werden
muss. Geprueft wird, ob und wie das den Kreislauf stoert.

GEPRUEFT WIRD
-------------
  1. Verwaesserung: Wie stark verschiebt die Trainingsfinanzierung die
     Netto-Inflation gegenueber dem reinen Inferenzbetrieb?
  2. Treasury-Tragfaehigkeit: Reichen die 3 % aus Kap. 5.3 ueberhaupt?
  3. Fehlanreiz: Lohnt es sich, Kapazitaet von Inferenz auf Training zu
     verlagern, wenn Training nach Rechenzeit verguetet wird?
  4. Rueckkopplung: Verbessertes Modell erhoeht die Nachfrage und damit den
     Burn. Traegt sich Training langfristig selbst?
  5. Stake und Slashing: Gelten die Sicherheitsbedingungen aus Anhang B.1
     auch fuer Trainingssegmente?

Nur Standardbibliothek. Aufruf: python3 training_tokenomics_sim.py
"""

import math
import random

SEED = 20260804
EMA_WINDOW = 30
BURN_BASE = 10_000.0          # MYL je Epoche, aus tokenomics_sim.py


def run(epochs=2000, train_share=0.0, funding='treasury',
        quality_feedback=0.0, seed=SEED):
    """Simuliert Burn und Praegung ueber viele Epochen.

    train_share      : Anteil der Kapazitaet, der auf Training entfaellt
    funding          : 'treasury' | 'mint' | 'fee'
    quality_feedback : Wie stark verbessertes Modell die Nachfrage erhoeht
                       (0 = keine Rueckkopplung)
    """
    rng = random.Random(seed)
    ema_burn = BURN_BASE
    minted = burned = 0.0
    treasury = 0.0
    quality = 1.0
    for e in range(epochs):
        subsidy = 0.5 * 0.5 ** (e / 500)
        # Nachfrage waechst mit der Zeit und mit der Modellqualitaet
        organic = BURN_BASE * (1 + e / 1000) * quality * rng.lognormvariate(0, 0.25)
        if funding == 'fee':
            organic *= 1.05                       # 5 % Aufschlag auf Gebuehren
        burn = organic
        ema_burn += (burn - ema_burn) / EMA_WINDOW
        base_mint = ema_burn * (1 + subsidy)

        # Trainingsfinanzierung
        if funding == 'mint':
            train_cost = base_mint * train_share   # zusaetzliche Praegung
            mint = base_mint + train_cost
        elif funding == 'treasury':
            treasury += base_mint * 0.03           # 3 % lt. Kap. 5.3
            train_cost = min(treasury, base_mint * train_share)
            treasury -= train_cost
            mint = base_mint                       # keine Zusatzpraegung
        else:                                      # 'fee'
            train_cost = burn * 0.05 / 1.05        # aus dem Aufschlag
            mint = base_mint

        minted += mint
        burned += burn
        # Rueckkopplung: Training verbessert das Modell, das erhoeht die Nachfrage
        if train_cost > 0:
            quality *= (1 + quality_feedback / epochs)
    return dict(minted=minted, burned=burned,
                net_inflation=(minted - burned) / burned,
                treasury_end=treasury, quality=quality)


def main():
    print("=" * 76)
    print("TRAINING UND TOKENOMICS: VERTRAEGT SICH DAS?")
    print("=" * 76)

    # ── 1. Verwaesserung ────────────────────────────────────────────────────
    print("\n[1] Wirkung auf die Netto-Inflation")
    base = run(train_share=0.0)
    print(f"    {'Variante':<38}{'Netto-Inflation':>18}{'Delta':>12}")
    print("    " + "-" * 68)
    print(f"    {'Referenz: nur Inferenz':<38}{base['net_inflation']:>17.2%}{'-':>12}")
    for funding, label in [('treasury', 'Training aus Treasury (3 %)'),
                           ('mint', 'Training aus Zusatzpraegung (10 %)'),
                           ('fee', 'Training aus Gebuehrenaufschlag (5 %)')]:
        r = run(train_share=0.10, funding=funding)
        d = r['net_inflation'] - base['net_inflation']
        print(f"    {label:<38}{r['net_inflation']:>17.2%}{d:>+11.2%}")
    print("\n    ==> Nur die Zusatzpraegung verschiebt die Inflation spuerbar.")
    print("        Treasury und Gebuehrenaufschlag lassen den Kreislauf unberuehrt:")
    print("        Ersteres verteilt vorhandene Praegung um, Letzteres erhoeht den")
    print("        Burn im gleichen Mass wie die Ausgabe.")

    # ── 2. Treasury-Tragfaehigkeit ──────────────────────────────────────────
    print("\n[2] Reichen die 3 % Treasury?")
    print(f"    {'Trainingsanteil':>18}{'gedeckt aus Treasury':>24}{'Restbedarf':>14}")
    print("    " + "-" * 58)
    for share in (0.02, 0.03, 0.05, 0.10):
        covered = min(0.03, share)
        rest = max(0.0, share - 0.03)
        status = 'vollstaendig' if rest == 0 else f'{rest:.0%} offen'
        print(f"    {share:>17.0%}{covered:>23.0%}{status:>14}")
    print("\n    ==> Die Treasury deckt Trainingsanteile bis 3 % der Praegung.")
    print("        Die in Kap. 7.1 genannten 5-10 % beziehen sich auf die freie")
    print("        KAPAZITAET, nicht auf die Praegung: Bei einer Netzauslastung von")
    print("        70 % entsprechen 10 % freier Kapazitaet nur 3 % der Gesamtleistung.")
    print("        Die Groessen sind also vertraeglich, aber die Doppelbedeutung")
    print("        von 'Anteil' gehoert im Text klargestellt.")

    # ── 3. Fehlanreiz Kapazitaetsverlagerung ────────────────────────────────
    print("\n[3] Lohnt es sich, von Inferenz auf Training auszuweichen?")
    print("    Miner waehlen die hoehere Verguetung je Rechenstunde.\n")
    print(f"    {'Verguetungsverhaeltnis':>24}{'Verhalten der Miner':>34}")
    print("    " + "-" * 58)
    for ratio, note in [(0.5, 'Training halb so lukrativ'),
                        (1.0, 'gleich lukrativ'),
                        (1.5, 'Training lukrativer')]:
        if ratio < 0.9:
            behav = 'Training nur bei freier Kapazitaet ✅'
        elif ratio <= 1.1:
            behav = 'indifferent, Zuteilung entscheidet'
        else:
            behav = 'Inferenz wird verdraengt ⚠️'
        print(f"    {ratio:>23.1f}x{behav:>34}")
    print("\n    ==> Die Trainingsverguetung MUSS unter der Inferenzverguetung je")
    print("        Rechenstunde liegen, sonst verdraengt Training die Inferenz und")
    print("        damit die Einnahmequelle des Netzwerks. Empfehlung: hoechstens")
    print("        70 % der Inferenzverguetung, festgelegt per Governance.")

    # ── 4. Rueckkopplung ────────────────────────────────────────────────────
    print("\n[4] Traegt sich Training langfristig selbst?")
    print("    Besseres Modell -> mehr Nachfrage -> mehr Burn -> mehr Praegung.\n")
    print(f"    {'Qualitaetseffekt':>20}{'Burn nach 2000 Epochen':>26}{'ggue. Referenz':>18}")
    print("    " + "-" * 64)
    ref = run(train_share=0.10, funding='treasury', quality_feedback=0.0)
    for fb, label in [(0.0, 'keine Wirkung'), (0.15, 'schwach (+15 %)'),
                      (0.40, 'deutlich (+40 %)')]:
        r = run(train_share=0.10, funding='treasury', quality_feedback=fb)
        print(f"    {label:>20}{r['burned']:>25,.0f}{r['burned']/ref['burned']-1:>+17.1%}")
    print("\n    ==> Sobald Training die Modellqualitaet messbar verbessert, finanziert")
    print("        es sich ueber die gestiegene Nachfrage selbst. Der Kreislauf")
    print("        bleibt geschlossen, nur mit einer zusaetzlichen Rueckkopplung.")
    print("        Voraussetzung ist, dass die Qualitaetsverbesserung tatsaechlich")
    print("        eintritt: Sie ist die eigentliche Rechtfertigung der Ausgabe.")

    # ── 5. Stake und Slashing ───────────────────────────────────────────────
    print("\n[5] Gelten die Sicherheitsbedingungen auch fuer Training?")
    print("""    Anhang B.1 leitet S_min = g/p^2 aus dem Gewinn g je betrogenem
    Segment und der Stichprobenrate p her. Fuer Training gilt dieselbe
    Struktur, mit zwei Unterschieden:

      - g ist kleiner, da die Trainingsverguetung niedriger liegt (Punkt 3).
        Der erforderliche Stake sinkt entsprechend.
      - Der Schaden ist jedoch groesser: Ein durchgerutschtes Inferenz-Segment
        betrifft eine Antwort, ein durchgerutschter Gradient das Modell und
        damit alle kuenftigen Antworten.

    Daraus folgt: Fuer Trainingssegmente ist die Stichprobenrate p HOEHER
    anzusetzen als fuer Inferenz, nicht der Stake. Eine hoehere Prueframe
    kostet Kapazitaet, keine Kapitalbindung, und wirkt unmittelbar.""")

    print("\n" + "=" * 76)
    print("ERGEBNIS: DREI ANPASSUNGEN AN KAPITEL 5")
    print("""
  1. Die Trainingsverguetung je Rechenstunde muss unter der Inferenz-
     verguetung liegen (Empfehlung: hoechstens 70 %), sonst verdraengt
     Training die Einnahmequelle des Netzwerks.
  2. Finanzierung aus Treasury und optionalem Gebuehrenaufschlag, NICHT
     aus Zusatzpraegung: Nur so bleibt die Netto-Inflation unveraendert.
  3. Fuer Trainingssegmente gilt eine hoehere Stichprobenrate als fuer
     Inferenz, da der Schaden eines unentdeckten Fehlers groesser ist.

  Der Burn-and-Mint-Kreislauf selbst bleibt unveraendert gueltig. Training
  fuegt ihm eine Rueckkopplung hinzu: Bessere Modellqualitaet erhoeht die
  Nachfrage und damit den Burn, aus dem sich die Praegung speist.
""")
    print("=" * 76)


if __name__ == "__main__":
    main()
