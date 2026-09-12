# BENCHMARKS

Hier liegen die Datensätze, mit denen dieses Projekt seine Modelle misst,
und die Erzeuger, die sie herstellen. Nach Gegenstand sortiert:

| Ordner | Wofür |
|---|---|
| `Inferenz/` | Was ein Modell **kann**, ohne dass trainiert wurde |
| `Training/` | Ob ein Trainingslauf etwas **beigebracht** hat |
| `Agent/` | Ob die Werkzeugschleife eine Aufgabe **erledigt** |

## ⚠️ `BENCHMARKS/Training/` ist nicht die Komponente `TRAINING/`

Die beiden heissen fast gleich und sind verschiedene Dinge:

| | was es ist | was drin liegt |
|---|---|---|
| `TRAINING/` (Wurzel) | eine **Komponente**, Code der ausgeliefert wird | die Kiste `myl-train` (Provenienz, Wachstum, Zuweisung) samt ihrer Planung und ihren Entwürfen |
| `BENCHMARKS/Training/` | **Messapparatur** | Datensätze und die Erzeuger dazu (Referenzleiter, Wissenssaat) |

⚑ **Sie gehören nicht zusammengelegt**, und zwar aus demselben Grund,
aus dem `INTEGER_LLM/eval/` am 2026-09-07 hierher gezogen ist:
**Messungen liegen beieinander, Code liegt bei seiner Komponente.** Wer
beides mischt, sucht eine Messung künftig wieder in fünf Ordnern.

**Was sie verbindet**, ist die Richtung des Verweises: Ein Trainingslauf
der Komponente misst mit den Datensätzen von hier, nicht umgekehrt.

## ⚑ Die eine Regel, die für jede Messung hier gilt

**Eine Haltemenge braucht einen Rauschnullpunkt, keinen Nullpunkt bei
null.**

Am 2026-09-07 hat dieses Projekt gemessen, dass **reines Rauschen** auf
ein quantisiertes Modell, ohne jeden Gradienten und ohne einen einzigen
Trainingsschritt, die Perplexität auf ungesehenem Text um **0,67
Prozent** senkt. Drei Trainingsläufe hatten zuvor zwischen 0,34 und 0,50
Prozent erreicht und galten als Lernerfolg.

Die Ursache ist Dithering: Die Gewichte sind mit **deterministischer**
Rundung quantisiert, ihre Rundungsfehler sind daher korreliert. Jede
Störung mit anschliessender Requantisierung zerlegt diese Korrelation
und verbessert das Modell, ganz gleich, wohin der Gradient zeigte.

⚑ **Wer gegen null misst, misst das Dithering mit.** Jede Aussage über
eine Haltemenge braucht deshalb einen Vergleichslauf mit **gleicher
Störung des int8-Gewichts und ohne Gradienten**. Was darüber
hinausgeht, ist Lernen. Was darunter liegt, ist Arithmetik.

## ⚑ Die zweite Regel: erfunden schlägt echt

Von keiner wahren Tatsache weiss man, ob das Modell sie schon aus dem
Vortraining kennt. Ein Rückgang der Perplexität wäre dann Erinnerung
statt Lernen. Die Sätze hier handeln deshalb von Personen, Orten und
Ereignissen, die es **nicht gibt**. Was ein Modell darüber weiss, hat es
in der gemessenen Runde gelernt.

## Was hier NICHT liegt

Die **Erzeugung** der Artefakte (Quantisierung, Kalibrierung) liegt
unter `INTEGER_LLM/calibrate/`, die Integrationstests unter
`INTEGER_LLM/tests/`. Hier steht, was **misst**, nicht was baut oder
prüft.

⚑ **Und datierte Laufberichte werden nicht hierher geräumt.** Sie
halten einen Stand zu einem Zeitpunkt fest, samt der Pfade, die damals
galten. Wer sie nachträglich umschreibt, macht aus einer Messung eine
Behauptung.
