# Modellkarte (erzeugt)

> ⚑ **Diese Datei wird erzeugt, nicht geschrieben.**
> Quelle: `INTEGER_LLM/theta_v/spec.json` und
> `INTEGER_LLM/eval/results/`. Wer sie von Hand ändert, verliert
> die Änderung beim nächsten Lauf von
> `ETHICS/werkzeuge/modellkarte.py`.

**θ_v-Fassung:** `0.17.0`

## Ausführungsspezifikation

| Feld | Wert |
|---|---|
| Zahlenformat der Gewichte | *nicht gemessen* |
| Zahlenformat der Aktivierungen | *nicht gemessen* |
| Akkumulator | *nicht gemessen* |
| Nichtlinearitäten | *nicht gemessen* |
| Abtastung | `integer_cdf` |

## Gemessene Qualität gegen die Gleitkomma-Referenz

| Basismodell | Datensatz | Token | Perplexität | Quelle |
|---|---|---|---|---|
| Qwen/Qwen2.5-0.5B (HF, BF16) | wikitext-2-raw-v1 (Testsplit) | 435 | 14.953218401528314 | `baseline_wikitext2.json` |
| Qwen/Qwen2.5-7B (HF, BF16) | wikitext-2-raw-v1 (Testsplit) | 435 | 8.681428252995742 | `baseline_wikitext2_qwen25-7b.json` |
| Qwen/Qwen3-30B-A3B (HF, BF16) | wikitext-2-raw-v1 (Testsplit) | 435 | 10.48255448892767 | `baseline_wikitext2_qwen3-30b-a3b.json` |
| Qwen/Qwen3-4B (HF, BF16) | wikitext-2-raw-v1 (Testsplit) | 435 | 19.62817430289872 | `baseline_wikitext2_qwen3-4b.json` |

## Was diese Karte nicht sagt

⚑ **Sie sagt nichts über Eignung.** Wofür das Netz geeignet ist
und wofür nicht, steht in `ETHICS/Risikoklassen.toml`; das ist eine
Aussage über Vertraulichkeit und keine über Qualität.

⚑ **Und sie bewertet den Inhalt der Trainingsdaten nicht.**
Grundsatz G1 verbietet das ausdrücklich: Geprüft wird die Herkunft
eines Korpus, nicht seine Meinung. Die Herkunft steht im
Aufnahmeantrag, nicht hier.
