#!/bin/sh
# Myelith unter Linux einrichten, aus einem Klon oder einer Kopie des Baums.
#
# Aufruf:
#   sh installieren-linux.sh                 baut aus dem Vorrat, legt nach ~/.local
#   sh installieren-linux.sh --aktualisieren holt erst neuen Stand, baut dann
#   sh installieren-linux.sh --pruefen       sagt nur, was fehlt, und baut nicht
#   sh installieren-linux.sh --ohne-fenster  baut alles ausser dem Fenster
#   sh installieren-linux.sh --nix           nimmt die Nix-Umgebung (wie auf NixOS)
#
# ⚑ **Ein Einstieg fuer jedes Linux.** Auf NixOS (oder mit `--nix`) gibt
# dieses Skript an `installieren-nixos.sh` ab, denn dort liegt nichts an
# einem Ort, den ein Linker raet. Sonst baut es selbst: Debian, Ubuntu,
# Fedora, Arch, openSUSE und was ihnen gleicht.
#
# ⚑ **Netz braucht der Bau nicht.** Alle Rust-Abhaengigkeiten liegen als
# Archive im Baum und werden hier ausgepackt.
#
# ⚑ **Eine fehlende Systembibliothek kostet eine Funktion, nie den Bau.**
# Nur das Fenster braucht WebKitGTK und Nachbarn. Fehlen sie, baut das
# Skript die anderen Programme, laesst das Fenster aus und nennt den
# Befehl, der die Pakete holt.
#
# ⚑ **Und es installiert nichts hinter dem Ruecken.** Es ruft keinen
# Paketverwalter und kein sudo; es sagt, was fehlt, und hoert auf oder
# laesst aus.
set -eu

# 📌 **Zwei Ebenen hoeher**: Diese Datei liegt unter `SYSTEM/install/`.
# Mit einem `/..` zu wenig zeigte die Wurzel auf `SYSTEM/`, und der Bau
# faende keine einzige Kiste (dieselbe Klasse wie Fund 298).
cd "$(dirname "$0")/../.."
WURZEL=$(pwd)

AKTUALISIEREN=nein
NUR_PRUEFEN=nein
OHNE_FENSTER=nein
MIT_NIX=nein
for a in "$@"; do
  case "$a" in
    --aktualisieren) AKTUALISIEREN=ja ;;
    --pruefen)       NUR_PRUEFEN=ja ;;
    --ohne-fenster)  OHNE_FENSTER=ja ;;
    --nix)           MIT_NIX=ja ;;
    -h|--hilfe|--help)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "unbekannter Schalter: $a" >&2; exit 2 ;;
  esac
done

if [ "$(uname -s)" != "Linux" ]; then
  echo "Dieses Skript ist fuer Linux. Fuer macOS: installieren-macos.sh," >&2
  echo "fuer Windows: installieren-windows.ps1." >&2
  exit 2
fi

# ⚑ **NixOS erkennt man an `/etc/NIXOS`**, das die Distribution selbst
# anlegt. Die Schalter gehen unveraendert weiter, bis auf `--nix`, den
# das andere Skript nicht kennt.
if [ "$MIT_NIX" = ja ] || [ -e /etc/NIXOS ]; then
  echo "── NixOS: weiter mit installieren-nixos.sh"
  weiter=""
  [ "$AKTUALISIEREN" = ja ] && weiter="$weiter --aktualisieren"
  [ "$NUR_PRUEFEN" = ja ] && weiter="$weiter --pruefen"
  # shellcheck disable=SC2086
  exec sh "$WURZEL/SYSTEM/install/installieren-nixos.sh" $weiter
fi

echo "── Myelith einrichten"
echo "   Quelle: $WURZEL"

# ── Welche Distribution, und welcher Befehl holt die Pakete ─────────
#
# ⚑ **Aus `/etc/os-release`**, das jede heutige Distribution traegt. Ist
# sie unbekannt, steht da die Liste der Bibliotheken statt eines Befehls.
DISTRO=""
if [ -r /etc/os-release ]; then
  # shellcheck disable=SC1091
  DISTRO=$(. /etc/os-release; echo "${ID:-} ${ID_LIKE:-}")
fi
# ⚑ **Die Paketnamen stehen in `systempakete.txt`**, je Familie eine Zeile,
# und nicht hier: eine zweite Liste liefe mit der ersten auseinander.
paketzeile() {
  sed -n "s/^$1: //p" "$WURZEL/SYSTEM/install/systempakete.txt" | tr -d '|' | tr -s ' '
}
pakete_befehl() {
  case " $DISTRO " in
    *" debian "*|*" ubuntu "*)       echo "sudo apt-get install -y $(paketzeile debian)" ;;
    *" fedora "*|*" rhel "*|*" centos "*) echo "sudo dnf install -y $(paketzeile fedora)" ;;
    *" arch "*|*" manjaro "*)        echo "sudo pacman -S --needed $(paketzeile arch)" ;;
    *" opensuse"*|*" suse "*)        echo "sudo zypper install -y $(paketzeile opensuse)" ;;
    *)
      echo "(unbekannte Distribution) die Entwicklungspakete von WebKitGTK 4.1, GTK 3, libsoup 3, dbus, dazu ein C-Compiler, pkg-config und python3" ;;
  esac
}

# ── Was da sein muss ────────────────────────────────────────────────
#
# ⚑ **Erst alles pruefen, dann bauen.** Ein Bau, der nach acht Minuten
# an einem fehlenden Werkzeug abbricht, hat acht Minuten gekostet und
# nichts gesagt, was er nicht vorher haette sagen koennen.
# ⚑ **Die Fassung aus `rust-toolchain.toml`, und ohne Netz die vorhandene**
# (2026-10-22). rustup holt die dort festgelegte Fassung beim ersten Aufruf
# im Baum nach. Ohne Netz scheitert das, und dann baut die eingestellte
# Fassung weiter, mit Hinweis: Eine fehlende Werkzeugkette darf eine
# Pruefung kosten, nie den Bau. Ohne rustup (Werkzeugvorrat, Nix) gilt die
# Datei ohnehin nicht.
if command -v rustup >/dev/null 2>&1 && ! cargo --version >/dev/null 2>&1; then
  vorhanden=$(rustup toolchain list 2>/dev/null | awk '/default/ {print $1; exit}')
  [ -n "$vorhanden" ] || vorhanden=$(rustup toolchain list 2>/dev/null | awk 'NR == 1 && $1 ~ /-/ {print $1}')
  if [ -n "$vorhanden" ]; then
    RUSTUP_TOOLCHAIN="$vorhanden"
    export RUSTUP_TOOLCHAIN
    echo "   Hinweis: Die festgelegte Rust-Fassung liess sich nicht holen (kein Netz?); gebaut wird mit $vorhanden."
  fi
fi

fehlt=""

# 📌 **Nicht „gibt es den Befehl", sondern „laeuft er".** Ein
# rustup-Schalter ohne eingestellte Werkzeugkette liegt im PATH und
# beantwortet `command -v` mit ja; `cargo --version` bricht dann ab.
if ! cargo --version >/dev/null 2>&1; then
  fehlt="$fehlt
  Die Rust-Werkzeugkette fehlt oder hat keine eingestellte Fassung. Holen mit:
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  Danach eine neue Sitzung oeffnen oder:
      . \"\$HOME/.cargo/env\"
  Ist rustup schon da, fehlt vielleicht nur die Voreinstellung:
      rustup default stable"
fi

# ⚑ Einige Fremdpakete uebersetzen C-Quellen (etwa die Kurvenarithmetik
# der Signaturen); ohne C-Compiler baut keines der Programme.
if ! cc --version >/dev/null 2>&1; then
  fehlt="$fehlt
  Ein C-Compiler fehlt (cc). Holen mit:
      $(pakete_befehl)"
fi

# ⚑ python3 packt den Vorrat aus; ohne ihn gibt es keinen Bau ohne Netz.
if ! python3 --version >/dev/null 2>&1; then
  fehlt="$fehlt
  python3 fehlt; es packt den Vorrat der Rust-Pakete aus. Holen mit:
      $(pakete_befehl)"
fi

if [ "$AKTUALISIEREN" = ja ] && ! command -v git >/dev/null 2>&1; then
  fehlt="$fehlt
  git fehlt, und ohne git gibt es keinen neuen Stand zu holen."
fi

if [ -n "$fehlt" ]; then
  echo "── Es fehlt etwas:$fehlt" >&2
  exit 1
fi

echo "   cargo: $(cargo --version)"

# ⚑ **Das Fenster braucht diese Bibliotheken, die anderen nichts davon.**
# Gefragt wird pkg-config nach genau den Modulen, die die Fremdpakete
# des Fensters beim Bauen suchen.
FENSTER_MODULE="webkit2gtk-4.1 javascriptcoregtk-4.1 gtk+-3.0 libsoup-3.0 dbus-1"
FENSTER=ja
FENSTER_FEHLT=""
if [ "$OHNE_FENSTER" = ja ]; then
  FENSTER=nein
elif ! pkg-config --version >/dev/null 2>&1; then
  FENSTER=nein
  FENSTER_FEHLT="pkg-config"
else
  for m in $FENSTER_MODULE; do
    pkg-config --exists "$m" 2>/dev/null || FENSTER_FEHLT="$FENSTER_FEHLT $m"
  done
  [ -z "$FENSTER_FEHLT" ] || FENSTER=nein
fi
if [ "$FENSTER" = ja ]; then
  echo "   Fenster: Bibliotheken da"
elif [ -n "$FENSTER_FEHLT" ]; then
  echo "   ⚠️  Fenster: es fehlt$FENSTER_FEHLT"
  echo "      Das Fenster wird ausgelassen, alles andere gebaut. Fuer das Fenster:"
  echo "      $(pakete_befehl)"
else
  echo "   Fenster: ausgelassen (--ohne-fenster)"
fi

# ⚑ **Der Vorrat liegt als Archive im Baum und wird hier ausgepackt**,
# der lokale Zusatzvorrat eigener Module gleich mit. Neu ausgepackt wird,
# wenn es noch kein Lager gibt oder einer der Vorraete neuer ist.
ARCHIVE=$(find "$WURZEL/SYSTEM/crates-vorrat" -maxdepth 1 -name '*.crate' 2>/dev/null | wc -l | tr -d " ")
if [ "$ARCHIVE" -eq 0 ]; then
  echo "── Im Baum liegt kein Vorrat unter SYSTEM/crates-vorrat; ohne ihn baut nichts ohne Netz." >&2
  exit 1
fi
LAGER="$WURZEL/SYSTEM/crates-lager"
if [ ! -f "$LAGER/cargo-home/config.toml" ] \
   || [ "$WURZEL/SYSTEM/crates-vorrat" -nt "$LAGER/vendor" ] \
   || { [ -d "$WURZEL/SYSTEM/crates-vorrat-lokal" ] && [ "$WURZEL/SYSTEM/crates-vorrat-lokal" -nt "$LAGER/vendor" ]; }; then
  echo "   Vorrat: $ARCHIVE Archive werden ausgepackt"
  python3 "$WURZEL/SYSTEM/install/vorrat.py" auspacken >/dev/null || {
    echo "   FEHLER beim Auspacken des Vorrats" >&2
    exit 1
  }
fi
CARGO_HOME="$LAGER/cargo-home"
export CARGO_HOME
echo "   Vorrat: $ARCHIVE Pakete aus SYSTEM/crates-vorrat/, Netz aus"

ARTEFAKTE=$(find "$WURZEL/INTEGER_LLM/artifacts" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')
echo "   Artefakte da: $ARTEFAKTE"

if [ "$NUR_PRUEFEN" = ja ]; then
  echo "── Geprueft. Ohne --pruefen wird gebaut."
  exit 0
fi

# ── Neuen Stand holen, wenn gewuenscht ──────────────────────────────
#
# 📌 **`--ff-only`, und das ist die ganze Vorsicht.** Wer im Klon
# gearbeitet hat, soll seine Arbeit nicht durch ein Installationsskript
# verlieren.
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
  # Der neue Stand kann neue Archive bringen.
  python3 "$WURZEL/SYSTEM/install/vorrat.py" auspacken >/dev/null
fi

# ── Was ausgeliefert wird ───────────────────────────────────────────
#
# ⚑ **Die Liste wird gefunden und nicht gefuehrt** (Fund 300): Jede
# Kiste, die ausgeliefert werden soll, sagt es in ihrer eigenen
# `Cargo.toml` unter `[package.metadata.myelith]`.
#
# ⚑ **Gesucht wird nur in den Komponenten**, nicht im Lager, nicht im
# Bauordner und nicht in fremden Baeumen: Dort liegen Tausende
# `Cargo.toml`, und keine davon meldet etwas an.
programme() {
  find "$WURZEL" \( -path "$WURZEL/SYSTEM" -o -path "$WURZEL/.git" -o -path "$WURZEL/MODELS" \
       -o -path "$WURZEL/README" -o -name target -o -name node_modules \) -prune \
       -o -name Cargo.toml -print | sort | while read -r kiste; do
    name=$(sed -n 's/^ausliefern = "\(.*\)"$/\1/p' "$kiste" | head -1)
    [ -n "$name" ] || continue
    verzeichnis=$(dirname "$kiste")
    echo "${verzeichnis#"$WURZEL"/} $name"
  done
}

PROGRAMME=$(programme)
if [ -z "$PROGRAMME" ]; then
  echo "Keine einzige Kiste ist zum Ausliefern angemeldet." >&2
  echo "Gesucht wurde [package.metadata.myelith] mit ausliefern." >&2
  exit 1
fi

# 📌 **Eine Schleife in dieser Shell und nicht hinter einer Roehre**,
# damit ein fehlgeschlagener Bau dieses Skript wirklich beendet.
echo "── bauen, das dauert beim ersten Mal einige Minuten"
GEBAUT=""
ALTES_IFS=$IFS
IFS='
'
for zeile in $PROGRAMME; do
  IFS=$ALTES_IFS
  # shellcheck disable=SC2086
  set -- $zeile
  if [ "$2" = myl-oberflaeche ] && [ "$FENSTER" = nein ]; then
    echo "   $2 (ausgelassen)"
  else
    echo "   $2"
    # ⚑ **`--locked --offline` immer**: Die Sperrdateien sind Teil des
    # Standes, und genommen wird ausschliesslich der Vorrat.
    ( cd "$WURZEL/$1" && cargo build --release --quiet --locked --offline )
    GEBAUT="$GEBAUT $2"
  fi
  IFS='
'
done
IFS=$ALTES_IFS

# ── Legen ───────────────────────────────────────────────────────────
#
# ⚑ **Dort, wohin gebaut wurde.** `CARGO_TARGET_DIR` hat Vorrang vor
# `.cargo/config.toml`; ohne sie gilt `SYSTEM/full-build`.
ZIEL="${CARGO_TARGET_DIR:-$WURZEL/SYSTEM/full-build}"
BIN="$HOME/.local/bin"
ANWENDUNGEN="$HOME/.local/share/applications"
SYMBOLE="$HOME/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$BIN"
for p in $GEBAUT; do
  # ⛔️ **Erst weg, dann hin**: Ein laufendes Programm laesst sich unter
  # Linux nicht ueberschreiben (`Text file busy`), und das Update aus
  # Konsole oder Fenster laeuft genau aus dem Programm, das es ersetzt.
  rm -f "$BIN/$p"
  cp "$ZIEL/release/$p" "$BIN/$p"
done

# ⚑ **Ein Eintrag im Anwendungsmenue, und das Symbol zieht mit um.** Der
# Eintrag nennt das Symbol beim Namen und nicht mit einem Pfad in diesen
# Baum: Ein Pfad hierher stuende auf dem Rechner, bis jemand den Baum
# verschiebt oder loescht, und dann fehlte das Symbol ohne ein Wort.
if [ "$FENSTER" = ja ]; then
  mkdir -p "$ANWENDUNGEN" "$SYMBOLE"
  cp "$WURZEL/CLIENT/myl-oberflaeche/icons/icon.png" "$SYMBOLE/myelith.png"
  cat > "$ANWENDUNGEN/myelith.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Myelith
Comment=Ganzzahlige Inferenz als Konsensarbeit
Exec=$BIN/myl-oberflaeche
Icon=myelith
Terminal=false
Categories=Development;Utility;
EOF
fi

echo "── fertig"
echo "   Programme:"
for p in $GEBAUT; do
  echo "     $BIN/$p"
done
if [ "$FENSTER" = ja ]; then
  echo "   Menueeintrag: $ANWENDUNGEN/myelith.desktop"
else
  echo "   ⚠️  ohne Fenster; mit den Paketen oben und einem zweiten Lauf kommt es dazu"
fi

case ":$PATH:" in
  *":$BIN:"*) ;;
  *)
    echo
    echo "⚠️  $BIN steht nicht in deinem PATH. Fuer die Shell-Datei:"
    echo "      export PATH=\"$BIN:\$PATH\"" ;;
esac
