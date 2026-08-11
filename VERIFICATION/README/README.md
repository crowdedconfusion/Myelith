# verification (`myl-verifier`)

> **Version:** 0.0.0
> **Datum:** 2026-08-11
> **Status:** Planungsphase — inhaltlich blockiert (siehe Abhängigkeiten)

Redundanzvergleich, optimistische Stichproben, Bisektions-Spiel,
Kontrollsegmente. Referenzimplementierung von Whitepaper Kap. 6.4–6.9 und
Anhang A.4.

## Aufgabe

Die drei Verifikationsstufen aus Kap. 3.4/6.4: deterministische Redundanz
(Stufe 1, sofort), optimistische Stichproben mit Bisektions-Spiel (Stufe 2,
verzögert) sowie das Kontrollsegment-Verfahren gegen den einmaligen
gezielten Eingriff (Kap. 6.7). Stufe 3 (zkML-Anker) ist explizit als
späterer Aufrüstpfad benannt (Kap. 6.4) und nicht Teil dieser Komponente.

## Abhängigkeiten

CONSENSUS (Challenges und Verdicts sind Blockinhalt, Kap. 3.5) sowie eine
**harte inhaltliche Voraussetzung** aus INTEGER_LLM: Das Bisektions-Spiel
(Kap. 6.6) setzt voraus, dass eine Referenz-Ausführung gemäß θ_v auf jeder
Validator-Hardware dasselbe Ergebnis liefert — Bitgleichheit und tragfähige
Qualität ganzzahliger Inferenz sind die noch unbestätigte Kernaussage von
INTEGER_LLM (Whitepaper Kap. 6.2) und werden dort am Referenzmodell gemessen,
bevor diese Komponente sinnvoll gebaut werden kann.

## Struktur

Entsteht mit der Implementierung.

## Changelog

Noch keine Version veröffentlicht.
