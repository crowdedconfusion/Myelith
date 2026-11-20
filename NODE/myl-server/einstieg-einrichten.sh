#!/bin/sh
# Einen Einstiegsknoten unter Linux mit systemd einrichten und pflegen.
#
# Aufruf (als root, aus dem Klon):
#   sh einstieg-einrichten.sh --oeffentlich /dns4/<name>/tcp/4150 [--name einstieg]
#        [--netz test-int4] [--port 4150] [--erzeuger] [--programm <myl-node>]
#                         einrichten oder Einstellungen aendern, dann starten
#   sh einstieg-einrichten.sh --aktualisieren [--programm <myl-node>]
#                         nach einem Update: Programm neu legen, neu starten
#   sh einstieg-einrichten.sh --zeile [GEWICHT]   Zeile fuer den Startsatz
#   sh einstieg-einrichten.sh --adresse           eigene Adresse fuer die Einstiegsliste
#   sh einstieg-einrichten.sh --stand             Dienst, Hoehe, Gegenstellen
#
# ⚑ Das Programm kommt aus dem eigenen Bau: Vorgabe ist `~/.local/bin/myl-node`
# des Nutzers, der sudo aufruft (dorthin legt es `installieren-linux.sh`). Es
# wird nach /usr/local/lib/myelith kopiert, denn der Dienst sieht kein Home.
#
# ⚑ Es installiert keine Pakete und oeffnet keine Firewall. Es sagt, was der
# Betreiber dafuer tun muss.
set -eu

cd "$(dirname "$0")/../.."
WURZEL=$(pwd)

DIENST=myl-einstieg
NUTZER=myl-einstieg
ZIEL=/usr/local/lib/myelith
KONF=/etc/myl-einstieg
ZUSTAND=/var/lib/myl-einstieg
UNIT=/etc/systemd/system/$DIENST.service

AKTION=einrichten
NAME=""
NETZ=""
PORT=""
OEFFENTLICH=""
ERZEUGER=""
PROGRAMM=""
GEWICHT=100

while [ $# -gt 0 ]; do
  case "$1" in
    --oeffentlich) OEFFENTLICH=${2:?--oeffentlich braucht eine Adresse}; shift ;;
    --name)        NAME=${2:?--name braucht einen Namen}; shift ;;
    --netz)        NETZ=${2:?--netz braucht ein Netz}; shift ;;
    --port)        PORT=${2:?--port braucht eine Zahl}; shift ;;
    --erzeuger)    ERZEUGER=ja ;;
    --ohne-erzeuger) ERZEUGER=nein ;;
    --programm)    PROGRAMM=${2:?--programm braucht einen Pfad}; shift ;;
    --aktualisieren) AKTION=aktualisieren ;;
    --zeile)       AKTION=zeile
                   case "${2:-}" in ''|-*) ;; *) GEWICHT=$2; shift ;; esac ;;
    --adresse)     AKTION=adresse ;;
    --stand)       AKTION=stand ;;
    -h|--hilfe|--help)
      sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "unbekannter Schalter: $1" >&2; exit 2 ;;
  esac
  shift
done

if [ "$(uname -s)" != "Linux" ] || ! command -v systemctl >/dev/null 2>&1; then
  echo "Dieses Skript ist fuer Linux mit systemd. Unter NixOS: NODE/myl-server/modul.nix." >&2
  exit 2
fi
if [ "$(id -u)" -ne 0 ]; then
  echo "Bitte mit sudo aufrufen: sudo sh $0 $*" >&2
  exit 2
fi

# Die eigene Adresse aus der Zeile `eigene_adresse` des juengsten Protokolls.
adresse() {
  datei=$(ls -t "$ZUSTAND"/logs/*.jsonl 2>/dev/null | head -n 1 || true)
  [ -n "$datei" ] || return 1
  zeile=$(grep '"art":"eigene_adresse"' "$datei" | head -n 1 || true)
  [ -n "$zeile" ] || return 1
  peer=$(printf '%s' "$zeile" | sed 's/.*"peer":"\([^"]*\)".*/\1/')
  addr=$(printf '%s' "$zeile" | sed 's/.*"addr":"\([^"]*\)".*/\1/')
  printf '%s/p2p/%s\n' "$addr" "$peer"
}

programm_legen() {
  if [ -z "$PROGRAMM" ]; then
    heim=$(getent passwd "${SUDO_USER:-root}" | cut -d: -f6)
    PROGRAMM="$heim/.local/bin/myl-node"
  fi
  if [ ! -x "$PROGRAMM" ]; then
    echo "Kein Programm unter $PROGRAMM. Erst bauen: sh SYSTEM/install/installieren-linux.sh" >&2
    echo "(als der Nutzer, nicht als root), oder --programm <pfad> angeben." >&2
    exit 1
  fi
  install -d -m 0755 "$ZIEL" "$ZIEL/conformance/vectors/op"
  install -m 0755 "$PROGRAMM" "$ZIEL/myl-node.neu"
  mv -f "$ZIEL/myl-node.neu" "$ZIEL/myl-node"
  # Die Konformitaetsvektoren neben das Programm: Dort sucht der Knoten sie,
  # wenn er nicht aus dem Klon heraus startet.
  for v in "$WURZEL"/INTEGER_LLM/conformance/vectors/op/*.json; do
    install -m 0644 "$v" "$ZIEL/conformance/vectors/op/"
  done
  echo "Programm gelegt: $ZIEL/myl-node ($("$ZIEL/myl-node" --hilfe 2>/dev/null | head -n 1 || echo '?'))"
}

nutzer_anlegen() {
  if ! getent passwd "$NUTZER" >/dev/null; then
    schale=$(command -v nologin || echo /bin/false)
    useradd --system --user-group --home-dir "$ZUSTAND" --no-create-home \
      --shell "$schale" --comment "Myelith Einstiegsknoten" "$NUTZER"
  fi
  install -d -m 0700 -o "$NUTZER" -g "$NUTZER" "$ZUSTAND"
  install -d -m 0750 -o root -g "$NUTZER" "$KONF"
}

# Ein Wert aus der bestehenden Einstellungsdatei, sonst die Vorgabe.
bisher() {
  if [ -f "$KONF/einstieg.env" ]; then
    w=$(sed -n "s/^$1=\"\{0,1\}\([^\"]*\)\"\{0,1\}\$/\1/p" "$KONF/einstieg.env" | head -n 1)
    [ -n "$w" ] && { printf '%s' "$w"; return; }
  fi
  printf '%s' "$2"
}

einstellungen_schreiben() {
  NAME=${NAME:-$(bisher NAME einstieg)}
  NETZ=${NETZ:-$(bisher NETZ test-int4)}
  PORT=${PORT:-$(bisher PORT 4150)}
  OEFFENTLICH=${OEFFENTLICH:-$(bisher OEFFENTLICH "")}
  zusatz=$(bisher ZUSATZ "")
  case "$ERZEUGER" in
    ja)   case " $zusatz " in *" --erzeuger "*) ;; *) zusatz="$zusatz --erzeuger" ;; esac ;;
    nein) zusatz=$(printf '%s' " $zusatz " | sed 's/ --erzeuger / /g') ;;
  esac
  zusatz=$(printf '%s' "$zusatz" | sed 's/^ *//; s/ *$//')
  if [ -z "$OEFFENTLICH" ]; then
    echo "Ein Einstiegsknoten braucht seine Adresse von aussen:" >&2
    echo "  --oeffentlich /dns4/<name>/tcp/$PORT   (Name ueber dynamisches DNS)" >&2
    exit 2
  fi
  case "$OEFFENTLICH" in
    /dns4/*|/dns6/*|/dns/*|/ip4/*|/ip6/*) ;;
    *) echo "--oeffentlich ist eine Multiadresse, etwa /dns4/<name>/tcp/$PORT" >&2; exit 2 ;;
  esac
  umask 027
  cat > "$KONF/einstieg.env.neu" <<EOF
# Einstellungen des Einstiegsknotens; gelesen von $DIENST.service.
# Geaendert mit: sudo sh NODE/myl-server/einstieg-einrichten.sh --name ... --oeffentlich ...
NAME="$NAME"
NETZ="$NETZ"
PORT="$PORT"
OEFFENTLICH="$OEFFENTLICH"
# Weitere Schalter an myl-node, durch Leerzeichen getrennt. --erzeuger nur,
# wenn der Konsensschluessel dieses Knotens im Startsatz steht.
ZUSATZ="$zusatz"
EOF
  chown root:"$NUTZER" "$KONF/einstieg.env.neu"
  mv -f "$KONF/einstieg.env.neu" "$KONF/einstieg.env"
}

case "$AKTION" in
  zeile)
    nutzer_anlegen
    [ -x "$ZIEL/myl-node" ] || programm_legen
    # Als Dienstnutzer, damit der Schluessel ihm gehoert und 0600 bleibt.
    runuser -u "$NUTZER" -- "$ZIEL/myl-node" \
      --konsensschluessel "$ZUSTAND/konsens.key" --stimmsatzzeile "$GEWICHT"
    exit 0 ;;
  adresse)
    adresse || { echo "Noch keine Adresse im Protokoll; laeuft der Dienst? sudo systemctl status $DIENST" >&2; exit 1; }
    exit 0 ;;
  stand)
    systemctl --no-pager status "$DIENST" | head -n 5 || true
    if command -v curl >/dev/null 2>&1; then
      curl -s --max-time 3 http://127.0.0.1:4151/metriken \
        | grep -E '^myelith_(kette_hoehe|kette_rueckstand|peers) ' || true
    fi
    adresse || true
    exit 0 ;;
  aktualisieren)
    [ -f "$KONF/einstieg.env" ] || { echo "Noch nicht eingerichtet; erst ohne --aktualisieren aufrufen." >&2; exit 2; }
    programm_legen
    install -m 0644 "$WURZEL/NODE/myl-server/$DIENST.service" "$UNIT"
    systemctl daemon-reload
    systemctl restart "$DIENST"
    echo "Neu gestartet. Stand: sudo sh $0 --stand"
    exit 0 ;;
esac

nutzer_anlegen
programm_legen
einstellungen_schreiben
install -m 0644 "$WURZEL/NODE/myl-server/$DIENST.service" "$UNIT"
systemctl daemon-reload
systemctl enable "$DIENST" >/dev/null 2>&1 || true
systemctl reset-failed "$DIENST" >/dev/null 2>&1 || true
systemctl restart "$DIENST"

cat <<EOF

Eingerichtet: Dienst $DIENST, Einstellungen $KONF/einstieg.env,
Zustand $ZUSTAND (Schluessel, Kette, Protokolle).

Noch zu tun, ausserhalb dieses Skripts:
  1. Firewall: Port $PORT fuer TCP und UDP oeffnen, etwa
       sudo ufw allow $PORT/tcp && sudo ufw allow $PORT/udp
     oder mit firewalld:
       sudo firewall-cmd --permanent --add-port=$PORT/tcp --add-port=$PORT/udp && sudo firewall-cmd --reload
  2. Router: Port $PORT TCP und UDP an diesen Rechner weiterleiten (feste
     Adresse im LAN per DHCP-Reservierung, kein UPnP).
  3. Dynamisches DNS: der Name in $OEFFENTLICH muss auf die Adresse des
     Anschlusses zeigen (im Router oder mit einem Dienst wie ddclient).
  4. Die eigene Adresse fuer die Einstiegsliste, nach dem Start:
       sudo sh $0 --adresse
EOF
sleep 3
if ! systemctl is-active --quiet "$DIENST"; then
  echo
  echo "⚠️  Der Dienst laeuft nicht. Letzte Meldungen:"
  journalctl -u "$DIENST" -n 15 --no-pager 2>/dev/null || true
  exit 1
fi
adresse 2>/dev/null && true
