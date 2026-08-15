#!/bin/bash
# Build Oracle 8-bit — single WASM binary, all apps via ?app= param
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

export PATH="$HOME/.cargo/bin:$PATH"

OUT="dist"
mkdir -p "$OUT"

echo "=== Building oracle-8bit ==="
cargo build --release --target wasm32-unknown-unknown --bin oracle-8bit

wasm-bindgen target/wasm32-unknown-unknown/release/oracle-8bit.wasm \
  --out-dir "$OUT" \
  --target web \
  --no-typescript

WASM="$OUT/oracle-8bit_bg.wasm"
if command -v wasm-opt &> /dev/null; then
  echo "  Optimizing WASM..."
  wasm-opt -Os --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int --enable-sign-ext \
    "$WASM" -o "$WASM" 2>/dev/null || echo "  wasm-opt skipped"
fi

# Copy SDK
cp web/hub.html "$OUT/index.html"
cp web/app.html "$OUT/app.html"
cp web/bridge.js "$OUT/"
cp web/canvas-bridge.js "$OUT/"

echo ""
echo "=== Build complete ==="
ls -lh "$OUT/"
echo ""
echo "Usage: app.html?app=office-8bit|war-room|race-track|superman"
