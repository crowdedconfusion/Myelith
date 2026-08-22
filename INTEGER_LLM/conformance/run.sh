#!/usr/bin/env bash
# Konformitäts-Prüflauf: validiert alle Golden Vectors gegen ein Backend.
#
# Usage:
#   ./run.sh [backend]
#
# backend = Cargo-Feature des zu zertifizierenden Backends:
#           "reference" (default, goldener Standard), "cpu-simd",
#           "cuda", "rocm".
#
# Nicht jedes davon laeuft ueberall. Der Pruefstand lehnt ein Backend ab
# (Exit 2), das auf DIESER Uebersetzung keinen eigenen Rechenpfad hat:
#   - "cuda"/"rocm" ueberall, solange backends/ nur delegiert (Fund 33)
#   - "cpu-simd" auf jedem Ziel ausser aarch64, weil kernels/src/dot.rs
#     bisher nur eine NEON-Fassung hat (Fund 34)
# Massgeblich ist kernels/src/rechenpfad.rs, nicht diese Liste.
#
# Bis 2026-08-19 wurde der Parameter nur ausgegeben und dann ignoriert:
# beide cargo-Aufrufe standen fest auf `--features reference`. Damit
# konnte der Prüflauf ausschliesslich sich selbst zertifizieren — genau
# das, wofuer er nicht da ist. Ein fremdes Backend gilt als konform, wenn
# es alle Vektoren bitgleich reproduziert; dafuer muss es auch laufen.
#
# Exit 0 = alle Vektoren bestanden, Exit 1 = mindestens einer fehlgeschlagen.

set -euo pipefail

BACKEND="${1:-reference}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VECTORS_DIR="${SCRIPT_DIR}/vectors"
PROJECT_ROOT="${SCRIPT_DIR}/.."
KERNELS_DIR="${PROJECT_ROOT}/kernels"
RUNTIME_DIR="${PROJECT_ROOT}/runtime"
ARTIFACT_DIR="${PROJECT_ROOT}/artifacts/qwen2.5-0.5b"

TOTAL=0
PASSED=0
FAILED=0

echo "=== Konformitäts-Prüflauf ==="
echo "Backend: ${BACKEND}"
echo "Vektoren: ${VECTORS_DIR}"
echo ""

# ── Hat das Backend überhaupt einen Rechenpfad? ────────────────────
#
# FUND (2026-08-22): `run.sh cuda` meldete auf einem Mac ohne NVIDIA-
# Hardware 30/30 bestanden. Die Features in kernels/Cargo.toml sind alle
# leer und schalten nur, ob backends/cuda.rs uebersetzt wird; der
# Rechenpfad kennt keine cuda-Weiche, und golden_runner verwarf den
# Backend-Namen. Der Prueflauf zertifizierte also die Referenz unter
# fremdem Namen.
#
# Das ist derselbe Fehler, den der Kopf dieser Datei fuer behoben
# erklaert. Die damalige Behebung reichte den Parameter in den
# cargo-Aufruf; da er die Rechnung nie erreichte, blieb die
# Selbstzertifizierung und trug nur ein besseres Etikett.
#
# Deshalb EINE Probe vorweg, mit sichtbarer Fehlerausgabe. Im Prueflauf
# darunter geht stderr nach /dev/null, damit die Vektorliste lesbar
# bleibt; die Ablehnung waere dort unsichtbar und der Lauf endete mit
# 0/30 ohne Begruendung.
PROBE="${VECTORS_DIR}/op"
PROBE=$(ls "${PROBE}"/*.golden.json 2>/dev/null | head -1)
if [ -n "${PROBE}" ]; then
    if ! cargo run --manifest-path "${KERNELS_DIR}/Cargo.toml" \
            --bin golden_runner --no-default-features --features "${BACKEND}" --quiet -- \
            "${PROBE}" "${BACKEND}" >/dev/null; then
        echo ""
        echo "=== Prueflauf abgelehnt ==="
        exit 2
    fi
fi

# ── Op-Level: golden_runner (kernels-Crate) ────────────────────────
echo "--- Op-Level ---"
if [ -d "${VECTORS_DIR}/op" ]; then
    for f in "${VECTORS_DIR}/op"/*.golden.json; do
        [ -f "$f" ] || continue
        TOTAL=$((TOTAL + 1))
        NAME=$(basename "$f" .golden.json)
        if cargo run --manifest-path "${KERNELS_DIR}/Cargo.toml" \
                --bin golden_runner --no-default-features --features "${BACKEND}" --quiet -- \
                "$f" "$BACKEND" 2>/dev/null | grep -q "^PASS:"; then
            PASSED=$((PASSED + 1))
            echo "  PASS: ${NAME}"
        else
            FAILED=$((FAILED + 1))
            echo "  FAIL: ${NAME}"
        fi
    done
fi

# ── Layer + E2E: golden_model Batch-Modus (runtime-Crate) ──────────
echo ""
echo "--- Layer + E2E ---"
if [ -d "${ARTIFACT_DIR}" ]; then
    TOTAL_BEFORE=$TOTAL
    PASSED_BEFORE=$PASSED

    OUTPUT=$(cargo run --manifest-path "${RUNTIME_DIR}/Cargo.toml" \
        --bin golden_model --no-default-features --features "${BACKEND}" --quiet -- \
        "$ARTIFACT_DIR" --batch "$VECTORS_DIR" 2>/dev/null) || true

    while IFS= read -r line; do
        case "$line" in
            *"PASS:"*)
                TOTAL=$((TOTAL + 1))
                PASSED=$((PASSED + 1))
                echo "  ${line}"
                ;;
            *"FAIL:"*)
                TOTAL=$((TOTAL + 1))
                FAILED=$((FAILED + 1))
                echo "  ${line}"
                ;;
        esac
    done <<< "$OUTPUT"
else
    echo "  SKIP: Artefakte nicht vorhanden (${ARTIFACT_DIR})"
    echo "  Layer/E2E-Vektoren benötigen kalibrierte Modell-Artefakte."
fi

# ── Zusammenfassung ────────────────────────────────────────────────
echo ""
echo "=== Ergebnis: ${PASSED}/${TOTAL} bestanden, ${FAILED} fehlgeschlagen ==="

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
exit 0
