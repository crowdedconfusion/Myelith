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

# 📌 **Zwei Ebenen hoeher, seit dem 2026-09-24.** Diese Datei lag in der
# Wurzel, zog am 2026-09-10 nach `INSTALL/` und liegt jetzt unter
# `SYSTEM/install/`. **Ein Verschieben sieht aus wie eine Aenderung ohne
# Verhalten und ist keine:** Mit einem `/..` zu wenig zeigt die Wurzel
# auf `SYSTEM/`, und der Bau faende keine einzige Crate. Dieselbe Klasse
# wie Fund 298.
#
# ⚠️ **Beim zweiten Umzug ist genau das passiert**, obwohl die Warnung
# drei Zeilen darueber stand. Eine Lehre im Kommentar schuetzt den
# nicht, der die Datei verschiebt und den Kommentar nicht liest; sie
# schuetzt den, der danach sucht. **Deshalb steht sie trotzdem da.**
cd "$(dirname "$0")/../.."
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
# 📌 **Einmal und nicht zweimal.** `--in-der-shell` ist der Anschlag:
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
  # ⚑ **Ohne Netz aus dem Werkzeugvorrat**, wenn er fuer dieses System da
  # ist (2026-10-21). Er traegt die Quellen der Eingaben aus `flake.lock` und
  # jede Abhaengigkeit der Umgebung; eingespielt wird er einmal als root.
  # Liegt er da, ist die Bauumgebung aber nicht im Speicher, sagt das Skript
  # den Befehl und hoert auf, statt ins Netz zu gehen.
  OFFLINE=""
  case "$(uname -m)" in
    x86_64)        NIXSYSTEM=x86_64-linux ;;
    aarch64|arm64) NIXSYSTEM=aarch64-linux ;;
    *)             NIXSYSTEM="" ;;
  esac
  NIXVORRAT="$WURZEL/SYSTEM/werkzeug-vorrat/nix/$NIXSYSTEM"
  if [ -n "$NIXSYSTEM" ] && [ -f "$NIXVORRAT/UMGEBUNG" ]; then
    if [ ! -e "$(cat "$NIXVORRAT/UMGEBUNG")" ]; then
      echo "── Der Nix-Vorrat liegt bereit, ist aber noch nicht eingespielt. Einmal, als root:" >&2
      echo "      sudo sh $WURZEL/SYSTEM/install/nix-vorrat-einspielen.sh" >&2
      exit 1
    fi
    OFFLINE="--offline"
    echo "   Nix: Bauumgebung aus dem Werkzeugvorrat, Netz aus"
  fi
  # ⚑ **Ein Profil als Wurzel fuer die Speicherbereinigung.** Die Programme
  # sind gegen Bibliotheken im Nix-Speicher gelinkt; ohne Wurzel naehme
  # `nix-collect-garbage` sie weg, und das Fenster startete nicht mehr.
  PROFIL="${XDG_STATE_HOME:-$HOME/.local/state}/myelith/bauumgebung"
  mkdir -p "$(dirname "$PROFIL")"
  echo "── Bauumgebung aus flake.nix holen, beim ersten Mal dauert das"
  # shellcheck disable=SC2086
  exec nix $MERKMALE develop $OFFLINE --profile "$PROFIL" "$WURZEL" --command sh "$0" $ARGUMENTE
fi

# ── Ab hier laeuft es in der Umgebung ───────────────────────────────
fehlt=""
# 📌 **Nicht „gibt es den Befehl", sondern „laeuft er".** Siehe den
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

# ⚑ **Derselbe Vorrat wie ueberall**, und dieselbe Naht: Das Auspacken
# macht `SYSTEM/install/vorrat.py`, hier steht nur der Aufruf. **Der schwierige
# Teil ist damit auf allen Systemen derselbe Quelltext** und nicht
# dreimal nachgebaut.
#
# ⚑ **Die Nix-Eingaben deckt der Werkzeugvorrat** (oben, seit dem
# 2026-10-21); dieser hier deckt die Fremdkisten. Ohne Werkzeugvorrat holt
# `nix develop` die Umgebung aus dem Netz, wenn sie nicht schon im Speicher
# liegt.
CARGO_NETZ=""
ARCHIVE=$(find "$WURZEL/SYSTEM/crates-vorrat" -maxdepth 1 -name '*.crate' 2>/dev/null | wc -l | tr -d " ")
if [ "$ARCHIVE" -gt 0 ]; then
  if [ ! -d "$WURZEL/SYSTEM/crates-lager/vendor" ] \
     || [ ! -f "$WURZEL/SYSTEM/crates-lager/cargo-home/config.toml" ] \
     || [ "$WURZEL/SYSTEM/crates-vorrat" -nt "$WURZEL/SYSTEM/crates-lager/vendor" ] \
     || { [ -d "$WURZEL/SYSTEM/crates-vorrat-lokal" ] \
          && [ "$WURZEL/SYSTEM/crates-vorrat-lokal" -nt "$WURZEL/SYSTEM/crates-lager/vendor" ]; }; then
    echo "   Vorrat: $ARCHIVE Archive werden ausgepackt"
    python3 "$WURZEL/SYSTEM/install/vorrat.py" auspacken >/dev/null || {
      echo "   FEHLER beim Auspacken des Vorrats" >&2
      exit 1
    }
  fi
  CARGO_HOME="$WURZEL/SYSTEM/crates-lager/cargo-home"
  export CARGO_HOME
  CARGO_NETZ="--offline"
  echo "   Vorrat: $ARCHIVE Pakete aus SYSTEM/crates-vorrat/, Netz aus"
fi

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

# 📌 **Eine Schleife in dieser Shell und nicht hinter einer Roehre**,
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
  ( cd "$WURZEL/$1" && cargo build --release --quiet --locked $CARGO_NETZ )
  IFS='
'
done
IFS=$ALTES_IFS

BIN="$HOME/.local/bin"
ANWENDUNGEN="$HOME/.local/share/applications"
SYMBOLE="$HOME/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$BIN" "$ANWENDUNGEN" "$SYMBOLE"
ALTES_IFS=$IFS
IFS='
'
for zeile in $PROGRAMME; do
  IFS=$ALTES_IFS
  # shellcheck disable=SC2086
  set -- $zeile
  # ⛔️ **Erst weg, dann hin** (2026-10-17): Ein laufendes Programm laesst
  # sich unter Linux nicht ueberschreiben (`Text file busy`), und das Update
  # aus Konsole oder Fenster laeuft genau aus dem Programm, das es ersetzt.
  # Geloescht behaelt der laufende Prozess seine Datei, und der Neustart
  # findet unter demselben Pfad die neue.
  rm -f "$BIN/$2"
  cp "$WURZEL/SYSTEM/full-build/release/$2" "$BIN/$2"
  IFS='
'
done
IFS=$ALTES_IFS

# ⚑ **Ein Eintrag im Anwendungsmenue**, denn ein Fenster, das man nur
# aus einer Shell starten kann, ist fuer die meisten kein Programm.
#
# ⚑ **Das Symbol zieht mit um und wird beim Namen genannt.** Bis zum
# 2026-10-21 trug `Icon` einen absoluten Pfad in diesen Baum; wer ihn
# verschob oder loeschte, verlor das Symbol ohne ein Wort. Jetzt liegt es
# im Symbolordner des Nutzers, und der Eintrag nennt nur `myelith`.
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

echo "── fertig"
# 📌 Siehe Fund 300: Auch eine Schlussmeldung ist eine Liste.
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
