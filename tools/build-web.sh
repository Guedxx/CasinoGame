#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -d "$(rustc --print target-libdir --target wasm32-unknown-unknown)" ]; then
    cargo build --target wasm32-unknown-unknown --profile web "$@"
else
    # System Rust ships sources instead of a precompiled WASM standard library.
    RUSTC_BOOTSTRAP=1 cargo build -Z build-std=std,panic_abort --target wasm32-unknown-unknown --profile web "$@"
fi
mkdir -p dist
wasm-bindgen --target web --no-typescript --out-dir dist target/wasm32-unknown-unknown/web/CasinoGame.wasm
cp web/index.html dist/index.html
mkdir -p dist/assets/models
cp assets/models/apartment_city.glb dist/assets/models/
if [ -d art/review ]; then
    cp -R art/review dist/
fi
test -s dist/CasinoGame_bg.wasm
test -s dist/assets/models/apartment_city.glb
