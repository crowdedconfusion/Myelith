#!/bin/sh
# Legt einen Anwendungsstarter mit Myelith-Symbol an. Fuer Linux.
#
# Warum ein Skript und keine mitgelieferte .desktop-Datei: Der Standard
# verlangt in `Exec` und `Icon` absolute Pfade. Eine im Repository abgelegte
# Datei zeigte auf den Rechner, auf dem sie erzeugt wurde. Dieses Skript
# setzt die Pfade dort ein, wo sie gebraucht werden.

set -eu

WURZEL=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)
STARTER="$WURZEL/Myelith Testclient - Linux, macOS (Shell).sh"
SYMBOL="$WURZEL/README/Grafiken/myelith-icon.png"
ZIEL="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
DATEI="$ZIEL/myelith-testclient.desktop"

if [ ! -x "$STARTER" ]; then
    echo "Fehler: $STARTER nicht gefunden oder nicht ausfuehrbar." >&2
    exit 1
fi

mkdir -p "$ZIEL"
cat > "$DATEI" <<ENDE
[Desktop Entry]
Type=Application
Name=Myelith Testclient
Comment=Hardwaretests, Bitgleichheit und geshardete Inferenz
Exec=$STARTER
Path=$WURZEL
Icon=$SYMBOL
Terminal=true
Categories=Development;Utility;
ENDE
chmod +x "$DATEI"

# Ohne Aktualisierung der Datenbank taucht der Eintrag in manchen
# Arbeitsumgebungen erst nach der naechsten Anmeldung auf.
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$ZIEL" >/dev/null 2>&1 || true
fi

echo "Eintrag angelegt: $DATEI"
echo "Er erscheint im Anwendungsmenue als \"Myelith Testclient\"."
