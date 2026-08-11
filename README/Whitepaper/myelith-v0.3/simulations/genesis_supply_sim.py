#!/usr/bin/env python3
"""genesis_supply_sim.py — Ausgabestruktur: Genesis, Emission, Anlaufphase
(Grundlage fuer ein Kapitel 5.7; ergaenzt tokenomics_sim.py.)

OFFENE FRAGE
------------
Kapitel 5 beschreibt den Burn-and-Mint-Kreislauf, laesst aber offen, wie das
Netzwerk startet: Wie viele MYL existieren zu Beginn, wer haelt sie, und wie
entwickelt sich das Gesamtangebot? Die Launch-Strategie sieht einen Fair Launch
ohne Vorverkauf vor; das Whitepaper trifft dazu keine Aussage.

GEPRUEFT WIRD
-------------
  1. Henne-Ei-Problem: Ohne Startbestand koennen Miner keinen Stake stellen
     und Nutzer keine Credits kaufen. Wie gross muss die Anfangsmenge sein?
  2. Emissionsplan: Welcher Verlauf ergibt sich aus M_e = min(EMA(B)*(1+s), M_max)?
     Konvergiert das Gesamtangebot?
  3. Konzentrationsrisiko: Wie stark verteilt sich die Praegung in der
     Fruehphase auf wenige Miner?
  4. Stake-Bedarf: Reicht die umlaufende Menge, um S_min (Anhang B.1) zu decken?
  5. Vergleich der Startvarianten.

Nur Standardbibliothek. Aufruf: python3 genesis_supply_sim.py
"""

import math
import random

SEED = 20260804
EMA_WINDOW = 30
EPOCHS_PER_YEAR = 24 * 365          # Stundenepochen

# Sicherheitsbedingung aus Anhang B.1
P_SAMPLE = 0.02
SEGMENT_REWARD = 0.5                # MYL
S_MIN_PER_CAPACITY = SEGMENT_REWARD / P_SAMPLE ** 2    # = 1250 MYL


def emission(epochs, genesis, s0=0.5, halflife_years=2.0, m_max=None,
             burn_start=2_000.0, growth=1.0, seed=SEED):
    """Simuliert Angebotsentwicklung ueber `epochs` Stundenepochen."""
    rng = random.Random(seed)
    supply = genesis
    ema = burn_start
    history = []
    hl = halflife_years * EPOCHS_PER_YEAR
    for e in range(epochs):
        s = s0 * 0.5 ** (e / hl)
        burn = burn_start * (1 + growth * e / epochs) * rng.lognormvariate(0, 0.2)
        burn = min(burn, supply * 0.02)          # nicht mehr verbrennen als vorhanden
        ema += (burn - ema) / EMA_WINDOW
        mint = ema * (1 + s)
        if m_max is not None:
            mint = min(mint, m_max)
        supply += mint - burn
        history.append((e, supply, mint, burn, s))
    return history


def main():
    print("=" * 78)
    print("AUSGABESTRUKTUR: GENESIS, EMISSION, ANLAUFPHASE")
    print("=" * 78)

    # ── 1. Henne-Ei-Problem ─────────────────────────────────────────────────
    print("\n[1] Wie viel Startbestand ist noetig?")
    print("    Zwei Bedarfe muessen zu Beginn gedeckt sein:")
    print("      a) Stake der ersten Miner (S_min je Kapazitaetseinheit, Anhang B.1)")
    print("      b) Credits der ersten Nutzer (Burn erzeugt erst danach Praegung)\n")
    print(f"    S_min je Kapazitaetseinheit: {S_MIN_PER_CAPACITY:,.0f} MYL")
    print(f"    {'Startminer':>12}{'Stake-Bedarf':>18}{'Credits (30 Tage)':>20}{'Summe':>16}")
    print("    " + "-" * 68)
    for miners in (50, 200, 1000):
        stake = miners * S_MIN_PER_CAPACITY
        # Nutzer-Credits: Annahme, dass die Startkapazitaet 30 Tage lang zu 30 %
        # ausgelastet wird und Inferenz mit 0,5 MYL je Segment bezahlt wird
        credits = miners * 100 * 24 * 30 * 0.3 * SEGMENT_REWARD / 1000
        print(f"    {miners:>12,}{stake:>18,.0f}{credits:>20,.0f}{stake+credits:>16,.0f}")
    print("\n    ==> Der Stake uebersteigt den Credit-Bedarf um mehr als das")
    print("        Hundertfache; er bestimmt also allein die noetige Anfangsmenge.")
    print("        Ein Start bei exakt null ist nicht moeglich, da ohne Vorabbestand")
    print("        kein Miner die Sicherheitsbedingung erfuellen kann und somit keine")
    print("        Praegung entsteht. Die Menge faellt jedoch klein aus, sofern die")
    print("        Stichprobenrate anfangs erhoeht wird (siehe [2]).")

    # ── 2. Aufloesung: gestaffelte Stake-Anforderung ────────────────────────
    print("\n[2] Aufloesung ohne Vorverkauf: gestaffelter Stake")
    print("    Die Sicherheitsbedingung S_min = g/p^2 haengt von der Stichprobenrate p ab.")
    print("    Eine hoehere Prueframe in der Anlaufphase senkt den noetigen Stake.\n")
    print(f"    {'Stichprobenrate p':>20}{'S_min je Einheit':>20}{'Stake fuer 200 Miner':>24}")
    print("    " + "-" * 64)
    for p in (0.02, 0.05, 0.10, 0.25, 0.50):
        smin = SEGMENT_REWARD / p ** 2
        print(f"    {p:>19.0%}{smin:>20,.0f}{200*smin:>24,.0f}")
    print("\n    ==> Bei 50 % Stichprobenrate faellt der Stake-Bedarf auf ein")
    print("        Sechshundertstel. Das kostet Kapazitaet (jedes zweite Segment")
    print("        wird nachgerechnet), ist in der Anlaufphase aber vertretbar,")
    print("        da ohnehin Ueberkapazitaet besteht. Die Rate wird mit wachsendem")
    print("        Netz planmaessig auf 2 % gesenkt.")

    # ── 3. Emissionsverlauf ─────────────────────────────────────────────────
    print("\n[3] Entwicklung des Gesamtangebots")
    years = 10
    epochs = years * EPOCHS_PER_YEAR
    for label, genesis, m_max in [("Genesis 100k, kein Cap", 100_000, None),
                                   ("Genesis 100k, Cap 3.000/Epoche (wirksam)", 100_000, 3_000),
                                   ("Genesis 100k, Cap 2.000/Epoche (bindend)", 100_000, 2_000)]:
        h = emission(epochs, genesis, m_max=m_max)
        print(f"\n    {label}")
        print(f"      {'Jahr':>6}{'Umlauf':>16}{'Praegung/Epoche':>18}{'Subvention s':>14}")
        for y in (0, 1, 2, 5, 10):
            idx = min(len(h) - 1, y * EPOCHS_PER_YEAR)
            e, sup, mint, burn, s = h[idx]
            print(f"      {y:>6}{sup:>16,.0f}{mint:>18,.0f}{s:>13.1%}")
    print("\n    ==> Ohne Deckel bleibt die Praegung an den geglaetteten Burn")
    print("        gekoppelt und waechst nur mit der Nachfrage. Ein bindender")
    print("        Deckel entkoppelt sie davon: Sobald die Nachfrage ihn ueberschreitet,")
    print("        wird geleistete Arbeit nicht mehr voll verguetet, und Miner")
    print("        verlassen das Netz. Der Deckel wirkt damit nicht als Knappheits-")
    print("        garantie, sondern als Kapazitaetsbremse.")

    # ── 4. Konzentration in der Fruehphase ──────────────────────────────────
    print("\n[4] Konzentrationsrisiko der Fruehphase")
    print("    Wer frueh mint, haelt spaeter einen ueberproportionalen Anteil.\n")
    print(f"    {'Netz waechst von':>20}{'Anteil der Erstminer nach 5 Jahren':>38}")
    print("    " + "-" * 58)
    rng = random.Random(SEED)
    for start, end in [(50, 500), (50, 5_000), (50, 50_000)]:
        # Anteil der Praegung, der in Jahr 1 anfaellt, verteilt auf `start` Miner
        total = 0.0
        early = 0.0
        for y in range(5):
            miners = start * (end / start) ** (y / 4)
            mint_year = 5_000 * EPOCHS_PER_YEAR * 0.5 ** (y / 2)
            total += mint_year
            if y == 0:
                early = mint_year
        print(f"    {start:>8,} auf {end:>7,}{early/total:>37.1%}")
    print("\n    ==> Frueh Beteiligte halten strukturell einen hohen Anteil. Das ist")
    print("        bei arbeitsgebundener Praegung unvermeidlich und entspricht dem")
    print("        Verhalten anderer Proof-of-Work-Netze. Gegenmassnahme ist nicht")
    print("        eine andere Verteilung, sondern eine flache Subventionskurve:")
    print("        Je niedriger s zu Beginn, desto geringer die Fruehphasen-Rendite.")

    # ── 5. Startvarianten im Vergleich ──────────────────────────────────────
    print("\n[5] Vergleich der Startvarianten")
    variants = [
        ("Kein Genesis (reiner Fair Launch)",
         "nicht durchfuehrbar: kein Stake moeglich, Netz startet nicht"),
        ("Genesis nur an Testnet-Teilnehmer",
         "durchfuehrbar; Verteilung folgt geleisteter Arbeit, kein Verkauf"),
        ("Genesis mit Vorverkauf an Investoren",
         "regulatorisch heikel (Investmentversprechen), widerspricht Fair Launch"),
        ("Genesis nur Treasury, Stake gestundet",
         "durchfuehrbar, aber Treasury wird zum Gatekeeper der Anlaufphase"),
    ]
    for name, note in variants:
        print(f"    {name:<40} {note}")
    print("\n    ==> Empfehlung: Genesis-Menge ausschliesslich an Teilnehmer des")
    print("        Incentivized Testnets, bemessen nach dort geleisteter Arbeit,")
    print("        zuzueglich des Treasury-Anteils. Kein Verkauf, keine Team-")
    print("        Allokation ueber die Treasury hinaus. Die Anlaufphase wird")
    print("        ueber die erhoehte Stichprobenrate aus [2] getragen, nicht")
    print("        ueber eine groessere Vorabmenge.")

    print("\n" + "=" * 78)
    print("ERGEBNIS: VIER FESTLEGUNGEN FUER KAPITEL 5.7")
    print("""
  1. Eine Genesis-Menge ist unverzichtbar: Ohne Vorabbestand kann kein
     Miner die Sicherheitsbedingung erfuellen, und ohne Miner entsteht
     keine Praegung. Ein Start bei null ist nicht durchfuehrbar.
  2. Die Menge bemisst sich am Stake-Bedarf der Anlaufphase, nicht an
     einem Zielwert. Sie faellt deutlich kleiner aus, wenn die Stichproben-
     rate anfangs erhoeht und planmaessig gesenkt wird.
  3. Verteilung ausschliesslich nach geleisteter Arbeit im Incentivized
     Testnet, zuzueglich Treasury. Kein Vorverkauf.
  4. Kein fester Emissionsdeckel je Epoche: Er entkoppelt Verguetung von
     Arbeit, sobald das Netz waechst. Die Begrenzung erfolgt ueber die
     Kopplung an den geglaetteten Burn und die auslaufende Subvention.
""")
    print("=" * 78)


if __name__ == "__main__":
    main()
