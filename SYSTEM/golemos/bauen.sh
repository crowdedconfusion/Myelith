#!/bin/sh
# GolemOS bauen.
#
#   sh SYSTEM/golemos/bauen.sh [--arch x86_64|aarch64]
#
# ⚑ **Ein Linux-Abbild entsteht unter Linux.** Das Skript selbst laeuft
# ueberall; die Arbeit machen Behaelter. Einzige Voraussetzung: Docker.
set -eu

# ⚠️ **Der Rechner darf nicht einschlafen.** Schlaeft er ein, nimmt er
# Docker Desktop mit, der Behaelter stirbt mit `unexpected EOF`, und weil
# das Skript darin getoetet wird, schreibt es keine Endmarke mehr.
# 📌 **Ein Waechter, der auf die Endmarke wartet, wartet dann fuer
# immer**, und die Meldung des Hintergrundlaufs sagt sogar
# „Rueckgabewert 0". Drei Zeugen sprachen dagegen (keine Marke,
# `unexpected EOF`, 23 MB statt Hunderten), und die Zahl hatte unrecht.
# ⚑ Deshalb laeuft der Bau unter `caffeinate`, wo es das gibt.
if command -v caffeinate >/dev/null 2>&1 && [ -z "${GOLEMOS_WACH:-}" ]; then
  GOLEMOS_WACH=1 exec caffeinate -dims "$0" "$@"
fi

ARCH=x86_64
# ⚠️ **Die Fassung ist gesetzt und wird nicht "die neueste".** Ein Bau,
# der sich seine Grundlage selbst aussucht, ist beim naechsten Mal ein
# anderer Bau.
BUILDROOT_FASSUNG=2026.02.3
# ⛔️ **Die Pruefsumme steht hier, nicht nur in der Ausgabe.** Ein Bau,
# der sich seine Grundlage aus dem Netz holt und nicht nachsieht, baut
# irgendetwas.
BUILDROOT_SUMME=65528a544f1e07c2f5ec487beca483bd380a6af8351a45f3649a19a0e8b63de2

while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    -h|--hilfe) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unbekannt: $1" >&2; exit 2 ;;
  esac
done

HIER=$(cd "$(dirname "$0")" && pwd)
WURZEL=$(cd "$HIER/../.." && pwd)
# ⛔️ **Je Architektur ein eigener Ausgabeordner.** 📌 Ohne das legen
# beide Baeume ihr `rootfs.cpio.gz` an dieselbe Stelle, und der zweite
# Bau ersetzt den ersten **wortlos**. Genau so ist am 2026-09-24 das
# fertige aarch64-Dateisystem verschwunden; aufgefallen ist es nur an
# der Uhrzeit in `ls -lh`. ⚑ Dieselbe Falle wie bei den Volumen, dort
# rechtzeitig gesehen, hier nicht.
AUS="$HIER/aus/$ARCH"
BAU="$HIER/bau"

# ⛔️ **Der Baubaum lebt in einem Docker-Volumen, nicht im Bindpfad.**
#
# 📌 **Gemessen am 2026-09-24, und es hat einen ganzen Lauf gekostet.**
# Der erste Versuch legte den Baubaum auf das macOS-Dateisystem,
# eingehaengt ueber Dockers Dateifreigabe. Nach 26 Minuten war Buildroot
# nicht einmal durch die Wirtswerkzeuge und stand bei `host-tar
# Configuring`; nativ sind das fuenf Minuten. **Ein `configure`-Skript
# macht Zehntausende winziger Dateizugriffe**, und genau die sind ueber
# diese Bruecke pathologisch langsam. Hochgerechnet ueber zehn Stunden
# statt ein bis zwei.
#
# ⚑ Im Volumen liegt der Baum auf dem ext4 der Linux-VM. Ueber den
# Bindpfad kommen nur noch die **Eingaben** (Konfiguration, Overlay) und
# die **Erzeugnisse** gehen als Kopie zurueck.
# ⚑ **Je Architektur ein eigenes Volumen.** Beide Baeume heissen
# `buildroot-<Fassung>`; in einem Volumen wuerde der zweite Bau den
# ersten ueberschreiben, und niemand saehe es.
VOLUMEN="golemos-bau-$ARCH"

# ⚑ **Die Plattform fuer die Behaelter, in denen UNSERE Programme
# entstehen.** 📌 Der Umweg ueber einen musl-Kreuzuebersetzer ist nicht
# noetig: Docker faehrt auf Apple-Silizium auch amd64, und darin ist
# alles nativ, `g++` fuer `esaxx-rs` eingeschlossen. Gemessen: gcc
# 15.2.0, Rust-Wirt `x86_64-alpine-linux-musl`.
case "$ARCH" in
  x86_64)  PLATTFORM=linux/amd64 ;;
  aarch64) PLATTFORM=linux/arm64 ;;
  *) echo "unbekannte Architektur: $ARCH" >&2; exit 2 ;;
esac

# ── Docker ist die einzige Voraussetzung ────────────────────────────
#
# 📌 **Hier stand zuerst ein Neustart des ganzen Skripts im Behaelter.**
# Das war richtig, solange es EINE Bauumgebung gab. Inzwischen ruft jeder
# Schritt seinen eigenen Behaelter (Alpine fuer unsere Programme, weil
# dort musl zu Hause ist; Debian fuer Buildroot, das einen glibc-Wirt
# will), und ein aeusserer Neustart haette Behaelter in Behaeltern
# verlangt. ⚑ **Das Skript laeuft auf dem Wirt und dirigiert.**
if ! docker info >/dev/null 2>&1; then
  cat >&2 <<'HINWEIS'
GolemOS braucht Docker, und der Dienst laeuft nicht.

  Starte Docker Desktop und ruf dieses Skript noch einmal auf.
HINWEIS
  exit 1
fi

if ! docker image inspect golemos-baumeister >/dev/null 2>&1; then
  echo "── Bauumgebung fuer Buildroot erstellen"
  docker build -q -t golemos-baumeister -f "$HIER/Containerfile" "$HIER" >/dev/null
fi

mkdir -p "$AUS" "$BAU"

# ⛔️ **Fund 480: Zwei Voraussetzungen, die nur ein benutzter Rechner
#    erfuellt.** Bis zum 2026-09-25 lag GolemOS nicht im Repositorium und
#    wurde nur auf einer Maschine gebaut, auf der beides laengst da war:
#    das ausgepackte Kistenlager und `overlay/usr/bin/`. In einem frischen
#    Klon fehlt beides, denn beides ist ausgeschlossen. Der Bau waere am
#    ersten `cargo --offline` gescheitert und, waere er durchgekommen, am
#    ersten `install` in ein Verzeichnis, das es nicht gibt.
#
# ⚑ Dasselbe Auspacken wie in `SYSTEM/install/installieren-macos.sh`:
#   nur, wenn das Lager fehlt oder aelter ist als der Vorrat.
if [ ! -d "$WURZEL/SYSTEM/crates-lager/vendor" ] \
   || [ "$WURZEL/SYSTEM/crates-vorrat" -nt "$WURZEL/SYSTEM/crates-lager/vendor" ]; then
  echo "── Vorrat auspacken (SYSTEM/crates-vorrat nach SYSTEM/crates-lager)"
  python3 "$WURZEL/SYSTEM/install/vorrat.py" auspacken >/dev/null || {
    echo "⛔️ Der Vorrat liess sich nicht auspacken" >&2
    exit 9
  }
fi
mkdir -p "$HIER/overlay/usr/bin"

# ── 1. Unsere Programme, statisch gegen musl ────────────────────────
#
# ⚑ **Statisch und gegen musl**, obwohl das System glibc traegt. Dann
# haengen sie an gar nichts: kein Versionsabgleich mit dem frisch
# gebauten glibc, kein nachtraegliches Brechen, wenn Buildroot seine
# Fassung hebt. **Ein Programm, das nichts braucht, kann auch nichts
# vermissen.** Belegt: Das Erzeugnis laeuft unveraendert auf Debian mit
# glibc, also auf einem System, das keine einzige seiner Bibliotheken
# hat.
#
# ⚑ **Und dafuer ein eigener Behaelter.** Alpine ist musl, also sind
# `gcc` und `g++` dort von Haus aus musl-Uebersetzer; Debian bringt nur
# `musl-gcc` mit und kein `musl-g++`, und `esaxx-rs` (ueber
# `tokenizers`) uebersetzt C++. ⛔️ **Die Abkuerzung waere gewesen,
# `tokenizers` ohne Standardmerkmale zu bauen.** Damit fiele auch `onig`
# weg, die Regexmaschine mancher Vorzerleger: **eine andere Zerlegung
# ist eine andere Tokenfolge**, und das ist der Zahlenvertrag. Also
# wurde die Umgebung gewechselt und nicht der Vertrag.
#
# ⚠️ **`alpine:edge` ist ein rollendes Ziel und gehoert festgenagelt**,
# sobald eine veroeffentlichte Fassung Rust 1.88 oder neuer traegt. 3.22
# hat 1.87 und ist damit zu alt.
echo "── Programme bauen (Alpine, musl nativ)"
docker run --rm -t --platform "$PLATTFORM" -v "$WURZEL:$WURZEL" -w "$WURZEL" \
  -e CARGO_HOME="$WURZEL/SYSTEM/crates-lager/cargo-home" \
  -e CARGO_TARGET_DIR="$HIER/bau/programme-$ARCH" \
  alpine:edge sh -c '
    set -eu
    apk add --no-cache -q rust cargo build-base
    # ⛔️ **Das Ziel muss ausdruecklich dastehen.** Ohne es gilt
    #    `RUSTFLAGS` auch fuer die Prozeduralmakros, und die muessen
    #    dynamische Bibliotheken sein, damit rustc sie laden kann. Ein
    #    statisches Makro ist ein Widerspruch, und der Bau bricht ab.
    #    ⚠️ Die naheliegende Abhilfe waere, `+crt-static` fallen zu
    #    lassen; dann braeuchte das Programm ein `ld-musl-*.so`, das es
    #    auf einem glibc-System nicht gibt, und das faellt erst im
    #    Abbild auf.
    CARGO_BUILD_TARGET=$(rustc -vV | awk "/^host:/{print \$2}")
    export CARGO_BUILD_TARGET
    export RUSTFLAGS="-C target-feature=+crt-static"
    for kiste in CLIENT/myl-console CLIENT/myl-client SYSTEM/golemos/einrichten; do
      cargo build --release --offline --locked --manifest-path "$kiste/Cargo.toml"
    done
  '

for prog in myelith myl golem-einrichten; do
  gefunden=$(find "$HIER/bau/programme-$ARCH" -type f -name "$prog" -path "*release*" | head -1)
  [ -n "$gefunden" ] || { echo "$prog wurde nicht gebaut" >&2; exit 4; }
  install -m 0755 "$gefunden" "$HIER/overlay/usr/bin/$prog"
  # ⚠️ **Das Overlay ist je Lauf eines.** Wer erst aarch64 und dann
  #    x86_64 baut, ueberschreibt hier; das ist richtig, denn jeder Bau
  #    nimmt sein eigenes Overlay mit. ⛔️ Nur parallel darf es nicht
  #    laufen.
  echo "   $prog  $(du -h "$HIER/overlay/usr/bin/$prog" | cut -f1)"
done

# ⛔️ **Nachsehen, ob es wirklich statisch ist.** Ein dynamisch
#    gebundenes Programm faellt sonst erst im Abbild auf, und dort
#    fehlt jedes Werkzeug, um es zu untersuchen.
for prog in myelith myl golem-einrichten; do
  if ! file "$HIER/overlay/usr/bin/$prog" | grep -q "static"; then
    echo "⛔️ $prog ist nicht statisch gebunden; im Abbild fehlt seine Bibliothek" >&2
    exit 6
  fi
done
echo "   alle drei statisch gebunden"

# ── 2 bis 4. Buildroot holen, konfigurieren, bauen ──────────────────
#
# Alles in EINEM Behaelterlauf, damit der Baubaum im Volumen bleibt.
echo "── Buildroot $BUILDROOT_FASSUNG im Volumen $VOLUMEN"
docker volume create "$VOLUMEN" >/dev/null
# ⛔️ **Ein frisches Volumen gehoert root, der Bau laeuft aber nicht als
#    root.** Buildroot weigert sich mit einer eigenen Meldung, und zwar
#    zu Recht: Ein Bau als root kann den Wirt beschaedigen, wenn ein
#    Paket sich vergreift. 📌 Ohne diesen Vorlauf scheiterte schon das
#    Auspacken an `Cannot mkdir: Permission denied`.
docker run --rm --user 0 -v "$VOLUMEN:/bau" golemos-baumeister \
  chown -R 1000:1000 /bau
# ⛔️ **`-i` und NICHT `-t`.** 📌 Hier stand `-t`: Docker legte einen
#    Terminal an, reichte aber `stdin` nicht durch, und `sh -s` fand
#    nichts zu lesen. Der Behaelter sass in einer interaktiven Shell und
#    wartete auf eine Eingabe, die nie kam. ⚑ **Gefangen hat das der
#    Stillstandswaechter**, nicht der Rueckgabewert: Es gab keinen.
docker run --rm -i \
  -v "$VOLUMEN:/bau" \
  -v "$HIER/konfigurationen:/eingabe/konfigurationen:ro" \
  -v "$HIER/overlay:/eingabe/overlay:ro" \
  -v "$HIER/abbild:/eingabe/abbild:ro" \
  -e ARCH="$ARCH" \
  -e FASSUNG="$BUILDROOT_FASSUNG" \
  -e SUMME="$BUILDROOT_SUMME" \
  golemos-baumeister sh -s <<'IMBEHAELTER'
set -eu
BR="/bau/buildroot-$FASSUNG"

if [ ! -d "$BR" ]; then
  echo "   holen"
  wget -qO /tmp/br.tar.gz "https://buildroot.org/downloads/buildroot-$FASSUNG.tar.gz"
  ist=$(sha256sum /tmp/br.tar.gz | cut -d' ' -f1)
  if [ "$ist" != "$SUMME" ]; then
    echo "⛔️ andere Pruefsumme: erwartet $SUMME, bekommen $ist" >&2
    exit 8
  fi
  tar -xzf /tmp/br.tar.gz -C /bau
  rm -f /tmp/br.tar.gz
fi

# ⚑ Das Overlay wird kopiert und nicht eingehaengt: Buildroot fasst es
#   beim Zusammensetzen an, und ein schreibgeschuetzter Bindpfad waere
#   dort eine Ueberraschung.
rm -rf /bau/overlay && cp -a /eingabe/overlay /bau/overlay
KONF="/eingabe/konfigurationen/golemos_${ARCH}_defconfig"
sed "s|^BR2_ROOTFS_OVERLAY=.*|BR2_ROOTFS_OVERLAY=\"/bau/overlay\"|" "$KONF" > /tmp/konf
make -C "$BR" defconfig BR2_DEFCONFIG=/tmp/konf >/dev/null

# ⛔️ **Buildroot verschluckt unbekannte Namen wortlos.** Ein Tippfehler
#    ist dann ein Merkmal, das einfach fehlt, und es faellt erst auf,
#    wenn das Abbild etwas nicht kann. ⚑ **Schweigen sieht aus wie
#    Erfolg.** 📌 Beim ersten Lauf hat diese Zaehlung drei von zwanzig
#    Einstellungen gefangen (`bash`, `less` und die Shellwahl; Buildroot
#    versteckt sie hinter `BR2_PACKAGE_BUSYBOX_SHOW_OTHERS`, weil
#    BusyBox sie schon mitbringt).
fehlend=0
while IFS= read -r zeile; do
  case "$zeile" in \#*|"") continue ;; esac
  name=${zeile%%=*}
  grep -q "^${name}=" "$BR/.config" || { echo "   ⛔️ verschluckt: $zeile" >&2; fehlend=$((fehlend+1)); }
done < /tmp/konf
[ "$fehlend" -eq 0 ] || { echo "$fehlend Einstellungen verschluckt" >&2; exit 5; }
echo "   alle Einstellungen angekommen"

# ⛔️ **Buildroot raeumt nicht auf, wenn eine Einstellung verschwindet.**
#
#    📌 **Am 2026-09-24 gemessen, und es kostete ein falsches Abbild.**
#    `BR2_LINUX_KERNEL_INSTALL_TARGET` wurde aus der Konfiguration
#    genommen, weil es den Kern ein zweites Mal ins Dateisystem legte:
#    40 MB von 152. Der naechste Bau lief durch, meldete Erfolg, und die
#    40 MB lagen weiter da. **Die Aenderung war richtig und wirkungslos,
#    und nichts hat es gesagt.**
#
#    Der Zielbaum ist fortschreibend: Was einmal installiert wurde,
#    bleibt. ⚠️ **Und ihn allein zu loeschen genuegt nicht**, denn je
#    Paket steht eine Stempeldatei „ist installiert", die eine
#    Neuinstallation verhindert; ohne sie fehlt danach `/etc/inittab`
#    und `target-finalize` bricht ab. Beides zusammen setzt sauber neu
#    zusammen, **ohne etwas neu zu uebersetzen**: 19 Pakete, Sekunden.
#
#    ⚑ Ein frischer Bau ist davon nicht betroffen, ein zweiter schon,
#    und der zweite ist der haeufigere.
if [ -f "$BR/.golemos-konf" ] && ! cmp -s "$BR/.golemos-konf" /tmp/konf; then
  echo "   die Konfiguration hat sich geaendert: Zielbaum neu aufbauen"
  find "$BR/output/build" -name '.stamp_target_installed' -delete
  find "$BR/output/build" -name '.stamp_images_installed' -delete
  rm -rf "$BR/output/target"
fi
cp /tmp/konf "$BR/.golemos-konf"

echo "── Bauen mit $(nproc) Faeden"
make -C "$BR" -j"$(nproc)"

# ⛔️ **Dass eine Einstellung ankommt, heisst nicht, dass das Programm
#    da ist.** Die Nachzaehlung oben prueft die Konfiguration; hier
#    wird nachgesehen, was wirklich im Zielbaum liegt.
#    📌 **Der Anlass ist eigener Code:** `S05daten` ruft `sfdisk`,
#    `mkfs.vfat`, `blkid` und `lsblk`, jeweils mit `|| true`. Fehlt
#    eines, tut das Startskript **wortlos nichts**, und die
#    Datenpartition bleibt bei 32 MB, ohne dass jemand erfaehrt warum.
#    ⚑ **Ein Skript, das im Fehlerfall schweigt, braucht eine Pruefung
#    beim Bauen.**
echo "── Nachsehen, ob die noetigen Programme im Abbild liegen"
# ⚠️ **Ohne `-type f`, und das ist der ganze Punkt.** 📌 Die erste
#    Fassung suchte nur nach gewoehnlichen Dateien und meldete
#    `mkfs.vfat` und `mount` als fehlend, obwohl beide dalagen: Das
#    eine ist ein Verweis auf `mkfs.fat`, das andere ein
#    BusyBox-Applet, also auch ein Verweis. ⛔️ **Eine Pruefung, die
#    grundlos abbricht, ist schlimmer als keine**: Sie haette einen
#    richtigen Bau blockiert und die Suche nach einem Fehler
#    ausgeloest, den es nicht gibt.
fehlt=0
# ⚑ `partx` steht dabei, seit `partprobe` sich als untauglich erwiesen
#   hat: Der Kern liest die Tabelle einer Platte mit eingehaengter
#   Partition nicht neu. Siehe `overlay/etc/init.d/S05daten`.
# ⚑ `umount`, `poweroff` und `sync` braucht `golem-einrichten` dazu: Es
#   ruft sie, statt sie nachzubauen, und ein fehlendes fiele erst mitten
#   in einer Installation auf.
# ⚑ `xz` und `sha256sum` braucht `spread.sh`, wenn GolemOS aus dem
#   Myelith-Ordner auf dem Stick selbst einen neuen Stick schreibt.
for w in sfdisk partx mkfs.vfat fatlabel blkid lsblk mount umount poweroff sync xz sha256sum loadkeys; do
  if ! find "$BR/output/target" -name "$w" 2>/dev/null | grep -q .; then
    echo "   ⛔️ $w fehlt im Abbild; S05daten, golem-einrichten oder spread.sh scheitert daran" >&2
    fehlt=$((fehlt + 1))
  fi
done
# ⚑ **Jede Belegung, die der Assistent anbietet**, muss im Abbild
#   liegen; sonst scheitert `loadkeys` erst am Rechner des Menschen.
#   Dieselbe Liste steht in `einrichten/src/sprache.rs`, und eine Probe
#   dort verlangt, dass jede ihrer Karten hier genannt ist.
for k in de-latin1-nodeadkeys de_CH-latin1 us uk fr-latin9 es it nl pt-latin1 pl2; do
  if ! find "$BR/output/target/usr/share/keymaps" -name "$k.map*" 2>/dev/null | grep -q .; then
    echo "   ⛔️ Tastaturbelegung $k fehlt im Abbild" >&2
    fehlt=$((fehlt + 1))
  fi
done
[ "$fehlt" -eq 0 ] || { echo "$fehlt Programme fehlen" >&2; exit 7; }
echo "   alle da"

mkdir -p /bau/aus
for f in bzImage Image rootfs.cpio.gz golemos.img; do
  [ -f "$BR/output/images/$f" ] && cp "$BR/output/images/$f" /bau/aus/
done
ls -l /bau/aus
IMBEHAELTER

echo "── Erzeugnisse herausholen"
docker run --rm -v "$VOLUMEN:/bau" -v "$AUS:/aus" golemos-baumeister \
  sh -c 'cp /bau/aus/* /aus/ 2>/dev/null; ls /aus'

# ── 5. Fertig ───────────────────────────────────────────────────────
#
# 📌 Hier stand noch ein Einsammeln aus `$BR`, aus der Zeit, als der
# Baubaum im Bindpfad lag. Seit er im Volumen lebt, holt der Schritt
# davor die Erzeugnisse heraus, und `$BR` gibt es auf dem Wirt gar
# nicht mehr. ⚠️ **Der Bau war laengst durch, und das Skript endete
# trotzdem mit einem Fehler.**
echo
echo "── Fertig"
ls -lh "$AUS" | tail -n +2 | awk '{print "   "$9"  "$5}'
# 📌 Hier stand der Startbefehl fuer x86_64 als fester Text, und nach
# einem aarch64-Bau nannte er einen Kern (`bzImage`), den es dort nicht
# gibt, in einem Verzeichnis ohne die Architektur. Er wird jetzt aus
# `$ARCH` gebildet, damit er zu dem passt, was gerade gebaut wurde.
case "$ARCH" in
  x86_64)  QEMU="qemu-system-x86_64"; KERN=bzImage; KONSOLE=ttyS0 ;;
  aarch64) QEMU="qemu-system-aarch64 -M virt -cpu cortex-a57"; KERN=Image; KONSOLE=ttyAMA0 ;;
esac
cat <<START

Starten laesst es sich so (QEMU auf dem Wirt):

  $QEMU -m 1024 -nographic \\
    -kernel SYSTEM/golemos/aus/$ARCH/$KERN \\
    -initrd SYSTEM/golemos/aus/$ARCH/rootfs.cpio.gz \\
    -append "console=$KONSOLE quiet"

Und drinnen ist die Frage, um die es geht:

  myelith --hilfe
START
