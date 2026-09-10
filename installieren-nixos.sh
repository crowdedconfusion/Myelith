#!/bin/sh
# Myelith auf NixOS (oder mit Nix) einrichten, aus einem frischen Klon.
#
# Aufruf:
#   sh installieren-nixos.sh                 baut in der Nix-Umgebung, legt nach ~/.local
#   sh installieren-nixos.sh --aktualisieren holt erst neuen Stand, baut dann
#   sh installieren-nixos.sh --pruefen       sagt nur, was fehlt, und baut nicht
#   sh installieren-nixos.sh --in-der-shell  ueberspringt `nix develop`, weil man schon drin ist
#
# ⚑ **Es ruft sich selbst in `nix develop` noch einmal auf.** Der Bau
# braucht WebKitGTK und den halben GTK-Stapel, und auf NixOS liegt
# nichts davon an einem Ort, den ein Linker raet. Die Umgebung steht in
# `flake.nix` daneben.
#
# ⚑ **Und es aendert nichts an der Systemkonfiguration.** Ein
# Installationsskript, das `/etc/nixos/configuration.nix` anfasst, nimmt
# einem NixOS-Nutzer genau das weg, wofuer er NixOS benutzt.
set -eu

cd "$(dirname "$0")"
WURZEL=$(pwd)

AKTUALISIEREN=nein
NUR_PRUEFEN=nein
IN_DER_SHELL=nein
for a in "$@"; do
  case "$a" in
    --aktualisieren) AKTUALISIEREN=ja ;;
    --pruefen)       NUR_PRUEFEN=ja ;;
    --in-der-shell)  IN_DER_SHELL=ja ;;
    -h|--hilfe|--help)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "unbekannter Schalter: $a" >&2; exit 2 ;;
  esac
done

echo "── Myelith einrichten"
echo "   Quelle: $WURZEL"

# ── In die Bauumgebung wechseln ─────────────────────────────────────
#
# ⛑ **Einmal und nicht zweimal.** `--in-der-shell` ist der Anschlag:
# Ohne ihn riefe sich das Skript in der Shell erneut auf und liefe im
# Kreis.
if [ "$IN_DER_SHELL" = nein ]; then
  if ! command -v nix >/dev/null 2>&1; then
    echo "── Es fehlt etwas:" >&2
    echo "  nix fehlt. Auf NixOS ist es da; sonst von https://nixos.org/download" >&2
    exit 1
  fi
  if ! nix --version 2>/dev/null | grep -q .; then
    echo "  nix laesst sich nicht aufrufen." >&2
    exit 1
  fi
  # ⚑ Flakes sind auf vielen Anlagen noch nicht eingeschaltet; die
  # beiden Merkmale hier gelten nur fuer diesen einen Aufruf und
  # aendern nichts an der Anlage.
  MERKMALE="--extra-experimental-features nix-command --extra-experimental-features flakes"
  ARGUMENTE="--in-der-shell"
  [ "$AKTUALISIEREN" = ja ] && ARGUMENTE="$ARGUMENTE --aktualisieren"
  [ "$NUR_PRUEFEN" = ja ] && ARGUMENTE="$ARGUMENTE --pruefen"
  echo "── Bauumgebung aus flake.nix holen, beim ersten Mal dauert das"
  # shellcheck disable=SC2086
  exec nix $MERKMALE develop "$WURZEL" --command sh "$0" $ARGUMENTE
fi

# ── Ab hier laeuft es in der Umgebung ───────────────────────────────
fehlt=""
# ⛑ **Nicht „gibt es den Befehl", sondern „laeuft er".** Siehe den
# gleichen Anschlag im macOS-Skript.
cargo --version >/dev/null 2>&1 || fehlt="$fehlt
  cargo laesst sich nicht aufrufen, obwohl die Nix-Umgebung es mitbringen sollte."
command -v pkg-config >/dev/null 2>&1 || fehlt="$fehlt
  pkg-config fehlt, obwohl die Nix-Umgebung es mitbringen sollte."
if [ "$AKTUALISIEREN" = ja ] && ! command -v git >/dev/null 2>&1; then
  fehlt="$fehlt
  git fehlt, und ohne git gibt es keinen neuen Stand zu holen."
fi
if [ -n "$fehlt" ]; then
  echo "── Es fehlt etwas:$fehlt" >&2
  exit 1
fi

echo "   cargo: $(cargo --version)"

if [ "$NUR_PRUEFEN" = ja ]; then
  echo "── Alles da. Ohne --pruefen wird gebaut."
  exit 0
fi

if [ "$AKTUALISIEREN" = ja ]; then
  echo "── neuen Stand holen"
  if [ ! -d .git ]; then
    echo "   Das hier ist kein Klon, sondern ein entpacktes Verzeichnis." >&2
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

# ── Was ausgeliefert wird ───────────────────────────────────────────
#
# ⚑ **Die Liste wird gefunden und nicht gefuehrt.** Jede Kiste, die
# ausgeliefert werden soll, sagt es in ihrer eigenen `Cargo.toml`
# unter `[package.metadata.myelith]`; dieses Skript sammelt sie ein.
#
# ⛑ **Bis zum 2026-09-10 stand die Liste hier, von Hand** (Fund 300),
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

# ⛑ **Eine Schleife in dieser Shell und nicht hinter einer Roehre**,
# damit ein fehlgeschlagener Bau dieses Skript wirklich beendet.
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

BIN="$HOME/.local/bin"
ANWENDUNGEN="$HOME/.local/share/applications"
mkdir -p "$BIN" "$ANWENDUNGEN"
ALTES_IFS=$IFS
IFS='
'
for zeile in $PROGRAMME; do
  IFS=$ALTES_IFS
  # shellcheck disable=SC2086
  set -- $zeile
  cp "$WURZEL/target-shared/release/$2" "$BIN/$2"
  IFS='
'
done
IFS=$ALTES_IFS

# ⚑ **Ein Eintrag im Anwendungsmenue**, denn ein Fenster, das man nur
# aus einer Shell starten kann, ist fuer die meisten kein Programm.
#
# ⚑ **`Icon` traegt einen absoluten Pfad in diesen Baum**, und deshalb
# wird der Eintrag bei jedem Einrichten neu geschrieben statt einmal:
# Wer das Repositorium verschiebt, richtet einmal neu ein, und das
# Symbol steht wieder.
cat > "$ANWENDUNGEN/myelith.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Myelith
Comment=Ganzzahlige Inferenz als Konsensarbeit
Exec=$BIN/myl-oberflaeche
Icon=$WURZEL/CLIENT/myl-oberflaeche/icons/icon.png
Terminal=false
Categories=Development;Utility;
EOF

echo "── fertig"
# ⛑ Siehe Fund 300: Auch eine Schlussmeldung ist eine Liste.
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
echo "   Menueeintrag: $ANWENDUNGEN/myelith.desktop"

case ":$PATH:" in
  *":$BIN:"*) ;;
  *)
    echo
    echo "⚠️  $BIN steht nicht in deinem PATH. Fuer die Shell-Datei:"
    echo "      export PATH=\"$BIN:\$PATH\"" ;;
esac
