# verification (`myl-verifier`)

> **Version:** 0.0.1
> **Datum:** 2026-08-12
> **Status:** Planungsphase — blockiert durch CONSENSUS; die inhaltliche
> INTEGER_LLM-Voraussetzung (Qualität + Determinismus am Referenzmodell)
> ist seit 2026-08-12 bestätigt, der Cross-Hardware-Nachweis steht aus

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
Validator-Hardware dasselbe Ergebnis liefert. Bitgleichheit und tragfähige
Qualität ganzzahliger Inferenz sind am Referenzmodell (Qwen2.5-0.5B)
gemessen und bestätigt (Entscheidungspunkt 12.21 AKZEPTIERT am 2026-08-12:
Perplexität 15,59 vs. BF16-Baseline 14,95 = +4,29 %, Determinismus
laufübergreifend bit-identisch; Protokoll:
`INTEGER_LLM/eval/results/decision_12-21.md`). Noch offen ist der
Cross-Hardware-Nachweis über Validator-Hardware-Generationen hinweg — er
setzt die SIMD-/CUDA-/ROCm-Backends und GPU-Zugang voraus.

## Struktur

Entsteht mit der Implementierung.

## Changelog

Noch keine Version veröffentlicht.
