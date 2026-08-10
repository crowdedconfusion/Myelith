#!/usr/bin/env bash
# Fuehrt Kalibrierung und Export in einem Lauf aus: models/ -> artifacts/.
# Voraussetzung: Quellmodell liegt bereits unter models/ (siehe fetch_model.sh).
# Artefakte landen unter artifacts/<modell>/ (calibrate/src/paths.py,
# ueberschreibbar per INTEGER_LLM_ARTIFACTS_DIR wie auf der Rust-Seite).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

echo "[build_artifacts] Starte Kalibrierungs- und Export-Workflow ..."
python3 -m calibrate.src.main

echo "[build_artifacts] Fertig. Artefakte liegen unter artifacts/ (siehe artifacts/README.md)."
