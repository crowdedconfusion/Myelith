#!/usr/bin/env bash
# Fuehrt Kalibrierung und Export in einem Lauf aus: models/ -> artifacts/.
# Voraussetzung: Quellmodell liegt bereits unter models/ (siehe fetch_model.sh).
# Artefakte landen unter artifacts/<modell>/ (calibrate/src/paths.py,
# ueberschreibbar per INTEGER_LLM_ARTIFACTS_DIR wie auf der Rust-Seite).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

# ⛔️ **Erst den Interpreter suchen, dann rechnen.**
#
# 📌 Hier stand blankes `python3`. Auf einem frischen Klon ist das der
# Interpreter des Systems, und der hat kein `torch`: Der Lauf endete in
# einem nackten `ModuleNotFoundError`, aus dem niemand ablesen kann, dass
# eine Umgebung fehlt. **Eine Voraussetzung, die erst beim Absturz
# sichtbar wird, ist keine Voraussetzung, sondern eine Falle.**
PY_KANDIDATEN="${PYTHON:-} calibrate/.venv/bin/python3 python3"
PY=""
for K in ${PY_KANDIDATEN}; do
    if [ -n "${K}" ] && command -v "${K}" >/dev/null 2>&1 \
        && "${K}" -c "import torch, transformers" >/dev/null 2>&1; then
        PY="${K}"
        break
    fi
done

if [ -z "${PY}" ]; then
    cat >&2 <<'FEHLT'
[build_artifacts] FEHLER: kein Python mit torch und transformers gefunden.

Gesucht wurde in dieser Reihenfolge:
  $PYTHON, calibrate/.venv/bin/python3, python3

Die Umgebung entsteht so (sie braucht Netz und einige Gigabyte):
  cd INTEGER_LLM/calibrate
  uv venv --python 3.12 .venv
  uv pip install --python .venv/bin/python3 -r requirements.txt

⚑ Fuer den Artefaktbau ist das unvermeidlich: Die Gewichte kommen als
  Gleitkomma-Tensoren, und die liest das Oekosystem von torch. Der
  **Betrieb** der fertigen Artefakte braucht davon nichts.
FEHLT
    exit 1
fi

echo "[build_artifacts] Interpreter: ${PY}"
echo "[build_artifacts] Starte Kalibrierungs- und Export-Workflow ..."
"${PY}" -m calibrate.src.main

echo "[build_artifacts] Fertig. Artefakte liegen unter artifacts/ (siehe artifacts/README.md)."
