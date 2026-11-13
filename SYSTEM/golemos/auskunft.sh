#!/bin/sh
# Die Lizenzauskunft und die Quellen zu einem gebauten Abbild.
#
#   sh SYSTEM/golemos/auskunft.sh [--arch x86_64|aarch64]
#
# Ergebnis unter SYSTEM/golemos/aus/<arch>/:
#   lizenzen-<arch>.tar.xz   Verzeichnis der Pakete mit Fassung und Lizenz,
#                            die Lizenztexte, die Konfiguration (klein,
#                            gehoert neben jedes freigegebene Abbild)
#   quellen-<arch>.tar       die Quellen aller Pakete im Abbild und in der
#                            Schicht, dazu Buildroot selbst (gross, gehoert
#                            als Anhang an dieselbe Freigabe)
#
# ⚑ **Warum es das gibt:** Das Abbild enthaelt Teile unter der GPL (Kern,
# BusyBox, Werkzeuge aus Buildroot). Wer es weitergibt, gibt die Quellen
# mit. Die Quellen begleiten das Abbild, statt dass ein schriftliches
# Angebot ueber drei Jahre gegeben wird; `freigeben.sh` verweigert eine
# Freigabe ohne diese Auskunft. ⚠️ Das ist keine Rechtsberatung.
#
# ⚑ **Ohne Netz.** Buildroot sammelt mit `make legal-info`, was im Volumen
# schon liegt; der Behaelter laeuft mit `--network none`. Fehlt eine Quelle,
# scheitert es, statt sie still zu holen.
#
# ⚠️ **Buildroot sagt selbst, dass seine Auskunft keine fertige Lieferung
# ist:** Die Lizenzangaben der Pakete sind nicht vollstaendig geprueft,
# und die eigenen Quellen legt es nicht bei. Die zweite Luecke schliesst
# dieses Skript (das Quellarchiv von Buildroot kommt mit), die erste nicht.
#
# Voraussetzung: ein Bau derselben Architektur (`bauen.sh`, fuer die
# Schicht `schicht-bauen.sh`). Nicht neben einem Bau derselben
# Architektur fahren, beide arbeiten im selben Volumen.
set -eu

ARCH=x86_64
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    -h|--hilfe) sed -n '2,32p' "$0"; exit 0 ;;
    *) echo "unbekannt: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  x86_64|aarch64) ;;
  *) echo "unbekannte Architektur: $ARCH" >&2; exit 2 ;;
esac

HIER=$(cd "$(dirname "$0")" && pwd)
AUS="$HIER/aus/$ARCH"
VOLUMEN="golemos-bau-$ARCH"
# Dieselbe Fassung wie in `bauen.sh`; ein anderer Baum waere ein anderes
# Abbild.
FASSUNG=$(sed -n 's/^BUILDROOT_FASSUNG=//p' "$HIER/bauen.sh")

docker info >/dev/null 2>&1 || { echo "Docker laeuft nicht." >&2; exit 1; }
docker volume inspect "$VOLUMEN" >/dev/null 2>&1 || {
  echo "Kein Bauvolumen $VOLUMEN. Erst bauen:  sh SYSTEM/golemos/bauen.sh --arch $ARCH" >&2
  exit 1
}
mkdir -p "$AUS"

echo "── Lizenzauskunft fuer $ARCH (Buildroot $FASSUNG, ohne Netz)"
docker run --rm -i --network none \
  -v "$VOLUMEN:/bau" -v "$AUS:/aus" \
  -e FASSUNG="$FASSUNG" -e ARCH="$ARCH" \
  golemos-baumeister sh -s <<'IMBEHAELTER'
set -eu
BR="/bau/buildroot-$FASSUNG"
[ -f "$BR/.config" ] || { echo "⛔️ Der Baum hat keine Konfiguration; erst bauen" >&2; exit 4; }
ARBEIT=/bau/auskunft
rm -rf "$ARBEIT" && mkdir -p "$ARBEIT"

# ⚑ Basis und Schicht getrennt: Die Schicht hat einen eigenen
#   Ausgabeordner und eigene Pakete (WebKitGTK, Mesa, ...).
teile="basis"
[ -f /bau/schicht-oberflaeche/.config ] && teile="basis schicht"
for teil in $teile; do
  case "$teil" in
    basis)   O="$BR/output" ;;
    schicht) O=/bau/schicht-oberflaeche ;;
  esac
  rm -rf "$O/legal-info"
  if ! make -C "$BR" O="$O" legal-info > "$ARBEIT/$teil.log" 2>&1; then
    tail -20 "$ARBEIT/$teil.log" >&2
    echo "⛔️ legal-info fuer $teil scheiterte; fehlt eine Quelle, braucht es einen Bau mit Netz" >&2
    exit 5
  fi
  mkdir -p "$ARBEIT/$teil"
  # ⚑ Nur was ins Abbild geht: `sources` und nicht `host-sources`. Die
  #   Werkzeuge des Baus werden nicht weitergegeben.
  cp -a "$O/legal-info/manifest.csv" "$O/legal-info/licenses" "$O/legal-info/README" "$ARBEIT/$teil/"
  cp -a "$O/legal-info/buildroot.config" "$ARBEIT/$teil/" 2>/dev/null || true
  mv "$O/legal-info/sources" "$ARBEIT/$teil/quellen"
  # ⛔️ Ein Paket, dessen Quelle Buildroot nicht beilegen durfte oder
  #    konnte, steht in der README als Warnung. Das gehoert gesehen.
  #    Bekannt und hier geschlossen: Buildroot legt seine eigenen Quellen
  #    nicht bei; das Archiv dazu kommt unten mit.
  warnungen=$(grep -iE "^WARNING" "$ARBEIT/$teil/README" | grep -v "Buildroot source code has not been saved" || true)
  if [ -n "$warnungen" ]; then
    echo "   ⚠️ $teil: Buildroot warnt:" >&2
    echo "$warnungen" | sed 's/^/      /' >&2
  fi
  echo "   $teil: $(($(wc -l < "$ARBEIT/$teil/manifest.csv") - 1)) Pakete"
done

# ⚑ Buildroot selbst, ohne Bauordner und ohne das Quellenlager: Es gehoert
#   zu den Quellen (die Skripte, die das Abbild bauen) und ist GPL-2.0.
tar -C /bau --exclude="buildroot-$FASSUNG/output" --exclude="buildroot-$FASSUNG/dl" \
  --exclude="buildroot-$FASSUNG/.config*" --exclude="buildroot-$FASSUNG/.golemos-konf" \
  -cf "$ARBEIT/buildroot-$FASSUNG.tar" "buildroot-$FASSUNG"

cd "$ARBEIT"
klein=""
for teil in $teile; do klein="$klein $teil/manifest.csv $teil/licenses $teil/README"; done
[ -f basis/buildroot.config ] && klein="$klein basis/buildroot.config"
# shellcheck disable=SC2086
tar -cJf "/aus/lizenzen-$ARCH.tar.xz" $klein
grosse=""
for teil in $teile; do grosse="$grosse $teil/quellen"; done
# shellcheck disable=SC2086
tar -cf "/aus/quellen-$ARCH.tar" $grosse "buildroot-$FASSUNG.tar"
cd / && rm -rf "$ARBEIT"
IMBEHAELTER

( cd "$AUS" && for f in "lizenzen-$ARCH.tar.xz" "quellen-$ARCH.tar"; do
    if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$f" > "$f.sha256"; else sha256sum "$f" > "$f.sha256"; fi
  done )
echo "── Fertig"
ls -lh "$AUS/lizenzen-$ARCH.tar.xz" "$AUS/quellen-$ARCH.tar" | awk '{print "   "$9"  "$5}'
