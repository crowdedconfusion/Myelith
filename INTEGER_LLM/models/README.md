# models/

Ablageort für das Quellmodell, aus dem die θ_v-Artefakte entstehen.
Zweck: reproduzierbare Herkunft statt implizitem Hugging-Face-Cache.

Der Inhalt wird nicht versioniert (siehe `.gitignore`); nur dieses README und
die `.gitignore` bleiben im Repository.

## Modell

- **Modell:** Qwen/Qwen2.5-0.5B
- **Quelle:** Hugging Face Hub (`https://huggingface.co/Qwen/Qwen2.5-0.5B`)
- **Revision:** `060db6499f32faf8b98477b0a26969ef7d8b9987` — am 2026-08-11 per
  `scripts/fetch_model.sh` neu geholt und aufgelöst (der ursprüngliche manuelle
  Download hatte keine dokumentierte Revision). Für reproduzierbare Läufe:
  `REVISION=060db6499f32faf8b98477b0a26969ef7d8b9987`.
- **Lizenz des Basismodells:** Apache-2.0 laut HF-Modellkarte (Angabe ohne
  Rechtsprüfung; die Lizenzlage für quantisierte Ableitungen ist Gegenstand
  einer separaten, nicht-technischen Klärung)

## Erwartete Struktur

```
models/
├── .gitignore
├── README.md
└── Qwen2.5-0.5B/           # vollständiger HF-Snapshot, zur Laufzeit geholt
    ├── config.json
    ├── generation_config.json
    ├── model.safetensors
    ├── tokenizer.json
    └── ...
```

## Beschaffung

Der Download erfolgt mit fixierter Revision über `scripts/fetch_model.sh`;
der dabei ausgegebene Commit-Hash wird oben als Revision eingetragen. Die
Kalibrierung (`calibrate/`) liest das Modell ausschließlich aus diesem
Verzeichnis (nie aus dem impliziten Hugging-Face-Cache) und exportiert die
θ_v-Artefakte nach `artifacts/`.

## Pfad

Zentrale Pfadkonstante: `MODELS_DIR` in `runtime/src/paths.rs`.
