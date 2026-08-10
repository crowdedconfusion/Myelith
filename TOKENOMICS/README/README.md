# tokenomics (`myl-tokenomics`)

> **Version:** 0.0.0
> **Datum:** 2026-08-10
> **Status:** Phase 0 – nicht begonnen

Prägefunktion, Burn-and-Mint-Kreislauf, Credit-Preisbildung, Staking/Slashing-Matrix, Ausgabestruktur und Genesis. Referenzimplementierung von Whitepaper Kap. 5 und Anhang B.1–B.4, B.7–B.8.

## Ziel

Der geschlossene Wertkreislauf (Kap. 5.1): Nutzer verbrennen MYL gegen Inferenz-Credits, Miner erhalten neu geprägte MYL proportional zur verifizierten Arbeit. Diese Komponente implementiert die konkreten Formeln (Prägefunktion, Credit-Preisbildung, Sicherheitsbedingung S_min) auf Basis der Ledger-Zustandsübergänge aus CONSENSUS.

**Abhängigkeit:** CONSENSUS (Ledger-Zustandsübergänge `burn`/`mint_credits`/`apply_verdict`, Anhang A.5). Kann implementiert werden, sobald CONSENSUS Phase 1 (Ledger-Grundzustand) steht — braucht keine fertige BFT-Blockproduktion, nur die Zustandsübergangs-Schnittstelle.

## Struktur

(wird mit Phase 1 befüllt)

## Changelog

(noch keine Version veröffentlicht)
