#!/usr/bin/env python3
"""training_capacity_sim.py — Tragfaehigkeit des Trainings im Myelith-Netz
(Grundlage fuer ein Trainings-Kapitel; Meilenstein M3.)

GEPRUEFT WIRD
-------------
  1. Reicht eine Trainings-Grundlast von 5-10 % fuer sinnvolles Lernen?
     Bezugsgroesse: Token-Durchsatz gegenueber bekannten Trainingslaeufen.
  2. Was kostet die Merkle-Provenienz der Trainingsdaten an Bandbreite
     und Rechenzeit?
  3. Restrisiko Auswahl-Poisoning: Ein Angreifer kann keine Daten faelschen,
     aber aus dem zugelassenen Korpus gezielt AUSWAEHLEN. Wie stark wirkt das?
  4. Oekonomie: Training erzeugt keine Gebuehren. Woher kommt die Verguetung,
     und was macht das mit dem Burn-and-Mint-Gleichgewicht?

Nur Standardbibliothek. Aufruf: python3 training_capacity_sim.py
"""

import math
import random

SEED = 20260804

# ── Netzannahmen (konservativ, aus Kap. 3/4 des Whitepapers) ────────────────
SCENARIOS = {
    "Startphase":   dict(miners=500,    gpu_tflops=40,  util=0.35),
    "Wachstum":     dict(miners=5_000,  gpu_tflops=60,  util=0.55),
    "Reifephase":   dict(miners=50_000, gpu_tflops=80,  util=0.70),
}
MODEL_PARAMS = 24e9          # 24B dense (Mistral-Small-Klasse)
TFLOPS_EFF = 0.25            # realistischer Anteil der Spitzenleistung ueber WAN
REDUNDANCY = 2               # Stufe-1-Redundanz gilt auch fuer Training


def training_tokens_per_day(miners, gpu_tflops, util, base_rate):
    """Wie viele Trainings-Token schafft das Netz taeglich?

    Faustregel (Kaplan/Chinchilla): Ein Trainingsschritt kostet etwa
    6 * N FLOPs je Token (Vorwaerts- plus Rueckwaertspass), N = Parameterzahl.
    Die Redundanz halbiert den nutzbaren Durchsatz.
    """
    total_flops = miners * gpu_tflops * 1e12 * TFLOPS_EFF * 86400
    free = total_flops * (1 - util) * base_rate
    return free / (6 * MODEL_PARAMS * REDUNDANCY)


def main():
    rng = random.Random(SEED)
    print("=" * 76)
    print("TRAININGSKAPAZITAET, DATENPROVENIENZ UND OEKONOMIE")
    print("=" * 76)

    # ── 1. Durchsatz ────────────────────────────────────────────────────────
    print(f"\n[1] Reicht die Grundlast fuer sinnvolles Training?")
    print(f"    Modell: {MODEL_PARAMS/1e9:.0f}B dense, 6*N FLOPs/Token, "
          f"Redundanz r={REDUNDANCY}\n")
    print(f"    {'Szenario':<14}{'Miner':>8}{'Rate':>7}{'Token/Tag':>16}{'Tage fuer 1B':>14}")
    print("    " + "-" * 62)
    for name, cfg in SCENARIOS.items():
        for rate in (0.05, 0.10):
            tok = training_tokens_per_day(cfg['miners'], cfg['gpu_tflops'], cfg['util'], rate)
            days = 1e9 / tok if tok > 0 else float('inf')
            print(f"    {name:<14}{cfg['miners']:>8,}{rate:>6.0%}{tok:>16,.0f}{days:>14,.1f}")
    print()
    print("    Vergleichsgroessen: Ein Feintuning-Lauf umfasst typischerweise")
    print("    1-10 Mrd. Token, ein vollstaendiges Vortraining 1-15 Bio. Token.")
    print("    ==> Feintuning ist ab der Wachstumsphase in Tagen erreichbar.")
    print("        Vortraining aus dem Netz heraus ist NICHT realistisch:")
    print("        dafuer waeren Groessenordnungen mehr Kapazitaet noetig.")
    print("        Das Netz kann ein Basismodell also fortschreiben, nicht erzeugen.")

    # ── 2. Merkle-Provenienz ────────────────────────────────────────────────
    print("\n[2] Kosten der Datenprovenienz")
    corpus_docs = 10 ** 9                      # 1 Mrd. Dokumente im Korpus
    proof_depth = math.ceil(math.log2(corpus_docs))
    proof_bytes = proof_depth * 32
    seq_len = 4096
    bytes_per_token_data = 2                   # int16-Token-IDs
    payload = seq_len * bytes_per_token_data
    print(f"    Korpus: {corpus_docs:,} Dokumente -> Merkle-Tiefe {proof_depth}")
    print(f"    Beweisgroesse je Segment: {proof_bytes} Byte")
    print(f"    Nutzdaten je Segment ({seq_len} Token): {payload:,} Byte")
    print(f"    Overhead je Einzelbeweis: {proof_bytes/payload:.2%}")
    print(f"    Pruefaufwand: {proof_depth} Hash-Operationen, also mikrosekunden.")
    print("\n    Einzelbeweise sind damit NICHT vernachlaessigbar: Bandbreite ist")
    print("    der Engpass des WAN-Betriebs (Kap. 10). Abhilfe schafft ein")
    print("    gemeinsamer Beweis fuer einen ganzen Batch benachbarter Segmente")
    print("    (Merkle-Multiproof): Gemeinsame Pfadanteile werden nur einmal")
    print("    uebertragen.\n")
    print(f"    {'Batch':>8}{'Beweis gesamt':>16}{'Nutzdaten':>14}{'Overhead':>11}")
    print("    " + "-" * 49)
    for batch in (1, 16, 64, 256, 1024):
        # Zusammenhaengender Batch: Die Segmente bilden einen Teilbaum der
        # Tiefe log2(batch). Uebertragen werden der Pfad von dessen Wurzel zur
        # Korpuswurzel plus die Geschwisterknoten innerhalb des Teilbaums.
        sub = int(math.log2(batch)) if batch > 1 else 0
        nodes = (proof_depth - sub) + max(0, batch - 1)
        total_proof = nodes * 32
        total_payload = batch * payload
        print(f"    {batch:>8}{total_proof:>15,}B{total_payload:>13,}B"
              f"{total_proof/total_payload:>10.2%}")
    print("\n    ==> Bei zusammenhaengender Zuweisung faellt der Overhead ab etwa")
    print("        256 Segmenten unter ein halbes Prozent.")
    print("        Da Trainings-Segmente ohnehin gebuendelt zugewiesen werden,")
    print("        ist das die natuerliche Betriebsform. Der eigentliche Aufwand")
    print("        liegt nicht im Beweis, sondern in der Kuratierung des Korpus.")

    # ── 3. Auswahl-Poisoning ────────────────────────────────────────────────
    print("\n[3] Restrisiko: Auswahl statt Faelschung")
    print("    Ein Angreifer kann keine Daten erfinden (Merkle-Beweis fehlt),")
    print("    aber aus dem Korpus gezielt auswaehlen. Wirkung?\n")
    print(f"    {'Angreiferanteil':>16}{'ohne Zulosung':>18}{'mit VRF-Zulosung':>20}")
    print("    " + "-" * 56)
    for share in (0.05, 0.20, 0.40):
        # ohne Zulosung: Angreifer waehlt seine Segmente frei
        free_bias = share
        # mit VRF-Zulosung: Segmentzuweisung ist vorgegeben, Angreifer kann nur
        # ablehnen (und verliert dann Verguetung); Rest-Einfluss ~ share * p_reject
        vrf_bias = share * 0.05
        print(f"    {share:>15.0%}{free_bias:>17.1%}{vrf_bias:>19.1%}")
    print("\n    ==> Entscheidend ist, dass die DATENAUSWAHL ebenfalls per VRF")
    print("        erfolgt, nicht durch den Miner. Dann bleibt nur die Option,")
    print("        zugewiesene Segmente abzulehnen, was Verguetung kostet und")
    print("        ueber die Ablehnungsquote sichtbar wird.")

    # ── 4. Oekonomie ────────────────────────────────────────────────────────
    print("\n[4] Wer bezahlt das Training?")
    print("    Training erzeugt keine Inferenzgebuehren, also keinen Burn.")
    print("    Drei Finanzierungswege im Vergleich:\n")
    burn_per_epoch = 10_000            # MYL, aus tokenomics_sim.py
    for label, share in [("Aus der Praegung (Anteil am Block-Reward)", 0.05),
                         ("Aus der Treasury (3 % lt. Kap. 5.3)", 0.03),
                         ("Aufschlag auf Inferenzgebuehren", 0.05)]:
        cost = burn_per_epoch * share
        print(f"    {label:<44} {cost:>8,.0f} MYL/Epoche")
    print()
    print("    Bewertung:")
    print("      - Praegungsanteil: erhoeht die Inflation, verwaessert Halter,")
    print("        aber bindet Training an die Netzgroesse. Gefahr: Training")
    print("        wuerde auch dann verguetet, wenn es keinen Nutzen bringt.")
    print("      - Treasury: bereits vorhanden, aber begrenzt und governance-")
    print("        gebunden. Passt zur Rolle des Modells als Allmende.")
    print("      - Gebuehrenaufschlag: Nutzer zahlen fuer kuenftige Qualitaet.")
    print("        Sauberste Zuordnung von Kosten und Nutzen, aber verteuert")
    print("        Inferenz und schwaecht die Wettbewerbsposition.")
    print("    ==> Empfehlung: Treasury als Grundfinanzierung, ergaenzt um einen")
    print("        kleinen, per Governance abschaltbaren Gebuehrenaufschlag.")
    print("        Kein Praegungsanteil, da er Training unabhaengig vom Nutzen")
    print("        belohnt und damit dieselbe Fehlanreizstruktur schafft wie")
    print("        eine Verguetung nach Rechenzeit statt nach Ergebnis.")

    # ── 5. Verifikationskosten ──────────────────────────────────────────────
    print("\n[5] Verifikation des Trainings")
    print("    Gradienten sind assoziativ berechenbar und damit bitgleich")
    print("    vergleichbar (integer_training_sim.py, Abschnitt 4).")
    print("    Es gilt dieselbe Stufenstruktur wie bei Inferenz:")
    print("      Stufe 1: r=2 Pods rechnen denselben Gradienten -> Hash-Vergleich")
    print("      Stufe 2: Stichprobe durch Checker")
    print("      Stufe 3: Bisektion im Streitfall")
    print("    Zusaetzlich noetig, weil Berechnung allein nicht genuegt:")
    print("      - Merkle-Beweis der Datenherkunft (Abschnitt 2)")
    print("      - VRF-gesteuerte Datenzuweisung (Abschnitt 3)")
    print("      - Shadow-Phase vor Uebernahme neuer Gewichte (Kap. 9.2)")

    print("\n" + "=" * 76)
    print("BEWERTUNG")
    print("""
  + Feintuning aus der Restkapazitaet ist ab mittlerer Netzgroesse in Tagen
    moeglich. Die vorgeschlagene Grundlast von 5-10 % ist tragfaehig.
  + Provenienzpruefung kostet unter 1 % Overhead und ist vernachlaessigbar.
  + Die Verifikation der Berechnung uebertraegt sich unveraendert vom
    Vorwaerts- auf den Rueckwaertspass.

  - Vortraining eines eigenen Basismodells ist aus der Restkapazitaet NICHT
    erreichbar. Das Netz kann ein bestehendes Open-Weight-Modell fortschreiben,
    aber keines von Grund auf erzeugen.
  - Auswahl-Poisoning bleibt moeglich, wenn die Datenzuweisung nicht ebenfalls
    per VRF erfolgt. Dies ist eine zwingende Entwurfsauflage.
  - Die Finanzierung erzeugt in jeder Variante Fehlanreize. Verguetung nach
    Rechenzeit belohnt Training unabhaengig vom Ergebnis; eine ergebnis-
    abhaengige Verguetung waere subjektiv und damit angreifbar.
    Dies ist die schwaechste Stelle des Entwurfs.
""")
    print("=" * 76)


if __name__ == "__main__":
    main()
