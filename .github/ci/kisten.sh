#!/usr/bin/env bash
# Die Kisten der CI nach Gruppen, und was je Kiste geprueft wird.
#
#   .github/ci/kisten.sh pruefen <gruppe>       Clippy und Proben jeder Kiste der Gruppe
#   .github/ci/kisten.sh wache                  jede Kiste in genau einer Linux-Gruppe, jede Fassung gleich
#   .github/ci/kisten.sh konformitaet <system>  die vier Abdruecke der Konformitaetsvektoren
#
# ⚑ **Eine Stelle fuer die Liste.** Bis zum 2026-10-22 stand jede Kiste je
# Job von Hand in der Workflow-Datei, und dreimal lief eine Kiste deshalb in
# keinem Test (NODE mit 132 Proben; der Klient; drei Kisten des Agenten).
# Die Wache vergleicht die Gruppen mit dem Dateisystem: Eine neue Kiste ohne
# Gruppe macht die CI rot, bevor sie ungeprueft bleibt.
#
# ⚑ **Alles offline aus dem Vorrat** (`--locked --offline`): Die CI belegt
# damit bei jedem Push, dass jede Kiste ohne Netz baut, nicht nur die
# ausgelieferten.
#
# Ohne `declare -A` und ohne andere Neuerungen von bash 4: Das Skript laeuft
# auch unter dem bash 3.2 von macOS und unter Git Bash auf Windows.
set -euo pipefail
cd "$(dirname "$0")/../.."

C="--locked --offline --quiet"
LINUX_GRUPPEN="rechnen protokoll knoten klient"

gruppe() {
  case "$1" in
    # Kerne, Laufzeit und alles, was auf ihnen rechnet.
    rechnen) echo "INTEGER_LLM/kernels INTEGER_LLM/runtime INTEGER_LLM/pipeline
                   COMPUTE_PIPELINE/myl-pod COMPUTE_PIPELINE/dispatcher TRAINING/myl-train" ;;
    # Das Protokoll. GOVERNANCE steht am Ende: `tests/gleichstand.rs` haelt
    # ihre Registry gegen die Konstanten der anderen, und ein davongelaufener
    # Wert soll dort auffallen, nachdem die anderen fuer sich bestanden haben.
    protokoll) echo "SHARED_TYPES/myl-types NETWORKING/myl-net NETWORKING/myl-siegel
                     CONSENSUS/myl-ledger CONSENSUS/myl-scheduler CONSENSUS/myl-consensus
                     TOKENOMICS/myl-tokenomics VERIFICATION/myl-verifier STORAGE/myl-store
                     GATEWAY/myl-gateway SIMULATION/myl-simulation AGENT_LAYER/myl-agent
                     SYSTEM/golemos/einrichten GOVERNANCE/myl-governance" ;;
    # Der Knoten und der Pruefstand, der auf ihm aufsetzt.
    knoten) echo "NODE/myl-node TESTCLIENT/myl-testclient" ;;
    # Was ein Mensch anfasst; braucht unter Linux die Bibliotheken des Fensters.
    klient) echo "AGENT_LAYER/local-agent CLIENT/myl-senses CLIENT/myl-module
                  CLIENT/myl-console CLIENT/myl-client CLIENT/myl-oberflaeche" ;;
    # Windows und macOS: nur, was sich zwischen den Systemen unterscheiden
    # kann (Rechnen auf einer zweiten Architektur, Pfade, Prozesse, Fenster).
    plattform-kerne) echo "INTEGER_LLM/kernels SHARED_TYPES/myl-types CONSENSUS/myl-ledger
                           CONSENSUS/myl-scheduler CONSENSUS/myl-consensus NODE/myl-node
                           TESTCLIENT/myl-testclient CLIENT/myl-client" ;;
    plattform-klient) echo "STORAGE/myl-store COMPUTE_PIPELINE/dispatcher AGENT_LAYER/myl-agent
                            AGENT_LAYER/local-agent CLIENT/myl-senses CLIENT/myl-module
                            CLIENT/myl-console CLIENT/myl-oberflaeche" ;;
    *) echo "unbekannte Gruppe: $1" >&2; return 2 ;;
  esac
}

# Die Proben einer Kiste. Der Normalfall ist `cargo test`; die Ausnahmen
# stehen hier mit ihrem Grund.
proben() {
  local kiste=$1 g=$2
  case "$kiste:$g" in
    INTEGER_LLM/kernels:rechnen)
      cargo test $C
      # x86_64: Hier greift die Weiche des SIMD-Wegs, und nur mit dem
      # Merkmal findet `tests/test_backend_parity.rs` ein zweites Backend.
      cargo test $C --features cpu-simd
      cargo build $C --features cuda
      cargo build $C --features rocm
      # Die Kerne fuer NVIDIA und AMD ohne Karte: der Nachbau gegen die CPU.
      cargo test $C --features cuda,rocm --lib ressourcen::karten
      # Was nur mit seinem Merkmal uebersetzt wird, sieht Clippy sonst nie.
      for m in cpu-simd cuda rocm metal; do
        cargo clippy --locked --offline --all-targets --features "$m" -- -D warnings
      done ;;
    # Laufzeit und Pipeline: Die Integrationsproben brauchen Artefakte.
    INTEGER_LLM/runtime:*) cargo test $C --lib ;;
    INTEGER_LLM/pipeline:*) cargo test $C --lib; cargo build $C --all-targets ;;
    # Der Pod läuft ganz: Jede Probe, die ein Artefakt braucht, fragt
    # `artefakte::vorhanden` und springt mit der Erlaubnis des Jobs ab (bis
    # zum 2026-10-23 liefen hier nur drei Teile, weil sechs Dateien still
    # absprangen und zwei hart behaupteten).
    # Auf Windows und macOS der rechnende und dateinahe Teil; das Netz der
    # Integrationsproben prueft Linux.
    NODE/myl-node:plattform-*|TESTCLIENT/myl-testclient:plattform-*) cargo test $C --lib ;;
    *) cargo test $C ;;
  esac
}

pruefen() {
  local g=$1 n=0
  for kiste in $(gruppe "$g"); do
    n=$((n + 1))
    echo "::group::$kiste ($g)"
    (cd "$kiste" && cargo clippy --locked --offline --all-targets -- -D warnings && proben "$kiste" "$g")
    echo "::endgroup::"
  done
  # Eine Gruppe ohne Kiste waere ein gruener Job ohne Gegenstand.
  [ "$n" -gt 0 ] || { echo "FEHLER: Gruppe $g ohne Kiste"; exit 1; }
  echo "$n Kiste(n) der Gruppe $g geprueft"
}

wache() {
  local fehler=0 alle gelistet fassung
  alle=$(ls */*/Cargo.toml SYSTEM/*/*/Cargo.toml | sed 's|/Cargo.toml$||' | sort)
  gelistet=$(for g in $LINUX_GRUPPEN; do gruppe "$g"; done | tr -s ' \n' '\n\n' | grep -v '^$' | sort)
  for k in $(comm -23 <(echo "$alle") <(echo "$gelistet")); do echo "FEHLER: $k steht in keiner Gruppe"; fehler=1; done
  for k in $(echo "$gelistet" | uniq -d); do echo "FEHLER: $k steht in mehr als einer Linux-Gruppe"; fehler=1; done
  for g in $LINUX_GRUPPEN plattform-kerne plattform-klient; do
    for k in $(gruppe "$g"); do [ -f "$k/Cargo.toml" ] || { echo "FEHLER: $k (Gruppe $g) gibt es nicht"; fehler=1; }; done
  done
  # ⚑ Eine Fassung: Jede Kiste nennt als `rust-version` die Fassung aus
  # `rust-toolchain.toml` (ohne die letzte Stelle). Mit ihr baut die CI.
  fassung=$(sed -n 's/^channel = "\([0-9]*\.[0-9]*\)\..*"/\1/p' rust-toolchain.toml)
  [ -n "$fassung" ] || { echo "FEHLER: rust-toolchain.toml nennt keine Fassung"; exit 1; }
  for k in $alle; do
    hat=$(sed -n 's/^rust-version = "\(.*\)"/\1/p' "$k/Cargo.toml" | head -1)
    [ "$hat" = "$fassung" ] || { echo "FEHLER: $k nennt rust-version \"$hat\", die Werkzeugkette ist $fassung"; fehler=1; }
  done
  [ "$fehler" -eq 0 ] || exit 1
  echo "$(echo "$alle" | wc -l | tr -d ' ') Kisten, jede in einer Gruppe, jede auf $fassung"
}

# Der Konformitaetswert muss auf jeder Maschine derselbe sein; das ist die
# Zusage, auf der das Protokoll steht. Vier Abdruecke: drei benennen bei
# einem Fehlschlag die Gruppe (Operationen, ternaerer Rechenweg,
# Vierbitformat), der vierte gilt allen vierundzwanzig Vektoren. Ohne
# Artefakt; der Wert ist vom Bauprofil unabhaengig.
konformitaet() {
  local system=$1 ordner
  ordner=$(mktemp -d)
  # `--profile test`: was die Proben schon uebersetzt haben, kein zweiter Bau.
  (cd TESTCLIENT/myl-testclient && cargo run $C --profile test -- --name ci --logs "$ordner" konformitaet)
  for paar in "894d8357ae92b5c1:konformitaet_op (6 Vektoren)" \
              "f1b60342b9cfcdb6:konformitaet_ternaer (5 Vektoren)" \
              "9102be0890358ceb:konformitaet_vierbit (2 Vektoren)" \
              "cb696efd3817e575:konformitaet gesamt (24 Vektoren)"; do
    if ! grep -q "${paar%%:*}" "$ordner"/*.jsonl; then
      echo "FEHLER: ${paar#*:} weicht ab, erwartet ${paar%%:*}"
      grep -o '"name":"[^"]*","digest":"[^"]*"' "$ordner"/*.jsonl || cat "$ordner"/*.jsonl
      exit 1
    fi
  done
  echo "Konformitaet belegt auf $system: 24 Vektoren, vier Abdruecke"
}

case "${1:-}" in
  pruefen) pruefen "${2:?Gruppe fehlt}" ;;
  wache) wache ;;
  konformitaet) konformitaet "${2:?System fehlt}" ;;
  *) echo "Aufruf: $0 pruefen <gruppe> | wache | konformitaet <system>" >&2; exit 2 ;;
esac
