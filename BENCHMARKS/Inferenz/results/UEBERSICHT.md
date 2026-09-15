# Perplexität der Modellreihe

> ⚙️ **Erzeugt von `BENCHMARKS/Inferenz/uebersicht.py`.**
> Nicht von Hand bearbeiten: Der nächste Lauf überschreibt die Datei.
> Stand: 2026-09-15

Gemessen wird Perplexität auf WikiText-2 mit Teacher-Forcing, für
beide Pfade auf **identischen Sequenzen**; niedriger ist besser.
Der **Abstand** ist der relative Aufschlag des Ganzzahlpfads auf
seine eigene Gleitkomma-Referenz, nicht auf ein anderes Modell.

| Modell | Gleitkomma | Ganzzahl | Positionen | Abstand | Kriterium ≤ 5 % |
|---|---|---|---|---|---|
| Myelith 0,6B | 31,86 | 33,29 | 435 | **+4,48 %** | erfüllt |
| Myelith 4B | 19,63 | 19,95 | 435 | **+1,65 %** | erfüllt |
| Myelith 30B-A3B | 10,48 | 10,42 | 435 | **kein messbarer Abstand** | erfüllt |

## Was daraus folgt

⚑ **Je kleiner das Modell, desto grösser der gemessene Abstand**,
und das ist keine Vermutung mehr, sondern über die ganze Reihe
gemessen. Das kleinste Modell liegt als einziges nennenswert nahe
an der Grenze; wer es wählt, wählt auch das, bei dem der Abstand
am grössten ist.

⛔️ **Der Abstand allein sagt nicht, woher er kommt.** Wieviel
das Quantisierungsschema selbst kostet, steht im nächsten
Abschnitt; der Rest ist Umsetzung.


## Der Boden des Schemas

⚑ **Was das Verfahren selbst kostet**, gemessen mit W8A16 und
sonst Gleitkomma (`INTEGER_LLM/tests/diag/w8a16_reference_simulation.py`).
Der Rest des Abstands oben ist Umsetzung.

⛔️ **Diese Zahlen gehören nicht in die Tabelle oben.** Sie sind
über eine andere, grössere Stichprobe gemessen, also über
anderen Text. Eine Differenz zwischen beiden Tabellen wäre eine
erfundene Zahl; das Messwerkzeug weigert sich deshalb, sie zu
bilden. **Für einen belastbaren Umsetzungsverlust muss auch der
Ganzzahlpfad über denselben Umfang gemessen werden**, und das
steht aus.

📌 **Warum so viele Positionen.** Auf den 435 der Tabelle oben
ergab dieselbe Messung beim 4B einen Boden von −2,45 %, also
eine Quantisierung, die das Modell verbessert. Auf 13 797
Positionen blieb davon −0,07 %. Ein einzelnes Token, dessen
Wahrscheinlichkeit um eine Grössenordnung springt, verschiebt
bei 435 Positionen die Perplexität schon um ein halbes Prozent.

| Modell | Positionen | Gleitkomma | nur Gewichte (W8) | Boden (W8A16) |
|---|---|---|---|---|
| Myelith 0,6B | 13797 | 42,31 | 42,99 | 42,99 (+1,61 %) |
| Myelith 0,6B | 435 | 31,86 | 32,04 | 32,12 (+0,80 %) |
| Myelith 4B | 13797 | 28,09 | 28,07 | 28,07 (−0,07 %) |
| Myelith 4B | 435 | 19,63 | 19,39 | 19,15 (−2,45 %) |

## Wo die Einzelbelege stehen

Je Modell ein Entscheidungsprotokoll und ein Vergleich, beide
erzeugt von `perplexity.py`:

- **Myelith 0,6B**: `decision_12-21_myelith-06b.md`, `perplexity_comparison_myelith-06b.json`
- **Myelith 4B**: `decision_12-21_myelith-4b.md`, `perplexity_comparison_myelith-4b.json`
- **Myelith 30B-A3B**: `decision_12-21_myelith-30b-a3b.md`, `perplexity_comparison_myelith-30b-a3b.json`

⚑ **Die Einzeldateien bleiben, und das ist Absicht.** Jede ist der
Beleg ihres Modells, mit Methode, Datensatz und Einordnung. Diese
Übersicht ersetzt sie nicht, sie erspart nur das Nebeneinanderlegen.

## Abgelöste Modelle (Aufzeichnung)

⚑ **Eine Messung verfällt nicht, weil das Modell geht.** Diese
Größen sind nicht mehr Teil des Projekts; ihre Zahlen sind
gemessen, mit derselben Methode und auf denselben 435
Positionen, und in Whitepaper-Vorarbeit und Changelog zitiert.
Sie stehen hier, damit man sie findet, ohne zu wissen, dass es
sie gibt.

⛔️ **Nicht mit der Tabelle oben verrechnen.** Zwei der drei
stammen aus einer anderen Modellfamilie (Qwen2.5); ein
Größenvergleich über die Grenze hinweg misst den
Familienunterschied mit.

| Modell | Gleitkomma | Ganzzahl | Positionen | Abstand | Boden des Schemas |
|---|---|---|---|---|---|
| Qwen/Qwen2.5-0.5B | 14,95 | 15,27 | 435 | **+2,11 %** | *keine Datei* |
| Qwen/Qwen3-14B | 11,41 | 11,54 | 435 | **+1,16 %** | *keine Datei* |
| Qwen/Qwen2.5-7B | 8,68 | 8,78 | 435 | **+1,14 %** | *keine Datei* |

⚠️ **Der Boden von Qwen2.5-7B (+0,84 %) hat keine Ergebnisdatei.**
Er wurde am 2026-08-20 gemessen, als das Werkzeug sein Ergebnis
nur ausgab und nicht ablegte; die Zahl steht im Ergebnisblock
von `INTEGER_LLM/README/README.md` und gilt dort. Hier bleibt
die Zelle leer, statt eine Datei zu erfinden, die es nicht gibt.

⚠️ **Die Dateien ohne Modellnamen** (`decision_12-21.md`,
`baseline_wikitext2.json`, `perplexity_comparison.json`) gehören
zu Qwen2.5-0,5B und behalten ihre Namen mit Absicht: Sie sind
unter diesen Namen zitiert.
