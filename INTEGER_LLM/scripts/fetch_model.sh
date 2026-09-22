#!/usr/bin/env bash
# Laedt das Quellmodell mit fixierter Revision nach MODELS/llm/<Name>/.
# Siehe MODELS/llm/README.md fuer Herkunft, Struktur und die Revision-Angabe.
#
# Env-Variablen (optional):
#   MODEL_ID  HF-Modell-ID, Default: Qwen/Qwen3-0.6B (das Ankermodell
#             des Projekts, siehe MODELS/llm/README.md)
#   REVISION  HF-Revision (Branch, Tag oder Commit-Hash). Default ist
#             die fixierte Revision des Ankermodells, NICHT `main`.
#
# ⚑ **Warum die Vorgabe eine feste Revision ist** (2026-09-11): Mit
# `main` holt ein frischer Klon, was heute dort liegt, und das ergibt
# andere Gewichte, andere Artefakte und einen anderen θ_v-Hash als die
# hier dokumentierten Zahlen. Wer ein anderes Modell holt, setzt
# REVISION ausdruecklich mit; die Revisionen stehen in
# MODELS/llm/KATALOG.json.
#
# Ohne fixierte REVISION ist der Download nicht reproduzierbar. Das Skript
# loest die tatsaechliche Commit-Revision auf und gibt sie am Ende aus --
# dieser Hash gehoert danach in MODELS/llm/KATALOG.json unter
# "hf_revision"; das README entsteht daraus.

set -euo pipefail

MODEL_ID="${MODEL_ID:-Qwen/Qwen3-0.6B}"
REVISION="${REVISION:-c1899de289a04d12100db370d81485cdf75e47ca}"

# ⚑ **Zwei Basen, seit dem Umzug der Gewichte am 2026-09-21.**
# `INTEGER_LLM_DIR` ist die Komponente, `WURZEL` das Repositorium
# darueber; die Quellmodelle liegen an der Wurzel unter `MODELS/llm`.
INTEGER_LLM_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WURZEL="$(cd "${INTEGER_LLM_DIR}/.." && pwd)"
TARGET_DIR="${WURZEL}/MODELS/llm/$(basename "${MODEL_ID}")"

if ! command -v hf >/dev/null 2>&1; then
    echo "[fetch_model] hf-CLI nicht gefunden." >&2
    echo "[fetch_model] 'pip install -r calibrate/requirements.txt' ausfuehren (huggingface_hub >= 1.x stellt den 'hf'-Befehl bereit)." >&2
    exit 1
fi

echo "[fetch_model] Lade ${MODEL_ID}@${REVISION} nach ${TARGET_DIR} ..."
mkdir -p "${TARGET_DIR}"
hf download "${MODEL_ID}" --revision "${REVISION}" --local-dir "${TARGET_DIR}"

RESOLVED_COMMIT="$(python3 -c "
from huggingface_hub import HfApi
info = HfApi().model_info('${MODEL_ID}', revision='${REVISION}')
print(info.sha)
")"

echo "[fetch_model] Fertig: ${TARGET_DIR}"
echo "[fetch_model] Aufgeloeste Revision: ${RESOLVED_COMMIT}"
echo "[fetch_model] Diesen Commit-Hash in MODELS/llm/KATALOG.json unter 'hf_revision' eintragen,"
echo "[fetch_model] damit kuenftige Laeufe (REVISION=${RESOLVED_COMMIT}) reproduzierbar sind."
