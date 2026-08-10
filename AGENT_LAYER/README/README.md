# agent-layer (`myl-agent`)

> **Version:** 0.0.0
> **Datum:** 2026-08-10
> **Status:** Phase 0 – blockiert (siehe Abhängigkeiten)

Session-Kontrakte, Schadensbegrenzung, Dual-LLM-Trennung gegen eingeschleuste Anweisungen, Segmentketten-Verifikation, Agentengedächtnis. Referenzimplementierung von Whitepaper Kap. 8 (L3 Agent Layer).

## Ziel

Macht aus dem Inferenznetz ein handlungsfähiges System, ohne die Verifikationsgarantien aus Kap. 6 stillschweigend zu überdehnen: Werkzeugergebnisse werden als attestierte, nicht verifizierte Eingabe behandelt (Kap. 8.1); Budget, Empfängerliste und Zeitfenster stehen im Session-Kontrakt außerhalb des Modellkontexts und werden vom Konsens durchgesetzt, nicht vom Agenten selbst (Kap. 8.2); architektonische Trennung (Dual-LLM-Muster) begrenzt den Schaden eingeschleuster Anweisungen auf das gesetzte Budget (Kap. 8.3).

**Abhängigkeit:** COMPUTE_PIPELINE (jeder Agentenschritt ist ein Inferenz-Segment), CONSENSUS (Session-Kontrakt-Durchsetzung ist ein Ledger-Zustandsübergang), VERIFICATION (Kopplung von Transaktionshöhe und bestätigter Auslieferung, Kap. 8.2).

## Struktur

(wird mit Phase 1 befüllt)

## Changelog

(noch keine Version veröffentlicht)
