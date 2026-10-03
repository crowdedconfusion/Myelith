#!/bin/sh
# Sieht nach, ob FreeCAD fuer die CAD-Werkzeuge des Agenten da ist, und
# sagt sonst, wie es auf die Maschine kommt.
#
# ⚑ **FreeCAD gehoert nicht ins Repositorium.** Es ist ein fremdes
# Programm, je System verschieden und mehrere hundert Megabyte gross. Der
# Bau des Repositoriums braucht es nicht; ohne FreeCAD fehlen dem Agenten
# drei Werkzeuge und sonst nichts.
#
# ⚑ **Dieses Skript installiert nichts von selbst.** Ein Paketverwalter,
# der ungefragt mehrere hundert Megabyte holt, ist keine Pruefung mehr.
# Mit --holen ruft es den Paketverwalter des Systems.
#
# Aufruf:
#   sh SYSTEM/install/cad-einrichten.sh            nachsehen und pruefen
#   sh SYSTEM/install/cad-einrichten.sh --holen    ueber den Paketverwalter installieren
set -u

HIER=$(cd "$(dirname "$0")/../.." && pwd)
KISTE="$HIER/AGENT_LAYER/local-toolkits/CAD"
holen=0
[ "${1:-}" = "--holen" ] && holen=1

suchen() {
  if [ -n "${MYL_FREECAD:-}" ] && [ -x "$MYL_FREECAD" ]; then echo "$MYL_FREECAD"; return 0; fi
  for name in freecadcmd FreeCADCmd; do
    if command -v "$name" >/dev/null 2>&1; then command -v "$name"; return 0; fi
  done
  for pfad in \
    "/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd" \
    "$HOME/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd" \
    "/usr/lib/freecad/bin/freecadcmd"; do
    if [ -x "$pfad" ]; then echo "$pfad"; return 0; fi
  done
  return 1
}

if ! gefunden=$(suchen); then
  if [ "$holen" = 1 ]; then
    case "$(uname -s)" in
      Darwin)
        command -v brew >/dev/null 2>&1 || { echo "FEHLER: Homebrew fehlt; FreeCAD von freecad.org laden."; exit 1; }
        brew install --cask freecad || exit 1 ;;
      Linux)
        if command -v nix-env >/dev/null 2>&1; then nix-env -iA nixos.freecad || exit 1
        elif command -v apt-get >/dev/null 2>&1; then sudo apt-get install -y freecad || exit 1
        else echo "FEHLER: kein bekannter Paketverwalter; FreeCAD von freecad.org laden."; exit 1
        fi ;;
      *) echo "FEHLER: auf diesem System bitte FreeCAD von freecad.org laden."; exit 1 ;;
    esac
    gefunden=$(suchen) || { echo "FEHLER: FreeCAD ist installiert, aber freecadcmd nicht zu finden. MYL_FREECAD setzen."; exit 1; }
  else
    echo "FreeCAD fehlt. Die CAD-Werkzeuge des Agenten melden das und laufen nicht."
    echo "  macOS:   brew install --cask freecad     (oder freecad.org)"
    echo "  NixOS:   nix-env -iA nixos.freecad"
    echo "  Windows: winget install FreeCAD.FreeCAD  (oder freecad.org)"
    echo "Oder: sh SYSTEM/install/cad-einrichten.sh --holen"
    echo "Liegt es an einem anderen Ort: MYL_FREECAD auf freecadcmd setzen."
    exit 1
  fi
fi

echo "FreeCAD gefunden: $gefunden"
# ⚑ Gefunden ist nicht dasselbe wie laeuft: ein Probeteil bauen.
probe=$(mktemp -d "${TMPDIR:-/tmp}/myl-cad-probe.XXXXXX") || exit 1
trap 'rm -rf "$probe"' EXIT
cp "$HIER/AGENT_LAYER/local-skills/cad-erstellen/vorlagen/flansch.py" "$probe/flansch.py" || exit 1
if (cd "$probe" && sh "$KISTE/cad.sh" bauen flansch.py flansch) | tee "$probe/bericht.txt" | tail -n 3; then :; fi
if grep -q "^ERGEBNIS: gelungen" "$probe/bericht.txt" && [ -s "$probe/flansch.FCStd" ] && [ -s "$probe/flansch.step" ]; then
  echo "Fertig: die CAD-Werkzeuge laufen. Im Client die Werkzeugkiste CAD waehlen."
  exit 0
fi
echo "FEHLER: FreeCAD ist da, aber das Probeteil liess sich nicht bauen."
exit 1
