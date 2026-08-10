# eval/

Ablageort fuer die Qualitaetsmessung des Integer-Modells: Gleitkomma-Baseline,
Perplexitaetsberechnung und die dafuer benoetigten Datensaetze. Eine
Qualitaetsmessung ist kein Integrationstest (siehe `tests/`) und gehoert
deshalb in ein eigenes Verzeichnis.

## Struktur

```
eval/
├── README.md
└── datasets/
    └── .gitignore          # Datensaetze nicht versioniert, Verzeichnis behalten
```

`datasets/` nimmt die WikiText-2-Prompts auf, gegen die sowohl die
Gleitkomma-Baseline als auch das Integer-Modell gemessen werden. Der Inhalt
wird zur Laufzeit heruntergeladen beziehungsweise erzeugt und ist nicht
versioniert.

## Geplanter Inhalt (noch nicht Teil dieses Punkts)

- `baseline.py` — Gleitkomma-Baseline: Qwen2.5-0.5B FP16 auf WikiText-2,
  identische Messmethode wie das Integer-Modell (Fahrplan-Punkt 12.20)
- `perplexity.py` — Perplexitaetsberechnung fuer das Integer-Modell,
  Akzeptanzkriterium als relativer Abstand zur Baseline (Fahrplan-Punkt 12.21)

Der konkrete Schwellwert fuer den relativen Abstand ist konsensrelevant
(siehe Fahrplan-Abschnitt „Nicht Teil dieses Fahrplans“).

## Entscheidungspunkt

Fahrplan-Punkte 12.18–12.21 sind der Entscheidungspunkt des Projekts: Erst
wenn die Perplexitaet des Integer-Modells die Baseline um hoechstens den
festgelegten relativen Abstand ueberschreitet, folgen die weiteren Backends
(SIMD, CUDA, ROCm).
