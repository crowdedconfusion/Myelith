#!/bin/sh
# Baut die auslieferbaren Programme und schreibt Pruefsummen.
#
# ⚑ **Warum es dieses Skript gibt.** Die Entscheidung vom 2026-09-08
# lautet: keine Container, native Binaries. Damit tritt an die Stelle
# eines Images eine Sammlung von Programmen, und ohne Pruefsummen ist
# „auf allen Systemen abrufbar" eine Absicht ohne Artefakt.
#
# ⛑ **Was dieses Skript NICHT tut, und jedes davon ist eigene Arbeit:**
#
#   Es baut fuer DIESE Plattform.  Uebersetzen fuer eine andere braucht
#   die Zielkette und, bei der Oberflaeche, deren Systembibliotheken.
#   Der Weg ist, es auf jeder Plattform zu fahren und die Ergebnisse
#   zusammenzulegen.
#
#   Es signiert nicht.  Unter macOS meldet Gatekeeper dann einen
#   unbekannten Entwickler, unter Windows SmartScreen. Dafuer braucht es
#   ein Entwicklerzertifikat, und das ist eine Beschaffung und keine
#   Bauarbeit.
#
#   Es macht die Binaries nicht reproduzierbar.  Zwei Baeume mit
#   demselben Quelltext ergeben derzeit nicht dasselbe Programm; Pfade,
#   Zeitstempel und die Fassung der Werkzeugkette gehen mit ein.
#   ⚑ Genau deshalb steht daneben, WOMIT gebaut wurde: Eine Pruefsumme
#   ohne die Umstaende ihres Entstehens belegt nur, dass zwei Dateien
#   gleich sind, nicht dass sie dasselbe bedeuten.
set -eu
cd "$(dirname "$0")/.."
ZIEL=${1:-freigabe}

# ⚑ **Nur, was ein Nutzer aufruft.** Elf Crates erzeugen Programme;
# `trainingsguete`, `generate_golden_vectors` und die uebrigen sind
# Messwerkzeuge dieses Repositoriums. Wer sie mitliefert, liefert
# Angriffsflaeche fuer niemanden mit.
#
# Zeilen: Verzeichnis, Programmname
PROGRAMME="CLIENT/myl-client myl
CLIENT/myl-oberflaeche myl-oberflaeche
NODE/myl-node myl-node
TESTCLIENT/myl-testclient myl-test"

rm -rf "$ZIEL"
mkdir -p "$ZIEL"

printf '%s\n' "$PROGRAMME" | while read -r verz name; do
  [ -n "$verz" ] || continue
  echo "── baue $name"
  ( cd "$verz" && cargo build --release --quiet )
  cp "target-shared/release/$name" "$ZIEL/$name"
done

# ⚑ Unter macOS zusaetzlich das Buendel, denn ein Programm, das man
# nicht doppelklicken kann, ist fuer die meisten kein Programm.
if [ "$(uname -s)" = "Darwin" ]; then
  sh CLIENT/myl-oberflaeche/buendeln-macos.sh "$ZIEL/Myelith.app" >/dev/null
  echo "── Myelith.app gebuendelt"
fi

# Die Umstaende des Baus, neben die Pruefsummen.
{
  echo "# Myelith, Freigabe"
  echo "Datum:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "Plattform:  $(uname -s) $(uname -m)"
  echo "Rust:       $(rustc --version)"
  echo "Cargo:      $(cargo --version)"
  echo
  echo "# Fassungen"
  printf '%s\n' "$PROGRAMME" | while read -r verz name; do
    [ -n "$verz" ] || continue
    echo "$name: $(grep -m1 '^version' "$verz/Cargo.toml" | cut -d'"' -f2)"
  done
} > "$ZIEL/BAU.txt"

# ⚑ `shasum -a 256` gibt es auf macOS und Linux; `sha256sum` nur auf
# Linux. Die Ausgabe ist dieselbe, und das Format ist das, das
# `shasum -c` wieder liest.
#
# ⛑ **Ohne sich selbst.** Die Summendatei kann ihre eigene Summe nicht
# enthalten: Sobald sie geschrieben ist, stimmt der Eintrag nicht mehr.
# Der erste Entwurf tat es doch, und die Gegenprobe meldete prompt
# `SHA256SUMS: FAILED` neben lauter `OK`. Wer das einmal sieht, traut
# der ganzen Datei nicht mehr.
#
# ⚑ Die Ursache ist feiner, als sie aussieht, und sie ist der Grund,
# warum dieselbe Zeile in `release.yml` gutgeht: Bei einem **einfachen
# Befehl** (`shasum * > SUMS`) expandiert die Schale den Glob, bevor die
# Umlenkung die Datei anlegt, und `SUMS` ist nicht dabei. Der erste
# Entwurf hier schrieb `( ... ) > SHA256SUMS`; die Umlenkung einer
# **Unterschale** wird vor deren Rumpf eingerichtet, die Datei liegt also
# schon leer da, wenn der Glob expandiert, und sie steht in ihrer
# eigenen Liste. Nachgestellt und in beiden Formen bestaetigt.
#
# ⚑ **Und Verzeichnisse werden durchlaufen.** `Myelith.app` ist eines;
# `shasum` uebergeht es wortlos, und dann fehlt genau das Stueck, das
# ein Mensch anklickt.
summieren() {
  if command -v shasum >/dev/null; then shasum -a 256 "$@"; else sha256sum "$@"; fi
}
(
  cd "$ZIEL"
  find . -type f ! -name SHA256SUMS | sed 's|^\./||' | sort | while read -r f; do
    summieren "$f"
  done
) > "$ZIEL/SHA256SUMS"

echo
echo "$ZIEL/ ist fertig:"
ls -1 "$ZIEL"
echo
echo "Dateien:  $(grep -c . "$ZIEL/SHA256SUMS") mit Pruefsumme"
echo "Pruefen:  (cd $ZIEL && shasum -a 256 -c SHA256SUMS)"
