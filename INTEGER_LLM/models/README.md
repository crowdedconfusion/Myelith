# models/

Ablageort für das Quellmodell, aus dem die θ_v-Artefakte entstehen.
Zweck: reproduzierbare Herkunft statt implizitem Hugging-Face-Cache.

Der Inhalt wird nicht versioniert (siehe `.gitignore`); nur dieses README und
die `.gitignore` bleiben im Repository.

## Modelle

Jede Variante braucht eine **eigene Lizenzprüfung** (Whitepaper Kap. 10.1,
ETHICS-Grundsatz G7: Apache 2.0 oder MIT) und eine **fixierte Revision** —
ohne beides ist der Lauf weder zulässig noch reproduzierbar. Es werden
ausschließlich **Basis-Varianten** verwendet, keine Instruct-Varianten
(Scope-Entscheidung 12.15).

| Modell | Revision | Lizenz | Größe | Stand |
|---|---|---|---|---|
| [Qwen/Qwen2.5-0.5B](https://huggingface.co/Qwen/Qwen2.5-0.5B) | `060db6499f32faf8b98477b0a26969ef7d8b9987` | Apache-2.0 | 1,9 GB | lokal vorhanden |
| [Qwen/Qwen2.5-7B](https://huggingface.co/Qwen/Qwen2.5-7B) | `d149729398750b98c0af14eb82c78cfe92750796` | Apache-2.0 | 15,2 GB | **noch nicht geholt** (Fahrplan 12.73) |

Die 0.5B-Revision wurde am 2026-08-11 per `scripts/fetch_model.sh` neu
geholt und aufgelöst; der ursprüngliche manuelle Download hatte keine
dokumentierte Revision. Die 7B-Angaben stammen aus der HF-API und der
`config.json` der Variante (geprüft am 2026-08-18, festgehalten in
`tests/test_export_workflow.py::test_7b_config_matches_published_hf_config`).

**Zur Lizenzangabe:** Apache-2.0 laut Modellkarte, ohne eigene
Rechtsprüfung. Die Lizenzlage **quantisierter Ableitungen** ist Gegenstand
einer separaten, nicht-technischen Klärung (siehe
`README/Intern/State-of-the-Project.md`, Abschnitt 7).

## Erwartete Struktur

```
models/
├── .gitignore
├── README.md
├── Qwen2.5-0.5B/           # vollständiger HF-Snapshot, zur Laufzeit geholt
│   ├── config.json
│   ├── model.safetensors
│   ├── tokenizer.json
│   └── ...
└── Qwen2.5-7B/             # dito; hier vier safetensors-Teile + index.json
    ├── config.json
    ├── model-0000{1..4}-of-00004.safetensors
    ├── model.safetensors.index.json
    └── ...
```

## Beschaffung

Der Download erfolgt mit fixierter Revision über `scripts/fetch_model.sh`;
der dabei ausgegebene Commit-Hash wird oben als Revision eingetragen:

```bash
MODEL_ID=Qwen/Qwen2.5-7B REVISION=d149729398750b98c0af14eb82c78cfe92750796 \
  scripts/fetch_model.sh
```

Die Kalibrierung wählt die Variante über `INTEGER_LLM_MODEL` (Vorgabe
`qwen2.5-0.5b`) und legt je Modell ein eigenes Artefaktverzeichnis an:

```bash
INTEGER_LLM_MODEL=qwen2.5-7b python -m calibrate.src.main
```

Die
Kalibrierung (`calibrate/`) liest das Modell ausschließlich aus diesem
Verzeichnis (nie aus dem impliziten Hugging-Face-Cache) und exportiert die
θ_v-Artefakte nach `artifacts/`.

## Pfad

Zentrale Pfadkonstante: `MODELS_DIR` in `runtime/src/paths.rs`.
