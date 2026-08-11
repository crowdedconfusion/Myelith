# Golden Vectors

Golden Vectors sind die normative Wahrheit für alle numerischen Ergebnisse.

## Struktur

```
tests/golden/
├── generate.py   # Erzeugt Vektoren aus der Python-Referenz
├── validate.py   # Prüft ein Backend gegen die Vektoren
└── vectors/
    ├── op/        # Operation-Level (rmsnorm, linear, softmax, ...)
    ├── layer/     # Layer-Level (kompletter Transformer-Block)
    └── e2e/       # End-to-End (Prompt -> Token-Sequenz)
```

- `*.golden.json` — JSON-Dateien mit Inputs, erwarteten Outputs und Hashes
- Der Ablageort ist in beiden Skripten als Pfadkonstante hinterlegt
  (`VECTORS_DIR` in `generate.py`, `VECTORS_DIRNAME`/`LEVELS` in `validate.py`).

## Regeln

1. Nur das **Referenz-Backend** darf Golden Vectors erzeugen.
2. Jedes optimierte Backend muss **alle** Golden Vectors bestehen.
3. Bei Änderung von θ_v müssen **alle** Golden Vectors neu generiert werden.
4. Golden Vectors werden im CI gegen alle Backends geprüft.

## Verwendung

Vektoren erzeugen:

```bash
python3 tests/golden/generate.py
```

Ein Backend gegen die Vektoren prüfen:

```bash
python3 tests/golden/validate.py <backend> tests/golden/vectors
```

Zum aktuellen Stand enthalten die Vektoren synthetische Referenzwerte; die
Neu-Generierung aus echten Modellwerten (Layer- und E2E-Vektoren mit realen
Wertebereichen und Ausreißern) folgt in einer späteren Ausbaustufe — das
Modell ist inzwischen vollständig kalibriert, die Vektoren selbst stammen
aber noch aus der Frühphase.
