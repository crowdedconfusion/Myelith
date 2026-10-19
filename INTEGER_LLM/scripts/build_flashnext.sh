#!/usr/bin/env bash
# Baut Qwen3.8-Flash-Next (Bauart mit vier Stroemen) zu einem Artefakt: das
# allgemeine Bauskript (`modell_bauen.sh`) mit festgehaltener Revision und
# festem Namen. Es laedt die Quelle, kalibriert ebenenweise, exportiert in
# int8, laedt zur Probe und packt die Experten in vier Bit.
#
#     bash INTEGER_LLM/scripts/build_flashnext.sh --probe   # kurzer Lauf zuerst
#     bash INTEGER_LLM/scripts/build_flashnext.sh           # der lange Lauf
#
# ⛔️ Lizenz `qwen-community-1.0`: nur lokal. Inferenz fuer Dritte (das Netz
# von Myelith) braucht eine eigene Lizenz von Qwen. Das Artefakt traegt
# `betrieb = "lokal"`, und der Knoten laedt es nicht.
#
# Ergebnis: INTEGER_LLM/artifacts/myelith-125b-a6b (int8, geschaetzt rund
# 176 GB) und INTEGER_LLM/artifacts/myelith-125b-a6b-int4 (Experten in vier
# Bit, geschaetzt rund 118 GB, davon 51 GB N-Gramm-Tabelle in int8, nur
# nachgeschlagen).
#
# ⚑ **Der kurze Lauf** (`--probe`): Er holt nur die Konfiguration, den
# Tokenizer und die Dateien der Einbettung und der ersten Ebene (rund 10 GB),
# richtet die Umgebung ein, prueft den Eintrag gegen die Quelle und rechnet
# die erste Ebene ueber den ganzen Kalibrierkorpus. Geschrieben wird nichts.
# Erst wenn er mit "Fertig" endet, lohnt der lange.
#
# ⚑ **Platz**: BF16 360 GB, int8 rund 176 GB, fuer den Bau in vier Bit
# Mitschnitt, Behaelter und Ziel zusammen rund 255 GB. Mit
# `FLASHNEXT_QUELLE_LOESCHEN=1` (oder `--quelle-loeschen`) wird die
# BF16-Quelle geloescht, sobald das int8-Artefakt geladen und gerechnet hat.
# Geprueft wird vorher.
#
# Dauer (geschaetzt, nicht gemessen): Laden je nach Leitung, Kalibrierung eine
# bis wenige Stunden (jede Gewichtsdatei wird einmal gelesen), Mitschnitt fuer
# den Bau in vier Bit mehrere Stunden (16 384 Token, Token fuer Token).
#
# Weitere Schalter gehen an das Bauskript durch (etwa `--ja`).
#
# ⚠️ Das Skript nicht bearbeiten, waehrend es laeuft: Die Shell liest es
# stueckweise. Die letzte Zeile ist "Fertig" oder beginnt mit "FEHLER".
set -euo pipefail

INTEGER_LLM_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Die Revision, gegen die `model_configs.py` den Eintrag verifiziert hat.
REVISION="de4b8e4d43b917e7706784d8bb445c9af86a3540"

ZUSATZ=()
[ -n "${FLASHNEXT_QUELLE_LOESCHEN:-}" ] && ZUSATZ+=(--quelle-loeschen)

exec bash "${INTEGER_LLM_DIR}/scripts/modell_bauen.sh" \
    --repo Qwen/Qwen3.8-Flash-Next --revision "${REVISION}" \
    --name myelith-125b-a6b-int4 --projekt "${ZUSATZ[@]+"${ZUSATZ[@]}"}" "$@"
