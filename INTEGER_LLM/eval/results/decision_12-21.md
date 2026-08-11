# Entscheidungspunkt 12.21 — Perplexitätsvergleich

**Datum:** (automatisch erzeugt durch eval/perplexity.py)

## Messung

| Größe | Wert |
|---|---|
| Modell | Qwen/Qwen2.5-0.5B (Basis-Variante) |
| FP-Baseline | BF16, HF-Implementierung: Perplexität 14.95 |
| Integer-Modell | θ_v 0.5.2 (W8-Gewichte, int16-Aktivierungen, Per-Layer-Skalen): Perplexität 14546.38 |
| Datensatz | WikiText-2, Testsplit; 4 Sequenzen à 128 Tokens (435 ausgewertete Positionen) |
| Relativer Anstieg | **+97179.25 %** |
| Akzeptanzkriterium | max. 5.0 % relativer Anstieg (Vorschlag des Fahrplans) |
| **Ergebnis** | **VERFEHLT** |

## Zwingende Einordnung (Fahrplan-Vorgabe)

1. **Decodierstrategie:** Perplexität ist unabhängig von der
   Decodierstrategie, die beobachtete Repetitionsneigung nicht — Greedy
   verstärkt sie. Die hier gemessene Perplexität (Teacher-Forcing) ist
   daher das maßgebliche Qualitätsmaß; die in Fund 9 beobachteten
   Repetitions-Loops bei Greedy-Generierung sind ein Teil-Decodier-
   strategie-Effekt und nicht allein der Quantisierung zuzurechnen.
2. **0,5 Mrd. Parameter sind der ungünstigste Fall für Quantisierung.**
   Größere Modelle sind nachweislich robuster (größere Logit-Spannweiten,
   gutmütigere Gewichtsverteilungen). Falls das Kriterium verfehlt wurde: Das ist ein Urteil über 0,5B — nicht über die Zielgrößenordnung des Whitepapers.

## Konsequenz

Das Akzeptanzkriterium ist verfehlt. Vor Fortsetzung ist gemäß Fahrplan zu klären, welcher Eskalationspfad verfolgt wird (priorisiert: 1. Weight-Tying aufbrechen (Embedding int8/LM-Head int16) + 2. Per-Channel-Skalen für den LM-Head; weitere: GPTQ, Hadamard-Rotation, Low-Rank-Fehlerkorrektur, deterministisch-stochastisches Runden). Die Eskalationsstrategien sind im Fahrplan (Abschnitt zu Phase 12.18–12.21) dokumentiert.
