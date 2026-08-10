#!/usr/bin/env bash
# Laedt das Quellmodell mit fixierter Revision nach models/<Name>/.
# Siehe models/README.md fuer Herkunft, Struktur und die Revision-Angabe.
#
# Env-Variablen (optional):
#   MODEL_ID  HF-Modell-ID, Default: Qwen/Qwen2.5-0.5B-Instruct
#   REVISION  HF-Revision (Branch, Tag oder Commit-Hash), Default: main
#
# Ohne fixierte REVISION ist der Download nicht reproduzierbar. Das Skript
# loest die tatsaechliche Commit-Revision auf und gibt sie am Ende aus --
# dieser Hash gehoert danach in models/README.md unter "Revision".

set -euo pipefail

MODEL_ID="${MODEL_ID:-Qwen/Qwen2.5-0.5B-Instruct}"
REVISION="${REVISION:-main}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${REPO_ROOT}/models/$(basename "${MODEL_ID}")"

if ! command -v huggingface-cli >/dev/null 2>&1; then
    echo "[fetch_model] huggingface-cli nicht gefunden." >&2
    echo "[fetch_model] 'pip install -r calibrate/requirements.txt' ausfuehren." >&2
    exit 1
fi

echo "[fetch_model] Lade ${MODEL_ID}@${REVISION} nach ${TARGET_DIR} ..."
mkdir -p "${TARGET_DIR}"
huggingface-cli download "${MODEL_ID}" --revision "${REVISION}" --local-dir "${TARGET_DIR}"

RESOLVED_COMMIT="$(python3 -c "
from huggingface_hub import HfApi
info = HfApi().model_info('${MODEL_ID}', revision='${REVISION}')
print(info.sha)
")"

echo "[fetch_model] Fertig: ${TARGET_DIR}"
echo "[fetch_model] Aufgeloeste Revision: ${RESOLVED_COMMIT}"
echo "[fetch_model] Diesen Commit-Hash in models/README.md unter 'Revision' eintragen,"
echo "[fetch_model] damit kuenftige Laeufe (REVISION=${RESOLVED_COMMIT}) reproduzierbar sind."
