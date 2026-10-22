#!/usr/bin/env bash
# Baut aus einem int8-Gemisch das Artefakt mit Experten in vier Bit,
# trainingsfrei: Mitschnitt der Expertenaufrufe, GPTQ mit ganzem Gitter
# und Schrumpfen je Experte, Packen auf 4,125 Bit je Gewicht.
#
# Aufruf aus beliebigem Verzeichnis:
#
#     bash INTEGER_LLM/scripts/build_vierbit.sh [quelle] [ziel]
#
# Vorgabe: quelle raven-35b-a3b-int8, ziel raven-35b-a3b-int4 (beide unter
# INTEGER_LLM/artifacts/). Braucht das int8-Artefakt der Quelle und die
# Kalibrier-Umgebung (scripts/python_einrichten.sh); die Rust-Werkzeuge
# baut es selbst, ohne Netz.
#
# Platz waehrend des Baus: der Mitschnitt und der Behaelter in int8
# (`zwischen_bytes` im Katalog) und das Ziel (`artefakt_bytes`); beides
# wird vorher gegen den freien Platz geprueft. Nach dem Bau bleiben nur
# Quelle und Ziel.
#
# ⛔️ Das Ergebnis wird gegen den Manifest-Hash im Register der
# Skalenpakete geprueft (`weights_manifest_sha256`). Nur dann ist es das
# gemessene Artefakt. Weicht es ab (andere Maschine, andere Bibliothek),
# bleibt es als <ziel>-abweichend liegen, und das Skript endet mit FEHLER:
# Ein anderes Artefakt ist ein anderes Modell, dessen Guete niemand
# gemessen hat.
#
# Dauer auf einem M5 Pro: Mitschnitt rund 15 min, Gitterbau 1 h 45 min,
# Packen unter einer Minute. Die letzte Zeile ist "Fertig" oder beginnt
# mit "FEHLER".
#
# ⚑ **Probelauf vor dem langen Lauf:** `VIERBIT_PROBE=1` baut nur die
# letzte Ebene in vier Bit, packt und prueft die Struktur, aber nicht den
# Hash, legt das Ergebnis als <ziel>-probe ab und laesst den Arbeitsordner
# samt Mitschnitt stehen; der lange Lauf nutzt den Mitschnitt weiter.
set -euo pipefail

INTEGER_LLM_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WURZEL="$(cd "${INTEGER_LLM_DIR}/.." && pwd)"
QUELLE_NAME="${1:-raven-35b-a3b-int8}"
ZIEL_NAME="${2:-raven-35b-a3b-int4}"
ARTEFAKTE="${INTEGER_LLM_DIR}/artifacts"
QUELLE="${ARTEFAKTE}/${QUELLE_NAME}"
ZIEL="${ARTEFAKTE}/${ZIEL_NAME}"
ARBEIT="${ARTEFAKTE}/.${ZIEL_NAME}.arbeit"
BEHAELTER="${ARBEIT}/behaelter"
PY="${INTEGER_LLM_DIR}/calibrate/.venv/bin/python3"
GV="${INTEGER_LLM_DIR}/calibrate/src/gittervergleich.py"
BIN="${WURZEL}/SYSTEM/full-build/release"
# ⚑ Vier Faeden je Teil und drei Teile, immer: Mit dieser Zahl ist der Bau
#   gemessen bitgleich. Eine andere Fadenzahl kann Summen in torch anders
#   ordnen.
FAEDEN=4

fehler() { echo "FEHLER: $*" >&2; exit 1; }

[ -f "${QUELLE}/model_config.json" ] || fehler "${QUELLE} fehlt; erst das int8-Artefakt bauen (build_artifacts.sh mit INTEGER_LLM_MODEL=${QUELLE_NAME})"
[ -e "${ZIEL}" ] && fehler "${ZIEL} gibt es schon; das Skript ueberschreibt nichts"
[ -x "${PY}" ] || fehler "die Kalibrier-Umgebung fehlt (bash INTEGER_LLM/scripts/python_einrichten.sh)"

# ── Angaben aus Katalog und Register, an einer Stelle gelesen ─────────
read -r ZWISCHEN BRAUCHT SOLL_MANIFEST SOLL_QUELLE < <("${PY}" - "${WURZEL}" "${QUELLE_NAME}" "${ZIEL_NAME}" <<'EOF'
import json, sys
from pathlib import Path
w, quelle, ziel = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
k = json.loads((w / "MODELS/llm/KATALOG.json").read_text())
r = json.loads((w / "INTEGER_LLM/scale_packs/REGISTER.json").read_text())
e = k.get(ziel, {})
print(e.get("zwischen_bytes", 0), e.get("artefakt_bytes", 0),
      r.get(ziel, {}).get("weights_manifest_sha256", "-"),
      r.get(quelle, {}).get("weights_manifest_sha256", "-"))
EOF
)

# ⛔️ Ohne Platz kein Bau: erst Speicher schaffen.
FREI=$(df -Pk "${ARTEFAKTE}" | awk 'NR == 2 { print $4 * 1024 }')
NOETIG=$((ZWISCHEN + BRAUCHT))
if [ "${FREI}" -lt "${NOETIG}" ]; then
    fehler "zu wenig Platz: gebraucht $((NOETIG / 1000000000)) GB, frei $((FREI / 1000000000)) GB unter ${ARTEFAKTE}. Erst Speicher schaffen."
fi

# ⚑ Die Quelle muss die gemessene sein, sonst baut sich ein anderes Ziel.
if [ "${SOLL_QUELLE}" != "-" ]; then
    IST_QUELLE=$("${PY}" -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "${QUELLE}/weights_manifest.json")
    [ "${IST_QUELLE}" = "${SOLL_QUELLE}" ] || fehler "${QUELLE_NAME} ist nicht das registrierte Artefakt (Manifest ${IST_QUELLE}, erwartet ${SOLL_QUELLE})"
fi

# ⚑ **Gebaut wird aus dem Vorrat des Repositoriums**, wie im
#   Installationsskript (`SYSTEM/install/installieren-*.sh`): Die Archive in
#   `SYSTEM/crates-vorrat/` werden bei Bedarf ausgepackt, und `CARGO_HOME`
#   zeigt auf das Lager. 📌 Vorher nahm dieses Skript das `~/.cargo` der
#   Maschine; auf der Entwicklungsmaschine lag dort alles, im kalten Klon
#   mit frischem Heim nichts, und `cargo --offline` fand keine Kiste.
if ls "${WURZEL}/SYSTEM/crates-vorrat/"*.crate >/dev/null 2>&1; then
    if [ ! -d "${WURZEL}/SYSTEM/crates-lager/vendor" ] \
        || [ "${WURZEL}/SYSTEM/crates-vorrat" -nt "${WURZEL}/SYSTEM/crates-lager/vendor" ]; then
        echo "[vierbit] Vorrat auspacken"
        python3 "${WURZEL}/SYSTEM/install/vorrat.py" auspacken >/dev/null || fehler "der Vorrat laesst sich nicht auspacken"
    fi
    export CARGO_HOME="${WURZEL}/SYSTEM/crates-lager/cargo-home"
fi
echo "[vierbit] Werkzeuge bauen (ohne Netz)"
(cd "${INTEGER_LLM_DIR}/runtime" && cargo build --release --locked --offline --features cpu-simd \
    --bin expertenmitschnitt --bin vierbit_packen --quiet)

mkdir -p "${ARBEIT}"
echo "[vierbit] Kalibriertext"
"${PY}" "${INTEGER_LLM_DIR}/calibrate/src/kalibriertext.py" "${ARBEIT}/kalibriertext.txt"

if [ ! -f "${ARBEIT}/mitschnitt.fertig" ]; then
    echo "[vierbit] Mitschnitt der Expertenaufrufe (16 384 Token)"
    rm -rf "${ARBEIT}/mitschnitt"
    "${BIN}/expertenmitschnitt" "${QUELLE}" "${ARBEIT}/kalibriertext.txt" "${ARBEIT}/mitschnitt" --token 16384
    touch "${ARBEIT}/mitschnitt.fertig"
else
    echo "[vierbit] Mitschnitt liegt schon da (abgebrochener Lauf), er wird weiter benutzt"
fi

# 📌 Das Artefakt nennt die Zahl `num_layers` (die Gewichte von Hugging
#   Face `num_hidden_layers`); hier stand bis zum ersten Lauf im kalten
#   Klon der zweite Name, und das Skript brach nach dem Mitschnitt ab.
EBENEN=$("${PY}" -c "import json,sys; d=json.load(open(sys.argv[1])); print(d.get('num_layers') or d['num_hidden_layers'])" "${QUELLE}/model_config.json")
# Drei Teile, die vorderen um eine Ebene groesser (bei 40: 0-13, 14-26,
# 27-39, wie beim gemessenen Bau). Jede Ebene wird fuer sich gebaut.
GRUND=$((EBENEN / 3))
REST=$((EBENEN % 3))
PROBE="${VIERBIT_PROBE:-}"
if [ -n "${PROBE}" ]; then
    echo "[vierbit] PROBELAUF: nur Ebene $((EBENEN - 1))"
fi
echo "[vierbit] Gitterbau: ${EBENEN} Ebenen in drei Teilen zu ${FAEDEN} Faeden"
rm -rf "${BEHAELTER}.unfertig"
PIDS=()
VON=0
for TEIL in 0 1 2; do
    GROESSE=${GRUND}
    [ "${TEIL}" -lt "${REST}" ] && GROESSE=$((GRUND + 1))
    BIS=$((VON + GROESSE - 1))
    if [ -n "${PROBE}" ]; then
        # Im Probelauf ein Teil mit der letzten Ebene, sonst nichts.
        [ "${TEIL}" -gt 0 ] && break
        VON=$((EBENEN - 1))
        BIS=${VON}
    fi
    if [ "${GROESSE}" -eq 0 ]; then
        continue
    fi
    OMP_NUM_THREADS=${FAEDEN} "${PY}" "${GV}" bauen "${QUELLE}" "${ARBEIT}/mitschnitt" "${BEHAELTER}.unfertig" \
        int4ganz --schrumpf 8192 --ebenen "${VON}-${BIS}" > "${ARBEIT}/bau_${VON}-${BIS}.log" 2>&1 &
    PIDS+=("$!")
    VON=$((BIS + 1))
done
SCHLECHT=0
for P in "${PIDS[@]}"; do
    wait "${P}" || SCHLECHT=1
done
for L in "${ARBEIT}"/bau_*.log; do
    tail -n 1 "${L}" | sed "s|^|[vierbit] $(basename "${L}"): |"
done
[ "${SCHLECHT}" -eq 0 ] || fehler "ein Teil des Gitterbaus ist gescheitert, siehe ${ARBEIT}/bau_*.log"
if grep -h "Werte geklemmt" "${ARBEIT}"/bau_*.log | grep -vq " 0 Werte geklemmt"; then
    fehler "der Gitterbau hat Werte geklemmt; mit dem ganzen Gitter darf das nicht sein"
fi

echo "[vierbit] Abschluss des Behaelters"
"${PY}" "${GV}" abschluss "${QUELLE}" "${BEHAELTER}.unfertig"
mv "${BEHAELTER}.unfertig" "${BEHAELTER}"

echo "[vierbit] Packen"
rm -rf "${ZIEL}.unfertig"
"${BIN}/vierbit_packen" "${BEHAELTER}" "${ZIEL}.unfertig"

if [ -n "${PROBE}" ]; then
    rm -rf "${ZIEL}-probe"
    mv "${ZIEL}.unfertig" "${ZIEL}-probe"
    rm -rf "${BEHAELTER}"
    GEPACKT=$(ls "${ZIEL}-probe" | grep -c "\.codes\.bin$" || true)
    [ "${GEPACKT}" -gt 0 ] || fehler "im Probelauf ist kein Tensor gepackt"
    echo "[vierbit] Probelauf: ${GEPACKT} Tensoren gepackt, Ergebnis ${ZIEL}-probe (nicht gegen das Register geprueft)"
    echo "Fertig"
    exit 0
fi
IST=$("${PY}" -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "${ZIEL}.unfertig/weights_manifest.json")
if [ "${SOLL_MANIFEST}" != "-" ] && [ "${IST}" != "${SOLL_MANIFEST}" ]; then
    rm -rf "${ZIEL}-abweichend"
    mv "${ZIEL}.unfertig" "${ZIEL}-abweichend"
    rm -rf "${ARBEIT}"
    fehler "das Ergebnis ist nicht das gemessene Artefakt (Manifest ${IST}, erwartet ${SOLL_MANIFEST}); es liegt als ${ZIEL_NAME}-abweichend da, seine Guete ist nicht gemessen"
fi
mv "${ZIEL}.unfertig" "${ZIEL}"
rm -rf "${ARBEIT}"
if [ "${SOLL_MANIFEST}" = "-" ]; then
    echo "[vierbit] ⚠️ kein Manifest-Hash im Register; die Guete muss gemessen werden (Manifest ${IST})"
else
    echo "[vierbit] bitgleich zum gemessenen Artefakt (Manifest ${IST})"
fi
echo "[vierbit] ${ZIEL}"
echo "Fertig"
