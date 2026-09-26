#!/bin/sh
# Legt ein gebautes Abbild als versionierten Stand ab.
#
#   sh SYSTEM/golemos/freigeben.sh --arch x86_64
#
# ⛔️ **Ein eigener Schritt, und das ist der ganze Punkt.**
#
# ⚑ **Festlegung des Projektinhabers vom 2026-09-24: Ein neues Abbild
# kommt nur bei einem BENANNTEN STAND herein, nicht nach jedem Bau.**
#
# Der Grund ist die Geschichte: Ein gepacktes Abbild wiegt 31 MB je
# Architektur, und was einmal in der Versionsverwaltung steht, bleibt
# dort. Nach jedem Bau freizugeben waere bei zwoelf Baeuen im Jahr ein
# Klon, der um 740 MB gewachsen ist, ohne dass jemand es gewollt hat.
#
# ⚑ **Deshalb schreibt `bauen.sh` nach `aus/`, und das ist ignoriert.**
# Nur dieses Skript fasst `abbild/fertig/` an. Wer baut, aendert nichts
# am versionierten Stand; wer freigibt, tut es mit Absicht.
set -eu

HIER=$(cd "$(dirname "$0")" && pwd)
ARCH=x86_64
GRUND=""

while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --grund) GRUND="$2"; shift 2 ;;
    -h|--hilfe) sed -n '2,22p' "$0"; exit 0 ;;
    *) echo "unbekannt: $1" >&2; exit 2 ;;
  esac
done

QUELLE="$HIER/aus/$ARCH/golemos.img"
ZIEL="$HIER/abbild/fertig/golemos-$ARCH.img.xz"

[ -f "$QUELLE" ] || {
  echo "Kein gebautes Abbild unter $QUELLE." >&2
  echo "Erst bauen:  sh SYSTEM/golemos/bauen.sh --arch $ARCH" >&2
  exit 1
}

# ⚠️ **Nachsehen, ob sich ueberhaupt etwas geaendert hat.** Ein
#    Freigeben, das denselben Inhalt noch einmal ablegt, kostet 31 MB
#    Geschichte fuer nichts.
if [ -f "$ZIEL" ]; then
  ALT=$(xz -dc "$ZIEL" 2>/dev/null | shasum -a 256 2>/dev/null | cut -d" " -f1 || true)
  NEU=$(shasum -a 256 "$QUELLE" 2>/dev/null | cut -d" " -f1 || true)
  if [ -n "$ALT" ] && [ "$ALT" = "$NEU" ]; then
    echo "── Der abgelegte Stand ist bitgleich mit dem gebauten."
    echo "   Nichts zu tun."
    exit 0
  fi
fi

# ⛔️ **Ein Grund gehoert dazu.** Ein 31-MB-Blob ohne Begruendung in der
#    Geschichte ist in einem halben Jahr nicht mehr einzuordnen.
if [ -z "$GRUND" ]; then
  echo "⛔️ Ein Grund fehlt." >&2
  echo "   sh $0 --arch $ARCH --grund \"was sich geaendert hat\"" >&2
  exit 3
fi

echo "── Packen, das dauert einen Moment"
mkdir -p "$(dirname "$ZIEL")"
xz -9 -c "$QUELLE" > "$ZIEL"

( cd "$(dirname "$ZIEL")" && \
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$(basename "$ZIEL")" > "$(basename "$ZIEL").sha256"
  else
    sha256sum "$(basename "$ZIEL")" > "$(basename "$ZIEL").sha256"
  fi )

cat <<ENDE

── Freigegeben
   $ZIEL
   $(du -h "$ZIEL" | cut -f1) gepackt, $(du -h "$QUELLE" | cut -f1) belegt
   $(cut -d" " -f1 < "$ZIEL.sha256")

⚠️  Jetzt gehoert HERKUNFT.md nachgezogen: Datum, Fassungen, und
    warum dieser Stand freigegeben wurde.

    Grund dieses Standes: $GRUND
ENDE
