# Perplexität der Modellreihe

> ⚙️ **Erzeugt von `BENCHMARKS/Inferenz/uebersicht.py`.**
> Nicht von Hand bearbeiten: Der nächste Lauf überschreibt die Datei.
> Stand: 2026-09-12

Gemessen wird Perplexität auf WikiText-2 mit Teacher-Forcing, für
beide Pfade auf **identischen Sequenzen**; niedriger ist besser.
Der **Abstand** ist der relative Aufschlag des Ganzzahlpfads auf
seine eigene Gleitkomma-Referenz, nicht auf ein anderes Modell.

| Modell | Gleitkomma | Ganzzahl | Positionen | Abstand | Kriterium ≤ 5 % |
|---|---|---|---|---|---|
| Myelith 0,6B | 31,86 | 33,28 | 435 | **+4,47 %** | erfüllt |
| Myelith 4B | 19,63 | 19,95 | 435 | **+1,64 %** | erfüllt |
| Myelith 30B-A3B | 10,48 | 10,42 | 435 | **kein messbarer Abstand** | erfüllt |

## Was daraus folgt

⚑ **Je kleiner das Modell, desto teurer die Quantisierung**, und
das ist keine Vermutung mehr, sondern über die ganze Reihe
gemessen. Das kleinste Modell liegt als einziges nennenswert nahe
an der Grenze; wer es wählt, wählt auch das, bei dem die
Quantisierung am meisten kostet.

⚠️ **Was diese Tabelle nicht sagt.** Die Zahlen einer Zeile lassen
sich mit denen einer anderen **nicht** vergleichen: Jede misst
gegen ihre eigene Referenz. Ein Modell mit 33,28 ist nicht
schlechter als eines mit 11,54, sondern kleiner.

⚠️ **Der Boden des Quantisierungsschemas fehlt hier.** Er ist an
einem Modell gemessen, das nicht mehr Teil des Projekts ist, und
für diese Reihe nicht neu bestimmt.

## Wo die Einzelbelege stehen

Je Modell ein Entscheidungsprotokoll und ein Vergleich, beide
erzeugt von `perplexity.py`:

- **Myelith 0,6B**: `decision_12-21_myelith-06b.md`, `perplexity_comparison_myelith-06b.json`
- **Myelith 4B**: `decision_12-21_myelith-4b.md`, `perplexity_comparison_myelith-4b.json`
- **Myelith 30B-A3B**: `decision_12-21_myelith-30b-a3b.md`, `perplexity_comparison_myelith-30b-a3b.json`

⚑ **Die Einzeldateien bleiben, und das ist Absicht.** Jede ist der
Beleg ihres Modells, mit Methode, Datensatz und Einordnung. Diese
Übersicht ersetzt sie nicht, sie erspart nur das Nebeneinanderlegen.

⚠️ **Ältere Dateien ohne Modellnamen** (`decision_12-21.md`,
`baseline_wikitext2.json`, `perplexity_comparison.json`) gehören zu
Qwen2.5-0,5B und bleiben als Aufzeichnung stehen. Das Modell ist
seit dem 2026-09-11 aus dem Projekt heraus; seine Messung ist in
Whitepaper-Vorarbeit und Changelog zitiert.
