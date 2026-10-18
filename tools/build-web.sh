#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -d "$(rustc --print target-libdir --target wasm32-unknown-unknown)" ]; then
    cargo build --target wasm32-unknown-unknown --profile web-perf "$@"
else
    # System Rust ships sources instead of a precompiled WASM standard library.
    RUSTC_BOOTSTRAP=1 cargo build -Z build-std=std,panic_abort --target wasm32-unknown-unknown --profile web-perf "$@"
fi
python3 tools/build_window_scene.py
mkdir -p dist
wasm-bindgen --target web --no-typescript --out-dir dist target/wasm32-unknown-unknown/web-perf/CasinoGame.wasm
cp web/index.html dist/index.html
mkdir -p dist/assets/models
cp assets/models/apartment_windows.glb dist/assets/models/
cp assets/models/menu_sleeper.glb dist/assets/models/
cp -R assets/ui assets/fonts dist/assets/
if [ -d art/review ]; then
    cp -R art/review dist/
fi
test -s dist/CasinoGame_bg.wasm
test -s dist/assets/models/apartment_windows.glb
test -s dist/assets/models/menu_sleeper.glb
