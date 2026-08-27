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
# Fehlt Rust, bietet der Starter die automatische Installation an
# (rustup, im Heimatverzeichnis, ohne Administratorrechte). Fehlt der
# C-Compiler zum Binden, nennt er den passenden Befehl des Systems.
# Beides geschieht nur nach Rückfrage; nichts davon läuft still ab.
#
# POSIX-sh, keine Bashismen: Auf manchen Systemen ist /bin/sh dash.

set -eu

# Repository-Wurzel durch Aufwaertssuche bestimmen, nicht ueber eine feste
# Tiefe. Die erste Fassung rechnete mit dem Wurzelverzeichnis als
# Ablageort und war sofort kaputt, als der Starter in TESTCLIENT/ verschoben wurde.
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

rust_anleitung() {
    cat >&2 <<'ENDE'
Rust von Hand installieren (einmalig, ohne Administratorrechte):

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Danach eine neue Terminalsitzung öffnen und diesen Starter erneut
aufrufen.
ENDE
}

# Rust suchen, notfalls installieren. Zwei Stufen: der Suchpfad, dann das
# Standardverzeichnis von rustup. Die zweite Stufe fängt Sitzungen ab,
# die vor einer frischen Installation geöffnet wurden und den neuen Pfad
# noch nicht kennen.
CARGO_BIN=""
if command -v cargo >/dev/null 2>&1; then
    CARGO_BIN="cargo"
elif [ -x "$HOME/.cargo/bin/cargo" ]; then
    CARGO_BIN="$HOME/.cargo/bin/cargo"
    PATH="$HOME/.cargo/bin:$PATH"
    export PATH
elif binary_finden >/dev/null; then
    # Kein cargo, aber ein fertiges Binary: dann wird nichts installiert.
    echo "Hinweis: cargo nicht gefunden, benutze das vorhandene Binary." >&2
else
    echo "" >&2
    echo "Rust ist auf dieser Maschine nicht installiert." >&2
    echo "Der Testclient braucht es einmalig, um sich selbst zu bauen." >&2
    echo "" >&2
    printf 'Rust jetzt automatisch installieren? Enter = ja, n = nein: ' >&2
    read -r ANTWORT || ANTWORT="n"
    case "$ANTWORT" in
        n|N|nein|Nein) rust_anleitung; exit 1 ;;
    esac
    if ! command -v curl >/dev/null 2>&1; then
        echo "curl fehlt; Rust kann nicht automatisch geholt werden." >&2
        rust_anleitung
        exit 1
    fi
    # -y beantwortet die rustup-Fragen selbst, minimal holt nur Compiler
    # und cargo. Die Installation landet im Heimatverzeichnis und braucht
    # keine Administratorrechte.
    if ! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --profile minimal --default-toolchain stable; then
        echo "Die Rust-Installation wurde abgebrochen oder ist fehlgeschlagen." >&2
        rust_anleitung
        exit 1
    fi
    if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
        rust_anleitung
        exit 1
    fi
    CARGO_BIN="$HOME/.cargo/bin/cargo"
    PATH="$HOME/.cargo/bin:$PATH"
    export PATH
fi

# C-Compiler und Linker prüfen. Der Rust-Übersetzer braucht beides zum
# Binden; fehlt es, scheitert der erste Bau nach mehreren Minuten am
# letzten Schritt. Die Probe davor spart genau diese Zeit.
case "$(uname -s)" in
    Darwin)
        if ! xcode-select -p >/dev/null 2>&1; then
            echo "" >&2
            echo "Die Xcode-Kommandozeilenwerkzeuge fehlen (Compiler und" >&2
            echo "Binder). Ohne sie kann der Testclient nicht bauen." >&2
            printf 'Jetzt installieren? Ein Systemdialog erscheint. Enter = ja, n = nein: ' >&2
            read -r XCA || XCA="n"
            case "$XCA" in
                n|N|nein|Nein)
                    echo "Abbruch: xcode-select --install von Hand ausführen." >&2
                    exit 1 ;;
            esac
            xcode-select --install || true
            echo "Bitte warten, bis der Dialog die Installation abgeschlossen" >&2
            printf 'hat, dann hier Enter drücken ... ' >&2
            read -r _ || true
            if ! xcode-select -p >/dev/null 2>&1; then
                echo "Die Werkzeuge sind weiterhin nicht sichtbar; Abbruch." >&2
                exit 1
            fi
        fi
        ;;
    *)
        if ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1; then
            echo "Kein C-Compiler gefunden; ohne ihn kann Rust nicht binden." >&2
            if command -v apt-get >/dev/null 2>&1; then
                echo "Vorschlag: sudo apt-get install -y build-essential pkg-config libssl-dev" >&2
            elif command -v dnf >/dev/null 2>&1; then
                echo "Vorschlag: sudo dnf install -y gcc openssl-devel pkgconf" >&2
            elif command -v pacman >/dev/null 2>&1; then
                echo "Vorschlag: sudo pacman -S --needed base-devel openssl pkgconf" >&2
            elif command -v zypper >/dev/null 2>&1; then
                echo "Vorschlag: sudo zypper install -y gcc make libopenssl-devel pkg-config" >&2
            fi
            exit 1
        fi
        ;;
esac

# `cargo build` prüft selbst, ob etwas zu tun ist, und ist im
# unveränderten Fall in unter einer Sekunde durch. Eine eigene
# Zeitstempel-Logik wäre eine zweite, schlechtere Antwort auf dieselbe
# Frage. Leer bleibt CARGO_BIN nur im Zweig „kein cargo, aber Binary":
# Dann gibt es nichts zu bauen.
if [ -n "$CARGO_BIN" ]; then
    echo "Baue Testclient (beim ersten Mal dauert das einige Minuten) ..."
    "$CARGO_BIN" build --release --quiet --manifest-path "$MANIFEST"
fi

BIN=$(binary_finden) || {
    echo "Fehler: myl-test wurde gebaut, ist aber nicht auffindbar." >&2
    echo "Gesucht in target-shared/release und myl-testclient/target/release." >&2
    exit 1
}

exec "$BIN" "$@"
