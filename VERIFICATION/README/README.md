# verification (`myl-verifier`)

> **Version:** 0.0.0
> **Datum:** 2026-08-10
> **Status:** Phase 0 – blockiert (siehe Abhängigkeiten)

Redundanzvergleich, optimistische Stichproben, Bisektions-Spiel, Kontrollsegmente. Referenzimplementierung von Whitepaper Kap. 6.4–6.9 und Anhang A.4 (`myl-verifier`).

## Ziel

Die drei Verifikationsstufen aus Kap. 3.4/6.4: deterministische Redundanz (Stufe 1, sofort), optimistische Stichproben mit Bisektions-Spiel (Stufe 2, verzögert), sowie das Kontrollsegment-Verfahren gegen den einmaligen gezielten Eingriff (Kap. 6.7). Stufe 3 (zkML-Anker) ist explizit als späterer Aufrüstpfad benannt (Kap. 6.4) und **nicht** Teil dieses Fahrplans.

**Harte Abhängigkeit:** Diese Komponente kann nur so weit sinnvoll gebaut werden, wie INTEGER_LLM den Entscheidungspunkt 12.18–12.21 (Qualitätsmessung ganzzahliger Inferenz) erreicht hat. Das Bisektions-Spiel (Kap. 6.6) setzt voraus, dass eine Referenz-Ausführung gemäß θ_v auf jeder Validator-Hardware dasselbe Ergebnis liefert — genau das ist die noch unbestätigte Kernaussage von INTEGER_LLM Kap. 6.2.

## Struktur

(wird mit Phase 1 befüllt)

## Changelog

(noch keine Version veröffentlicht)
