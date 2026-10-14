#!/bin/sh
# Die Oberflaechen-Schicht von GolemOS bauen: das Myelith-Fenster im Vollbild.
#
#   sh SYSTEM/golemos/schicht-bauen.sh [--arch x86_64|aarch64] [--nur-pruefen]
#
# `--nur-pruefen` haelt nach dem Nachzaehlen der Einstellung an: der kurze
# Lauf, der den langen vorher bestaetigt (Minuten statt Stunden).
#
# Ergebnis: SYSTEM/golemos/aus/<arch>/oberflaeche-<arch>.sqfs und daneben
# die Pruefsumme. Auf GolemOS holt `golem-schicht holen` sie und
# `golem-schicht starten` startet sie.
#
# ⚑ **Voraussetzung ist ein Bau der Basis derselben Architektur**
# (`bauen.sh`): Der Buildroot-Baum im Volumen wird mitbenutzt, mit eigenem
# Ausgabeordner. Allein WebKitGTK braucht beim ersten Mal ein bis drei
# Stunden; danach geht ein Lauf in Minuten.
set -eu

# ⚠️ Wie in `bauen.sh`: Der Rechner darf waehrend des Baus nicht einschlafen.
if command -v caffeinate >/dev/null 2>&1 && [ -z "${GOLEMOS_WACH:-}" ]; then
  # ⚑ Ueber `sh` und nicht die Datei selbst: Mit `sh schicht-bauen.sh`
  #   aus ihrem Ordner gerufen, saehe `caffeinate` nur einen Namen ohne
  #   Pfad und faende ihn nicht (gemessen am 2026-10-13).
  GOLEMOS_WACH=1 exec caffeinate -dims sh "$0" "$@"
fi

ARCH=x86_64
NUR_PRUEFEN=0
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --nur-pruefen) NUR_PRUEFEN=1; shift ;;
    -h|--hilfe) sed -n '2,16p' "$0"; exit 0 ;;
    *) echo "unbekannt: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  x86_64 | aarch64) ;;
  *) echo "unbekannte Architektur: $ARCH" >&2; exit 2 ;;
esac

HIER=$(cd "$(dirname "$0")" && pwd)
WURZEL=$(cd "$HIER/../.." && pwd)
AUS="$HIER/aus/$ARCH"
VOLUMEN="golemos-bau-$ARCH"
# ⚑ **Die Fassung wird aus `bauen.sh` gelesen**, nicht ein zweites Mal
#   hingeschrieben: Basis und Schicht muessen aus demselben Baum kommen.
FASSUNG=$(sed -n 's/^BUILDROOT_FASSUNG=//p' "$HIER/bauen.sh")
[ -n "$FASSUNG" ] || { echo "BUILDROOT_FASSUNG nicht in bauen.sh gefunden" >&2; exit 2; }

docker info >/dev/null 2>&1 || { echo "Docker laeuft nicht; Docker Desktop starten und noch einmal." >&2; exit 1; }
docker image inspect golemos-baumeister >/dev/null 2>&1 \
  || { echo "Die Bauumgebung fehlt; erst: sh SYSTEM/golemos/bauen.sh --arch $ARCH" >&2; exit 1; }
docker run --rm -v "$VOLUMEN:/bau" golemos-baumeister test -d "/bau/buildroot-$FASSUNG" \
  || { echo "Kein Buildroot-Baum im Volumen $VOLUMEN; erst: sh SYSTEM/golemos/bauen.sh --arch $ARCH" >&2; exit 1; }

mkdir -p "$AUS"

# ── Alles in einem Behaelterlauf, damit Baum und Zwischenstaende im Volumen
#    bleiben (siehe `bauen.sh`, warum nicht im Bindpfad) ──────────────────
docker run --rm -i \
  -v "$VOLUMEN:/bau" \
  -v "$WURZEL:/repo:ro" \
  -e ARCH="$ARCH" \
  -e FASSUNG="$FASSUNG" \
  -e NUR_PRUEFEN="$NUR_PRUEFEN" \
  golemos-baumeister sh -s <<'IMBEHAELTER'
set -eu
BR="/bau/buildroot-$FASSUNG"
O=/bau/schicht-oberflaeche
EIN=/repo/SYSTEM/golemos
case "$ARCH" in
  x86_64) TUPEL=x86_64-buildroot-linux-gnu; RZIEL=x86_64-unknown-linux-gnu ;;
  aarch64) TUPEL=aarch64-buildroot-linux-gnu; RZIEL=aarch64-unknown-linux-gnu ;;
esac

# ── 1. Die Einstellung: Vorgabe der Basis, ohne was nur die Basis braucht,
#       und das Fragment der Schicht ──────────────────────────────────────
#
# ⚑ **Herausgenommen wird, was ein startbares System ausmacht**: Kern,
#   Startprogramm, Abbilder und die Skripte und Konten der Basis. Die
#   Schicht startet nichts davon; sie laeuft auf dem Kern der Basis.
mkdir -p /bau/eingabe
grep -v -E '^(BR2_LINUX_KERNEL|BR2_TARGET_GRUB2|BR2_TARGET_ROOTFS_(CPIO|EXT2)|BR2_PACKAGE_HOST_(GENIMAGE|DOSFSTOOLS|MTOOLS)|BR2_ROOTFS_(POST_BUILD_SCRIPT|POST_IMAGE_SCRIPT|OVERLAY|USERS_TABLES))' \
  "$EIN/konfigurationen/golemos_${ARCH}_defconfig" > /tmp/konf
cat "$EIN/konfigurationen/schicht_oberflaeche.fragment" >> /tmp/konf
# Die Pfade im Fragment zeigen auf /eingabe wie in `bauen.sh`; hier liegt
# das Repositorium unter /repo.
sed -i "s|/eingabe/konfigurationen/|$EIN/konfigurationen/|g" /tmp/konf

# ⚑ Das Overlay der Schicht: das Startskript, die Einhaengepunkte, die das
#   nur lesbare squashfs selbst mitbringen muss, und spaeter das Fenster.
mkdir -p /bau/schicht-overlay
cp -a "$EIN/schicht/oberflaeche/." /bau/schicht-overlay/
mkdir -p /bau/schicht-overlay/daten /bau/schicht-overlay/medien

make -C "$BR" O="$O" defconfig BR2_DEFCONFIG=/tmp/konf >/dev/null

# ⛔️ **Nachzaehlen, wie in `bauen.sh`, aber mit dem, was zuletzt gilt.**
#    Eine Zeile `X=…` muss genau so in der Einstellung stehen, eine Zeile
#    `# X is not set` darf es nicht. Buildroot verschluckt beides wortlos.
fehlend=0
while IFS= read -r zeile; do
  case "$zeile" in
    "# BR2_"*" is not set")
      name=${zeile#\# }; name=${name%% *}
      grep -q "^${name}=" "$O/.config" && { echo "   ⛔️ nicht abgeschaltet: $name" >&2; fehlend=$((fehlend + 1)); }
      ;;
    \#* | "") ;;
    *)
      grep -qxF "$zeile" "$O/.config" || { echo "   ⛔️ verschluckt: $zeile" >&2; fehlend=$((fehlend + 1)); }
      ;;
  esac
done < /tmp/konf
[ "$fehlend" -eq 0 ] || { echo "$fehlend Einstellungen nicht angekommen" >&2; exit 5; }
echo "   alle Einstellungen angekommen"
if [ "$NUR_PRUEFEN" = 1 ]; then
  echo "── Nur geprueft; der Bau selbst folgt ohne --nur-pruefen"
  exit 0
fi

# ── 2. Bauen ─────────────────────────────────────────────────────────
#
# ⛔️ **So viele Faeden, wie der Speicher traegt, nicht wie es Kerne gibt.**
#    📌 Gemessen am 2026-10-13: Mit 15 Faeden in einer Docker-VM mit
#    7,75 GiB wurde der Uebersetzer in WebKitGTK siebenmal vom Kern
#    abgeschossen (`Killed signal terminated program cc1plus`); eine
#    WebKit-Uebersetzungseinheit braucht bis zu zwei Gigabyte. Der Lauf
#    waere am Ende gescheitert, nach Stunden. 📌 Auch drei Faeden (zwei
#    Gigabyte je Faden) reichten nicht: Die Einheiten von WebCore brauchen
#    auf aarch64 rund drei Gigabyte, und bei Schritt 550 von 1835 kam der
#    naechste Abschuss. ⚑ Dreieinhalb Gigabyte je Faden, hoechstens einer
#    je Kern; wer Docker mehr Speicher gibt, baut schneller.
SPEICHER_KB=$(awk '/MemTotal/{print $2}' /proc/meminfo)
SPEICHER_GB=$((SPEICHER_KB / 1048576))
FAEDEN=$((SPEICHER_KB * 2 / 7 / 1048576))
[ "$FAEDEN" -ge 1 ] || FAEDEN=1
[ "$FAEDEN" -le "$(nproc)" ] || FAEDEN=$(nproc)
# 📌 **Und wenn es trotzdem nicht reicht, mit halb so vielen weiter.** Mit
#    zwei Faeden kam ein dritter Abschuss (Schritt 401 von 1283, 7,3 GB
#    frei, `-g0 -O2`): Einzelne Einheiten brauchen mehr als die Haelfte des
#    Speichers. Statt die Zahl weiter zu raten, wird nach einem Abschuss mit
#    der Haelfte am Stand weitergebaut, bis hinunter zu einem Faden. Ein
#    anderer Fehler bricht ab wie zuvor.
while :; do
  echo "── Schicht bauen mit $FAEDEN Faeden fuer ${SPEICHER_GB} GB (beim ersten Mal Stunden: WebKitGTK)"
  ( make -C "$BR" O="$O" BR2_JLEVEL="$FAEDEN"; echo "MAKE_RUECKGABE=$?" ) 2>&1 | tee /tmp/bau.log
  grep -q "^MAKE_RUECKGABE=0$" /tmp/bau.log && break
  if grep -q "Killed signal" /tmp/bau.log && [ "$FAEDEN" -gt 1 ]; then
    FAEDEN=$((FAEDEN / 2))
    echo "── Speicher reichte nicht; weiter mit $FAEDEN Faeden"
    continue
  fi
  echo "⛔️ Der Bau der Schicht ist gescheitert" >&2
  exit 6
done

# ── 3. Das Fenster gegen die Bibliotheken der Schicht ────────────────
#
# ⚑ **Mit rustup und nicht mit dem Rust von Buildroot**: Das Fenster und
#   seine Kisten verlangen die Fassung, gegen die das Repositorium geprueft
#   wird; gebunden wird trotzdem gegen den sysroot der Schicht, mit ihrem
#   Uebersetzer, damit es dort laeuft und nicht in Debian.
#
# ⚑ **Aus einer Kopie der Quellen im Volumen**, nicht aus dem
#   Repositorium: `tauri-build` schreibt beim Bauen Schemata nach
#   `gen/schemas/`, unter Linux eines, das es im Repositorium nicht gibt.
#   Das Repositorium bleibt nur lesbar eingehaengt.
echo "── Fenster bauen ($RZIEL)"
rsync -a --delete \
  --exclude .git --exclude SYSTEM/full-build --exclude SYSTEM/golemos/bau \
  --exclude SYSTEM/golemos/aus --exclude SYSTEM/golemos/abbild/fertig \
  --exclude SYSTEM/crates-vorrat --exclude 'SYSTEM/crates-lager/cargo-home' \
  --exclude MODELS --exclude INTEGER_LLM/artifacts --exclude INTEGER_LLM/calibrate/.venv \
  --exclude README --exclude MODULES \
  /repo/ /bau/quelle/
mkdir -p /bau/cargo-home
cat > /bau/cargo-home/config.toml <<KONFIG
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "/bau/quelle/SYSTEM/crates-lager/vendor"
KONFIG
rustup target add "$RZIEL" >/dev/null
SYSROOT="$O/host/$TUPEL/sysroot"
ZIEL_KLEIN=$(echo "$RZIEL" | tr '-' '_')
# ⚑ Auch Rust nach dem Speicher: eine Einheit braucht selten mehr als
#   anderthalb Gigabyte, also doppelt so viele Faeden wie fuer WebKit.
# 📌 **Der Linker steht in RUSTFLAGS, nicht in CARGO_TARGET_<ZIEL>_LINKER.**
#   Auf aarch64 tragen Bau-Container und Schicht dieselbe Kennung
#   (`aarch64-unknown-linux-gnu`), und dann gilt die Zieleinstellung auch
#   fuer Bauskripte und Prozedurmakros: Sie wurden gegen die glibc der
#   Schicht gelinkt (2.39) und liefen im Container (aeltere glibc) nicht
#   mehr. RUSTFLAGS dagegen gelten bei ausdruecklichem `--target` nur fuer
#   den Zielcode.
# ⚑ **pkg-config ist das aus Buildroot** (`host/bin/pkg-config`): Der
#   Bau-Container bringt keines mit, und dieses kennt die Sysroot der
#   Schicht schon von sich aus.
# ⚑ **Ohne die Vorgaben der Kiste, also ohne X11**: Die Schicht hat nur
#   Wayland, und Tauri verlangte sonst beim Bauen GDK fuer X11.
env \
  CARGO_BUILD_JOBS=$((FAEDEN * 2)) \
  CARGO_HOME=/bau/cargo-home \
  CARGO_TARGET_DIR=/bau/fenster-ziel \
  "RUSTFLAGS=-C linker=$O/host/bin/$TUPEL-gcc" \
  "CC_${ZIEL_KLEIN}=$O/host/bin/$TUPEL-gcc" \
  "CXX_${ZIEL_KLEIN}=$O/host/bin/$TUPEL-g++" \
  "AR_${ZIEL_KLEIN}=$O/host/bin/$TUPEL-ar" \
  PKG_CONFIG="$O/host/bin/pkg-config" \
  PKG_CONFIG_SYSROOT_DIR="$SYSROOT" \
  PKG_CONFIG_LIBDIR="$SYSROOT/usr/lib/pkgconfig:$SYSROOT/usr/share/pkgconfig" \
  PKG_CONFIG_ALLOW_CROSS=1 \
  cargo build --release --offline --locked --target "$RZIEL" --no-default-features \
    --manifest-path /bau/quelle/CLIENT/myl-oberflaeche/Cargo.toml
install -m 0755 "/bau/fenster-ziel/$RZIEL/release/myl-oberflaeche" /bau/schicht-overlay/usr/bin/myelith-fenster

# ── 4. Noch einmal zusammensetzen, jetzt mit dem Fenster ─────────────
make -C "$BR" O="$O" BR2_JLEVEL="$FAEDEN" >/dev/null

# ⛔️ **Findet das Fenster in der Schicht jede Bibliothek, die es braucht?**
#    Ein fehlendes `libwebkit2gtk` faellt sonst erst am Bildschirm auf,
#    als Fenster, das nicht aufgeht, ohne Meldung im Kiosk.
fehlt=0
for bib in $("$O/host/bin/$TUPEL-readelf" -d /bau/schicht-overlay/usr/bin/myelith-fenster | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p'); do
  if ! find "$O/target/lib" "$O/target/usr/lib" -name "$bib" 2>/dev/null | grep -q .; then
    echo "   ⛔️ $bib fehlt in der Schicht" >&2
    fehlt=$((fehlt + 1))
  fi
done
for prog in usr/bin/cage usr/bin/golem-kiosk usr/bin/myelith-fenster; do
  [ -e "$O/target/$prog" ] || { echo "   ⛔️ $prog fehlt in der Schicht" >&2; fehlt=$((fehlt + 1)); }
done
[ "$fehlt" -eq 0 ] || { echo "$fehlt Teile fehlen" >&2; exit 7; }
echo "   Fenster, Kompositor und alle Bibliotheken da"

mkdir -p /bau/aus
cp "$O/images/rootfs.squashfs" "/bau/aus/oberflaeche-$ARCH.sqfs"
IMBEHAELTER

[ "$NUR_PRUEFEN" = 1 ] && exit 0

echo "── Schicht herausholen"
docker run --rm -v "$VOLUMEN:/bau" -v "$AUS:/aus" golemos-baumeister \
  cp "/bau/aus/oberflaeche-$ARCH.sqfs" /aus/
( cd "$AUS" && shasum -a 256 "oberflaeche-$ARCH.sqfs" > "oberflaeche-$ARCH.sqfs.sha256" )

echo
echo "── Fertig"
ls -lh "$AUS/oberflaeche-$ARCH.sqfs" | awk '{print "   "$9"  "$5}'
cat <<WEITER

Auf GolemOS (0.3.0 oder neuer), mit der Datei auf dem Stick:

  golem-schicht holen /medien/<laufwerk>/oberflaeche-$ARCH.sqfs
  golem-schicht starten
WEITER
