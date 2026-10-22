#!/bin/sh
# Die Bauumgebung aus dem Werkzeugvorrat in den Nix-Speicher legen, ohne Netz.
#
# Aufruf, einmal je Rechner, als root:
#   sudo sh SYSTEM/install/nix-vorrat-einspielen.sh
# danach wie immer:
#   sh SYSTEM/install/installieren-linux.sh
#
# ⚑ **Warum root.** Die Pfade aus dem Cache von NixOS tragen dessen
# Unterschrift, die selbst gebauten (die Quellen der Flake-Eingaben, die
# Huelle der Umgebung) keine. Ohne Unterschrift nimmt der Nix-Dienst einen
# Pfad nur von einem vertrauten Nutzer an, und das ist root. Die Pruefsummen
# jedes Archivs prueft Nix trotzdem, gegen die `.narinfo` daneben.
#
# ⚑ **Es aendert nichts an der Systemkonfiguration**, es legt nur Pfade in
# den Speicher, wie ein `nix build` es auch taete.
set -eu

cd "$(dirname "$0")/../.."
WURZEL=$(pwd)

case "$(uname -m)" in
  x86_64)        SYSTEM=x86_64-linux ;;
  aarch64|arm64) SYSTEM=aarch64-linux ;;
  *) echo "Fuer $(uname -m) gibt es keinen Nix-Vorrat." >&2; exit 2 ;;
esac
QUELLE="$WURZEL/SYSTEM/werkzeug-vorrat/nix/$SYSTEM"

if [ ! -f "$QUELLE/UMGEBUNG" ]; then
  echo "Kein Nix-Vorrat fuer $SYSTEM unter $QUELLE." >&2
  echo "Er entsteht mit Netz: python3 SYSTEM/install/werkzeugvorrat.py holen --nur nix" >&2
  exit 1
fi
if [ "$(id -u)" -ne 0 ]; then
  echo "Das braucht root (siehe Kopf dieser Datei):" >&2
  echo "    sudo sh $WURZEL/SYSTEM/install/nix-vorrat-einspielen.sh" >&2
  exit 2
fi

M="--extra-experimental-features nix-command --extra-experimental-features flakes"
echo "── Nix-Vorrat fuer $SYSTEM einspielen"
echo "   Quelle: $QUELLE"
# shellcheck disable=SC2086
nix $M copy --all --no-check-sigs --from "file://$QUELLE"

UMGEBUNG=$(cat "$QUELLE/UMGEBUNG")
if [ ! -e "$UMGEBUNG" ]; then
  echo "   FEHLER: Die Bauumgebung liegt danach nicht im Speicher: $UMGEBUNG" >&2
  exit 1
fi
echo "   Bauumgebung im Speicher: $UMGEBUNG"
echo "── Fertig. Weiter, ohne Netz: sh SYSTEM/install/installieren-linux.sh"
