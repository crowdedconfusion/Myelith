#!/bin/sh
# Myelith auf macOS einrichten, aus einem frischen Klon.
#
# Aufruf:
#   sh installieren-macos.sh                 baut und legt alles nach ~/
#   sh installieren-macos.sh --system        nach /usr/local/bin und /Applications
#   sh installieren-macos.sh --aktualisieren holt erst neuen Stand, baut dann
#   sh installieren-macos.sh --pruefen       sagt nur, was fehlt, und baut nicht
#
# ⚑ **Warum aus dem Quelltext und nicht aus einem Buendel.** Die
# Freigabebuendel dieses Projekts sind **nicht signiert**. Ein Skript,
# das ein unsigniertes Buendel holt und an Gatekeeper vorbeischiebt,
# brächte einem Nutzer bei, genau das zu tun. Wer aus dem Klon baut,
# baut aus dem, was er lesen kann.
#
# ⚑ **Und es installiert nichts hinter dem Ruecken.** Fehlt die
# Rust-Werkzeugkette, nennt dieses Skript den einen Befehl und hoert
# auf. Ein Installationsskript, das ein zweites aus dem Netz nachlaedt
# und ausfuehrt, ist die Angriffsflaeche, gegen die dieses Projekt an
# jeder anderen Stelle argumentiert.
set -eu

# 📌 **Eine Ebene hoeher, seit dem 2026-09-10.** Diese Datei lag in der
# Wurzel und liegt jetzt in `INSTALL/`. **Ein Verschieben sieht aus wie
# eine Aenderung ohne Verhalten und ist keine:** Ohne das `/..` zeigte
# die Wurzel auf das Skriptverzeichnis, und der Bau faende keine
# einzige Kiste. Dieselbe Klasse wie Fund 298.
cd "$(dirname "$0")/.."
WURZEL=$(pwd)

SYSTEMWEIT=nein
AKTUALISIEREN=nein
NUR_PRUEFEN=nein
for a in "$@"; do
  case "$a" in
    --system)        SYSTEMWEIT=ja ;;
    --aktualisieren) AKTUALISIEREN=ja ;;
    --pruefen)       NUR_PRUEFEN=ja ;;
    -h|--hilfe|--help)
      sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "unbekannter Schalter: $a" >&2; exit 2 ;;
  esac
done

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Dieses Skript ist fuer macOS. Fuer NixOS: installieren-nixos.sh," >&2
  echo "fuer Windows: installieren-windows.ps1." >&2
  exit 2
fi

echo "── Myelith einrichten"
echo "   Quelle: $WURZEL"

# ── Was da sein muss ────────────────────────────────────────────────
#
# ⚑ **Erst alles pruefen, dann bauen.** Ein Bau, der nach acht Minuten
# an einem fehlenden Werkzeug abbricht, hat acht Minuten gekostet und
# nichts gesagt, was er nicht vorher haette sagen koennen.
fehlt=""

if ! xcode-select -p >/dev/null 2>&1; then
  fehlt="$fehlt
  Xcode-Befehlszeilenwerkzeuge fehlen. Holen mit:
      xcode-select --install"
fi

# 📌 **Nicht „gibt es den Befehl", sondern „laeuft er".** Ein
# rustup-Schalter ohne eingestellte Werkzeugkette liegt im PATH und
# beantwortet `command -v` mit ja; `cargo --version` bricht dann ab.
# Gefunden beim ersten Probelauf dieses Skripts: Es meldete „alles da",
# druckte eine leere Version und scheiterte drei Zeilen spaeter im Bau.
if ! cargo --version >/dev/null 2>&1; then
  fehlt="$fehlt
  Die Rust-Werkzeugkette fehlt oder hat keine eingestellte Fassung. Holen mit:
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  Danach eine neue Sitzung oeffnen oder:
      . \"\$HOME/.cargo/env\"
  Ist rustup schon da, fehlt vielleicht nur die Voreinstellung:
      rustup default stable"
fi

if [ "$AKTUALISIEREN" = ja ] && ! command -v git >/dev/null 2>&1; then
  fehlt="$fehlt
  git fehlt, und ohne git gibt es keinen neuen Stand zu holen."
fi

if [ -n "$fehlt" ]; then
  echo "── Es fehlt etwas:$fehlt" >&2
  exit 1
fi

echo "   Xcode-Werkzeuge: da"
echo "   cargo: $(cargo --version)"

if [ "$NUR_PRUEFEN" = ja ]; then
  echo "── Alles da. Ohne --pruefen wird gebaut."
  exit 0
fi

# ── Neuen Stand holen, wenn gewuenscht ──────────────────────────────
#
# 📌 **`--ff-only`, und das ist die ganze Vorsicht.** Wer im Klon
# gearbeitet hat, soll seine Arbeit nicht durch ein Installationsskript
# verlieren. Geht es nicht vorwaerts, bricht es ab und sagt, warum.
if [ "$AKTUALISIEREN" = ja ]; then
  echo "── neuen Stand holen"
  if [ ! -d .git ]; then
    echo "   Das hier ist kein Klon, sondern ein entpacktes Verzeichnis." >&2
    echo "   Es gibt nichts zu holen; ohne --aktualisieren baut es aus dem, was da ist." >&2
    exit 1
  fi
  git fetch --quiet origin
  if ! git merge --ff-only --quiet FETCH_HEAD 2>/dev/null; then
    echo "   Der Klon laesst sich nicht vorwaerts bewegen." >&2
    echo "   Wahrscheinlich liegen eigene Aenderungen darauf. Erst sichern, dann noch einmal." >&2
    exit 1
  fi
  echo "   Stand: $(git rev-parse --short HEAD)"
fi

# ── Bauen ───────────────────────────────────────────────────────────
#
# ── Was ausgeliefert wird ───────────────────────────────────────────
#
# ⚑ **Die Liste wird gefunden und nicht gefuehrt.** Jede Kiste, die
# ausgeliefert werden soll, sagt es in ihrer eigenen `Cargo.toml`
# unter `[package.metadata.myelith]`; dieses Skript sammelt sie ein.
#
# 📌 **Bis zum 2026-09-10 stand die Liste hier, von Hand** (Fund 300),
# und dasselbe noch dreimal in den anderen Skripten. Zwei der vier
# waren am ersten Tag schon uneinig: `myl-test` fehlte in dreien.
# **Eine Liste, die an vier Stellen von Hand gefuehrt wird, ist kein
# Verzeichnis, sondern vier Behauptungen.**
programme() {
  find "$WURZEL" -name Cargo.toml -not -path '*/target*' | sort | while read -r kiste; do
    name=$(sed -n 's/^ausliefern = "\(.*\)"$/\1/p' "$kiste" | head -1)
    [ -n "$name" ] || continue
    verzeichnis=$(dirname "$kiste")
    echo "${verzeichnis#"$WURZEL"/} $name"
  done
}

PROGRAMME=$(programme)
if [ -z "$PROGRAMME" ]; then
  echo "Keine einzige Kiste ist zum Ausliefern angemeldet." >&2
  echo "Gesucht wurde `[package.metadata.myelith]` mit `ausliefern`." >&2
  exit 1
fi

# 📌 **Eine Schleife in dieser Shell und nicht hinter einer Roehre.**
# `... | while read` laeuft in einer Unterschale; ein fehlgeschlagener
# Bau darin beendet dieses Skript **nicht**, und es kopierte danach
# munter Dateien, die es nicht gibt. Ebenfalls beim ersten Probelauf
# gefunden.
echo "── bauen, das dauert beim ersten Mal einige Minuten"
ALTES_IFS=$IFS
IFS='
'
for zeile in $PROGRAMME; do
  IFS=$ALTES_IFS
  # shellcheck disable=SC2086
  set -- $zeile
  echo "   $2"
  ( cd "$WURZEL/$1" && cargo build --release --quiet )
  IFS='
'
done
IFS=$ALTES_IFS

echo "── Myelith.app buendeln"
sh CLIENT/myl-oberflaeche/buendeln-macos.sh >/dev/null

# ── Legen ───────────────────────────────────────────────────────────
#
# ⚑ **Ohne sudo, solange es geht.** Der Standardweg legt alles unter
# das eigene Benutzerverzeichnis; wer es systemweit will, sagt es.
if [ "$SYSTEMWEIT" = ja ]; then
  BIN=/usr/local/bin
  APPS=/Applications
  SUDO=sudo
else
  BIN="$HOME/.local/bin"
  APPS="$HOME/Applications"
  SUDO=""
fi

$SUDO mkdir -p "$BIN" "$APPS"
ALTES_IFS=$IFS
IFS='
'
for zeile in $PROGRAMME; do
  IFS=$ALTES_IFS
  # shellcheck disable=SC2086
  set -- $zeile
  # ⛔️ **Erst weg, dann hin.** Wird eine Programmdatei an Ort und Stelle
  # ueberschrieben, behaelt macOS die alte Signatur im Zwischenspeicher;
  # sie passt dann nicht mehr zum neuen Inhalt, und der Kern erschlaegt
  # das Programm beim Start mit SIGKILL, ohne ein Wort. Gesehen am
  # 2026-09-15 an `myelith`: Rueckgabe 137, keine Ausgabe, waehrend
  # dieselbe Datei aus `target-shared` lief. Fuer `Myelith.app` stand das
  # `rm -rf` schon darunter; fuer die Programme fehlte es.
  $SUDO rm -f "$BIN/$2"
  $SUDO cp "$WURZEL/target-shared/release/$2" "$BIN/$2"
  IFS='
'
done
IFS=$ALTES_IFS
$SUDO rm -rf "$APPS/Myelith.app"
$SUDO cp -R "$WURZEL/target-shared/Myelith.app" "$APPS/Myelith.app"

echo "── fertig"
# 📌 **Auch hier stand die Liste von Hand** und nannte drei, waehrend
# vier installiert wurden (Fund 300, zweite Stelle im selben Skript).
# Eine Schlussmeldung, die etwas anderes aufzaehlt als das, was getan
# wurde, ist schlimmer als keine.
echo "   Programme:"
ALTES_IFS=$IFS
IFS='
'
for zeile in $PROGRAMME; do
  IFS=$ALTES_IFS
  # shellcheck disable=SC2086
  set -- $zeile
  echo "     $BIN/$2"
  IFS='
'
done
IFS=$ALTES_IFS
echo "   Fenster:   $APPS/Myelith.app"

case ":$PATH:" in
  *":$BIN:"*) ;;
  *)
    echo
    echo "⚠️  $BIN steht nicht in deinem PATH. Zeile fuer die Shell-Datei:"
    echo "      export PATH=\"$BIN:\$PATH\"" ;;
esac

# 📌 **Der erste Start meldet einen unbekannten Entwickler**, denn das
# Buendel ist nicht signiert. Das steht hier, weil es sonst wie ein
# Fehler aussieht.
echo
echo "Beim ersten Start meldet Gatekeeper einen unbekannten Entwickler:"
echo "Rechtsklick auf Myelith.app, dann Oeffnen, einmal bestaetigen."
