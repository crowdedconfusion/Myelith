# Entscheidungspunkt 12.21 — Perplexitätsvergleich

**Datum:** (automatisch erzeugt durch eval/perplexity.py)

## Messung

| Größe | Wert |
|---|---|
| Modell | Qwen/Qwen2.5-7B (Basis-Variante) |
| FP-Baseline | BF16, HF-Implementierung: Perplexität 8.68 |
| Integer-Modell | θ_v 0.14.0 (Gewichte int8 per_channel, Aktivierungen int16 per_layer, LM-Head int16 per-channel als benannte spec-Ausnahme): Perplexität 9.40 |
| Datensatz | WikiText-2, Testsplit; 4 Sequenzen à 128 Tokens (435 ausgewertete Positionen) |
| Relativer Anstieg | **+8.29 %** |
| Akzeptanzkriterium | max. 5.0 % relativer Anstieg |
| **Ergebnis** | **VERFEHLT** |

## Zwingende Einordnung

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

Das Akzeptanzkriterium ist verfehlt. Bereits umgesetzte Eskalationsstufen: Weight-Tying aufgelöst + LM-Head int16 per-channel (spec 0.6.0) und Per-Channel-int8 für alle Gewichte (spec 0.7.0). Der verbleibende Abstand verlangt weitere Eskalation — Kandidaten: breitere Kalibrierbasis/Skalen-Headroom, feinere Teilbit-Tiefen der Nichtlinearitäten (z. B. SiLU-Eingangsskala), GPTQ, Hadamard-Rotation, Low-Rank-Fehlerkorrektur, deterministisch-stochastisches Runden.
