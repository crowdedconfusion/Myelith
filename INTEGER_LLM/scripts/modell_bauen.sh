#!/usr/bin/env bash
# Ein Modell aus einem Repositorium bauen, benennen und lokal einpflegen.
# Fuer versierte Nutzer mit Admin-Rechten; alles Weitere steht im Kopf von
# INTEGER_LLM/tools/modell_bauen.py.
#
#     bash INTEGER_LLM/scripts/modell_bauen.sh --probe     # kurzer Lauf zuerst
#     bash INTEGER_LLM/scripts/modell_bauen.sh             # fragt Repositorium und Namen ab
#
# ⛔️ Was hier entsteht, ist privat und lokal: nicht fuer den Netzbetrieb
# zugelassen. Die letzte Zeile ist "Fertig" oder beginnt mit "FEHLER".
set -euo pipefail
INTEGER_LLM_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# ⚑ "Fertig" allein auf einer Zeile schreibt nur das Bauskript, als letzte:
#   Ein Waechter wartet darauf (dasselbe in `laufen` im Bauskript).
bash "${INTEGER_LLM_DIR}/scripts/python_einrichten.sh" | tail -n 1 | sed 's/^Fertig$/[bauen] Umgebung bereit/'
exec "${INTEGER_LLM_DIR}/calibrate/.venv/bin/python3" "${INTEGER_LLM_DIR}/tools/modell_bauen.py" "$@"
