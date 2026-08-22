# training (`myl-train`)

> **Version:** 0.2.0
> **Datum:** 2026-08-22
> **Status:** **Die eine Messung ist gemacht.** Punkt 0.1 ist beantwortet:
> Das Schema **trägt**, sofern die Gewichte stochastisch gerundet werden
> (+0,67 % gegen die Gleitkomma-Referenz; mit Rundung zur nächsten Stufe
> +29,9 %). Dazu 0.2: Ein Trainingsschritt **ohne Gleitkommazustand**
> geht, mit ganzzahligem Master und zählerbasiertem Würfel, +0,75 %.
> Protokolle:
> [`entscheidung_0-1.md`](../tests/diag/results/entscheidung_0-1.md) und
> [`entscheidung_0-2.md`](../tests/diag/results/entscheidung_0-2.md).
> Der Fahrplan für alles Weitere entsteht jetzt auf dieser Grundlage.

Trainings als nachrangige Arbeitsklasse, lokale Verlustblöcke,
Datenprovenienz, robuste Aggregation, Modellwachstum.
Referenzimplementierung von Whitepaper Kap. 7.

## Warum hier nur ein Punkt steht

Whitepaper Kap. 7 setzt voraus, dass „die ganzzahlige Ausführung aus
Kapitel 6 unverändert auf den Rückwärtspass überträgt". Das ist eine
**Annahme, keine Messung** — und sie trägt alles: Ohne bit-exakten
ganzzahligen Rückwärtspass gibt es keine verifizierbare Trainingsarbeit,
ohne die keine Vergütung, ohne die kein Modellwachstum.

Es gibt einen konkreten Grund für Zweifel, und er stammt aus der eigenen
Erfahrung des Projekts. **Fund 20** hat gezeigt, dass der Residualstrom
eine Skala **je Kanal** braucht, weil einzelne Kanäle um Größenordnungen
aus der Verteilung ragen (Massive Activations); eine Skala je Tensor
löschte die feinskalierten Kanäle aus. Gradienten haben typischerweise
einen **größeren** Dynamikbereich als Aktivierungen — und über die
Trainingsschritte hinweg einen wandernden. Ob die Block-Skalierung aus
Anhang B.6.2 das trägt, ist offen.

Der Fahrplan hatte 22 Punkte in vier Phasen, die alle darauf ruhten. Am
2026-08-19 wurde er auf die Messung zurückgeschnitten, die das
entscheidet. Die alte Planung steht im Fahrplan als Vorüberlegung ohne
Statusmarken — sie geht nicht verloren, wird aber nach dem Ergebnis neu
geschnitten, möglicherweise anders.

**Die Methode ist erprobt:** In der 7B-Fehlersuche haben zwei
PyTorch-Referenzsimulationen in Stunden entschieden, was vorher tagelang
im falschen Code gesucht wurde. Sie trennen „trägt das Verfahren?" von
„ist unsere Implementierung richtig?" — und nur die erste Frage steht
hier an. Deshalb: erst simulieren, dann bauen.

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
