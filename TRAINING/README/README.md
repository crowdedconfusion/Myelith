# training (`myl-train`)

> **Version:** 0.5.0
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

### myl-train v0.2.0 – 2026-08-23 (Wachstumsoperator, Bitbudget, Tiefenwachstum)

Drei Fahrplanpunkte, alle drei ohne fremde Hardware machbar und alle drei
unabhängig vom ganzzahligen Rückwärtspass.

**1.2, der Wachstumsoperator** (`src/wachstum.rs`). Die ganzzahlige
Aufteilung `a = ⌊m/2⌋`, `b = m − a` statt der Halbierung aus Net2Net und
bert2BERT. `a + b = m` gilt für jede ganze Zahl, es wird nichts gerundet,
und bei jedem ungeraden Eintrag trennt die Aufteilung die beiden Kopien
um genau ein LSB, **ohne jeden Zufall**.

Dazu die Identitätsebene für das Tiefenwachstum (Ausgabegewicht null,
exakt darstellbar) und der Digest über Form **und** Werte.

**Das Akzeptanzkriterium ist ein Digestvergleich, kein Toleranzvergleich.**
Geprüft über alle drei Einheiten des Beispiels und über 200 zufällige
Matrizen aus einem reproduzierbaren xorshift: Die Ausgabe vor und nach der
Expansion ist bitgleich.

**Ein Detail, an dem es hätte scheitern können:** `⌊m/2⌋` muss
**abrunden**, auch bei negativen Zahlen. Rusts `/` trunkiert zur Null, und
`-3 / 2 = -1` ergäbe zwar `a + b = -3`, aber ein anderes `a` als die
Referenzsimulation mit `torch.floor`. Zwei Implementierungen desselben
Operators müssen dasselbe liefern, sonst ist der Digestvergleich wertlos.
Als Test festgehalten.

**1.1, das Bitbudget** (`tests/diag/results/bitbudget_uebersicht.md`).
Vier Lernraten gemessen statt einer:

| Lernrate | F empfohlen | W_master | Wort |
|---|---|---|---|
| 1e-3 | 19 | 27 | **int32** |
| 1e-4 | 22 | 30 | **int32** |
| 1e-5 | 25 | 33 | int64 |
| 1e-6 | 29 | 37 | int64 |

**Die Grenze zwischen int32 und int64 liegt zwischen 1e-4 und 1e-5.** Der
bisherige einzelne Messpunkt bei 1e-5 empfahl int64, und das stimmt dort
und nur dort. Der Master ist die größte Datenstruktur des
Trainingsschritts; ob er 32 oder 64 Bit breit ist, entscheidet über die
Hälfte seines Speichers.

Die Abhängigkeit ist auch herleitbar (`log2(10) = 3,32` Bit je Faktor 10)
und stimmt mit der Messung überein: 3, 3 und 4 Bit über die drei
Übergänge. **Die Modellgrößen-Achse bleibt bei einem Punkt**, weil 7B in
float32 rund 30 GB bräuchte und diese Maschine 24 GB hat.

**1.3, das Tiefenwachstum** (`tests/diag/tiefenwachstum_simulation.py`).
Die Frage war, ob eine als Identität startende Ebene tot bleibt.

**Sie bleibt es nicht**, und zwar ab dem ersten Schritt: Der Gradient nach
dem Ausgabegewicht ist `aᵀ·g` und hängt nicht vom Ausgabegewicht ab.
Über 20 Schritte bewegen sich mit stochastischem Runden 120 von 128
Gewichten, mit Rundung zur nächsten Stufe 33.

**Warum der Unterschied so groß ist**, und das ist ein eigener Befund:
Das stochastische Runden verändert auch die *Eingangs*gewichte je
Schritt, damit die Aktivierungen und damit, welche Einträge überhaupt
einen Gradienten sehen. Mit Rundung zur nächsten Stufe sind die
Aktivierungen über alle Schritte gleich, und ein Drittel der Einträge
bekommt **nie** einen.

**Fund dabei:** `Konzept-Wachstum.md` führte diese Messung seit dem
2026-08-22 als erledigt, mit konkreten Zahlen und einem Beleg, den es
nicht gab. Das genannte Skript misst ausschließlich Breitenwachstum, ein
Protokoll mit diesen Zahlen existierte nirgends. Der Fahrplan hatte den
Punkt zu Recht als „nicht gemessen" geführt. Dieselbe Klasse wie Fund 27
und Fund 37; das Konzept trägt jetzt die gemessenen Zahlen und den
Vermerk.

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

Der Einzelbeweis stimmte genau, dort gibt es keine Bündelung. Die
Abweichung ging in die sichere Richtung, der Anhang gab das Verfahren
teurer an, als es ist. **Das Papier ist am selben Tag korrigiert worden**
(DE und EN, MD und PDF); die gerechneten Werte stehen zusätzlich als
Test, damit Papier und Code nicht wieder auseinanderlaufen.

**Nicht gebaut:** die Ablehnungsquote für verweigerte Segmente. Sie ist
eine Buchführung über das Verhalten eines Miners über Epochen hinweg und
gehört zum Ledger.


Noch keine Version veröffentlicht.
