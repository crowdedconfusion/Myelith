# Entscheidungspunkt 12.21: Perplexitätsvergleich

**Datum:** (automatisch erzeugt durch BENCHMARKS/Inferenz/perplexity.py)

## Messung

| Größe | Wert |
|---|---|
| Modell | Qwen/Qwen3-0.6B (Basis-Variante) |
| FP-Baseline | BF16, HF-Implementierung: Perplexität 42.26 |
| Integer-Modell | θ_v 0.21.0 (Gewichte int8 per_channel, Aktivierungen int16 per_layer, LM-Head int16 per-channel als benannte spec-Ausnahme): Perplexität 45.15 |
| Datensatz | WikiText-2, Testsplit; 32 Sequenzen à 128 Tokens (3558 ausgewertete Positionen) |
| Relativer Anstieg | **+6.84 %** |
| Akzeptanzkriterium | max. 5.0 % relativer Anstieg |
| **Ergebnis** | **VERFEHLT** |

## Zwingende Einordnung

1. **Decodierstrategie:** Perplexität ist unabhängig von der
   Decodierstrategie, die beobachtete Repetitionsneigung nicht; Greedy
   verstärkt sie. Die hier gemessene Perplexität (Teacher-Forcing) ist
   daher das maßgebliche Qualitätsmaß; die in Fund 9 beobachteten
   Repetitions-Loops bei Greedy-Generierung sind ein Teil-Decodier-
   strategie-Effekt und nicht allein der Quantisierung zuzurechnen.
2. **Kleine Modelle sind der ungünstigste Fall für Quantisierung.**
   Gemessen wurde hier ein Modell mit 0,6 Mrd. Parametern. Größere
   Modelle sind nachweislich robuster (größere Logit-Spannweiten,
   gutmütigere Gewichtsverteilungen). Falls das Kriterium verfehlt wurde: Das ist ein Urteil über dieses Modell, nicht über die Zielgrößenordnung des Whitepapers.

## Konsequenz

Das Akzeptanzkriterium ist verfehlt. Bereits umgesetzte Eskalationsstufen: Weight-Tying aufgelöst + LM-Head int16 per-channel (spec 0.6.0) und Per-Channel-int8 für alle Gewichte (spec 0.7.0). Der verbleibende Abstand verlangt weitere Eskalation, Kandidaten: breitere Kalibrierbasis/Skalen-Headroom, feinere Teilbit-Tiefen der Nichtlinearitäten (z. B. SiLU-Eingangsskala), GPTQ, Hadamard-Rotation, Low-Rank-Fehlerkorrektur, deterministisch-stochastisches Runden.
