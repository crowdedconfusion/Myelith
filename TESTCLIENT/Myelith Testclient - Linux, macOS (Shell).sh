#!/bin/sh
# Startet den Myelith-Testclient. Für macOS und Linux.
#
# Baut bei Bedarf und reicht alle Argumente weiter:
#
#     ./myl-test.sh                 interaktives Menü
#     ./myl-test.sh artefakte       Artefakte prüfen
#     ./myl-test.sh determinismus   Bitgleichheit auf dieser Maschine
#     ./myl-test.sh --help          alle Befehle
#
# Warum ein Starter und nicht die Aufrufzeile aus dem README: Der Client
# soll auf fremden Maschinen laufen, oft auf solchen, deren Besitzer mit
# Rust nichts zu tun hat. Wer erst herausfinden muss, in welches
# Verzeichnis er wechseln und welche Cargo-Flagge er setzen muss, führt
# den Test seltener aus. Genau das ist die Hürde, die dieser Test nicht
# haben darf.
#
# POSIX-sh, keine Bashismen: Auf manchen Systemen ist /bin/sh dash.

set -eu

# Repository-Wurzel durch Aufwaertssuche bestimmen, nicht ueber eine feste
# Tiefe. Die erste Fassung rechnete mit dem Wurzelverzeichnis als Ablageort
# und war sofort kaputt, als der Starter in TESTCLIENT/ verschoben wurde.
# Ein Starter, der beim Verschieben bricht, ist ein schlechter Starter.
WURZEL=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
while [ ! -f "$WURZEL/TESTCLIENT/myl-testclient/Cargo.toml" ]; do
    UEBER=$(dirname -- "$WURZEL")
    if [ "$UEBER" = "$WURZEL" ]; then
        echo "Fehler: TESTCLIENT/myl-testclient/Cargo.toml oberhalb von" >&2
        echo "$(dirname -- "$0") nicht gefunden." >&2
        echo "Dieser Starter gehört in ein Myelith-Repository." >&2
        exit 1
    fi
    WURZEL="$UEBER"
done
MANIFEST="$WURZEL/TESTCLIENT/myl-testclient/Cargo.toml"

# Ausgabeort: gemeinsames target-shared/ (siehe .cargo/config.toml), sonst
# das crate-eigene target/.
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    BIN="$CARGO_TARGET_DIR/release/myl-test"
elif [ -d "$WURZEL/target-shared" ]; then
    BIN="$WURZEL/target-shared/release/myl-test"
else
    BIN="$WURZEL/TESTCLIENT/myl-testclient/target/release/myl-test"
fi

if command -v cargo >/dev/null 2>&1; then
    # `cargo build` prüft selbst, ob etwas zu tun ist, und ist im
    # unveränderten Fall in unter einer Sekunde durch. Eine eigene
    # Zeitstempel-Logik wäre eine zweite, schlechtere Antwort auf dieselbe
    # Frage.
    echo "Baue Testclient (beim ersten Mal dauert das einige Minuten) ..."
    cargo build --release --quiet --manifest-path "$MANIFEST"
elif [ -x "$BIN" ]; then
    echo "Hinweis: cargo nicht gefunden, benutze das vorhandene Binary." >&2
else
    cat >&2 <<'ENDE'
Fehler: Weder cargo noch ein gebautes Binary gefunden.

Rust installieren (einmalig, ohne Administratorrechte):

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Danach eine neue Terminalsitzung öffnen und diesen Starter erneut
aufrufen.
ENDE
    exit 1
fi

exec "$BIN" "$@"
