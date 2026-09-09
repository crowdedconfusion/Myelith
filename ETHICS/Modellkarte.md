# Modellkarte (erzeugt)

> ⚑ **Diese Datei wird erzeugt, nicht geschrieben.**
> Quelle: `INTEGER_LLM/theta_v/spec.json` und
> `BENCHMARKS/Inferenz/results/`. Wer sie von Hand ändert, verliert
> die Änderung beim nächsten Lauf von
> `ETHICS/werkzeuge/modellkarte.py`.

**θ_v-Fassung:** `0.18.0`

## Ausführungsspezifikation

| Feld | Wert |
|---|---|
| Zahlenformat der Gewichte | *nicht gemessen* |
| Zahlenformat der Aktivierungen | *nicht gemessen* |
| Akkumulator | *nicht gemessen* |
| Nichtlinearitäten | *nicht gemessen* |
| Abtastung | `integer_cdf` |

## Gemessene Qualität gegen die Gleitkomma-Referenz

*nicht gemessen*, keine Datei unter `BENCHMARKS/Inferenz/results/`.

## Was diese Karte nicht sagt

⚑ **Sie sagt nichts über Eignung.** Wofür das Netz geeignet ist
und wofür nicht, steht in `ETHICS/Risikoklassen.toml`; das ist eine
Aussage über Vertraulichkeit und keine über Qualität.

⚑ **Und sie bewertet den Inhalt der Trainingsdaten nicht.**
Grundsatz G1 verbietet das ausdrücklich: Geprüft wird die Herkunft
eines Korpus, nicht seine Meinung. Die Herkunft steht im
Aufnahmeantrag, nicht hier.
