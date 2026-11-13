#!/usr/bin/env bash
# Die Kisten der CI nach Gruppen, und was je Kiste geprueft wird.
#
#   .github/ci/kisten.sh pruefen <gruppe>       Clippy und Proben jeder Kiste der Gruppe
#   .github/ci/kisten.sh wache                  jede Kiste in genau einer Linux-Gruppe, jede Fassung gleich
#   .github/ci/kisten.sh konformitaet <system>  die vier Abdruecke der Konformitaetsvektoren
#   .github/ci/kisten.sh artefakte [log]        NICHT in der CI: jede Probe, die ein Artefakt laedt
#   .github/ci/kisten.sh grosse [log]           NICHT in der CI: die Proben mit `#[ignore]`, 8 bis 29 GB
#   .github/ci/kisten.sh lange [log]            NICHT in der CI: die Proben mit `#[ignore]`, die an der Uhr haengen
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
    klient) echo "AGENT_LAYER/local-agent CLIENT/myl-senses CLIENT/myl-scan CLIENT/myl-module
                  CLIENT/myl-console CLIENT/myl-client CLIENT/myl-oberflaeche" ;;
    # Windows und macOS: nur, was sich zwischen den Systemen unterscheiden
    # kann (Rechnen auf einer zweiten Architektur, Pfade, Prozesse, Fenster).
    plattform-kerne) echo "INTEGER_LLM/kernels SHARED_TYPES/myl-types CONSENSUS/myl-ledger
                           CONSENSUS/myl-scheduler CONSENSUS/myl-consensus NODE/myl-node
                           TESTCLIENT/myl-testclient CLIENT/myl-client" ;;
    plattform-klient) echo "STORAGE/myl-store COMPUTE_PIPELINE/dispatcher AGENT_LAYER/myl-agent
                            AGENT_LAYER/local-agent CLIENT/myl-senses CLIENT/myl-scan CLIENT/myl-module
                            CLIENT/myl-console CLIENT/myl-oberflaeche" ;;
    # Kisten mit Proben, die ein Artefakt laden. In der CI springen sie mit
    # `MYL_OHNE_ARTEFAKTE` ab; ganz laufen sie nur ueber `artefakte`.
    artefakt) echo "INTEGER_LLM/runtime COMPUTE_PIPELINE/myl-pod TESTCLIENT/myl-testclient
                    CLIENT/myl-client" ;;
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
  local fehler=0 alle gelistet fassung k g d p hat
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
  # Wer den Absprung ohne Artefakte kennt, muss dort stehen, wo er ganz laeuft.
  for k in $alle; do
    if grep -rqs MYL_OHNE_ARTEFAKTE "$k/src" "$k/tests" && ! gruppe artefakt | tr -s ' \n' '\n\n' | grep -qx "$k"; then
      echo "FEHLER: $k springt ohne Artefakte ab und steht nicht in der Gruppe artefakt"; fehler=1
    fi
  done
  # Eine Probe mit `#[ignore]` laeuft nirgends von selbst. Ohne Platz in
  # der Liste der grossen oder der langen faehrt sie niemand, und sie
  # verrottet ungesehen.
  for k in $(gruppe artefakt); do
    for d in $(grep -rls '^ *#\[ignore' "$k/src" "$k/tests" || true); do
      p=$(basename "$d" .rs)
      { grosse_proben; lange_proben; } | grep -qE "^$k $p( |$)" || { echo "FEHLER: $d hat eine Probe mit #[ignore] und steht weder in grosse_proben noch in lange_proben"; fehler=1; }
    done
  done
  [ "$fehler" -eq 0 ] || exit 1
  echo "$(echo "$alle" | wc -l | tr -d ' ') Kisten, jede in einer Gruppe, jede auf $fassung"
}

# Die Zeilen eines Protokolls, die einen Absprung melden. Zwei Meldungen
# sehen so aus und sind keiner; sie stehen hier mit ihrem Grund, und alles
# andere zaehlt:
#  - „liegt unter der Aufloesung“: Der Trainingslauf laesst eine Ebene aus,
#    deren Ausgabe zu klein zum Messen ist, und verlangt selbst, dass
#    mindestens eine Ebene messbar war.
#  - der Schalter mit dem Verzeichnis des Pods oder einem, das es nicht
#    gibt: die Probe des Schalters selbst. Sie setzt ihn, um ihn zu pruefen.
#    Ein echter Absprung nennt den Ordner eines Artefakts.
abspruenge() {
  { grep -E 'SKIP|[Uu]ebersprungen|übersprungen' "$1" || true; } | { grep -vE \
    -e 'liegt unter der Aufloesung' \
    -e 'SKIP \(MYL_OHNE_ARTEFAKTE gesetzt\): "(/dieses/verzeichnis/gibt/es/nicht|[^"]*COMPUTE_PIPELINE[/\\]+myl-pod)"$' || true; }
}

# Ein Ordner fuer Protokolle unter `TMPDIR`. ⚠️ `mktemp -d` ohne Vorlage
# uebergeht `TMPDIR` unter macOS und legt im Temp-Ordner des Systems an;
# erst die Vorlage mit Pfad nimmt ihn (gefunden am 2026-11-12, drei Runden
# hatten dort Protokolle hinterlassen).
zwischenordner() {
  mktemp -d "${TMPDIR:-/tmp}/myl-ci.XXXXXX" | sed 's|//*|/|g'
}

# Die Gegenseite der CI, fuer eine Maschine MIT Artefakten: Hier laeuft, was
# dort abspringt, und **ein Absprung ist hier ein Fehler**. Eine Probe, die
# ihr Artefakt nicht findet und still zurueckkehrt, sieht in `cargo test`
# aus wie eine bestandene; so standen an einem Tag fuenf Pruefungen rot
# oder liefen gar nicht, ohne dass es jemand sah.
#
# Laedt Modelle: einzeln fahren, nicht neben einer Messung. Je Kiste ein
# Protokoll im Ordner `log`, am Ende eine Zeile je Kiste.
artefakte() {
  local log=${1:-} fehler=0 n=0 kiste name datei spruenge
  [ -z "${MYL_OHNE_ARTEFAKTE:-}" ] || { echo "FEHLER: MYL_OHNE_ARTEFAKTE ist gesetzt, damit spraenge alles ab"; exit 1; }
  [ -n "$log" ] || log=$(zwischenordner)
  mkdir -p "$log"
  for kiste in $(gruppe artefakt); do
    n=$((n + 1))
    name=$(echo "$kiste" | tr '/' '-')
    datei="$log/$name.log"
    # `--nocapture`: Der Absprung meldet sich nur auf der Fehlerausgabe, und
    # die zeigt `cargo test` bei einer bestandenen Probe sonst nicht.
    if (cd "$kiste" && cargo test $C -- --nocapture) > "$datei" 2>&1; then
      spruenge=$(abspruenge "$datei" | wc -l | tr -d ' ')
      if [ "$spruenge" -eq 0 ]; then
        echo "gut      $kiste"
      else
        echo "FEHLER   $kiste: $spruenge Absprung/Abspruenge, siehe $datei"
        abspruenge "$datei" | sort | uniq -c
        fehler=1
      fi
    else
      echo "FEHLER   $kiste: Proben rot, siehe $datei"
      grep -E '^test .* FAILED|panicked at|^error' "$datei" | head -20 || true
      fehler=1
    fi
  done
  [ "$n" -gt 0 ] || { echo "FEHLER: Gruppe artefakt ohne Kiste"; exit 1; }

  # ⚑ Was ausser den Kisten nur hier laeuft (seit 2026-11-11): der volle
  # Konformitaetslauf je Rechenweg und die Mehrknoten- und Chaosprobe der
  # Pipeline ueber getrennte Prozesse. Die beiden Python-Proben verlangen
  # selbst einen Auslieferungsbau (Kopf von `test_pipeline_multinode.py`).
  # 📌 Vorher liefen sie nur von Hand, obwohl sie die Bitgleichheit ueber
  # Prozesse und unter Paketverlust belegen.
  local wege="reference cpu-simd" b ergebnis zusatz=0
  [ "$(uname -s)" = Darwin ] && wege="$wege metal"
  for b in $wege; do
    zusatz=$((zusatz + 1))
    datei="$log/konformitaet-$b.log"
    (cd INTEGER_LLM/conformance && INTEGER_LLM_MODEL=sparrow-0.6b-int8 sh run.sh "$b") > "$datei" 2>&1
    case $? in
      0) ergebnis=$(sed -n 's/^=== Ergebnis: \([0-9]*\)\/\([0-9]*\) bestanden, 0 fehlgeschlagen ===$/\1 \2/p' "$datei")
         set -- $ergebnis
         if [ -n "${1:-}" ] && [ "$1" = "$2" ]; then
           echo "gut      Konformitaet $b: $1/$2"
         else
           echo "FEHLER   Konformitaet $b: kein volles Ergebnis, siehe $datei"; fehler=1
         fi ;;
      # Der Pruefstand lehnt einen Rechenweg ab, den diese Uebersetzung
      # nicht hat (Fund 33, Fund 34); das ist kein Absprung.
      2) echo "entfaellt Konformitaet $b: kein eigener Rechenweg auf dieser Maschine" ;;
      *) echo "FEHLER   Konformitaet $b: rot, siehe $datei"; fehler=1 ;;
    esac
  done
  if (cd INTEGER_LLM/pipeline && cargo build --release $C) > "$log/pipeline-bau.log" 2>&1 \
     && (cd INTEGER_LLM/runtime && cargo build --release $C) >> "$log/pipeline-bau.log" 2>&1; then
    for b in integration/test_pipeline_multinode chaos/test_chaos; do
      zusatz=$((zusatz + 1))
      datei="$log/$(basename "$b").log"
      if (cd INTEGER_LLM && python3 "tests/$b.py") > "$datei" 2>&1 && grep -qE 'Alle .*Tests PASSED' "$datei"; then
        echo "gut      $b"
      else
        echo "FEHLER   $b: rot, siehe $datei"; fehler=1
      fi
    done
  else
    echo "FEHLER   Auslieferungsbau der Pipeline, siehe $log/pipeline-bau.log"; fehler=1
  fi
  [ "$fehler" -eq 0 ] || exit 1
  echo "$n Kiste(n) mit Artefakten geprueft, kein Absprung; dazu $zusatz Lauf/Laeufe ausserhalb der Kisten"
}

# Die Proben mit `#[ignore]`: je Zeile die Kiste, die Probendatei und was
# `cargo test` fuer sie zusaetzlich braucht. Sie laden 8 bis 29 GB und laufen
# deshalb weder in der CI noch in `artefakte`. `--release` nur, wo die Datei
# selbst es begruendet: Ein Modell mit 8 oder 35 Milliarden Gewichten braucht
# im Testprofil Minuten je Token, und dort wird verglichen, nicht auf
# Ueberlauf geprueft.
grosse_proben() {
  echo "INTEGER_LLM/runtime training_moe"
  echo "INTEGER_LLM/runtime shardtraining"
  echo "INTEGER_LLM/runtime ternaer --release"
  echo "INTEGER_LLM/runtime vierbit --release --features cpu-simd"
  echo "CLIENT/myl-senses mel_wie_whisper_cpp"
  echo "CLIENT/myl-senses whisper_encoder"
  echo "CLIENT/myl-senses textzerleger"
  echo "CLIENT/myl-senses tonzerleger"
  echo "CLIENT/myl-senses sprecher"
  echo "CLIENT/myl-senses stimmabdruck"
  echo "CLIENT/myl-senses klonen"
  echo "CLIENT/myl-senses eigensprecher"
}

# Die Proben mit `#[ignore]`, die nicht am Speicher haengen, sondern an der
# Uhr: Die Probekette laeuft in Echtzeit, eine Epoche dauert eine
# Viertelstunde. Die Datei nennt selbst, welche Programme vorher gebaut sein
# muessen, und scheitert mit dem Befehl, wenn eines fehlt. Je Datei laeuft
# zuerst der kurze Lauf (unter einer Minute): Die Proben fahren nach Namen.
lange_proben() {
  echo "TESTCLIENT/myl-testclient pod_aus_klienten"
}

# Faehrt die Proben einer Liste mit `--ignored`, eine nach der anderen. Gut
# ist eine Datei nur, wenn mindestens eine Probe wirklich lief: `--ignored`
# mit einem Filter, der nichts trifft, endet auch mit Rueckgabe 0.
auf_zuruf() {
  local liste=$1 art=$2 log=${3:-} fehler=0 n=0 kiste probe zusatz datei
  [ -z "${MYL_OHNE_ARTEFAKTE:-}" ] || { echo "FEHLER: MYL_OHNE_ARTEFAKTE ist gesetzt, damit spraenge alles ab"; exit 1; }
  [ -n "$log" ] || log=$(zwischenordner)
  mkdir -p "$log"
  while read -r kiste probe zusatz; do
    n=$((n + 1))
    datei="$log/$probe.log"
    # `--test-threads=1`: Nebeneinander luede jede Probe dasselbe Artefakt
    # fuer sich, und die Maschine lagerte aus.
    if (cd "$kiste" && cargo test $C $zusatz --test "$probe" -- --ignored --test-threads=1 --nocapture) > "$datei" 2>&1 < /dev/null \
       && grep -qE '^test result: ok\. [1-9][0-9]* passed' "$datei" \
       && [ -z "$(abspruenge "$datei")" ]; then
      echo "gut      $kiste $probe: $(sed -n 's/^test result: ok\. \([0-9]* passed\).*finished in \(.*\)$/\1, \2/p' "$datei")"
    else
      echo "FEHLER   $kiste $probe: rot, abgesprungen oder nichts gelaufen, siehe $datei"
      grep -E '^test .* FAILED|panicked at|^error|^test result' "$datei" | head -20 || true
      abspruenge "$datei" | head -20
      fehler=1
    fi
  done < <($liste)
  [ "$n" -gt 0 ] || { echo "FEHLER: keine $art Probe gelistet"; exit 1; }
  [ "$fehler" -eq 0 ] || exit 1
  echo "$n $art Probe(n) gefahren"
}

grosse() { auf_zuruf grosse_proben grosse "${1:-}"; }
lange() { auf_zuruf lange_proben lange "${1:-}"; }

# Der Konformitaetswert muss auf jeder Maschine derselbe sein; das ist die
# Zusage, auf der das Protokoll steht. Vier Abdruecke: drei benennen bei
# einem Fehlschlag die Gruppe (Operationen, ternaerer Rechenweg,
# Vierbitformat), der vierte gilt allen vierundzwanzig Vektoren. Ohne
# Artefakt; der Wert ist vom Bauprofil unabhaengig.
konformitaet() {
  local system=$1 ordner
  ordner=$(zwischenordner)
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
  artefakte) artefakte "${2:-}" ;;
  grosse) grosse "${2:-}" ;;
  lange) lange "${2:-}" ;;
  *) echo "Aufruf: $0 pruefen <gruppe> | wache | konformitaet <system> | artefakte [log] | grosse [log] | lange [log]" >&2; exit 2 ;;
esac
