#!/usr/bin/env bash
# Richtet die Kalibrier-Umgebung ein: INTEGER_LLM/calibrate/.venv mit den
# festen Fassungen aus calibrate/requirements-fest.txt.
#
# Aufruf aus beliebigem Verzeichnis:
#
#     bash INTEGER_LLM/scripts/python_einrichten.sh
#
# Braucht Netz (rund 1 GB, vor allem torch) und entweder `uv` oder ein
# Python ab 3.10. Mit `uv` wird Python 3.12 bei Bedarf mitgeholt, sonst
# nimmt das Skript das neueste vorhandene Python ab 3.10.
#
# Wofuer: Gewichte holen (der Befehl `hf`), Artefakte kalibrieren (torch,
# transformers) und den Kalibriertext zusammenstellen (datasets). Der
# Betrieb der fertigen Artefakte braucht davon nichts.
#
# Liegt eine Umgebung schon da und bringt alles mit, tut das Skript nichts.
# Die letzte Zeile ist "Fertig" oder beginnt mit "FEHLER".
set -euo pipefail

INTEGER_LLM_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KALIBRIERUNG="${INTEGER_LLM_DIR}/calibrate"
UMGEBUNG="${KALIBRIERUNG}/.venv"
FEST="${KALIBRIERUNG}/requirements-fest.txt"
# ⚑ `--flashnext`: die zweite Umgebung fuer die Bauart mit vier Stroemen
#   (transformers ab 5.18.0, `requirements-flashnext.txt`). Die erste bleibt,
#   wie sie ist; die Begruendung steht im Kopf jener Datei.
PRUEFIMPORT="import torch, transformers, datasets, safetensors, huggingface_hub"
if [ "${1:-}" = "--flashnext" ]; then
    UMGEBUNG="${KALIBRIERUNG}/.venv-flashnext"
    FEST="${KALIBRIERUNG}/requirements-flashnext.txt"
    PRUEFIMPORT="${PRUEFIMPORT}, transformers.models.qwen4_exp"
elif [ -n "${1:-}" ]; then
    echo "FEHLER: unbekannter Schalter ${1} (bekannt: --flashnext)" >&2
    exit 2
fi
PY="${UMGEBUNG}/bin/python3"

vollstaendig() {
    [ -x "${PY}" ] && "${PY}" -c "${PRUEFIMPORT}" >/dev/null 2>&1 \
        && [ -x "${UMGEBUNG}/bin/hf" ]
}

if vollstaendig; then
    echo "[python] Die Umgebung liegt schon da: ${UMGEBUNG}"
    echo "Fertig"
    exit 0
fi

if [ -e "${UMGEBUNG}" ]; then
    # Eine halbe Umgebung (abgebrochene Einrichtung) wird nicht geflickt,
    # sondern neu angelegt: Was in ihr fehlt, ist nicht zu sehen.
    echo "[python] Unvollstaendige Umgebung unter ${UMGEBUNG}, sie wird neu angelegt"
    rm -rf "${UMGEBUNG}"
fi

if command -v uv >/dev/null 2>&1; then
    echo "[python] mit uv: Python 3.12 und die festen Fassungen"
    uv venv --python 3.12 "${UMGEBUNG}"
    uv pip install --python "${PY}" -r "${FEST}"
else
    GEFUNDEN=""
    for K in python3.13 python3.12 python3.11 python3.10 python3; do
        if command -v "${K}" >/dev/null 2>&1 \
            && "${K}" -c "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)" >/dev/null 2>&1; then
            GEFUNDEN="${K}"
            break
        fi
    done
    if [ -z "${GEFUNDEN}" ]; then
        echo "FEHLER: weder uv noch ein Python ab 3.10 gefunden." >&2
        echo "  uv:     https://docs.astral.sh/uv/ (eine Datei, holt Python selbst)" >&2
        echo "  Python: https://www.python.org/downloads/" >&2
        exit 1
    fi
    echo "[python] mit $("${GEFUNDEN}" --version): venv und pip"
    "${GEFUNDEN}" -m venv "${UMGEBUNG}"
    "${PY}" -m pip install --quiet --upgrade pip
    "${PY}" -m pip install -r "${FEST}"
fi

if ! vollstaendig; then
    echo "FEHLER: Die Umgebung ist angelegt, aber torch, transformers, datasets oder der Befehl hf fehlen." >&2
    exit 1
fi
echo "[python] $("${PY}" --version), torch $("${PY}" -c 'import torch; print(torch.__version__)')"
echo "Fertig"
