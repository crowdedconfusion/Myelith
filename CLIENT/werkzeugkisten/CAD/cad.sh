#!/bin/sh
# Ruft FreeCAD ohne Fenster fuer die drei CAD-Werkzeuge dieser Kiste.
#
#   sh cad.sh bauen <skript.py> <name>
#   sh cad.sh pruefen <datei.FCStd>
#   sh cad.sh parameter <datei.FCStd> "<name>=<wert>; ..."
#
# ⚑ FreeCAD ist ein fremdes Programm und liegt nicht im Repositorium.
#   Fehlt es, sagt dieses Skript, wie es auf die Maschine kommt, und
#   sonst nichts: Das Werkzeug fehlt dann, der Agent laeuft weiter.
#
# ⚑ Die Aufgabe geht ueber die Umgebung an den Laeufer und nicht ueber
#   die Kommandozeile: FreeCAD liest seine Argumente selbst und hielte
#   einen Dateinamen fuer eine Datei, die es oeffnen soll.
set -u

hier=$(cd "$(dirname "$0")" && pwd)

finde_freecad() {
  if [ -n "${MYL_FREECAD:-}" ] && [ -x "$MYL_FREECAD" ]; then
    echo "$MYL_FREECAD"
    return 0
  fi
  for name in freecadcmd FreeCADCmd freecadcmd-daily; do
    if command -v "$name" >/dev/null 2>&1; then
      command -v "$name"
      return 0
    fi
  done
  for pfad in \
    "/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd" \
    "$HOME/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd" \
    "/Applications/FreeCAD.app/Contents/MacOS/FreeCADCmd" \
    "/usr/lib/freecad/bin/freecadcmd" \
    "/c/Program Files/FreeCAD 1.0/bin/FreeCADCmd.exe" \
    "/c/Program Files/FreeCAD 1.1/bin/FreeCADCmd.exe"; do
    if [ -x "$pfad" ]; then
      echo "$pfad"
      return 0
    fi
  done
  return 1
}

if [ "$#" -lt 2 ]; then
  echo "FEHLER: cad.sh braucht eine Aufgabe (bauen, pruefen, parameter) und ihre Angaben."
  exit 2
fi

freecad=$(finde_freecad) || {
  echo "FEHLER: FreeCAD ist auf dieser Maschine nicht zu finden."
  echo "Einrichten: sh SYSTEM/install/cad-einrichten.sh (im Repositorium), oder FreeCAD ab 1.0"
  echo "von freecad.org installieren. Liegt es woanders: MYL_FREECAD auf freecadcmd setzen."
  exit 3
}

# ⚑ Der Laeufer wird gelesen und ausgefuehrt, nicht als Datei uebergeben:
#   FreeCAD legte sonst seinen Bytecode (`__pycache__`) in die Kiste, also
#   in einen Ordner, der dem Werkzeug nicht gehoert.
bericht=$(mktemp "${TMPDIR:-/tmp}/myl-cad-bericht.XXXXXX") || exit 4
lauf=$(mktemp "${TMPDIR:-/tmp}/myl-cad-lauf.XXXXXX") || exit 4
trap 'rm -f "$bericht" "$lauf"' EXIT

MYL_CAD_AUFGABE="$1" MYL_CAD_A="${2:-}" MYL_CAD_B="${3:-}" MYL_CAD_BERICHT="$bericht" MYL_CAD_KISTE="$hier" \
  "$freecad" -c 'import os, sys; sys.dont_write_bytecode = True; exec(compile(open(os.path.join(os.environ["MYL_CAD_KISTE"], "cad_lauf.py"), encoding="utf-8").read(), "cad_lauf.py", "exec"))' </dev/null >"$lauf" 2>&1
rc=$?

if [ -s "$bericht" ]; then
  cat "$bericht"
  # Der Laeufer schreibt als letzte Zeile, ob es gelungen ist.
  if tail -n 1 "$bericht" | grep -q "^ERGEBNIS: gelungen"; then
    exit 0
  fi
  exit 1
fi

# ⚠️ Kein Bericht heisst: FreeCAD ist nicht bis zum Laeufer gekommen oder
#   abgestuerzt. Dann ist seine eigene Ausgabe alles, was es gibt.
echo "FEHLER: FreeCAD hat keinen Bericht geschrieben (Rueckgabewert $rc). Seine letzte Ausgabe:"
tail -n 20 "$lauf"
exit 1
