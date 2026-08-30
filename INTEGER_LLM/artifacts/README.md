# artifacts/

Ablageort für alle θ_v-Artefakte: exportierte, vollständig ganzzahlige
Modellgewichte einschließlich Skalen, Lookup-Tabellen und Tokenizer.

Der Inhalt wird zur Laufzeit erzeugt und ist nicht versioniert (siehe
`.gitignore`). Maßgeblich für das Format ist `theta_v/spec.json`.

## Struktur

```
artifacts/
├── .gitignore
├── README.md
└── <modell>/                    # z. B. qwen2.5-0.5b, zur Laufzeit erzeugt
    ├── theta_v.json             # θ_v-Manifest (Version, SHA-256-Hashes der Artefakte)
    ├── weights_manifest.json    # Tensor-Manifest (Name, Form, Hash; seit θ_v 0.7.0
    │                            # Sentinel scale:-1.0/shift:-1 plus shifts_file/shifts_hash)
    ├── <tensor_name>.bin        # Gewichte, eine Datei pro Tensor
    │                            # (raw int8, row-major; HF-Tensorname, Punkte
    │                            # durch Unterstriche ersetzt)
    ├── <tensor_name>_shifts.bin # Per-Channel-Zweierpotenz-Shifts (raw int8,
    │                            # ein Shift je Ausgabe-Zeile; bei 1D-Tensoren
    │                            # je Element) — θ_v 0.7.0
    ├── lm_head.bin              # benannte spec-Ausnahme (θ_v 0.6.0): eigener
    │                            # Tensor in raw int16, row-major (Weight-Tying
    │                            # aufgelöst), dazu lm_head_shifts.bin
    ├── model_config.json        # Modellparameter (inkl. attention_bias,
    │                            # lm_head-Ausnahme)
    ├── scales.json              # Per-Layer-Aktivierungsskalen (Zweierpotenzen)
    ├── luts.json                # LUT-Manifest
    ├── <name>.lut.bin           # Lookup-Tabellen (exp, silu, sin, cos, rsqrt),
    │                            # flach im Artefakt-Verzeichnis
    └── tokenizer.json           # Tokenizer-Export
```

## Herkunft

Die Artefakte entstehen ausschließlich über den Kalibrierungs- und
Export-Workflow (`calibrate/`, gebündelt in `scripts/build_artifacts.sh`)
aus dem Quellmodell in `models/`. Artefakte werden nicht von Hand gepflegt
und nicht heruntergeladen.

## Pfad

Zentrale Pfadkonstante ist `ARTIFACTS_DIR` in `runtime/src/paths.rs`
(Python-Spiegelbild: `calibrate/src/paths.py`); calibrate, runtime und
pipeline verwenden denselben Ablageort. Der Pfad ist über die
Umgebungsvariable `INTEGER_LLM_ARTIFACTS_DIR` überschreibbar, damit die
Gewichte nicht neben dem Binary liegen müssen.
