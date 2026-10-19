# Let It Ride

A Rust / Bevy 0.19.1 project with a furnished first-person apartment and a dense urban exterior.

## Run

```sh
cargo run
```

The game opens on the apartment couch with a sleeping man, a subtle chest-breathing animation,
and **Continue / New Game / Settings** on the right. New Game starts Alex's ten-day campaign;
replacing existing progress requires confirmation. Continue restores the retained campaign,
including a daily report or ending. Wait for loading, then **click** to capture the mouse.
Use **WASD** to walk, **Shift** to sprint, **Space** to jump, and the **mouse** to look.
**P** pauses with the mouse captured. The in-game cursor operates the phone, household panels,
reports, pause menu and settings; closing them resumes control immediately.
**Escape** closes an overlay first on native. In the browser, Escape releases pointer lock and
pauses with the current phone page preserved; after Resume, click once to capture again.
Use **Tab / Shift+Tab** to navigate the menu and **Enter / Space** to select.
Losing window focus or browser pointer lock also opens the menu. The closed door, windows,
furniture, walls, floor, and ceiling are solid.

Use **E** near an object for household actions and **Tab** to raise or lower the phone.
The phone contains fictional betting games, banking, financial/betting history and saved daily
reports, exact status, contacts, support, and food options. **1–9** select visible phone choices,
**H** returns home, **Backspace** goes back, and the wheel or **PageUp / PageDown** scrolls.
Walking and looking stop while an overlay is open. Use **Tab** or its Close button to lower the
phone without releasing capture. Actual window focus or browser pointer-lock loss still pauses safely.

Each day has six shared action points (PA). Work, meals and other activities spend PA;
exploration, reading, bill payments and support information are free. Sleep at the bed opens
the daily report; unused PA still elapse for needs and scheduled commitments. Only the current
report's explicit Continue advances the campaign. Betting is voluntary. Endings stop gameplay
and keep history/reports readable, with Return to Title available.

Meaningful actions autosave one local campaign separately from graphics settings, using
`casino-game.campaign.v1` in browser localStorage or the native user configuration directory.
A storage failure keeps session progress and shows a retry notice; retry saving before closing.
Unreadable saves are preserved until an explicitly confirmed New Game replaces them.
The approved gameplay contract and focused verification record are in [IMPLEMENTATION.md](IMPLEMENTATION.md).
The first build compiles the engine and can take several minutes.
Install the native dependencies for your platform from the
[Bevy setup guide](https://bevy.org/learn/quick-start/getting-started/setup/).

## Browser

With Python 3, `wasm-bindgen-cli` 0.2.129 on your PATH, and the Rust
`wasm32-unknown-unknown` target installed:

```sh
tools/build-web.sh
python3 -m http.server 8080 --bind 127.0.0.1 --directory dist
```

Open <http://localhost:8080>. The build uses WebGPU and copies the apartment assets
into `dist/`. A WebGPU-enabled browser and HTTPS (or localhost) are required.
At High quality, both versions use one sunlight shadow cascade and six shadow-casting spotlights.
On system Rust installations with `rust-src` but no precompiled
WASM target, the script builds the standard library from those sources instead.
The first build takes several minutes; subsequent builds reuse compiled crates.
The web script uses the release-derived `web-perf` profile: optimization level 3,
thin LTO, and one codegen unit. The original `web` profile remains available for
quicker development builds. Release linking takes longer but produces a smaller WASM file.

## Graphics settings

Choose **Settings** on the title screen, or **P → Settings** during play.
Settings returns to the screen it was opened from. The pause and settings panels are centered over the scene.
High is the default and preserves the original appearance.
Presets can be adjusted individually; unmatched combinations display **Custom**.

| Setting | Low | Medium | High |
| --- | --- | --- | --- |
| Lighting | Shadows off | 1024px shadows, hardware filtering | Original 2048px shadows, Gaussian filtering |
| Anti-aliasing | Off | FXAA | 4× MSAA |

All lights, fog, scene geometry and original texture dimensions stay present at every setting.
Rendering always uses the full window resolution; there is no scaling camera or texture-resizing queue.
Anti-aliasing can also be selected individually: **Off, FXAA, SMAA (High), or 4× MSAA**.
FXAA is a fast post-process; SMAA preserves edges more accurately at additional processing cost.
The web supports only 1×/4× MSAA, so 2×/8× are not offered. FXAA/SMAA turn MSAA off and are mutually exclusive.
Implementation follows [Bevy 0.19.1 FXAA](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_anti_alias/src/fxaa/mod.rs) and
[SMAA](https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_anti_alias/src/smaa/mod.rs).
Changes preview immediately;
**Save settings** commits them, while **Cancel** restores the last saved settings. Escape also
cancels on native or with the browser mouse already unlocked; the first browser Escape while
captured releases the mouse and keeps settings open.
Version-one saved preferences migrate lighting and anti-aliasing only; removed texture and resolution values are discarded.
New saves use schema version two in the existing storage location.

Settings are stored in browser localStorage (`casino-game.graphics.v1`), or in
`CasinoGame/casino-game.graphics.v1.json` under the native user configuration directory
(`$XDG_CONFIG_HOME` / `~/.config` on Linux, `%APPDATA%` on Windows, and
`~/Library/Application Support` on macOS). Invalid data falls back to High. Storage failures
show a notice and keep settings usable for the current session. There is no account sync.

The centered panels and buttons share one generated, nine-sliced texture:
`assets/ui/pause-panel.png` (512×512, 97,946 bytes on disk, 1 MiB decoded RGBA on the GPU).
This reduces texture GPU storage by 83% versus the generated 1254×1254 source. Text stays
native-resolution using the included, unmodified Liberation Sans Bold font and its OFL license.
Pause and settings share three text sizes; the title heading scales with window width.
These texture figures exclude the CPU image copy and font atlases. The generation prompt,
style notes and compression details are in `art/ui/pause-panel-generation.json`.

The settings page's FPS counter measures game rendering cadence. Browser performance reports use
WebGPU canvas acquisitions per second; neither is a measurement of displayed frames.

## Layout

```text
src/
  main.rs             Entry point
  app.rs              Window, engine configuration, plugin registration
  state.rs            Application states
  graphics.rs         Live graphics controls and device settings
  game/
    mod.rs            Shared gameplay plugins
    camera.rs         Persistent 3D camera
    player.rs         First-person controls, static collision, and physics checks
    campaign.rs       Authoritative ten-day state, actions, clock, ledger, and reports
    campaign_save.rs  Validated versioned autosave and retained session
    routine.rs        World interactions, reports, endings, and read-only records
    phone.rs          Physical phone, apps, amount entry, and settled-result presentation
    betting.rs        Exact fictional wagering, saved outcomes, and shared sessions
    obligations.rs    Bills, rent extension, contacts, and commitments
    support.rs        Gradual care, planning, voluntary caps, and app block
    household.rs      Permanent sales, Pipoca, food/waste, and state-derived atmosphere
  screens/
    mod.rs            Screen plugins
    menu.rs           Title menu, pause menu, settings, and keyboard input
    playing.rs        Apartment scene, sunlight, and input
assets/
  audio/
  fonts/
  textures/
  shaders/
  models/             Window-visible apartment_windows.glb; full city and original park retained
```

Add gameplay features as plugins under `src/game/`, keeping their components,
resources, and systems together. Register them in `GamePlugin`. Run gameplay
systems with `in_state(GameState::Playing)` and attach `DespawnOnExit` to entities
that should be removed when a screen closes.

Asset paths are relative to `assets/`, for example
`asset_server.load("textures/cards.png")`.

The title uses the existing apartment and its single camera. Its character is baked from
`base/Characters_psx/Models/Male/Character_01.fbx` with the matching source texture.
`assets/models/menu_sleeper.glb` contains a seated, head-back pose and one `Breath` morph;
the chest rises and expands over a four-second breathing cycle, including in title settings.
Entering play removes the sleeper; Return to Title restores him and the original couch framing.
The existing pause menu preserves the player's view and position.
Rebuild the character with `blender --background --factory-startup --python tools/build_menu_character.py`.
The exporter keeps the enlarged character seated on the cushion, adjusts his knee bend
to rest the flat soles on the carpet with 1 mm clearance, and checks textures, seated height,
wall clearance, and the exported morph.
If a system Blender picks up an incompatible Python installation, launch it with
`--python-use-system-env` and set `PYTHONPATH` to that Blender installation's matching
standard-library, `lib-dynload`, and `site-packages` directories.
Check the title and pause flow against a running browser build with Playwright on `NODE_PATH`:
`node tools/test-menu.cjs`. Screenshots and results are saved in `art/review/title-menu/`.

## Apartment and city assets

The original downloads stay untouched in `base/`. The occupied Home interior is
retained; the other trailers, terrain, trees, background, porch, chassis, and trailer
roof are removed from the runtime scene. Its curtains are gathered to the sides,
and the solid window screens are clearer so the city is visible.

The entrance side is now a continuous opaque party wall, with a solid wood door
opening onto a 2.2 metre shared corridor. Neighbouring flats and a stair core fill
the rest of the host building; continuous floors connect it to street level.
Four original windows remain on the two exterior elevations.
The corridor retains its source geometry and materials for later door gameplay, but
its interior, stairs, and fixtures are absent from the runtime scene. The three
corridor lights remain to preserve the warm seam around the closed door.
The fridge is opaque, and plants use alpha cutouts so pots and leaves write depth
while the empty space around leaf silhouettes stays transparent.

Every new exterior model comes from `base/URBAN/`. Closely spaced apartment blocks,
balconies, rusted pipes, vents, roof machinery, graffiti, bins, litter, street lamps,
a bus shelter, fences, and roadworks surround the room. The street is 6 metres below
its floor. The player remains locked inside; the exterior is visual scenery and does
not create physics colliders. Interior walls, windows, doors and furniture still do.
Cool gray distance fog thickens over the city under dim, overcast lighting, while
nearby apartment details remain clear.

Bevy loads `assets/models/apartment_windows.glb`. The full `apartment_city.glb` remains
the authoring export. The editable, texture-packed Blender
scene is `art/apartment_city.blend`; `art/apartment_city_manifest.json` lists the
URBAN source models. Rebuild it with:

```sh
# Only needed if the original conversion is missing or the downloads changed:
blender --background --factory-startup --python tools/convert_trailer_park.py
# Build the apartment/city from the original park conversion and URBAN models:
blender --background --factory-startup --python tools/build_apartment_city.py
tools/build-web.sh
```

The Blender builder verifies one retained Home, four remaining solid windows,
sealed entrance-side wall sections, measured clearances between windows and mounted
fixtures, balcony door/floor alignment, source provenance for the exterior,
available textures, and no exported cameras/lights.
It is deterministic and leaves downloaded files untouched.

`tools/build_window_scene.py` derives the runtime asset from that full GLB. It keeps
all apartment geometry and a conservative set of exterior objects potentially
visible from **any of the four windows, anywhere the player can stand or jump**.
Opaque rectangular walls can exclude objects entirely behind them. Off-screen
sun-shadow casters are retained when needed to preserve the window views. Retained
mesh and texture bytes are copied exactly; unused resources are removed from the
GLB, so excluded objects are never loaded, instantiated, or given colliders.

This is a precomputed visibility set for the current closed apartment. Bevy still
frustum-culls it as the camera turns; this does not stream assets on each turn.
The corridor and stairs stay unloaded until door gameplay is implemented. Rebuild
and recheck the visibility set if the windows, player movement bounds, sunlight,
or closed-door restriction change. The full city is available for that expansion.
The builder and web build regenerate the reduced asset automatically; regenerate
it alone with `python3 tools/build_window_scene.py`.

The current cut reduces scene nodes from 930 to 731 (21.4%). The GLB shrinks only
2.1% (6,966,244 to 6,820,564 bytes): the retained objects still use all 78 source
images. This reduces scene objects, not texture memory.

## Temporary visual review

The current browser preview is built with `tools/build-web.sh --features scene-review`.
**F1** shows the closed entrance wall, **F2** the now-unloaded corridor location, **F3–F4** the
living-room and bedroom windows, and **F5** the host building down to street level.
**F7** shows the balcony across the kitchen window, **F8** the service court,
**F9** the apartment interior, and **F10** the stair landing.
**F11** shows the retained apartment windows from the courtyard.
**1–6** check the bathroom window (including jump height), kitchen edges, and bedroom
window edges. **F6** returns to the player camera. Review cameras work without mouse capture.
Exterior/corridor review cameras leave the playable visibility volume, so missing
scenery there is expected in the window-only runtime export. Normal builds do not
include these review controls. Remove `src/game/review.rs`,
the `scene-review` feature/hooks, and `PlayerCamera` scheduling marker after the
user approves the visuals; retain the screenshots and Blender source.

Browser screenshots are saved in `art/review/` and copied to `dist/review/`.
Open <http://localhost:8080/review/> to compare them. These are Chromium captures
of the running Bevy game, with no Blender rendering or post-processing. Each review
round is retained in its own directory alongside the independent Astra High report.
The completed review sequence scored 7.2, 7.5, and 8.3/10; Round 3 is the current
gallery, with ten browser captures and no required visual fixes identified.
The corridor texture preference and opaque fridge/plant fixes are included.
Capture a new round with Playwright available on `NODE_PATH`:

```sh
node tools/capture-city-review.cjs art/review/round-NN
```

## Browser performance

The camera uses a depth prepass to avoid shading hidden pixels in the main pass.
The browser benchmark runs the actual WebGPU game at the existing review cameras,
records canvas frame intervals, GPU/browser details and physical render resolution,
and saves browser screenshots plus full render-target PNGs in `canvas/` for visual
comparison. Captures happen after each timing sample. It also checks pointer lock and the
walk/jump/turn/pause/resume flow. Use a `scene-review` build and Playwright on `NODE_PATH`:

```sh
VIEWS=apartment,court-from-window ROUNDS=2 SAMPLE_MS=15000 node tools/benchmark-web.cjs art/review/performance/current
```

An optional second argument selects the game URL. `VIEWS=apartment,court-from-window` selects
a subset of cameras. `GRAPHICS_PRESET=low`, `medium`, or `high` initializes that preset
in the benchmark's isolated browser context; omit it to test the default. `VIEWPORT_WIDTH` and `VIEWPORT_HEIGHT` override the browser
viewport for display-resolution tests. Keep the same physical canvas size and browser/GPU for both
builds, warm up before sampling, and avoid compiling or running other benchmarks
at the same time. FPS is measured from the game's WebGPU canvas frame acquisitions,
not a separate browser animation loop or guaranteed displayed FPS. Raw timestamps,
focus/visibility changes, and duplicate acquisitions are retained. `TRACE=1` records
a separate diagnostic Chrome trace; do not mix traced samples into throughput comparisons.
Run `node tools/benchmark-web.cjs --self-test` to check the metric without a browser.
The harness uses the display's native pixel ratio so mouse hit targets match the canvas.
Desktop scaling can make the physical render target larger than the requested 1280×720 browser viewport.

The [measured comparison](art/review/performance/index.html) retains all ten
before/after views and raw samples. On Intel Iris Xe / Chromium at 1600×900,
the depth prepass improved measured FPS by 12.6–57.4%, with identical screenshots.
These results apply to that browser/GPU; other devices need their own measurements.

The [window-loading comparison](art/review/performance/window-loading/index.html)
records the reduced scene's before/after browser measurements and window-edge
screenshots. Removing corridor lights produced a larger speedup but changed the
closed door's warm seam, so those light sources remain. Geometry removal alone
showed little FPS benefit. Texture memory is unchanged, and per-pixel lighting
remains the main performance cost. The complete corridor meshes are still absent from the loaded runtime GLB.

The [lighting optimization report](art/review/performance/lighting-optimization/index.html)
compares the release-derived browser build with the previous window-only/depth-prepass
build at two physical resolutions. The camera keeps 4096 light clusters and 24 depth
slices, with a one-metre first slice and the existing 2000-metre camera distance.
The browser uses Bevy's CPU light assignment for this six-spotlight scene.
The spotlight shader skips direct shadow/BRDF work only when the existing range/cone
attenuation is exactly zero, and skips a direct BRDF with exactly zero shadow visibility.
Positive lighting and diffuse transmission keep their original calculations.
Fog, light sources, shadows, MSAA, textures, geometry and render resolution are unchanged.

Bevy is pinned to `=0.19.1` because the shader guard patches that version's embedded
WGSL. An unrecognized shader keeps the original source and logs a warning. Recheck
the patch and browser screenshots when upgrading Bevy. Remeasure CPU clustering if
the scene grows substantially beyond its current six spotlights.

The [original settings report](art/review/performance/settings/index.html) is historical: its preset FPS measurements include the removed texture and resolution reductions.
Current presets retain full resolution and original textures, so those figures do not describe current preset performance.
The [anti-aliasing checks](art/review/performance/antialiasing/interaction/results.json) cover actual GPU passes, fixed scene/UI size, migration, saving and cancellation. Reproduce
the settings checks against a running `scene-review` build with Playwright on `NODE_PATH`:

```sh
node tools/test-settings.cjs art/review/performance/antialiasing/interaction http://127.0.0.1:8080/
```

## Checks

```sh
cargo fmt --check
cargo check
cargo test
python3 -m unittest discover -s tools -p 'test_*.py'
```
