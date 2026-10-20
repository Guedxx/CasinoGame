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
python3 tools/build_household.py
mkdir -p dist
wasm-bindgen --target web --no-typescript --out-dir dist target/wasm32-unknown-unknown/web-perf/CasinoGame.wasm
python3 - <<'PY'
import hashlib
from pathlib import Path

digest = hashlib.sha256()
for name in ('CasinoGame.js', 'CasinoGame_bg.wasm'):
    digest.update(Path('dist', name).read_bytes())
page = Path('web/index.html').read_text().replace('__GAME_BUILD__', digest.hexdigest()[:16])
Path('dist/index.html').write_text(page)
PY
mkdir -p dist/assets/models
cp assets/models/apartment_windows.glb dist/assets/models/
cp assets/models/menu_sleeper.glb dist/assets/models/
cp assets/models/phone_hand.glb dist/assets/models/
cp assets/models/{pipoca,food_portion,waste_paper,waste_meal,waste_bag}.glb dist/assets/models/
cp -R assets/audio dist/assets/
cp -R assets/ui assets/fonts dist/assets/
if [ -d art/review ]; then
    cp -R art/review dist/
fi
test -s dist/CasinoGame_bg.wasm
test -s dist/assets/models/apartment_windows.glb
test -s dist/assets/models/menu_sleeper.glb
test -s dist/assets/models/phone_hand.glb
for model in pipoca food_portion waste_paper waste_meal waste_bag; do
    test -s "dist/assets/models/$model.glb"
done
test -s dist/assets/audio/pressure.wav
test -s dist/assets/ui/hud-medallions.png
test -s dist/assets/ui/phone-icons.png
test -s dist/assets/ui/phone-wallpaper.png
test -s dist/assets/ui/phone-betting.png
test -s dist/assets/ui/phone-apps.png
test -s dist/assets/fonts/LiberationSans-Bold.ttf
test -s dist/assets/fonts/LiberationSans-Regular.ttf
