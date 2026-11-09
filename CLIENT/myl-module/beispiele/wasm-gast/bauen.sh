#!/bin/sh
# Baut den Pruefgast neu und legt ihn samt Abdruck seiner Quelle nach tests/.
# Braucht das Ziel wasm32-unknown-unknown (`rustup target add`), die Pruefung
# des Wirts braucht es nicht: Sie nimmt die abgelegte Datei.
set -eu
cd "$(dirname "$0")"
ZIEL=${CARGO_TARGET_DIR:-target}
CARGO_TARGET_DIR=$ZIEL cargo build --release --locked --target wasm32-unknown-unknown
cp "$ZIEL/wasm32-unknown-unknown/release/wasm_gast.wasm" ../../tests/gast.wasm
python3 -c "import hashlib; print(hashlib.sha256(open('src/lib.rs','rb').read()).hexdigest())" > ../../tests/gast.wasm.quelle
ls -l ../../tests/gast.wasm
