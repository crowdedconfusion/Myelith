# governance (`myl-governance`)

> **Version:** 0.0.0
> **Datum:** 2026-08-11
> **Status:** Planungsphase — nicht begonnen

Genesis-Modellwahl, Modell-Update-Prozess, Parameter-Governance.
Referenzimplementierung von Whitepaper Kap. 10.

## Aufgabe

Anders als die übrigen Komponenten ist dies überwiegend eine **Prozess-,
nicht Code-Komponente**: Anforderungen an das Basismodell und Verantwortung
für die Quantisierung (Kap. 10.1), dreistufiger Modell-Update-Prozess aus
Vorschlag, Shadow-Phase und Abstimmung (Kap. 10.2) sowie die Frage, welche
Parameter änderbar sind und welche Verfassungsrang haben (Kap. 10.3). Der
Code-Anteil ist klein: Abstimmungsmechanik, Parameter-Registry mit
Änderbarkeits-Flags, Shadow-Phase-Automatisierung.

## Abhängigkeiten

CONSENSUS (Abstimmung nutzt Stake × Arbeitshistorie, dieselbe Gewichtung wie
die Validator-Wahl, Kap. 10.2), TOKENOMICS (Parameter wie p, s, κ, γ_train
sind dort implementiert — GOVERNANCE ändert sie, TOKENOMICS führt sie aus).

## Struktur

Entsteht mit der Implementierung.

## Changelog

Noch keine Version veröffentlicht.
