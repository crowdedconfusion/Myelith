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
# ⚑ Eine Umgebung fuer alle Bauarten (seit dem 2026-10-21; davor gab es mit
#   `--flashnext` eine zweite fuer die Bauart mit vier Stroemen, die Gruende
#   der Zusammenlegung stehen im Kopf von `requirements-fest.txt`). Der
#   Schalter wird noch angenommen und tut nichts mehr.
PRUEFIMPORT="import torch, transformers, datasets, safetensors, huggingface_hub, transformers.models.qwen4_exp"
if [ "${1:-}" = "--flashnext" ]; then
    echo "[python] --flashnext ist nicht mehr noetig: eine Umgebung fuer alle Bauarten"
elif [ -n "${1:-}" ]; then
    echo "FEHLER: unbekannter Schalter ${1}" >&2
    exit 2
fi
PY="${UMGEBUNG}/bin/python3"
# 📌 Eine Umgebung mit anderer Fassung von transformers ist nicht fertig, auch
#   wenn alles importiert: Sonst bliebe nach dem Heben der festen Fassungen
#   die alte liegen, und der Bau liefe still mit ihr.
SOLL_TRANSFORMERS="$(sed -n 's/^transformers==//p' "${FEST}")"

vollstaendig() {
    [ -x "${PY}" ] && "${PY}" -c "${PRUEFIMPORT}" >/dev/null 2>&1 \
        && [ -x "${UMGEBUNG}/bin/hf" ] \
        && [ "$("${PY}" -c 'import transformers; print(transformers.__version__)')" = "${SOLL_TRANSFORMERS}" ]
}

if vollstaendig; then
    echo "[python] Die Umgebung liegt schon da: ${UMGEBUNG}"
    echo "Fertig"
    exit 0
fi

if [ -e "${UMGEBUNG}" ]; then
    # Eine halbe oder veraltete Umgebung (abgebrochene Einrichtung, andere
    # Fassung) wird nicht geflickt, sondern neu angelegt: Was in ihr fehlt
    # oder anders ist, ist nicht zu sehen.
    echo "[python] Unvollstaendige oder veraltete Umgebung unter ${UMGEBUNG} (transformers soll ${SOLL_TRANSFORMERS}), sie wird neu angelegt"
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
