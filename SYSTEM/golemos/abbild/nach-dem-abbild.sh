#!/bin/sh
# Setzt das Plattenabbild zusammen, nachdem Buildroot fertig ist.
#
# ⚑ **Eine Vorlage für beide Architekturen.** Der einzige Unterschied
# ist der Name des Kerns: x86_64 legt ein komprimiertes `bzImage` ab,
# aarch64 ein rohes `Image`. Statt zweier fast gleicher Dateien steht
# in den Vorlagen `KERNNAME`, und hier wird er eingesetzt.
set -eu

VORLAGEN=$(dirname "$0")

# ⛔️ **Welcher Kern da ist, wird nachgesehen und nicht geraten.**
# ⚑ Mit dem Kern steht auch die serielle Schnittstelle fest (Fund 477):
#   x86 nennt sie `ttyS0`, die ARM-Maschinen `ttyAMA0`.
if   [ -f "$BINARIES_DIR/bzImage" ]; then KERN=bzImage; SERIELL=ttyS0
elif [ -f "$BINARIES_DIR/Image" ];   then KERN=Image;   SERIELL=ttyAMA0
else
  echo "nach-dem-abbild: kein Kern in $BINARIES_DIR gefunden" >&2
  exit 1
fi
echo "── Plattenabbild bauen, Kern: $KERN"

mkdir -p "$BINARIES_DIR/efi-part/EFI/BOOT"
# ⛔️ **Fund 479: Kennungen je Bau, an beiden Stellen dieselben.** Die
#    Platzhalter stehen in `grub.cfg` und `genimage.cfg`, und nur hier
#    werden sie gefuellt, einmal, fuer beide. Ein Wert an zwei Orten
#    laeuft auseinander; ein Wert, der an einem Ort entsteht und an
#    zwei eingesetzt wird, nicht.
UUID_A=$(cat /proc/sys/kernel/random/uuid)
UUID_B=$(cat /proc/sys/kernel/random/uuid)
UUID_D=$(cat /proc/sys/kernel/random/uuid)
ersetzen() {
  sed -e "s/KERNNAME/$KERN/g" -e "s/SERIELL/$SERIELL/g" \
      -e "s/WURZEL_A_UUID/$UUID_A/g" -e "s/WURZEL_B_UUID/$UUID_B/g" -e "s/DATEN_UUID/$UUID_D/g" "$1"
}
ersetzen "$VORLAGEN/grub.cfg" \
  > "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"
ersetzen "$VORLAGEN/genimage.cfg" \
  > "$BINARIES_DIR/genimage-golemos.cfg"

# ⚠️ **genimage will ein leeres Arbeitsverzeichnis.** Liegt dort noch
#    etwas vom letzten Lauf, bricht es ab, und die Meldung nennt nicht
#    den Grund.
rm -rf "$BUILD_DIR/genimage.tmp"

"$HOST_DIR/bin/genimage" \
  --config "$BINARIES_DIR/genimage-golemos.cfg" \
  --rootpath "$TARGET_DIR" \
  --inputpath "$BINARIES_DIR" \
  --outputpath "$BINARIES_DIR" \
  --tmppath "$BUILD_DIR/genimage.tmp"

echo "── fertig: $(du -h "$BINARIES_DIR/golemos.img" | cut -f1)"
