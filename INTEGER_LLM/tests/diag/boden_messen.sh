#!/bin/sh
# Misst den Boden des Quantisierungsschemas fuer ein Modell.
#
# Aufruf aus der Wurzel:
#
#     sh INTEGER_LLM/tests/diag/boden_messen.sh myelith-30b-a3b
#
# ⚑ **Warum es diesen Mantel gibt und nicht nur die eine Zeile.** Beim
# 30B-Gemisch laeuft die Messung Stunden: Die BF16-Referenz ist 57 GB
# gross, die Maschine hat 24 GiB, also laagert sie durchgehend aus, und
# die Simulation macht ausser den Perplexitaetslaeufen noch 64
# Kalibrierdurchgaenge. Ein solcher Lauf soll ein Protokoll hinterlassen,
# das Schliessen des Fensters ueberleben und vorher pruefen, ob er
# ueberhaupt allein auf der Maschine ist.
#
# ⛔️ **Ein zweiter Lauf daneben ist keine Ersparnis, sondern ein
# verlorener Abend.** Zwei Modelllaeufe nebeneinander haben diese
# Maschine schon auf 53 MB freien Speicher gebracht.
set -eu

MODELL="${1:-}"
if [ -z "$MODELL" ]; then
    echo "Aufruf: sh INTEGER_LLM/tests/diag/boden_messen.sh <modell>" >&2
    echo "Zum Beispiel: myelith-0.6b, myelith-4b, myelith-30b-a3b" >&2
    exit 2
fi

WURZEL=$(cd "$(dirname "$0")/../../.." && pwd)
PYTHON="$WURZEL/INTEGER_LLM/calibrate/.venv/bin/python3"
SKRIPT="$WURZEL/INTEGER_LLM/tests/diag/w8a16_reference_simulation.py"
ERGEBNISSE="$WURZEL/BENCHMARKS/Inferenz/results"
PROTOKOLL="$ERGEBNISSE/boden_${MODELL}.log"

if [ ! -x "$PYTHON" ]; then
    echo "Es fehlt die Kalibrier-Umgebung: $PYTHON" >&2
    exit 1
fi

# ⚑ Ohne den Ganzzahl-Vergleich dieses Modells gibt es keinen Abstand,
# und das faellt sonst erst nach Stunden auf.
VERGLEICH="$ERGEBNISSE/perplexity_comparison_$(echo "$MODELL" | tr -d '.').json"
if [ ! -f "$VERGLEICH" ]; then
    echo "Es fehlt der Ganzzahl-Vergleich fuer $MODELL:" >&2
    echo "  $VERGLEICH" >&2
    echo "Erst perplexity.py fuer dieses Modell laufen lassen." >&2
    exit 1
fi

# Laeuft schon eine Messung? Der eigene Prozess zaehlt nicht mit.
if pgrep -f "w8a16_reference_simulation.py" >/dev/null 2>&1; then
    echo "Es laeuft bereits eine Bodenmessung. Erst die abwarten." >&2
    exit 1
fi

echo "Modell:     $MODELL"
echo "Protokoll:  $PROTOKOLL"
echo "Ergebnis:   $ERGEBNISSE/schema_boden_$(echo "$MODELL" | tr -d '.').json"
echo
echo "Laeuft im Hintergrund weiter, auch wenn das Fenster zugeht."
echo "Mitlesen:   tail -f \"$PROTOKOLL\""
echo

cd "$WURZEL/INTEGER_LLM"
INTEGER_LLM_MODEL="$MODELL" nohup "$PYTHON" "$SKRIPT" >"$PROTOKOLL" 2>&1 &
echo "Gestartet als Prozess $!."
