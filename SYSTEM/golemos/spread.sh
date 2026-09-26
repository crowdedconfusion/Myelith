#!/bin/sh
# Verteilt GolemOS auf einen Datentraeger.
#
#   sh SYSTEM/golemos/spread.sh                 zeigt, was in Frage kommt
#   sh SYSTEM/golemos/spread.sh /dev/disk4      schreibt darauf
#
# ⛔️ **Das hier ist der gefaehrlichste Handgriff des ganzen Vorhabens.**
#
# `dd` fragt nicht und kennt kein Zurueck. Ein Zeichen falsch, und die
# Systemplatte ist weg, mit allem darauf. Deshalb steht zwischen dem
# Aufruf und dem Schreiben mehr als eine Ja-Nein-Frage:
#
#   1. Ohne Argument **zeigt** es nur, was in Frage kommt, und schreibt
#      nichts. ⚑ Der erste Aufruf kann nichts kaputtmachen.
#   2. Ein Datentraeger, der die Systemplatte ist, wird **abgelehnt**,
#      nicht nur bewarnt.
#   3. Ein Datentraeger, der nicht wechselbar ist, braucht ein
#      ausdrueckliches `--auch-feste-platten`.
#   4. Zur Bestaetigung wird der **Geraetename abgetippt**, nicht „j".
#      Ein `j` rutscht heraus; ein Name, den man selbst liest und
#      abschreibt, nicht.
#
# ⚠️ **Kein `--kraft`, das alles ueberspringt.** Wer einen Schalter
# baut, der jede Sicherung abschaltet, hat keine Sicherungen gebaut.
set -eu

HIER=$(cd "$(dirname "$0")" && pwd)
ARCH=x86_64
FESTE=nein
GERAET=""

while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --auch-feste-platten) FESTE=ja; shift ;;
    -h|--hilfe) sed -n '2,30p' "$0"; exit 0 ;;
    -*) echo "unbekannt: $1" >&2; exit 2 ;;
    *) GERAET="$1"; shift ;;
  esac
done

# ── Welches Abbild ─────────────────────────────────────────────────
#
# ⚑ **Ein selbst gebautes gewinnt vor dem mitgelieferten.** Wer gerade
# gebaut hat, will sein Erzeugnis schreiben und nicht den Stand aus
# dem Repositorium.
#
# ⚑ **Und ohne Bau geht es trotzdem.** Unter `abbild/fertig/` liegt ein
# gepacktes Abbild (31 MB), damit der allererste Schritt **kein Docker
# und keine Stunde Bauzeit** braucht. Der Zettel daneben sagt, woher es
# kommt.
GEPACKT="$HIER/abbild/fertig/golemos-$ARCH.img.xz"
ABBILD="$HIER/aus/$ARCH/golemos.img"
GEQUELLE=""

if [ -f "$ABBILD" ]; then
  GEQUELLE="selbst gebaut"
elif [ -f "$GEPACKT" ]; then
  # ⛔️ **Erst pruefen, dann auspacken.** Dieses Skript loescht einen
  #    ganzen Datentraeger; es soll wissen, dass es das Richtige
  #    schreibt. Eine Pruefsumme, die danebenliegt und niemand liest,
  #    ist Zierde.
  SUMMENDATEI="$GEPACKT.sha256"
  if [ -f "$SUMMENDATEI" ]; then
    printf "── Pruefsumme des mitgelieferten Abbilds … "
    if command -v shasum >/dev/null 2>&1; then
      IST=$(shasum -a 256 "$GEPACKT" | cut -d" " -f1)
    else
      IST=$(sha256sum "$GEPACKT" | cut -d" " -f1)
    fi
    SOLL=$(cut -d" " -f1 < "$SUMMENDATEI")
    if [ "$IST" != "$SOLL" ]; then
      echo "⛔️ stimmt nicht"
      echo "   erwartet $SOLL" >&2
      echo "   bekommen $IST" >&2
      echo "   Es wird nichts geschrieben." >&2
      exit 8
    fi
    echo "stimmt"
  else
    echo "⚠️  Keine Pruefsumme neben $GEPACKT; es wird ungeprueft ausgepackt." >&2
  fi
  ABBILD="$HIER/aus/$ARCH/golemos.img"
  mkdir -p "$(dirname "$ABBILD")"
  echo "── Abbild auspacken"
  xz -dc "$GEPACKT" > "$ABBILD"
  GEQUELLE="mitgeliefert, ausgepackt"
else
  echo "Kein Abbild fuer $ARCH." >&2
  echo "  erwartet: $GEPACKT" >&2
  echo "  oder bauen: sh SYSTEM/golemos/bauen.sh --arch $ARCH" >&2
  exit 1
fi

# ── Was in Frage kommt ──────────────────────────────────────────────
zeigen_macos() {
  echo "Wechselbare Datentraeger:"
  diskutil list external physical 2>/dev/null | grep -E "^/dev/disk" || echo "  (keine)"
  echo
  echo "⛔️ Deine Systemplatte (NICHT nehmen):"
  diskutil info / 2>/dev/null | awk -F: '/Part of Whole/{gsub(/ /,"",$2); print "  /dev/"$2}'
}
zeigen_linux() {
  echo "Datentraeger:"
  lsblk -dno NAME,SIZE,RM,MODEL 2>/dev/null | awk '{printf "  /dev/%-8s %6s  %s  %s\n", $1, $2, ($3=="1"?"wechselbar":"fest      "), substr($0, index($0,$4))}'
  echo
  echo "⛔️ Wurzel liegt auf:"
  lsblk -no PKNAME "$(findmnt -no SOURCE / 2>/dev/null)" 2>/dev/null | head -1 | sed 's|^|  /dev/|'
}

if [ -z "$GERAET" ]; then
  echo "── GolemOS auf einen Datentraeger schreiben"
  echo "   Abbild: $ABBILD ($(du -h "$ABBILD" | cut -f1), $GEQUELLE)"
  echo
  case "$(uname -s)" in
    Darwin) zeigen_macos ;;
    Linux)  zeigen_linux ;;
    *) echo "Unbekanntes System; nimm ein Werkzeug deiner Plattform." ;;
  esac
  echo
  echo "Dann:  sh $0 --arch $ARCH <geraet>"
  exit 0
fi

# ── Die Sicherungen ─────────────────────────────────────────────────
[ -e "$GERAET" ] || { echo "⛔️ $GERAET gibt es nicht." >&2; exit 3; }

SYSTEM=""
case "$(uname -s)" in
  Darwin)
    SYSTEM=$(diskutil info / 2>/dev/null | awk -F: '/Part of Whole/{gsub(/ /,"",$2); print "/dev/"$2}')
    WECHSELBAR=$(diskutil info "$GERAET" 2>/dev/null | awk -F: '/Removable Media/{gsub(/ /,"",$2); print $2}')
    [ "$WECHSELBAR" = "Removable" ] && WECHSELBAR=ja || WECHSELBAR=nein
    GROESSE=$(diskutil info "$GERAET" 2>/dev/null | awk -F: '/Disk Size/{print $2; exit}' | sed 's/^ *//')
    NAME=$(diskutil info "$GERAET" 2>/dev/null | awk -F: '/Device \/ Media Name/{print $2; exit}' | sed 's/^ *//')
    ;;
  Linux)
    # ⚑ Die Systemplatte ueber `lsblk`, dieselbe Quelle wie im
    #   Einrichtungsassistenten.
    #   📌 **Ein Verdacht, der sich nicht bestaetigt hat** (2026-09-25):
    #   `/proc/mounts` nennt die Wurzel unter GolemOS `/dev/root`, und es
    #   sah so aus, als liefe `findmnt` damit ins Leere. Gemessen nennt
    #   `findmnt` dort `/dev/sda2`; die alte Zeile haette also gegriffen.
    SYSTEM=$(lsblk -rno PKNAME,MOUNTPOINT 2>/dev/null | awk '$2=="/" {print "/dev/"$1; exit}')
    # ⚑ **Dazu jede Platte, auf der `/daten` oder das Abbild selbst
    #   liegt.** Laeuft GolemOS von der Platte und liegt der
    #   Myelith-Ordner auf einem USB-Laufwerk, ist dieses Laufwerk nicht
    #   die Systemplatte, und ohne diese Zeilen waere es beschreibbar,
    #   waehrend `dd` das Abbild von ihm liest.
    BESETZT=""
    for ort in $(lsblk -rno MOUNTPOINT "$GERAET" 2>/dev/null); do
      case "$ort" in
        /|/daten) BESETZT="$ort" ;;
        *) case "$ABBILD" in "$ort"/*) BESETZT="$ort" ;; esac ;;
      esac
    done
    if [ -n "$BESETZT" ]; then
      echo "⛔️ Auf $GERAET liegt $BESETZT; von dort laeuft oder liest dieses System." >&2
      echo "   Das wird nicht geschrieben, auch nicht auf Nachfrage." >&2
      exit 4
    fi
    B=$(basename "$GERAET")
    [ "$(cat "/sys/block/$B/removable" 2>/dev/null || echo 0)" = 1 ] && WECHSELBAR=ja || WECHSELBAR=nein
    GROESSE=$(lsblk -dno SIZE "$GERAET" 2>/dev/null)
    NAME=$(lsblk -dno MODEL "$GERAET" 2>/dev/null)
    ;;
esac

# ⛔️ Sicherung 2: niemals die Systemplatte.
if [ -n "$SYSTEM" ] && [ "$GERAET" = "$SYSTEM" ]; then
  echo "⛔️ $GERAET ist die Platte, auf der dieses System laeuft." >&2
  echo "   Das wird nicht geschrieben, auch nicht auf Nachfrage." >&2
  exit 4
fi

# ⛔️ Sicherung 3: feste Platten nur ausdruecklich.
if [ "${WECHSELBAR:-nein}" != ja ] && [ "$FESTE" != ja ]; then
  echo "⛔️ $GERAET ist kein wechselbarer Datentraeger." >&2
  echo "   Wenn du das wirklich willst: --auch-feste-platten" >&2
  exit 5
fi

cat <<ENDE

⚠️  Es wird ALLES auf diesem Datentraeger geloescht:

      Geraet    $GERAET
      Name      ${NAME:-unbekannt}
      Groesse   ${GROESSE:-unbekannt}
      Abbild    $(basename "$ABBILD") ($(du -h "$ABBILD" | cut -f1))

ENDE

# ⛔️ Sicherung 4: den Namen abtippen, nicht „j".
printf "Tipp zur Bestaetigung den Geraetenamen ab (%s): " "$GERAET"
read -r ANTWORT
[ "$ANTWORT" = "$GERAET" ] || { echo "Abgebrochen."; exit 6; }

case "$(uname -s)" in
  Darwin)
    diskutil unmountDisk "$GERAET" >/dev/null 2>&1 || true
    # ⚑ `rdisk` statt `disk`: der rohe Weg ist um ein Vielfaches
    #   schneller, weil er den Puffercache umgeht.
    ROH=$(echo "$GERAET" | sed 's|/dev/disk|/dev/rdisk|')
    echo "── Schreiben nach $ROH, das dauert einige Minuten"
    sudo dd if="$ABBILD" of="$ROH" bs=4m status=progress
    sudo diskutil eject "$GERAET" >/dev/null 2>&1 || true
    ;;
  Linux)
    for t in "$GERAET"*; do umount "$t" 2>/dev/null || true; done
    echo "── Schreiben nach $GERAET, das dauert einige Minuten"
    # ⚑ **Ohne `sudo`, wenn schon root.** Unter GolemOS gibt es kein
    #   `sudo`, und man ist dort root; der Myelith-Ordner auf dem Stick
    #   bringt dieses Skript mit, damit GolemOS selbst neue Sticks
    #   schreiben kann.
    ALS_ROOT=""
    [ "$(id -u)" = 0 ] || ALS_ROOT=sudo
    # `status=progress` kann das `dd` von BusyBox nicht immer; dann ohne.
    FORTSCHRITT=""
    dd if=/dev/zero of=/dev/null count=0 status=progress 2>/dev/null && FORTSCHRITT="status=progress"
    $ALS_ROOT dd if="$ABBILD" of="$GERAET" bs=4M conv=fsync $FORTSCHRITT
    $ALS_ROOT sync
    ;;
esac

cat <<'ENDE'

── Fertig.

Weiter so:

  1. Stick in den Rechner stecken, von dem GolemOS starten soll,
     und einmal davon booten; als "root" anmelden. Der Assistent
     erscheint. ⚑ Der erste Start dehnt die Datenpartition auf den
     ganzen Stick.
  2. Stick wieder an deinen Rechner: "Golem" erscheint, darin
     README.txt. Deinen Myelith-Ordner daraufziehen.
  3. Wieder davon starten. GolemOS findet den Ordner und stellt ein
     Modell ein. Im Assistenten:
       1  Myelith benutzen, ohne etwas zu installieren
       3  GolemOS dauerhaft auf eine Platte installieren

Die ganze Anleitung: SYSTEM/golemos/ANLEITUNG.md
ENDE
