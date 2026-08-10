# artifacts/

Ablageort fuer alle theta_v-Artefakte: exportierte, vollstaendig ganzzahlige
Modellgewichte einschliesslich Skalen, Lookup-Tabellen und Tokenizer.

Der Inhalt wird zur Laufzeit erzeugt und ist nicht versioniert (siehe
`.gitignore`). Massgeblich fuer das Format ist `theta_v/spec.json`.

## Struktur

```
artifacts/
├── .gitignore
├── README.md
└── <modell>/                 # z. B. qwen2.5-0.5b, zur Laufzeit erzeugt
    ├── theta_v.json          # theta_v-Manifest mit SHA-256-Hashes
    ├── weights_manifest.json # Tensor-Manifest (Name, Form, Skala, Hash)
    ├── scales.json           # Per-Layer Aktivierungsskalen (Zweierpotenzen)
    ├── layer_*.bin           # INT8-Gewichte, raw int8, row-major
    ├── luts/*.lut.bin        # Lookup-Tabellen (exp, silu, sin, cos, rsqrt)
    └── tokenizer.json        # Tokenizer-Export
```

## Herkunft

Die Artefakte entstehen ausschliesslich ueber den Kalibrierungs- und
Export-Workflow (`calibrate/`, ab Fahrplan-Punkt 12.7 gebuendelt in
`scripts/build_artifacts.sh`) aus dem Quellmodell in `models/`. Artefakte
werden nicht von Hand gepflegt und nicht heruntergeladen.

## Pfad

Zentrale Pfadkonstante ist `ARTIFACTS_DIR` in `runtime/src/paths.rs`;
calibrate, runtime und pipeline verwenden denselben Ablageort. Der Pfad ist
ueber die Umgebungsvariable `INTEGER_LLM_ARTIFACTS_DIR` ueberschreibbar
(wichtig fuer Container-Deployment, siehe `deploy/docker-compose.yml`).
