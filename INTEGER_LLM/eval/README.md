# eval/

Ablageort für die Qualitätsmessung des Integer-Modells: Gleitkomma-Baseline,
Perplexitätsberechnung und die dafür benötigten Datensätze. Eine
Qualitätsmessung ist kein Integrationstest (siehe `tests/`) und gehört
deshalb in ein eigenes Verzeichnis.

## Struktur

```
eval/
├── README.md
└── datasets/
    └── .gitignore          # Datensätze nicht versioniert, Verzeichnis behalten
```

`datasets/` nimmt die WikiText-2-Prompts auf, gegen die sowohl die
Gleitkomma-Baseline als auch das Integer-Modell gemessen werden. Der Inhalt
wird zur Laufzeit heruntergeladen beziehungsweise erzeugt und ist nicht
versioniert.

## Geplanter Inhalt (noch nicht implementiert)

- `baseline.py` — Gleitkomma-Baseline: Qwen2.5-0.5B in FP16 auf WikiText-2,
  identische Messmethode wie beim Integer-Modell
- `perplexity.py` — Perplexitätsberechnung für das Integer-Modell;
  Akzeptanzkriterium ist der relative Abstand zur Baseline

Der konkrete Schwellwert für den zulässigen relativen Abstand ist eine
Protokoll-Entscheidung (konsensrelevant) und bewusst noch nicht festgelegt.

## Entscheidungspunkt

Die Perplexitätsmessung ist der Entscheidungspunkt des Projekts: Erst wenn
das Integer-Modell die Gleitkomma-Baseline um höchstens den festgelegten
relativen Abstand überschreitet, ist die Kernthese bestätigt, dass
vollständig ganzzahlige Inferenz qualitativ trägt — darauf bauen die
weiteren Backends (SIMD, CUDA, ROCm) und die Verifikationsarchitektur auf.
