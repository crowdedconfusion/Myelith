#!/usr/bin/env bash
# Konformitäts-Prüflauf: validiert alle Golden Vectors gegen ein Backend.
#
# Usage:
#   ./run.sh [backend]
#
# backend = "reference" (default) → nutzt die Rust-Referenzimplementierung
#           (goldener Standard, keine Artefakte fuer Op-Level noetig;
#            Artefakte in ../artifacts/ fuer Layer/E2E)
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

# ── Op-Level: golden_runner (kernels-Crate) ────────────────────────
echo "--- Op-Level ---"
if [ -d "${VECTORS_DIR}/op" ]; then
    for f in "${VECTORS_DIR}/op"/*.golden.json; do
        [ -f "$f" ] || continue
        TOTAL=$((TOTAL + 1))
        NAME=$(basename "$f" .golden.json)
        if cargo run --manifest-path "${KERNELS_DIR}/Cargo.toml" \
                --bin golden_runner --features reference --quiet -- \
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
        --bin golden_model --quiet -- \
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
