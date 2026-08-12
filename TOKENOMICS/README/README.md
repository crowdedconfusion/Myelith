# tokenomics (`myl-tokenomics`)

> **Version:** 0.1.4
> **Datum:** 2026-08-13
> **Status:** Design-Entscheidungen getroffen (Fixed-Point bestätigt,
> vTFE-Skalierung 10⁻⁶, MYL-Kleinstbeträge 10⁶, EMA-Fenster 30 Epochen
> α=2/31 — Details im Fahrplan); 🎉 **Phase 1 vollständig**
> (`myl-tokenomics` v0.1.1–v0.1.4, Akzeptanzkriterium erfüllt); als
> Nächstes folgt Phase 2 (Credit-Preisbildung mit LUT-exp()).

Prägefunktion, Burn-and-Mint-Kreislauf, Credit-Preisbildung,
Staking/Slashing-Matrix, Ausgabestruktur und Genesis. Referenzimplementierung
von Whitepaper Kap. 5 und Anhang B.1–B.4, B.7–B.8.

## Aufgabe

Der geschlossene Wertkreislauf (Kap. 5.1): Nutzer verbrennen MYL gegen
Inferenz-Credits, Miner erhalten neu geprägte MYL proportional zur
verifizierten Arbeit. Diese Komponente implementiert die konkreten Formeln
(Prägefunktion, Credit-Preisbildung, Sicherheitsbedingung S_min) auf Basis
der Ledger-Zustandsübergänge aus CONSENSUS. Wo das Protokoll `exp()`
verwendet (Credit-Preisbildung), muss die Approximation ganzzahlig erfolgen
(LUT-basiert), um dieselbe Determinismus-Anforderung wie die Inferenzseite zu
erfüllen.

## Abhängigkeiten

CONSENSUS (Ledger-Zustandsübergänge `burn`/`mint_credits`/`apply_verdict`,
Anhang A.5). Benötigt wird nur die Zustandsübergangs-Schnittstelle — die
fertige BFT-Blockproduktion ist dafür noch nicht vorausgesetzt.

## Struktur

```
TOKENOMICS/
├── README/                   diese Kurzübersicht + Fahrplan
└── myl-tokenomics/           Tokenomik-Berechnungen (Kap. 5)
    └── src/
        ├── lib.rs             Fixed-Point-Grundregeln, Einheiten-Skalierungen
        │                      (1 MYL = 10⁶ Kleinstbeträge, vTFE 10⁻⁶)
        ├── ema.rs             Ganzzahlige EMA für B̄_e (α = 2/31, i128,
        │                      dokumentierte Totzone)
        ├── mint.rs            Prägefunktion M_e = min(B̄_e·(1+s), M_max)
        ├── distribute.rs      Kap.-5.3-Verteilung (Basispunkte, exakte
        │                      Summe, Redundanz-Normierung, proportionale
        │                      Aufteilung)
        └── training.rs        Trainingsvergütungs-Obergrenze (≤ 70 %)
```

## Changelog

### v0.1.1–v0.1.4 – 2026-08-13 (Phase 1)
- Durchgehend Fixed-Point-Ganzzahl-Arithmetik: Brüche als
  Zähler/Nenner-Paare, floor-Divisionen dokumentiert,
  u128/i128-Zwischenrechnungen gegen Überlauf — jede Formel ist ein
  Ledger-Zustandsübergang und muss auf jedem Node bitgleich
  nachrechenbar sein.
- Ganzzahlige EMA (α = 2/31, 30-Epochen-Fenster) mit dokumentierter
  Totzone; Prägefunktion mit M_max-Kappung; Verteilung 78/5/10/4/3 %
  mit Summe-exakt-M_e-Invariante (Rundungsrest ans Treasury);
  Trainingsvergütungs-Obergrenze 70 %.
- Akzeptanzkriterium erfüllt: 10.000-Epochen-Tests (Determinismus und
  Verteilungsexaktheit). 26 Tests grün, keine Warnungen.
