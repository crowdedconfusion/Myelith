#!/bin/sh
# Richtet die Umgebung der Buchkette ein, **ohne Netz**.
#
# ⚑ Die Kette laeuft auch ohne diese Umgebung: EPUB, HTML und Text
# brauchen nur die Standardbibliothek. Das hier ist fuer PDF.
#
# ⛔️ **Kein Zugriff nach draussen.** `pip` bekommt `--no-index`, es
# nimmt also ausschliesslich die Raeder aus `raeder/`. Ein frischer Klon
# auf einer Maschine ohne Netz kommt damit zum selben Ergebnis wie einer
# mit Netz, und das ist der Sinn der Sache.
set -eu

HIER=$(cd "$(dirname "$0")" && pwd)
PY=${PYTHON:-python3}

echo "== Buchkette einrichten =="

# 1. Der Interpreter, und ob er reicht.
"$PY" "$HIER/umgebung.py" >/dev/null || exit 1
echo "Interpreter: $("$PY" -c 'import sys; print(sys.executable)')"

# 2. Die Umgebung.
if [ -d "$HIER/.venv" ]; then
    echo "Umgebung: $HIER/.venv (vorhanden)"
else
    "$PY" -m venv "$HIER/.venv"
    echo "Umgebung: $HIER/.venv (neu)"
fi
VENV_PY="$HIER/.venv/bin/python3"

# 3. Die Raeder, ohne Netz.
if ! "$VENV_PY" -m pip install --quiet --no-index --find-links "$HIER/raeder" \
        -r "$HIER/requirements.txt"; then
    echo "FEHLER: die Raeder unter raeder/ passen nicht zu requirements.txt." >&2
    # ⛔️ **Ohne `--no-deps`, und das ist der Punkt.** Genau dieser
    # Schalter hat die transitive Abhaengigkeit verschluckt: `pypdf`
    # braucht unter Python vor 3.11 noch `typing_extensions`, und ohne
    # das Rad dazu scheitert der netzlose Lauf. **Ein Hinweis, der den
    # Fehler wiederholt, der ihn ausgeloest hat, ist schlimmer als
    # keiner.**
    echo "        Nachlegen MIT Abhaengigkeiten, auf einer Maschine mit Netz:" >&2
    echo "        python3 -m pip download -r requirements.txt -d raeder" >&2
    echo "        (und zwar mit der aeltesten Python-Fassung, die getragen werden soll:" >&2
    echo "         unter 3.11 kommt ein Rad mehr dazu)" >&2
    exit 1
fi
echo "Pakete:     aus $HIER/raeder, ohne Netz"

# 4. Was damit geht.
echo "Stand:      $("$VENV_PY" "$HIER/umgebung.py" | sed 's/^\[buchkette\] //')"

# 5. Und ob es wirklich geht.
echo
echo "== Selbsttests =="
FEHLER=0
for W in buch_zu_md buchkorpus md_zu_mappe; do
    if "$VENV_PY" "$HIER/$W.py" --selbsttest >/dev/null 2>&1; then
        echo "  $W: bestanden"
    else
        echo "  $W: GESCHEITERT"
        FEHLER=$((FEHLER + 1))
    fi
done

if [ "$FEHLER" -gt 0 ]; then
    echo
    echo "FEHLER: $FEHLER Selbsttest(s) gescheitert." >&2
    exit 1
fi

echo
echo "Fertig. Die Werkzeuge laufen mit:"
echo "  $VENV_PY $HIER/buch_zu_md.py <datei> ..."
