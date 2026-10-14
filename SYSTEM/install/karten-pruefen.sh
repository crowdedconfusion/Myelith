#!/bin/sh
# Prueft und misst die Grafikkarten einer (Miet-)Maschine fuer Myelith.
#
# ⚑ **Wozu:** Auf einer gemieteten Maschine mit NVIDIA- oder AMD-Karten in
# wenigen Minuten wissen, ob die Karten rechnen und ob sie so rechnen wie die
# CPU. Der Reihe nach:
#
#   1. Karten, Treiber und Laufzeituebersetzer finden (NVRTC bzw. hiprtc).
#   2. Die Kerne bauen und pruefen: der Nachbau auf der CPU und die
#      Selbstpruefung auf jeder Karte (`cargo test --lib ressourcen::karten`).
#   3. Messen: CPU gegen Karten je Matrixform und Stapelgroesse, das
#      Hochladen, ein Expertenbuendel (`karten_messung`).
#   4. Der Konformitaetslauf mit erzwungenen Karten (`run.sh cuda|rocm`),
#      falls das Artefakt `myelith-0.6b` da ist.
#   5. Ein ganzes Modell ueber alle Karten (`bench_probe`): eine lange
#      Vorbereitung einmal nur auf der CPU, dann auf allen Karten, dann auf
#      der Haelfte, dann mit ungleichen Gewichten. Der Abdruck ueber alle
#      Logits muss jedes Mal derselbe sein, und **jede Karte muss
#      gerechnet haben**; sonst ist der Lauf gescheitert.
#
# Alles landet in einem Bericht unter BENCHMARKS/Inferenz/results/.
#
# ⚑ **Der Bau braucht weder CUDA-Toolkit noch ROCm-SDK.** Treiber und
# Uebersetzer werden zur Laufzeit geladen. Gebraucht werden auf der
# Maschine nur: der Treiber (`libcuda` bzw. `libamdhip64`) und der
# Laufzeituebersetzer (`libnvrtc` bzw. `libhiprtc`). Bei NVIDIA fehlt
# NVRTC oft, wenn nur der Treiber installiert ist; `--einrichten` holt ihn
# dann ueber pip (Paket `nvidia-cuda-nvrtc-cu12`).
#
# ⚠️ **Das Artefakt fuer den Konformitaetslauf** (0,92 GB) baut sich auf der
# Mietmaschine nur mit torch und Netz. Schneller ist kopieren:
#   rsync -a INTEGER_LLM/artifacts/myelith-0.6b <maschine>:<klon>/INTEGER_LLM/artifacts/
#
# ⚠️ **Die Artefakte fuer Schritt 5** liegen ebenso nicht im Klon. Ohne
# Angabe nimmt das Skript `myelith-4b` (4,8 GB; die Matrizen werden nach
# Zeilen ueber die Karten geteilt). Ein Gemisch wie `myelith-30b-a3b`
# (31 GB) verteilt dazu ganze Experten; `--modell` darf mehrfach stehen.
#
# Aufruf, aus dem Klon:
#   sh SYSTEM/install/karten-pruefen.sh               pruefen und messen
#   sh SYSTEM/install/karten-pruefen.sh --einrichten  dazu fehlendes NVRTC und Rust holen (Netz)
#   sh SYSTEM/install/karten-pruefen.sh --nur-finden  nur Schritt 1
#   sh SYSTEM/install/karten-pruefen.sh --modell myelith-4b --modell myelith-30b-a3b
#   sh SYSTEM/install/karten-pruefen.sh --nur-modell  nur Schritt 5 (nach einem vollen Lauf)
#   sh SYSTEM/install/karten-pruefen.sh --prompt-woerter 400   Laenge der Vorbereitung
#   sh SYSTEM/install/karten-pruefen.sh --nachbau 6   Probelauf ohne Karte, auch auf macOS
#
# ⚑ **Der Probelauf** (`--nachbau N`) faehrt Schritt 5 mit N nachgebildeten
# Karten: Speicher im Hauptspeicher, Kerne auf der CPU, jeder Zugriff gegen
# den belegten Speicher geprueft. Er prueft vor der Miete alles ausser der
# Karte selbst: Verteilung, Zusammensetzen, Puffer und Versaetze, dieses
# Skript. Sein Bericht geht nach SYSTEM/logs/karten/ und nicht zu den
# Messungen, denn er misst nichts.
set -u

HIER=$(cd "$(dirname "$0")/../.." && pwd)
KERNE="$HIER/INTEGER_LLM/kernels"
ERGEBNISSE="$HIER/BENCHMARKS/Inferenz/results"

einrichten=0
nur_finden=0
nur_modell=0
NACHBAU=0
MODELLE=""
WOERTER=400
while [ $# -gt 0 ]; do
  case "$1" in
    --einrichten) einrichten=1 ;;
    --nur-finden) nur_finden=1 ;;
    --nur-modell) nur_modell=1 ;;
    --modell) shift; MODELLE="$MODELLE ${1:?--modell braucht einen Namen}" ;;
    --prompt-woerter) shift; WOERTER="${1:?--prompt-woerter braucht eine Zahl}" ;;
    --nachbau) shift; NACHBAU="${1:?--nachbau braucht eine Zahl}"; nur_modell=1 ;;
    *) echo "unbekannter Schalter: $1"; exit 2 ;;
  esac
  shift
done
[ -z "$MODELLE" ] && MODELLE="myelith-4b"

sagen() { printf '\n== %s ==\n' "$1"; }
da() { command -v "$1" >/dev/null 2>&1; }
# Ob der Lader des Systems eine Bibliothek kennt.
bekannt() { ldconfig -p 2>/dev/null | grep -q "$1"; }

if [ "$NACHBAU" -gt 0 ]; then
  export MYL_GPU_NACHBAU="$NACHBAU"
  export MYL_KARTEN_ERWARTET="$NACHBAU"
  ERGEBNISSE="$HIER/SYSTEM/logs/karten"
  echo "Probelauf mit $NACHBAU nachgebildeten Karten (rechnen auf der CPU, messen nichts)."
elif [ "$(uname -s)" != "Linux" ]; then
  echo "Dieses Skript ist fuer Linux. Unter Windows: dieselben Schritte von Hand, siehe SYSTEM/install/README.md."
  exit 2
fi

# ── 1. Karten, Treiber, Uebersetzer ──────────────────────────────────
WEGE=""
if [ "$NACHBAU" -gt 0 ]; then
  WEGE="cuda"
else
sagen "Karten"
if da nvidia-smi && nvidia-smi -L >/dev/null 2>&1; then
  nvidia-smi -L
  WEGE="$WEGE cuda"
fi
if da rocm-smi && rocm-smi --showproductname >/dev/null 2>&1; then
  rocm-smi --showproductname | grep -iE "card|gpu" | head -16
  WEGE="$WEGE rocm"
elif [ -e /dev/kfd ]; then
  echo "AMD-Rechenschnittstelle /dev/kfd ist da (rocm-smi fehlt)."
  WEGE="$WEGE rocm"
fi
if [ -z "$WEGE" ]; then
  echo "Keine NVIDIA- oder AMD-Karte gefunden (nvidia-smi, rocm-smi, /dev/kfd)."
  exit 2
fi
fi

[ "$NACHBAU" -gt 0 ] || case "$WEGE" in *cuda*)
  sagen "CUDA: Treiber und NVRTC"
  if bekannt libcuda.so.1; then echo "libcuda.so.1: da"; else echo "⚠️ libcuda.so.1 nicht im Suchweg (ldconfig)"; fi
  if bekannt libnvrtc.so || [ -n "${MYL_NVRTC:-}" ]; then
    echo "NVRTC: da ${MYL_NVRTC:-(Suchweg)}"
  else
    # NVRTC aus pip liegt in einem Python-Verzeichnis.
    GEFUNDEN=$(python3 -c 'import nvidia.cuda_nvrtc, os; d=os.path.join(list(nvidia.cuda_nvrtc.__path__)[0], "lib"); print(next((os.path.join(d,f) for f in sorted(os.listdir(d)) if f.startswith("libnvrtc.so")), ""))' 2>/dev/null || true)
    if [ -z "$GEFUNDEN" ] && [ "$einrichten" = 1 ]; then
      echo "NVRTC fehlt; hole nvidia-cuda-nvrtc-cu12 ueber pip …"
      python3 -m pip install --user --quiet nvidia-cuda-nvrtc-cu12 || echo "⚠️ pip ist gescheitert"
      GEFUNDEN=$(python3 -c 'import nvidia.cuda_nvrtc, os; d=os.path.join(list(nvidia.cuda_nvrtc.__path__)[0], "lib"); print(next((os.path.join(d,f) for f in sorted(os.listdir(d)) if f.startswith("libnvrtc.so")), ""))' 2>/dev/null || true)
    fi
    if [ -n "$GEFUNDEN" ]; then
      export MYL_NVRTC="$GEFUNDEN"
      # libnvrtc laedt seine eingebauten Kopfdateien aus demselben Ordner.
      export LD_LIBRARY_PATH="$(dirname "$GEFUNDEN")${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
      echo "NVRTC: $MYL_NVRTC"
    else
      echo "⛔️ NVRTC fehlt. Mit --einrichten holt das Skript es ueber pip, oder MYL_NVRTC auf libnvrtc.so setzen."
      WEGE=$(echo "$WEGE" | sed 's/cuda//')
    fi
  fi
esac

[ "$NACHBAU" -gt 0 ] || case "$WEGE" in *rocm*)
  sagen "ROCm: HIP und hiprtc"
  for b in libamdhip64.so libhiprtc.so; do
    if bekannt "$b" || [ -e "${ROCM_PATH:-/opt/rocm}/lib/$b" ]; then echo "$b: da"; else echo "⚠️ $b nicht gefunden (ldconfig, \$ROCM_PATH/lib)"; fi
  done
  # ROCm liegt meist unter /opt/rocm, und das ist der Ort, den ROCm selbst
  # vorgibt; gesucht wird dort nur, wenn ROCM_PATH nicht gesetzt ist.
  [ -z "${ROCM_PATH:-}" ] && [ -d /opt/rocm ] && export ROCM_PATH=/opt/rocm
esac

WEGE=$(echo $WEGE)
if [ -z "$WEGE" ]; then
  echo "Kein Rechenweg uebrig."
  exit 2
fi
[ "$nur_finden" = 1 ] && exit 0

# ── Rust ────────────────────────────────────────────────────────────
if ! da cargo; then
  if [ "$einrichten" = 1 ]; then
    sagen "Rust einrichten (rustup)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    . "$HOME/.cargo/env"
  else
    echo "⛔️ cargo fehlt. Mit --einrichten holt das Skript Rust ueber rustup."
    exit 2
  fi
fi
if ! da c++; then
  echo "⚠️ Kein C++-Uebersetzer (c++): Der Nachbau der Kerne scheitert dann. Ubuntu: apt install g++"
fi

DATUM=$(date +%Y-%m-%d)
mkdir -p "$ERGEBNISSE"
FEHLER=0
for WEG in $WEGE; do
  # ⚑ **Kartenmodell und Anzahl, kein Rechnername**: Der Bericht kann
  # versioniert werden, und ein Rechnername nennt oft den Menschen dahinter.
  if [ "$NACHBAU" -gt 0 ]; then
    MODELLNAME="nachbau"
  elif [ "$WEG" = cuda ]; then
    MODELLNAME=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)
  else
    MODELLNAME=$(rocm-smi --showproductname 2>/dev/null | sed -n 's/.*Card [Ss]eries:[[:space:]]*//p' | head -1)
  fi
  KENNUNG=$(printf '%s' "${MODELLNAME:-unbekannt}" | tr -cs 'A-Za-z0-9' '-' | sed 's/^-//;s/-$//')
  if [ "$WEG" = cuda ]; then
    ANZAHL=$(nvidia-smi -L 2>/dev/null | grep -c "^GPU")
  else
    ANZAHL=$(rocm-smi --showid 2>/dev/null | grep -oE "^GPU\[[0-9]+\]" | sort -u | wc -l | tr -d ' ')
  fi
  ANZAHL="${MYL_KARTEN_ERWARTET:-$ANZAHL}"
  BERICHT="$ERGEBNISSE/karten-$WEG-$DATUM-${ANZAHL}x-$KENNUNG.md"
  {
    echo "# Karten: $WEG, $DATUM, ${ANZAHL} x ${MODELLNAME:-unbekannt}"
    echo
    echo "Erzeugt von SYSTEM/install/karten-pruefen.sh."
    echo
    echo '```'
    uname -srm
    case "$WEG" in
      cuda) nvidia-smi --query-gpu=index,name,driver_version,memory.total --format=csv 2>/dev/null ;;
      rocm) rocm-smi --showproductname 2>/dev/null | grep -iE "card|gpu" ;;
    esac
    echo '```'
  } > "$BERICHT"

  if [ "$nur_modell" = 0 ]; then
  # ── 2. Bauen und pruefen ──────────────────────────────────────────
  sagen "$WEG: Nachbau und Selbstpruefung"
  if (cd "$KERNE" && cargo test --features "$WEG" --lib ressourcen::karten -- --nocapture) > "$BERICHT.pruefung" 2>&1; then
    echo "bestanden"
    PRUEFUNG="bestanden"
  else
    echo "⛔️ gescheitert, siehe $BERICHT.pruefung"
    PRUEFUNG="GESCHEITERT"
    FEHLER=1
  fi
  {
    echo
    echo "## Pruefung (Nachbau, Selbstpruefung je Karte): $PRUEFUNG"
    echo
    echo '```'
    grep -E "bestanden:|ohne Karte|Grafikkarte|test result|panicked" "$BERICHT.pruefung"
    echo '```'
  } >> "$BERICHT"
  rm -f "$BERICHT.pruefung"

  # ── 3. Messen ─────────────────────────────────────────────────────
  sagen "$WEG: Messung"
  if (cd "$KERNE" && cargo run --release --features "$WEG" --bin karten_messung) > "$BERICHT.messung" 2>&1; then
    echo "fertig"
  else
    echo "⛔️ Messung mit Fehler, siehe Bericht"
    FEHLER=1
  fi
  { echo; echo "## Messung"; echo; grep -vE "^\s*(Compiling|Finished|Running|warning)" "$BERICHT.messung"; } >> "$BERICHT"
  rm -f "$BERICHT.messung"

  # ── 4. Konformitaet ───────────────────────────────────────────────
  sagen "$WEG: Konformitaet"
  if [ -d "$HIER/INTEGER_LLM/artifacts/myelith-0.6b" ]; then
    if INTEGER_LLM_MODEL=myelith-0.6b sh "$HIER/INTEGER_LLM/conformance/run.sh" "$WEG" > "$BERICHT.konf" 2>&1; then
      echo "bestanden"
    else
      echo "⛔️ nicht bestanden"
      FEHLER=1
    fi
    { echo; echo "## Konformitaet"; echo; echo '```'; grep -E "Ergebnis|Buendelungen|abgelehnt|FAIL" "$BERICHT.konf"; echo '```'; } >> "$BERICHT"
    rm -f "$BERICHT.konf"
  else
    echo "uebersprungen: INTEGER_LLM/artifacts/myelith-0.6b fehlt (siehe Kopf dieses Skripts)"
    { echo; echo "## Konformitaet"; echo; echo "Nicht gelaufen: das Artefakt myelith-0.6b fehlte."; } >> "$BERICHT"
  fi
  fi

  # ── 5. Ein ganzes Modell ueber alle Karten ──────────────────────────
  # Wie viele Karten das System meldet; jede muss nachher gerechnet haben.
  # `rocm-smi` schreibt je Karte mehrere Zeilen, gezaehlt werden die
  # verschiedenen Nummern. MYL_KARTEN_ERWARTET geht vor, falls ein Werkzeug
  # anders zaehlt.
  case "$WEG" in
    cuda) PHYSISCH=$(nvidia-smi -L 2>/dev/null | grep -c "^GPU") ;;
    rocm) PHYSISCH=$(rocm-smi --showid 2>/dev/null | grep -oE "^GPU\[[0-9]+\]" | sort -u | wc -l | tr -d ' ') ;;
  esac
  PHYSISCH="${MYL_KARTEN_ERWARTET:-$PHYSISCH}"
  sagen "$WEG: Modell ueber alle Karten ($PHYSISCH Karten gemeldet)"
  if ! (cd "$HIER/INTEGER_LLM/runtime" && cargo build --release --features "$WEG" --bin bench_probe) > "$BERICHT.bau" 2>&1; then
    echo "⛔️ bench_probe laesst sich nicht bauen, siehe $BERICHT.bau"
    FEHLER=1
    continue
  fi
  rm -f "$BERICHT.bau"
  # Wohin Cargo baut: `SYSTEM/full-build` (siehe .cargo/config.toml), es
  # sei denn, CARGO_TARGET_DIR sagt etwas anderes.
  PROBE="${CARGO_TARGET_DIR:-$HIER/SYSTEM/full-build}/release/bench_probe"
  # Eine lange Vorbereitung: derselbe Absatz, bis die Wortzahl erreicht ist.
  ABSATZ="Myelith rechnet auf jeder Maschine dieselben Zahlen. Eine Vorbereitung ueber viele Token buendelt die Matrizen jeder Ebene, und genau diese Buendel verteilt der Rechner ueber alle Grafikkarten. Jede Karte rechnet einen Teil der Zeilen, und zusammengesetzt wird ohne Rechnung."
  PROMPT=""
  while [ "$(printf '%s' "$PROMPT" | wc -w)" -lt "$WOERTER" ]; do PROMPT="$PROMPT $ABSATZ"; done
  { echo; echo "## Modell ueber alle Karten"; echo; echo "Vorbereitung: $WOERTER Woerter; je Lauf derselbe Abdruck ueber alle Logits verlangt, und jede Karte muss gerechnet haben."; } >> "$BERICHT"
  for MODELL in $MODELLE; do
    ART="$HIER/INTEGER_LLM/artifacts/$MODELL"
    if [ ! -d "$ART" ]; then
      echo "uebersprungen: $ART fehlt"
      { echo; echo "### $MODELL"; echo; echo "Nicht gelaufen: Artefakt fehlt."; } >> "$BERICHT"
      continue
    fi
    { echo; echo "### $MODELL"; echo; echo "| Lauf | Karten mit Arbeit | Buendel | Abdruck | gleich | Vorbereitung ms |"; echo "|---|---|---|---|---|---|"; } >> "$BERICHT"
    # Zuerst die CPU: der Massstab.
    echo "$MODELL: CPU …"
    MYL_GPU_AB=0 "$PROBE" "$ART" "$PROMPT" 8 > "$BERICHT.cpu" 2>&1
    SOLL=$(sed -n 's/^decode_digest //p' "$BERICHT.cpu")
    if [ -z "$SOLL" ]; then
      echo "⛔️ die CPU hat keinen Abdruck geliefert"; tail -5 "$BERICHT.cpu"; FEHLER=1; continue
    fi
    echo "| nur CPU | 0 | 0 | ${SOLL%"${SOLL#????????????????}"} | Massstab | $(sed -n 's/^prefill_ms //p' "$BERICHT.cpu") |" >> "$BERICHT"
    # Dann alle Karten, die Haelfte, und ungleiche Gewichte.
    HAELFTE=$(seq -s, 0 $(( PHYSISCH / 2 - 1 )) 2>/dev/null | sed 's/,$//')
    GEWICHTE=$(i=1; g=""; while [ $i -lt "$PHYSISCH" ]; do g="${g}1,"; i=$((i+1)); done; echo "${g}3")
    for LAUF in alle haelfte gewichte; do
      case "$LAUF" in
        alle) UMGEBUNG="" ; ERWARTET=$PHYSISCH ;;
        haelfte) [ "$PHYSISCH" -lt 2 ] && continue; UMGEBUNG="MYL_GPU_GERAETE=$HAELFTE"; ERWARTET=$(( PHYSISCH / 2 )) ;;
        gewichte) [ "$PHYSISCH" -lt 2 ] && continue; UMGEBUNG="MYL_GPU_GEWICHTE=$GEWICHTE"; ERWARTET=$PHYSISCH ;;
      esac
      echo "$MODELL: $LAUF ${UMGEBUNG} …"
      env MYL_GPU_AB=1 $UMGEBUNG "$PROBE" "$ART" "$PROMPT" 8 > "$BERICHT.lauf" 2>&1
      IST=$(sed -n 's/^decode_digest //p' "$BERICHT.lauf")
      BUENDEL=$(sed -n 's/^karten_buendel //p' "$BERICHT.lauf")
      # Eine Zeile je Karte: karte <weg>:<nr> <pci> teile <n> zeilen <n> ausgefallen <ja/nein>
      MIT_ARBEIT=$(grep -E "^karte " "$BERICHT.lauf" | awk '$5 > 0 && $9 == "false"' | wc -l | tr -d ' ')
      if [ -n "$IST" ] && [ "$IST" = "$SOLL" ]; then GLEICH="ja"; else GLEICH="NEIN"; FEHLER=1; fi
      echo "| $LAUF ${UMGEBUNG} | $MIT_ARBEIT von $ERWARTET | ${BUENDEL:-?} | ${IST%"${IST#????????????????}"} | $GLEICH | $(sed -n 's/^prefill_ms //p' "$BERICHT.lauf") |" >> "$BERICHT"
      # Mit ungleichen Gewichten muss die letzte Karte (Gewicht 3) die
      # meisten Zeilen gerechnet haben.
      if [ "$LAUF" = gewichte ] && ! grep -E "^karte " "$BERICHT.lauf" | awk '{z[NR]=$7} END {for (i=1;i<NR;i++) if (z[i]>z[NR]) exit 1}'; then
        echo "⛔️ gewichte: die Karte mit Gewicht 3 hat nicht am meisten gerechnet"
        FEHLER=1
      fi
      if [ "$MIT_ARBEIT" != "$ERWARTET" ] || [ "${BUENDEL:-0}" -eq 0 ]; then
        echo "⛔️ $LAUF: $MIT_ARBEIT von $ERWARTET Karten haben gerechnet, $BUENDEL Buendel"
        FEHLER=1
      else
        echo "$LAUF: Abdruck $GLEICH, $MIT_ARBEIT Karten, $BUENDEL Buendel"
      fi
      # Je Karte, damit die Verteilung im Bericht nachzulesen ist.
      { echo; echo "<details><summary>$LAUF: je Karte</summary>"; echo; echo '```'; grep -E "^karte |Grafikkarte" "$BERICHT.lauf"; echo '```'; echo "</details>"; echo; } >> "$BERICHT.karten"
    done
    [ -f "$BERICHT.karten" ] && cat "$BERICHT.karten" >> "$BERICHT"
    rm -f "$BERICHT.cpu" "$BERICHT.lauf" "$BERICHT.karten"
  done
  echo
  echo "Bericht: $BERICHT"
done
exit $FEHLER
