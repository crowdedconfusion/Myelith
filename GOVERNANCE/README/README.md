# governance (`myl-governance`)

> **Version:** 0.0.0
> **Datum:** 2026-08-10
> **Status:** Phase 0 – nicht begonnen

Genesis-Modellwahl, Modell-Update-Prozess, Parameter-Governance. Referenzimplementierung von Whitepaper Kap. 10.

## Ziel

Anders als die übrigen Komponenten ist dies überwiegend eine **Prozess-, nicht Code-Komponente**: Kap. 10.1 (Anforderungen an das Basismodell, Verantwortung für die Quantisierung), Kap. 10.2 (dreistufiger Modell-Update-Prozess: Vorschlag, Shadow-Phase, Abstimmung) und Kap. 10.3 (welche Parameter änderbar sind, welche Verfassungsrang haben). Der Code-Anteil ist klein: Abstimmungsmechanik, Parameter-Registry mit Änderbarkeits-Flags, Shadow-Phase-Automatisierung.

**Abhängigkeit:** CONSENSUS (Abstimmung nutzt Stake × Arbeitshistorie, dieselbe Gewichtung wie Validator-Wahl, Kap. 10.2), TOKENOMICS (Parameter wie p, s, κ, γ_train sind hier verankert und dort implementiert — GOVERNANCE ändert sie, TOKENOMICS führt sie aus).

## Struktur

(wird mit Phase 1 befüllt)

## Changelog

(noch keine Version veröffentlicht)
