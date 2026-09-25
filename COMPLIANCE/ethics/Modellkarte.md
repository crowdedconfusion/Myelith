# Modellkarte (erzeugt)

> ⚑ **Diese Datei wird erzeugt, nicht geschrieben.**
> Quelle: `INTEGER_LLM/theta_v/spec.json` und
> `BENCHMARKS/Inferenz/results/`. Wer sie von Hand ändert, verliert
> die Änderung beim nächsten Lauf von
> `COMPLIANCE/ethics/werkzeuge/modellkarte.py`.

**θ_v-Fassung:** `0.22.0`

## Ausführungsspezifikation

| Feld | Wert |
|---|---|
| Zahlenformat der Gewichte | `int8` |
| Zahlenformat der Aktivierungen | `int16` |
| Akkumulator | `int64` |
| Nichtlinearitäten | rsqrt: `lut`, silu: `lut`, softmax: `lut_exp`, rope: `lut_sin_cos`, sigmoid: `lut`, softplus_rest: `lut_plus_linear`, zerfall_exp: `lut_plus_reihe`, silu_zerlegt: `decomposition` |
| Abtastung | `integer_cdf` |

## Gemessene Qualität gegen die Gleitkomma-Referenz

| Modell | Datensatz | Positionen | ganzzahlig | Gleitkomma | Abstand | Quelle |
|---|---|---|---|---|---|---|
| myelith-0.6b | wikitext-2-raw-v1 (Testsplit) | 3558 | 43,49 | 42,26 | 2,92 % | `perplexity_comparison_myelith-06b.json` |
| myelith-30b-a3b | wikitext-2-raw-v1 (Testsplit) | 435 | 10,42 | 10,48 | -0,59 % | `perplexity_comparison_myelith-30b-a3b.json` |
| myelith-4b | wikitext-2-raw-v1 (Testsplit) | 435 | 19,95 | 19,63 | 1,65 % | `perplexity_comparison_myelith-4b.json` |
| myelith-8b | wikitext-2-raw-v1 (Testsplit) | 435 | 13,27 | 12,79 | 3,75 % | `perplexity_comparison_myelith-8b.json` |

## Was diese Karte nicht sagt

⚑ **Sie sagt nichts über Eignung.** Wofür das Netz geeignet ist
und wofür nicht, steht in `COMPLIANCE/ethics/Risikoklassen.toml`; das ist eine
Aussage über Vertraulichkeit und keine über Qualität.

⚑ **Und sie bewertet den Inhalt der Trainingsdaten nicht.**
Grundsatz G1 verbietet das ausdrücklich: Geprüft wird die Herkunft
eines Korpus, nicht seine Meinung. Die Herkunft steht im
Aufnahmeantrag, nicht hier.
