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

# Das Binary wird NACH dem Bau gesucht, nicht vorher geraten.
#
# `.cargo/config.toml` lenkt alle Crates nach `target-shared/`. Auf einem
# frischen Klon gibt es dieses Verzeichnis noch nicht; die erste Fassung
# schloss daraus auf `target/` und suchte dort, waehrend cargo nach
# `target-shared/` baute. Der allererste Lauf schlug damit fehl, also
# genau der Lauf, auf den es ankommt.
# Ausgabeverzeichnis festnageln, bevor cargo aufgerufen wird.
#
# FUND (2026-08-21, beim Durchspielen eines frischen Klons): `.cargo/config.toml`
# des Repositoriums setzt `target-dir = "target-shared"`, und zwar RELATIV.
# Cargo loest diesen Pfad gegen das ARBEITSVERZEICHNIS auf, nicht gegen das
# per --manifest-path angegebene Crate. Wer den Starter aus einem anderen
# Verzeichnis aufruft, und das tut jeder, der ihn auf den Schreibtisch legt,
# baut deshalb irgendwohin: ins Verzeichnis eines anderen Klons, wenn dort
# eine solche Konfiguration liegt, sonst nach TESTCLIENT/myl-testclient/target.
# Der Bau lief danach durch, und der Starter meldete trotzdem
# "gebaut, aber nicht auffindbar".
#
# Eine gesetzte Umgebungsvariable hat Vorrang vor der Konfigurationsdatei und
# ist absolut. Damit haengt der Ablageort am Repositorium, nicht am Zufall des
# Arbeitsverzeichnisses. Ein von aussen gesetzter Wert bleibt unangetastet:
# die CI setzt CARGO_TARGET_DIR=target und soll das behalten.
: "${CARGO_TARGET_DIR:=$WURZEL/target-shared}"
export CARGO_TARGET_DIR

binary_finden() {
    for kandidat in \
        "$CARGO_TARGET_DIR/release/myl-test" \
        "$WURZEL/target-shared/release/myl-test" \
        "$WURZEL/TESTCLIENT/myl-testclient/target/release/myl-test"
    do
        [ -x "$kandidat" ] && { printf '%s' "$kandidat"; return 0; }
    done
    return 1
}

if command -v cargo >/dev/null 2>&1; then
    # `cargo build` prüft selbst, ob etwas zu tun ist, und ist im
    # unveränderten Fall in unter einer Sekunde durch. Eine eigene
    # Zeitstempel-Logik wäre eine zweite, schlechtere Antwort auf dieselbe
    # Frage.
    echo "Baue Testclient (beim ersten Mal dauert das einige Minuten) ..."
    cargo build --release --quiet --manifest-path "$MANIFEST"
elif binary_finden >/dev/null; then
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

BIN=$(binary_finden) || {
    echo "Fehler: myl-test wurde gebaut, ist aber nicht auffindbar." >&2
    echo "Gesucht in target-shared/release und myl-testclient/target/release." >&2
    exit 1
}

exec "$BIN" "$@"
