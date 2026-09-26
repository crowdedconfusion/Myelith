#!/bin/sh
# Laeuft, bevor das Dateisystem zusammengesetzt wird.
#
# ⛔️ **Eine Anmeldung auf BEIDEN Konsolen.**
#
# 📌 Die GRUB-Eintraege A und B nennen `tty1` zuletzt, also liegt
# `/dev/console` auf dem Bildschirm: Wer einen Stick in einen Laptop
# steckt, bekommt dort seine Anmeldung, und das ist richtig. ⚠️ **Wer
# nur ein serielles Kabel hat, saehe dann nach dem Init nichts mehr**,
# und genau so laeuft ein Knoten im Schrank.
#
# ⚑ Die Abhilfe ist nicht, die Reihenfolge umzudrehen (dann fehlt sie
# dem Laptop), sondern **beiden eine zu geben**. Zwei `getty` auf zwei
# verschiedenen Geraeten stoeren sich nicht.
set -e

INITTAB="$TARGET_DIR/etc/inittab"
[ -f "$INITTAB" ] || exit 0

# ⚠️ **Nur einmal.** Ein zweiter Lauf des Bauskripts haenge sonst eine
#    zweite Zeile an, und zwei `getty` auf demselben Geraet flattern.
#    📌 **Ersetzen statt Uebergehen:** Buildroot baut `target/` beim
#    naechsten Lauf nicht neu auf, die alte Zeile bleibt also stehen.
#    Hiess es hier „schon da, fertig", behielte ein bestehender Baubaum
#    fuer immer `ttyS0`, auch nach der Berichtigung unten.
sed -i '/GOLEMOS_SERIELL/,+1d' "$INITTAB"

# ⛔️ **Fund 477: die Schnittstelle je Architektur.** Hier stand fest
#    `ttyS0`, und auf aarch64 gibt es das nicht; die Anmeldung dort
#    blieb stumm. Buildroot nennt die Architektur in seiner
#    Konfiguration.
case "$(sed -n 's/^BR2_ARCH="\(.*\)"$/\1/p' "$BR2_CONFIG")" in
  aarch64) SERIELL=ttyAMA0 ;;
  *)       SERIELL=ttyS0 ;;
esac

cat >> "$INITTAB" <<ZEILEN
# Eine Anmeldung auch ueber die serielle Schnittstelle. GOLEMOS_SERIELL
$SERIELL::respawn:/sbin/getty -L $SERIELL 115200 vt100
ZEILEN
