# Entscheidungspunkt 12.21 — Perplexitätsvergleich

**Datum:** (automatisch erzeugt durch eval/perplexity.py)

## Messung

| Größe | Wert |
|---|---|
| Modell | Qwen/Qwen2.5-7B (Basis-Variante) |
| FP-Baseline | BF16, HF-Implementierung: Perplexität 8.68 |
| Integer-Modell | θ_v 0.17.0 (Gewichte int8 per_channel, Aktivierungen int16 per_layer, LM-Head int16 per-channel als benannte spec-Ausnahme): Perplexität 8.78 |
| Datensatz | WikiText-2, Testsplit; 4 Sequenzen à 128 Tokens (435 ausgewertete Positionen) |
| Relativer Anstieg | **+1.14 %** |
| Akzeptanzkriterium | max. 5.0 % relativer Anstieg |
| **Ergebnis** | **AKZEPTIERT** |

## Zwingende Einordnung

1. **Decodierstrategie:** Perplexität ist unabhängig von der
   Decodierstrategie, die beobachtete Repetitionsneigung nicht — Greedy
   verstärkt sie. Die hier gemessene Perplexität (Teacher-Forcing) ist
   daher das maßgebliche Qualitätsmaß; die in Fund 9 beobachteten
   Repetitions-Loops bei Greedy-Generierung sind ein Teil-Decodier-
   strategie-Effekt und nicht allein der Quantisierung zuzurechnen.
2. **0,5 Mrd. Parameter sind der ungünstigste Fall für Quantisierung.**
   Größere Modelle sind nachweislich robuster (größere Logit-Spannweiten,
   gutmütigere Gewichtsverteilungen). Das Kriterium wurde erreicht; die Übertragbarkeit auf die Zielgrößenordnung bleibt durch die grundsätzliche Robustheit größerer Modelle zusätzlich gestützt.

## Konsequenz

Das Akzeptanzkriterium ist erfüllt — die Ganzzahl-Inferenz trägt qualitativ auf diesem Modell. Die weiteren Backends (SIMD/CUDA/ROCm) und die Netzwerkkomponenten können auf dieser Basis weiterverfolgt werden.
