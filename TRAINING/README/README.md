# training (`myl-train`)

> **Version:** 0.4.0
> **Datum:** 2026-08-23
> **Status:** **Die Komponente hat Code.** Punkt 3.1 (Datenprovenienz)
> ist gebaut: `myl-train` v0.1.0 mit Merkle-verankerten Korpora,
> Segmentreferenz per Beweis, gebündelten Beweisen und VRF-gesteuerter
> Zuweisung, 23 Tests. Er stand im Fahrplan hinten, weil er inhaltlich
> dorthin gehört, ist aber der einzige Punkt, der **technisch
> unabhängig** vom ganzzahligen Rückwärtspass ist.
>
> **Die eine Messung ist gemacht.** Punkt 0.1 ist beantwortet:
> Das Schema **trägt**, sofern die Gewichte stochastisch gerundet werden
> (+0,67 % gegen die Gleitkomma-Referenz; mit Rundung zur nächsten Stufe
> +29,9 %). Dazu 0.2: Ein Trainingsschritt **ohne Gleitkommazustand**
> geht, mit ganzzahligem Master und zählerbasiertem Würfel, +0,75 %.
> Protokolle:
> [`entscheidung_0-1.md`](../tests/diag/results/entscheidung_0-1.md) und
> [`entscheidung_0-2.md`](../tests/diag/results/entscheidung_0-2.md).
> Das Konzept daraus steht in
> [`Konzept-Wachstum.md`](Konzept-Wachstum.md): der Trainingsschritt,
> seine Verifikation, die Aggregation und ein Modell, das wächst.
> Der Fahrplan steht damit; sein erster Punkt liegt in INTEGER_LLM,
> nicht hier: Solange Vorwärts- und Rückwärtspass in Gleitkomma
> rechnen, ist der Gradient geräteabhängig und mit ihm jedes Δm.

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

### myl-train v0.1.0 – 2026-08-23 (Punkt 3.1: Datenprovenienz)

**Die erste Zeile Code dieser Komponente.** Bis hierher bestand TRAINING
aus Fahrplan, Konzept und Diagnoseskripten; Kritikpunkt K7 führte sie
als eine von drei Komponenten ohne Code.

Gebaut ist Kap. 7.3, **Herkunft statt Inhalt**: Ein Miner, der
vergiftete Texte einspeist, rechnet bitgleich korrekt. Der Bitvergleich
aus Kap. 6 greift dort nicht, und eine inhaltliche Bewertung wäre genau
der subjektive Spielraum, den das Protokoll sonst vermeidet.

- **`provenienz`** — Korpora über eine Merkle-Wurzel verankern,
  Segmente per Beweis referenzieren statt per Rohdaten, Bündel
  zusammenhängender Segmente. Für eine nicht existierende Position gibt
  es keinen Beweis, und zwar nicht, weil die Prüfung ihn ablehnt,
  sondern weil er sich nicht erzeugen lässt.
- **`zuweisung`** — welcher Pod welche Abschnitte bearbeitet, folgt aus
  dem Epochen-Seed, nicht aus einer Wahl. Wer keine Daten fälschen kann,
  kann sonst immer noch auswählen: Bei freier Wahl entspräche der
  Einfluss dem Kapazitätsanteil (Anhang B.6.5).

**Der Seed wird hier nicht erzeugt**, sondern als 32 Bytes
entgegengenommen. Eine zweite Stelle, die Seeds erzeugt, wäre eine
zweite Quelle für dieselbe Aussage; die Bindung an den finalisierten
Block und die Epochennummer gehört genau einmal in den Scheduler
(Fund A20). Deshalb steht `myl-scheduler` auch nicht im Manifest.

**Ein Test, den es braucht:** Eine Referenz darf **nicht** gegen ihre
eigene mitgebrachte Wurzel geprüft werden, sonst baut sich ein Angreifer
mit selbstgewählten Daten einen gültigen Beweis. Das war Audit-Fund A11,
eine Ebene höher.

**Fund dabei: Anhang B.6.4 gibt gebündelte Beweise zu teuer an.** Wer
alle Blätter eines vollständigen Teilbaums hat, braucht für dessen
untere Ebenen keinen Geschwisterknoten; übertragen wird nur der Weg von
der Teilbaumwurzel zur Baumwurzel.

| Segmente | Knoten | Bytes | gerechnet | Anhang B.6.4 |
|---|---|---|---|---|
| 1 | 30 | 960 | 11,72 % | **11,7 %** ✅ |
| 16 | 26 | 832 | 0,63 % | **1 %** |
| 256 | 22 | 704 | **0,034 %** | **0,42 %** |

Der Einzelbeweis stimmt genau, dort gibt es keine Bündelung. Die
Abweichung geht in die sichere Richtung, der Anhang gibt das Verfahren
teurer an, als es ist; falsch ist er trotzdem, für 256 Segmente um den
Faktor 12,5. Beide Zahlenreihen stehen als Test.

**Nicht gebaut:** die Ablehnungsquote für verweigerte Segmente. Sie ist
eine Buchführung über das Verhalten eines Miners über Epochen hinweg und
gehört zum Ledger.


Noch keine Version veröffentlicht.
