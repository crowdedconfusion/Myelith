# models/

Ablageort fuer das Quellmodell, aus dem die theta_v-Artefakte entstehen.
Zweck: reproduzierbare Herkunft statt implizitem Hugging-Face-Cache.

Der Inhalt wird nicht versioniert (siehe `.gitignore`); nur dieses README und
die `.gitignore` bleiben im Repositorium.

## Modell

- **Modell:** Qwen/Qwen2.5-0.5B
- **Quelle:** Hugging Face Hub (`https://huggingface.co/Qwen/Qwen2.5-0.5B`)
- **Revision:** unbekannt — das Modell wurde vor Einrichtung von
  `scripts/fetch_model.sh` manuell heruntergeladen; der Commit-Hash ist nicht
  mehr rekonstruierbar (HF-Cache ohne Revisionsangabe, geprüft am 2026-08-11).
  Vor dem ersten echten Kalibrierungslauf (Punkt 12.15) das Modell per
  `scripts/fetch_model.sh` neu holen und den ausgegebenen Commit-Hash hier
  eintragen.
- **Lizenz des Basismodells:** Apache-2.0 laut HF-Modellkarte (Angabe ohne
  Rechtsprüfung; die Lizenzlage fuer quantisierte Ableitungen ist Bestandteil
  des Fahrplan-Abschnitts „Nicht Teil dieses Fahrplans“)

## Erwartete Struktur

```
models/
├── .gitignore
├── README.md
└── Qwen2.5-0.5B/           # vollstaendiger HF-Snapshot, zur Laufzeit geholt
    ├── config.json
    ├── generation_config.json
    ├── model.safetensors
    ├── tokenizer.json
    └── ...
```

## Beschaffung

Der Download erfolgt mit fixierter Revision ueber `scripts/fetch_model.sh`
(Fahrplan-Punkt 12.7); der dabei ausgegebene Commit-Hash wird oben als
Revision eingetragen. Der aktuelle lokale Stand stammt noch aus einem
manuellen Download ohne dokumentierte Revision (siehe oben). Die Kalibrierung
(`calibrate/`) liest das Modell aus diesem Verzeichnis und exportiert die
theta_v-Artefakte nach `artifacts/`.

## Pfad

Zentrale Pfadkonstante: `MODELS_DIR` in `runtime/src/paths.rs`.
