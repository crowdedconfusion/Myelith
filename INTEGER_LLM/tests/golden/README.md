# Golden Vectors

Golden Vectors sind die normative Wahrheit fuer alle numerischen Ergebnisse.

## Struktur

```
tests/golden/
├── generate.py   # Erzeugt Vektoren aus der Python-Referenz
├── validate.py   # Prueft ein Backend gegen die Vektoren
└── vectors/
    ├── op/        # Operation-Level (rmsnorm, linear, softmax, ...)
    ├── layer/     # Layer-Level (kompletter Transformer-Block)
    └── e2e/       # End-to-End (Prompt -> Token-Sequenz)
```

- `*.golden.json` – JSON-Dateien mit Inputs, erwarteten Outputs und Hashes
- Der Ablageort ist in beiden Skripten als Pfadkonstante hinterlegt
  (`VECTORS_DIR` in `generate.py`, `VECTORS_DIRNAME`/`LEVELS` in `validate.py`).

## Regeln

1. Nur das **Referenz-Backend** darf Golden Vectors erzeugen.
2. Jedes optimierte Backend muss **alle** Golden Vectors bestehen.
3. Bei Aenderung von theta_v muessen **alle** Golden Vectors neu generiert werden.
4. Golden Vectors werden im CI gegen alle Backends geprueft.

## Verwendung
