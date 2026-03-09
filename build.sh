#!/bin/bash
# Build office-8bit WASM apps and prepare dist
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

export PATH="$HOME/.cargo/bin:$PATH"

# Determine what to build
APP="${1:-all}"

build_app() {
  local name="$1"
  local out="$2"

  echo "=== Building $name ==="
  cargo build --release --target wasm32-unknown-unknown --bin "$name"

  mkdir -p "$out"
  wasm-bindgen "target/wasm32-unknown-unknown/release/${name}.wasm" \
    --out-dir "$out" \
    --target web \
    --no-typescript

  # Optimize
  local wasm_file="${out}/${name}_bg.wasm"
  if command -v wasm-opt &> /dev/null; then
    echo "  Optimizing $name WASM..."
    wasm-opt -Os --enable-bulk-memory --enable-mutable-globals --enable-nontrapping-float-to-int --enable-sign-ext \
      "$wasm_file" -o "$wasm_file" 2>/dev/null || echo "  wasm-opt skipped"
  fi

  local size=$(du -sh "$wasm_file" | cut -f1)
  echo "  $name: $size"
}

# --- Office 8-bit ---
if [ "$APP" = "all" ] || [ "$APP" = "office" ]; then
  build_app "office-8bit" "dist"
  cp web/index.html dist/
  cp web/bridge.js dist/
  cp web/hub.html dist/
  cp web/canvas-bridge.js dist/
  cp -r assets dist/
fi

# --- War Room ---
if [ "$APP" = "all" ] || [ "$APP" = "war-room" ]; then
  build_app "war-room" "dist-war-room"
  cp web/war-room.html dist-war-room/index.html
  cp web/bridge.js dist-war-room/
  cp web/canvas-bridge.js dist-war-room/
  cp -r assets dist-war-room/
fi

# --- Race Track ---
if [ "$APP" = "all" ] || [ "$APP" = "race-track" ]; then
  build_app "race-track" "dist-race-track"
  cp web/race-track.html dist-race-track/index.html
  cp web/bridge.js dist-race-track/
  cp web/canvas-bridge.js dist-race-track/
  cp -r assets dist-race-track/
fi

echo ""
echo "=== Build complete ==="
echo "Office:     dist/"
echo "War Room:   dist-war-room/"
echo "Race Track: dist-race-track/"
ls -la dist/*.wasm dist-war-room/*.wasm dist-race-track/*.wasm 2>/dev/null
