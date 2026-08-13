#!/usr/bin/env python3
"""model_growth_sim.py — Gekoppeltes Wachstum von Netz und Modell
(Grundlage fuer das Trainings-Kapitel; Meilenstein M3/M4.)

IDEE
----
training_capacity_sim.py zeigt: Vortraining eines Basismodells aus der
Restkapazitaet ist nicht erreichbar. Model Growth eroeffnet einen dritten Weg
zwischen Feintuning und Vortraining: Ein bestehendes Modell wird schrittweise
vergroessert, wobei die bereits investierte Rechenleistung erhalten bleibt.

Belegt in der Literatur:
  - Progressive Stacking / CompoundGrow: bis 68,7 % Beschleunigung (BERT-base)
  - LiGO: 51,7 % FLOPs-Ersparnis bei Tiefen-, 41,6 % bei Breitenerweiterung
  - Depthwise Stacking: 54,6 % Beschleunigung bei 7B-Modellen
  - LLaMA Pro: 7B -> 8,3B per Block-Expansion mit anschliessendem Nachtraining
  - Funktionserhaltende Transformationen (Net2Net, bert2BERT): Das vergroesserte
    Modell verhaelt sich unmittelbar nach der Expansion identisch zum kleineren.

GEPRUEFT WIRD
-------------
  1. Wie viele Token kostet ein Wachstumsschritt gegenueber Vortraining?
  2. Reicht die Trainings-Grundlast fuer solche Schritte, und wie lange dauern sie?
  3. Passt die Tiefenerweiterung zur Shard-Struktur (neue Layer = neue Shards)?
  4. Ist ein Wachstumsschritt selbst verifizierbar?
  5. Welche Wachstumskurve ergibt sich ueber die Netzentwicklung?

Nur Standardbibliothek. Aufruf: python3 model_growth_sim.py
"""

import math

TFLOPS_EFF = 0.25
REDUNDANCY = 2
CHINCHILLA = 20            # compute-optimale Token je Parameter


def daily_tokens(miners, gpu_tflops, util, base_rate, params):
    total = miners * gpu_tflops * 1e12 * TFLOPS_EFF * 86400
    free = total * (1 - util) * base_rate
    return free / (6 * params * REDUNDANCY)


def main():
    print("=" * 78)
    print("GEKOPPELTES WACHSTUM VON NETZ UND MODELL")
    print("=" * 78)

    # ── 1. Kosten eines Wachstumsschritts ───────────────────────────────────
    print("\n[1] Was kostet ein Wachstumsschritt gegenueber Vortraining?")
    print("    Annahme: Nach der Expansion ist ein Nachtraining noetig, um die")
    print("    neuen Parameter nutzbar zu machen. Die Literatur berichtet")
    print("    Ersparnisse von 40-69 %; konservativ wird hier mit 50 % gerechnet,")
    print("    zusaetzlich zur Ersparnis durch das erhaltene Vorwissen.\n")
    print(f"    {'Schritt':<20}{'Parameter':>12}{'Token Vortraining':>20}{'Token Wachstum':>18}{'Faktor':>9}")
    print("    " + "-" * 79)
    steps = [("Basis (extern)", 24e9), ("Schritt 1", 32e9), ("Schritt 2", 48e9),
             ("Schritt 3", 70e9), ("Schritt 4", 100e9)]
    prev = None
    growth_tokens = {}
    for name, p in steps:
        scratch = p * CHINCHILLA
        if prev is None:
            print(f"    {name:<20}{p/1e9:>10.0f}B{scratch/1e9:>18,.0f}B{'-':>18}{'-':>9}")
        else:
            # Nur die NEUEN Parameter muessen trainiert werden, mit 50 % Aufschlag
            # fuer die Anpassung der bestehenden; zusaetzlich Growth-Ersparnis 50 %.
            new_params = p - prev
            tok = (new_params * CHINCHILLA + 0.5 * prev * CHINCHILLA) * 0.5
            growth_tokens[name] = tok
            print(f"    {name:<20}{p/1e9:>10.0f}B{scratch/1e9:>18,.0f}B"
                  f"{tok/1e9:>16,.0f}B{scratch/tok:>8.1f}x")
        prev = p
    print("\n    ==> Ein Wachstumsschritt kostet ein Vielfaches weniger als ein")
    print("        Vortraining derselben Groesse, weil das Vorwissen erhalten bleibt.")

    # ── 2. Dauer bei realer Netzkapazitaet ──────────────────────────────────
    print("\n[2] Wie lange dauert ein Wachstumsschritt im Netz?")
    print(f"    {'Netzgroesse':<16}{'Rate':>6}{'Schritt 1':>13}{'Schritt 2':>13}{'Schritt 3':>13}{'Schritt 4':>13}")
    print("    " + "-" * 74)
    nets = [("500 Miner", 500, 40, 0.35), ("5.000 Miner", 5_000, 60, 0.55),
            ("50.000 Miner", 50_000, 80, 0.70)]
    for label, m, tf, util in nets:
        for rate in (0.10,):
            row = f"    {label:<16}{rate:>5.0%}"
            for (name, p), (_, prev_p) in zip(steps[1:], steps[:-1]):
                tok = growth_tokens[name]
                d = daily_tokens(m, tf, util, rate, p)
                days = tok / d
                row += f"{days:>12,.0f}d" if days < 10000 else f"{'>10000d':>13}"
            print(row)
    print("\n    ==> Ehrliche Einordnung der Zahlen:")
    print("        - 500 Miner: Wachstum praktisch ausgeschlossen (Schritt 1 ueber")
    print("          sieben Jahre). Die Startphase kann nur feintunen.")
    print("        - 5.000 Miner: Schritt 1 in etwa neun Monaten, spaetere Schritte")
    print("          in Jahren. Wachstum ist moeglich, aber langsam.")
    print("        - 50.000 Miner: Schritt 1 in einem Monat, Schritt 4 in zehn.")
    print("        Wachstum ist also kein kontinuierlicher Prozess, sondern ein")
    print("        seltenes Ereignis im Jahresmassstab, das an eine erhebliche")
    print("        Netzgroesse gebunden ist.")

    # ── 3. Passung zur Shard-Struktur ───────────────────────────────────────
    print("\n[3] Tiefenerweiterung und Shard-Struktur")
    print("    Tiefenwachstum fuegt Layer hinzu. In der Pipeline bedeutet das")
    print("    zusaetzliche Shards, also Plaetze fuer weitere Miner.\n")
    print(f"    {'Modell':>10}{'Layer':>8}{'Shards (10 L/Shard)':>22}{'Miner je Pod':>14}")
    print("    " + "-" * 56)
    for name, p in steps:
        layers = int(round(48 * (p / 24e9) ** 0.5))     # Tiefe waechst etwa mit sqrt(Parametern)
        shards = max(1, round(layers / 10))
        print(f"    {p/1e9:>8.0f}B{layers:>8}{shards:>22}{shards + 2:>14}")
    print("\n    ==> Netz- und Modellwachstum sind strukturell gekoppelt: Mehr")
    print("        Miner ermoeglichen mehr Shards, mehr Shards tragen mehr Layer.")
    print("        Zugleich steigt die Kollusionsschranke beta^(2k) mit k (Anhang B.2),")
    print("        Wachstum verbessert also auch die Sicherheit.")

    # ── 4. Verifizierbarkeit des Wachstumsschritts ──────────────────────────
    print("\n[4] Ist der Wachstumsschritt selbst verifizierbar?")
    print("""    Funktionserhaltende Expansion (Net2Net, bert2BERT) ist eine
    DETERMINISTISCHE Transformation der Gewichtsmatrix: Neuronen werden
    aufgespalten, neue Layer als Identitaet oder Kopie initialisiert.
    Daraus folgt:
      - Die Transformation laesst sich bitgleich nachrechnen und per
        Hash-Vergleich verifizieren, wie jede andere Berechnung (Kap. 6).
      - Die neue Modellversion theta_v+1 ergibt sich reproduzierbar aus
        theta_v und dem Wachstumsoperator; beide werden on-chain verankert.
      - Unmittelbar nach der Expansion ist das Verhalten identisch zum
        Vorgaenger. Ein Wachstumsschritt kann daher ohne Qualitaetsrisiko
        aktiviert werden; die Verbesserung entsteht erst durch das
        anschliessende Nachtraining.""")

    # ── 5. Langfristige Kurve ───────────────────────────────────────────────
    print("\n[5] Wachstumskurve ueber zehn Jahre (Reifephase, 10 % Grundlast)")
    params = 24e9
    d_cap = lambda p: daily_tokens(50_000, 80, 0.70, 0.10, p)
    total_days = 0
    print(f"    {'Jahr':>6}{'Parameter':>12}{'kumulierte Tage':>18}")
    print("    " + "-" * 36)
    year = 0
    print(f"    {year:>6}{params/1e9:>10.0f}B{0:>18}")
    for _ in range(6):
        new = params * 1.4
        tok = ((new - params) * CHINCHILLA + 0.5 * params * CHINCHILLA) * 0.5
        total_days += tok / d_cap(new)
        params = new
        year = total_days / 365
        print(f"    {year:>6.1f}{params/1e9:>10.0f}B{total_days:>18,.0f}")
        if year > 10:
            break
    print("\n    ==> Eine Verdopplung der Modellgroesse ist im Jahresmassstab")
    print("        erreichbar, sofern die Netzkapazitaet mitwaechst.")

    print("\n" + "=" * 78)
    print("BEWERTUNG")
    print("""
  + Model Growth eroeffnet den Weg zwischen Feintuning und Vortraining:
    Das Netz kann sein Modell schrittweise vergroessern, ohne je ein
    vollstaendiges Vortraining leisten zu muessen.
  + Funktionserhaltende Expansion ist deterministisch und damit im
    bestehenden Verifikationsmodell nachpruefbar.
  + Tiefenwachstum erzeugt neue Shards und damit Plaetze fuer neue Miner:
    Netz- und Modellwachstum sind strukturell gekoppelt, und die
    Kollusionsschranke verbessert sich mit.

  ! Die Literaturbelege stammen aus dem Vortrainings-Kontext mit
    zentralisierter Kontrolle ueber Daten und Zeitplan. Ob progressive
    Expansion unter den Bedingungen eines offenen Netzes (heterogene
    Kapazitaet, unterbrochene Laeufe, VRF-zugewiesene Daten) ebenso
    funktioniert, ist unbelegt.
  ! Model Growth ist bislang nicht mit ganzzahligem Training kombiniert
    worden. Beide Verfahren sind einzeln belegt, ihre Kombination nicht.
  ! Der Wachstumszeitpunkt wird zur Governance-Entscheidung: Wer zu frueh
    waechst, verschlechtert die Qualitaet je Parameter; wer zu spaet waechst,
    verschenkt Kapazitaet.
""")
    print("=" * 78)


if __name__ == "__main__":
    main()
