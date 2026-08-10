# training (`myl-train`)

> **Version:** 0.0.0
> **Datum:** 2026-08-10
> **Status:** Phase 0 – blockiert (siehe Abhängigkeiten)

Trainings als nachrangige Arbeitsklasse, lokale Verlustblöcke, Datenprovenienz, robuste Aggregation, Modellwachstum. Referenzimplementierung von Whitepaper Kap. 7.

## Ziel

Ermöglicht dem Netzwerk, das Netzwerkmodell fortzuschreiben, ohne Inferenzkapazität zu verdrängen (Kap. 7.1) und ohne die inhaltliche Bewertungsfrage zu öffnen, die das Protokoll sonst vermeidet (Kap. 7.3: Herkunfts- statt Inhaltsprüfung). Diese Komponente ist laut Whitepaper selbst die am wenigsten abgesicherte: Kap. 7.6 benennt drei ungelöste Punkte (Finanzierungs-Fehlanreize, unbelegte Verfahrenskombination, unbekanntes Verhalten unter offenen Netzbedingungen).

**Abhängigkeit:** COMPUTE_PIPELINE (Trainingssegmente nutzen dieselbe Pod-Infrastruktur), CONSENSUS (VRF-Datenzuweisung nutzt den Epochen-Scheduler, Ledger-Buchhaltung der Trainingsvergütung), VERIFICATION (Aggregations- und Gradienten-Segmente brauchen dieselbe Bisektions-/Redundanzlogik), TOKENOMICS (Trainingsvergütungs-Obergrenze), sowie eine **Erweiterung von INTEGER_LLM um einen ganzzahligen Rückwärtspass** — bisher nicht Teil des INTEGER_LLM-Fahrplans, der ausschließlich Inferenz behandelt.

## Struktur

(wird mit Phase 1 befüllt)

## Changelog

(noch keine Version veröffentlicht)
