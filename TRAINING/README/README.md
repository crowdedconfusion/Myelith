# training (`myl-train`)

> **Version:** 0.0.0
> **Datum:** 2026-08-11
> **Status:** Planungsphase — blockiert (siehe Abhängigkeiten)

Trainings als nachrangige Arbeitsklasse, lokale Verlustblöcke,
Datenprovenienz, robuste Aggregation, Modellwachstum.
Referenzimplementierung von Whitepaper Kap. 7.

## Aufgabe

Ermöglicht dem Netzwerk, das Netzwerkmodell fortzuschreiben, ohne
Inferenzkapazität zu verdrängen (Kap. 7.1) und ohne die inhaltliche
Bewertungsfrage zu öffnen, die das Protokoll sonst vermeidet (Kap. 7.3:
Herkunfts- statt Inhaltsprüfung). Diese Komponente ist laut Whitepaper
selbst die am wenigsten abgesicherte: Kap. 7.6 benennt drei ungelöste Punkte
(Finanzierungs-Fehlanreize, unbelegte Verfahrenskombination, unbekanntes
Verhalten unter offenen Netzbedingungen).

## Abhängigkeiten

COMPUTE_PIPELINE (Trainingssegmente nutzen dieselbe Pod-Infrastruktur),
CONSENSUS (VRF-Datenzuweisung nutzt den Epochen-Scheduler,
Ledger-Buchhaltung der Trainingsvergütung), VERIFICATION (Aggregations- und
Gradienten-Segmente brauchen dieselbe Bisektions-/Redundanzlogik),
TOKENOMICS (Trainingsvergütungs-Obergrenze) — sowie ein **ganzzahliger
Rückwärtspass** in INTEGER_LLM, der dort noch nicht implementiert ist
(INTEGER_LLM behandelt bislang ausschließlich Inferenz).

## Struktur

Entsteht mit der Implementierung.

## Changelog

Noch keine Version veröffentlicht.
