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
WURZEL_GROESSE=$(wc -c < "$BINARIES_DIR/rootfs.ext2" | tr -d ' ')
ersetzen() {
  sed -e "s/KERNNAME/$KERN/g" -e "s/SERIELL/$SERIELL/g" -e "s/WURZEL_GROESSE/$WURZEL_GROESSE/g" \
      -e "s/WURZEL_A_UUID/$UUID_A/g" -e "s/WURZEL_B_UUID/$UUID_B/g" -e "s/DATEN_UUID/$UUID_D/g" "$1"
}
ersetzen "$VORLAGEN/grub.cfg" \
  > "$BINARIES_DIR/efi-part/EFI/BOOT/grub.cfg"

# ⚑ **`grubenv` liegt von Anfang an da**, genau 1024 Bytes: GRUB schreibt
#   beim Start nur in eine Datei, die es schon gibt, und `golem-aktualisieren`
#   ebenso an Ort und Stelle. Am Anfang gilt A, und keine Probe steht an.
#   A traegt als Stand die ersten 16 Zeichen der SHA-256 seiner Wurzel, wie
#   `bauen.sh` ihn ins Paket legt. ⚑ **B traegt `leer`** (GolemOS 0.6.1): Das
#   Abbild liefert B ohne Inhalt aus, und der erste Start fuellt es aus A
#   (`S00seiteb`, siehe `golem-aktualisieren fuellen`).
UMG="$BINARIES_DIR/efi-part/EFI/BOOT/grubenv"
STAND=$(sha256sum "$BINARIES_DIR/rootfs.ext2" | cut -c1-16)
printf '# GRUB Environment Block\ngolem_seite=A\ngolem_probe=\ngolem_stand_A=%s\ngolem_stand_B=%s\n' "$STAND" leer > "$UMG"
head -c $((1024 - $(wc -c < "$UMG"))) /dev/zero | tr '\000' '#' >> "$UMG"
[ "$(wc -c < "$UMG")" -eq 1024 ] || { echo "nach-dem-abbild: grubenv nicht 1024 Bytes" >&2; exit 1; }

# ── Secure Boot: shim von Debian, GRUB und Kern mit Golem-Emet signiert ──
#
# ⚑ **Die Kette** (GolemOS 0.7.0): Die Firmware startet `shim`, das Microsoft
#   signiert hat (unveraendert aus Debian, `shim/HERKUNFT.md`). shim startet
#   GRUB, wenn GRUB mit einem eingetragenen Schluessel signiert ist, und GRUB
#   startet nur einen Kern, den shim fuer gut befindet. Unser Schluessel heisst
#   **Golem-Emet**; ein Rechner traegt ihn beim ersten Start einmal ueber den
#   MokManager ein (das Zertifikat liegt auf der Bootpartition).
# ⚠️ **Ohne den geheimen Teil wird nichts signiert.** Er liegt nie im
#   Repositorium; `bauen.sh` haengt ihn nur ein, wenn `GOLEM_SCHLUESSEL` auf
#   ihn zeigt. Ohne ihn entsteht das Abbild wie bisher, GRUB direkt als
#   Startdatei, und es startet nur mit ausgeschaltetem Secure Boot.
SCHLUESSEL=/schluessel/golem-emet.key
ZERT="$VORLAGEN/shim/GOLEM-EMET.crt"
BOOT="$BINARIES_DIR/efi-part/EFI/BOOT"
case "$KERN" in
  bzImage) A=x64;  GROSS=X64;  ZIEL=x86_64-efi; ARCH_ORDNER=x86_64 ;;
  *)       A=aa64; GROSS=AA64; ZIEL=arm64-efi;  ARCH_ORDNER=aarch64 ;;
esac
# 📌 Der Kern liegt nach dem ersten Signieren signiert im Bildordner, und
#    dieses Skript laeuft bei jedem `make`. Signiert wird deshalb immer das
#    unsignierte Original, und das ist ein Kern ohne Signaturtabelle.
if sbverify --list "$BINARIES_DIR/$KERN" 2>&1 | grep -q "No signature table"; then
  cp "$BINARIES_DIR/$KERN" "$BINARIES_DIR/$KERN.unsigniert"
fi
# ⚑ **GRUB neu, mit SBAT, in beiden Faellen.** shim startet GRUB nur mit einem
#   SBAT-Abschnitt, und seine Sperrliste verlangt `grub,5`; Buildroot legt
#   keinen an. Also dasselbe Abbild noch einmal, mit denselben Modulen und
#   `--sbat`. 📌 Auch ohne Schluessel, sonst laege nach einem signierten Bau
#   shim unter dem Startnamen und kein GRUB daneben.
GRUBMODULE=$(ls -d "$BUILD_DIR"/grub2-[0-9]*/build-"$ZIEL"/grub-core | head -1)
MODULE=$(sed -n 's/^BR2_TARGET_GRUB2_BUILTIN_MODULES_EFI="\(.*\)"$/\1/p' "$BR2_CONFIG")
printf '%s\n' \
  "sbat,1,SBAT Version,sbat,1,https://github.com/rhboot/shim/blob/main/SBAT.md" \
  "grub,5,Free Software Foundation,grub,2.14,https://www.gnu.org/software/grub/" \
  "grub.golemos,1,Myelith,grub,2.14,https://github.com/crowdedconfusion/Myelith" > "$BUILD_DIR/golemos-sbat.csv"
# shellcheck disable=SC2086 # die Module sind eine Liste
"$HOST_DIR/bin/grub-mkimage" -d "$GRUBMODULE" -O "$ZIEL" -o "$BUILD_DIR/grub$A.efi" \
  -p /EFI/BOOT --sbat "$BUILD_DIR/golemos-sbat.csv" $MODULE
# Buildroots GRUB unter dem Startnamen faellt in beiden Faellen weg (FAT kennt
# keine Gross- und Kleinschreibung, zwei Schreibweisen stiessen zusammen).
rm -f "$BOOT/boot$A.efi" "$BOOT/BOOT$GROSS.EFI" "$BOOT/grub$A.efi" "$BOOT/mm$A.efi" "$BINARIES_DIR/efi-part/EFI/GOLEM-EMET.cer"
if [ -f "$SCHLUESSEL" ]; then
  echo "── Secure Boot: signieren mit Golem-Emet"
  # ⛔️ **Nur mit dem Schluessel, dessen oeffentlicher Teil im Repositorium
  #    liegt.** Ein anderer Schluessel ergaebe Abbilder, die kein Rechner mit
  #    eingetragenem Golem-Emet startet.
  [ "$(openssl x509 -in "$ZERT" -noout -pubkey | sha256sum)" = "$(openssl pkey -in "$SCHLUESSEL" -pubout | sha256sum)" ] \
    || { echo "nach-dem-abbild: der Schluessel passt nicht zu shim/GOLEM-EMET.crt" >&2; exit 1; }
  sbsign --key "$SCHLUESSEL" --cert "$ZERT" --output "$BOOT/grub$A.efi" "$BUILD_DIR/grub$A.efi" >/dev/null
  sbsign --key "$SCHLUESSEL" --cert "$ZERT" --output "$BINARIES_DIR/$KERN" "$BINARIES_DIR/$KERN.unsigniert" >/dev/null
  # Die Startdatei ist shim, daneben GRUB und der MokManager.
  cp "$VORLAGEN/shim/$ARCH_ORDNER/shim$A.efi" "$BOOT/BOOT$GROSS.EFI"
  cp "$VORLAGEN/shim/$ARCH_ORDNER/mm$A.efi" "$BOOT/mm$A.efi"
  openssl x509 -in "$ZERT" -outform DER -out "$BINARIES_DIR/efi-part/EFI/GOLEM-EMET.cer"
  for f in "$BOOT/grub$A.efi" "$BINARIES_DIR/$KERN"; do
    sbverify --cert "$ZERT" "$f" >/dev/null 2>&1 || { echo "nach-dem-abbild: $f traegt keine gueltige Signatur" >&2; exit 1; }
  done
  echo "   GRUB und $KERN signiert, shim und MokManager auf der Bootpartition"
else
  echo "⚠️  Secure Boot: kein Schluessel unter /schluessel, das Abbild bleibt unsigniert" >&2
  echo "    und startet nur mit ausgeschaltetem Secure Boot (GOLEM_SCHLUESSEL fuer bauen.sh)" >&2
  cp "$BINARIES_DIR/$KERN.unsigniert" "$BINARIES_DIR/$KERN"
  cp "$BUILD_DIR/grub$A.efi" "$BOOT/BOOT$GROSS.EFI"
fi

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
