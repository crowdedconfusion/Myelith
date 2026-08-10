# client (Nutzer-Client inkl. Wallet)

> **Version:** –
> **Datum:** 2026-08-10
> **Status:** Frühe Konzeptphase — noch kein Fahrplan, nur Notizen und offene Fragen

Die einzige Komponente des Projekts, mit der Menschen tatsächlich interagieren: MYL-Wallet, Inferenz-Interface, Session-Kontrakt-Verwaltung für Agenten-Nutzung, ggf. Staking-/Governance-Ansicht für Miner und Validatoren.

## Warum das noch fehlt

Das Whitepaper spezifiziert die Netzwerk-/Protokollarchitektur (L0–L3, Tokenomics, Verifikation, Governance) vollständig, aber keine Referenz-Nutzeranwendung. Ohne Client kann niemand mit dem Netzwerk interagieren — MYL halten, Inferenz-Credits kaufen, eine Agenten-Session mit Budgetgrenzen starten. Das ist eine echte Lücke, kein Detail für später: Ohne Vorstellung davon, wie Nutzer Session-Kontrakte setzen (Kap. 8.2) oder Vertraulichkeitsklassen wahrnehmen (Kap. 9.3), bleiben Teile von AGENT_LAYER und CONSENSUS im luftleeren Raum entworfen.

## Struktur

(noch keine — Ideen und offene Design-Fragen werden intern gesammelt und vor
der Erstellung eines Fahrplans geklärt)

## Abhängigkeiten

CONSENSUS (Ledger-Zustand lesen), TOKENOMICS (Burn/Mint-Flow, Credit-Preis), AGENT_LAYER (Session-Kontrakte), NETWORKING (Gateway-Kommunikation), GOVERNANCE (Abstimmungs-UI). Ein sinnvoller Fahrplan setzt voraus, dass diese Komponenten mindestens strukturell stehen — der Client ist bewusst spät in der Baureihenfolge, aber die Konzeptarbeit sollte parallel beginnen, damit die Schnittstellen der anderen Komponenten nicht an den tatsächlichen Nutzerbedürfnissen vorbeientworfen werden.

## Changelog

(noch keine Version veröffentlicht)
