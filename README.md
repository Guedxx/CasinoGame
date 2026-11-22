# Saldo Zero

A Rust / Bevy 0.19.1 project with a furnished first-person apartment and a dense urban exterior.

**[Play in your browser](https://guedxx.github.io/CasinoGame/)** — requires WebGPU and hardware acceleration; progress saves in this browser only.

## Run

```sh
cargo run
```

The game opens on the apartment couch with a sleeping man, a subtle chest-breathing animation,
and **Continue / New Game / Settings** on the right. New Game starts Alex's four-day campaign.
Replacing existing progress requires confirmation. A 45-second first-person opening follows:
Alex wakes on the couch, blinks into focus, watches football on TV, reflects on his hands,
and stands up. Dialogue appears as subtitles. **Hold Enter for one second** to skip after
releasing the key used to start the game. Losing focus freezes the opening until you return.
Continue bypasses the opening and restores the retained campaign, including a daily report
or ending. Wait for loading and the opening, then **click** to capture the mouse.
Use **WASD** to walk, **Shift** to sprint, **Space** to jump, and the **mouse** to look.
**P** pauses with the mouse captured. The in-game cursor operates the phone, household panels,
reports, pause menu and settings; closing them resumes control immediately.
**Escape** closes an overlay first. From gameplay it opens the pause menu with the mouse
captured; press Escape again in that menu to release the mouse and stay paused.
Select **Resume** to continue playing or **Main Menu** to return to the title screen.
In supported browsers (Chromium), clicking to capture also enters
fullscreen and captures Escape through Keyboard Lock. Holding Escape still exits fullscreen.
Firefox and browsers that deny fullscreen/keyboard capture keep their normal Escape behavior:
Escape releases the mouse and pauses with the current phone page preserved. Use **P** to pause
with capture there; after releasing the mouse, Resume and click to capture again.
Use **Tab / Shift+Tab** to navigate the menu and **Enter / Space** to select.
Losing window focus or browser pointer lock also opens the menu. The closed door, windows,
furniture, walls, floor, and ceiling are solid.

Use **E** near an object for household actions and **Tab** to raise or lower the phone.
A shallow wall calendar left of the TV marks today's date and the bill deadlines.
Aim at a paper on the living-room table and click or press **E** to pay it. The paper
disappears after payment. Alex starts with R$60. Water costs R$100 and is due on day 2,
electricity costs R$100 and is due on day 3, and rent costs R$120 and is due on day 4.
Unpaid rent leads to housing loss after the final daily report.
The phone home screen shows History, Betting, Support and Chats in one row of
low-resolution, palette-limited icons. History shows daily income and spending by category.
Support includes care tools and household sales when relevant. Use the kitchen to get food.
**1–9** select visible phone choices, **H** returns home, **Backspace** goes back,
and the wheel or **PageUp / PageDown** scrolls.
Roulette and football use selection tiles followed by an explicit Bet button. Slots uses Spin.
Rules and My bets stay within the shared betting session. Contact rows open a thread before a call.
Marina calls on day 2 in the afternoon and Rafa calls on day 4 at night.
Sales opens a history of sold items with SOLD stamps; Sell an item appears when offers are available.
Walking and looking stop while an overlay is open. Use **Tab** or its Lower button to lower the
phone without releasing capture. Actual window focus or browser pointer-lock loss still pauses safely.

Each day has six shared action points (PA). Work, meals and other activities spend PA;
exploration, reading, bill payments and support information are free. Sleeping at the bed fades
to black before showing that day's income and spending. The all-days log keeps the same
breakdown for earlier days.
Unused PA still elapse for needs and scheduled commitments. A delivery shift is available on the last day.
Only the current report's explicit Continue advances the campaign. Betting is voluntary.
Endings stop gameplay and keep history/reports readable, with Return to Title available.
The city outside the windows gradually darkens as actions advance the day. Work shifts can
only start in the morning or afternoon.
Clean the apartment at the bathroom trash can. Each level of mess adds one point whenever
stress increases. Hunger at 80 or higher blocks watching TV and cleaning until Alex eats.
Stress at 80 blocks work and other activities that spend PA without relieving stress. Sleeping,
paying bills, betting, and adjusting betting controls are available under their usual rules.
After the first action that spends PA on day one, the landlady knocks and introduces each
status in turn. Press **Enter** or click to advance her dialogue. Stress and urge start at 30.
Stress turns yellow at 60 and red at 80. Urge turns yellow at 50 and red at 70.
After continuing from the daily report, Alex wakes in bed.
Press **Enter** again to skip the wake-up scene.

Meaningful actions autosave one local campaign separately from graphics settings, using
`casino-game.campaign.v1` in browser localStorage or the native user configuration directory.
A storage failure keeps session progress and shows a retry notice; retry saving before closing.
Unreadable saves are preserved until an explicitly confirmed New Game replaces them.
On load, the current prices apply to unpaid bills in campaigns with water, electricity and rent.
Recorded payments and deadlines are preserved. Change `DAYS` in `src/game/campaign.rs`
to set the duration of future new campaigns.
The original gameplay contract and focused verification record are in [IMPLEMENTATION.md](IMPLEMENTATION.md).
The first build compiles the engine and can take several minutes.
Install the native dependencies for your platform from the
[Bevy setup guide](https://bevy.org/learn/quick-start/getting-started/setup/).

## Browser

The [Pages workflow](.github/workflows/pages.yml) builds and deploys the game on every
push to `master`, or manually from GitHub Actions. In repository **Settings → Pages**,
the source must be **GitHub Actions**. Only the built HTML, JavaScript, WASM and runtime
assets are published; development sources and review galleries are excluded.

With Python 3, `wasm-bindgen-cli` 0.2.129 on your PATH, and the Rust
`wasm32-unknown-unknown` target installed:

```sh
tools/build-web.sh
python3 tools/serve_web.py
```

Open <http://localhost:8080>. The build uses WebGPU and copies the apartment assets
into `dist/`. A WebGPU-enabled browser and HTTPS (or localhost) are required.
The build stamps the JavaScript and WASM URLs with one content hash so browsers
load matching files after a rebuild. Reload an existing game tab to use the new build.
The local server disables HTTP caching for the page, code and assets. For an existing
cached tab, disable the HTTP cache in browser developer tools and reload once;
keep the toolbox open while developing. Keep site storage to preserve campaign saves.
Startup reports an error if the browser cannot provide a WebGPU graphics device.
Zen builds using Firefox 152–156 can crash on Linux during WebGPU canvas teardown
with `Queue[Id(...)] does not exist` ([Mozilla bug 2037013](https://bugzilla.mozilla.org/show_bug.cgi?id=2037013)).
Upgrade to a browser engine containing the Firefox 157 fix, or use Chromium.
The local readback workaround was removed because it substantially reduced FPS;
Zen uses its default shared-texture presentation again.
On this Linux development PC, Chromium needs WebGPU and the Vulkan backend enabled
explicitly. Enabling WebGPU alone selects the SwiftShader CPU fallback even when
WebGL is hardware accelerated. Use the [WebGPU working group's Linux flags](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status):
Quit all Chromium windows before running this command so the flags take effect:

```sh
chromium --ozone-platform=x11 --enable-unsafe-webgpu --use-angle=vulkan --enable-features=Vulkan,VulkanFromANGLE http://localhost:8080/
```

In the browser console, `(await navigator.gpu.requestAdapter()).info` should report
`isFallbackAdapter: false` and the hardware vendor (`amd` on this PC), not `google` /
`swiftshader`. The browser capture and benchmark tools use the same Vulkan flags.

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
High is the default. Presets select shadow quality and enable both PSX effects; Lighting changes shadow quality only. Disabling either effect displays **Custom**.

| Setting | Low | Medium | High |
| --- | --- | --- | --- |
| Lighting | Shadows off | 1024px shadows, hardware filtering | 2048px shadows, hardware filtering |

All lights, fog, scene geometry and original texture dimensions stay present at every setting.
Rendering always uses the full window resolution; there is no scaling camera or texture-resizing queue.
Antialiasing is disabled for the scene, phone, and UI edges.
Text with accented letters keeps glyph smoothing for legibility.
Game textures use nearest-neighbor sampling, without linear, mipmap blending, or anisotropic filtering.
Shadows retain Bevy's linear comparison samplers for hardware 2×2 filtering to avoid blocky shadow-map edges.
Tone mapping and debanding are disabled.
These choices are fixed for a sharper PSX appearance and have no settings toggles.
**Texture warping** blends 20% affine UVs with 80% perspective-correct UVs, capped at 0.01 UV
units per axis to keep large tiled surfaces subtle. Cutout materials retain perspective-correct UVs to preserve their silhouettes and shadow masks. **Model jitter**
snaps vertices to a one-pixel screen grid in the camera color and depth passes only; shadow maps
keep unsnapped geometry and perspective-correct UVs. Shadow lookup uses the original world position.
Both have independent **Off / On** controls, enabled by default, including for animated characters
and the handheld phone; menus and HUD stay crisp. The standard lighting and fog remain in use. The effects
follow the [URP-PSX reference shader](https://github.com/Math-Man/URP-PSX-FORKED/blob/11752b7717ecd2157ca67ba5f944963d2f63ebed/URP-PSX/Assets/Shaders/Other/PS1AffineFog.shader),
implemented with WGSL interpolation and vertex projection. These patches target Bevy 0.19.1;
review them when upgrading the engine. Switching effects recompiles render pipelines.

Changes preview immediately;
**Save settings** commits them, while **Cancel** restores the last saved settings. Escape also
cancels on native or with browser Escape capture active. Without browser Escape capture,
the first Escape while captured releases the mouse and keeps settings open.
Version-one and version-two preferences retain lighting only; removed antialiasing, texture, and resolution choices are discarded.
Older preferences enable both PSX effects by default. Existing PSX worktree saves retain their effect choices while discarding antialiasing.
New saves use schema version three in the existing storage location.

Settings are stored in browser localStorage (`casino-game.graphics.v1`), or in
`CasinoGame/casino-game.graphics.v1.json` under the native user configuration directory
(`$XDG_CONFIG_HOME` / `~/.config` on Linux, `%APPDATA%` on Windows, and
`~/Library/Application Support` on macOS). Invalid data falls back to High. Storage failures
show a notice and keep settings usable for the current session. There is no account sync.

The centered panels and buttons share one generated, nine-sliced texture:
`assets/ui/pause-panel.png` (128×128, 864 bytes on disk, 64 KiB decoded RGBA on the GPU).
It matches the HUD's charcoal backplates and gray rims, with a 64×64 pixel grid,
16 colors, 4-bit indexed PNG storage and nearest-neighbor sampling for a PSX look.
This reduces GPU texture storage by 94% versus the previous 512×512 panel. Text stays
native-resolution using the included, unmodified Liberation Sans Bold font and its OFL license.
Pause and settings share three text sizes; the title heading scales with window width.
These texture figures exclude the CPU image copy and font atlases. The generation prompt,
style notes and compression details are in `art/ui/pause-panel-generation.json`.

The phone's original SVG artwork is rasterized into nearest-sampled indexed PNGs:
`assets/ui/phone-icons.png` (128×96 atlas, 32×32 cells, up to 32 palette entries, 8-bit,
2,459 bytes) and `assets/ui/phone-wallpaper.png` (120×210, 16 colors, 4-bit, 1,992 bytes).
The two bitmaps total 4,451 bytes on disk, 95% less than the original full-resolution PNGs,
and decode to 149,952 bytes of RGBA texture storage, 92.6% less than the originals.
The composed screen also uses nearest sampling; launcher text uses Liberation Sans Regular,
while apps use Regular for detail and Bold for headings. These figures exclude the render target, CPU copies and font atlases.
Rasterization and Pillow processing details are in `art/ui/phone-generation.json`.

App artwork uses two additional nearest-sampled, 32-color indexed PNG atlases:
`phone-betting.png` (128×128) and `phone-apps.png` (192×160), totaling 13,063 bytes
on disk and 188,416 bytes decoded RGBA. Native text keeps amounts and controls readable.
The two-color roulette disk is generated in memory at 64×64 (16,384 RGBA bytes,
no additional download), so rotation does not depend on rectangular UI clipping.
Built-in imagegen prompts, source hashes and crop rectangles are recorded in
`art/ui/phone-apps-generation.json`. Repack retained generated sources with
`python3 tools/build_phone_apps.py`; its assertions enforce palette, placement, alpha and
a combined 32 KiB budget. Source manifests record the original generation paths;
raw generated masters are not included in the browser assets.

The TV loops three generated PSX football frames at two frames per second on its original
mesh and rounded glass UVs. Three 512×512 screen-only emissive atlases share 15 picture
colors plus transparent black in 4-bit indexed PNGs: 8,299 bytes on disk and 3 MiB decoded
RGBA. A 256 KiB runtime copy of the original case texture blacks out only the glass;
the bezel, case pixels and material lighting stay intact. A second UV set isolates the
antenna's shared glass samples in an unused atlas patch so the broadcast cannot light it.
The original off material uses the original UVs and returns in Title or when power is off;
household sales hide the existing TV mesh. The broadcast
stops advancing while paused or unfocused. Built-in imagegen
prompts, source hashes and processing details are in `art/tv-football-generation.json`;
`python3 tools/build_tv_football.py` repacks retained generated sources with format and size
assertions. The opening body uses Alex's existing character texture and one animation clip,
exported alongside the title sleeper by `tools/build_menu_character.py`.

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
    intro.rs          New-game waking animation, subtitles, skip, and control handoff
    tv.rs             Compact football broadcast loop on the apartment TV
    player.rs         First-person controls, static collision, and physics checks
    campaign.rs       Authoritative campaign duration, actions, clock, ledger, and reports
    campaign_save.rs  Validated versioned autosave and retained session
    routine.rs        World interactions, reports, endings, and read-only records
    phone.rs          Physical phone, app routing, amount entry and input guards
    phone/            Launcher, app renderers, compact artwork and phone action tests
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

The ten personal photos reuse the two original frame atlases and mesh UVs.
`art/apartment-photos/` contains the finished 256×256 and 242×178 indexed PNGs
(64 colors each, 80% saturation), plus their attachment-to-frame mapping and crops.
They total 42,467 bytes on disk and 434,448 bytes decoded RGBA, 93.3% and 75% less
than the stock atlases. Blender packs these textures into both apartment exports;
the full-resolution personal photos are not included. To regenerate the atlases,
run `python3 tools/build_apartment_photos.py` with the ten original photo paths in
attachment order, then rebuild the Blender scene as above.

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
That optimization preserves fog, light sources, geometry and render resolution; the fixed unfiltered rendering policy above applies at every quality.

Bevy is pinned to `=0.19.1` because the shader guard patches that version's embedded
WGSL. An unrecognized shader keeps the original source and logs a warning. Recheck
the patch and browser screenshots when upgrading Bevy. Remeasure CPU clustering if
the scene grows substantially beyond its current six spotlights.

The [original settings report](art/review/performance/settings/index.html) is historical: its preset FPS measurements include the removed texture and resolution reductions.
Current presets retain full resolution and original textures, so those figures do not describe current preset performance.
The [anti-aliasing report](art/review/performance/antialiasing/interaction/results.json) is also historical; its selectable AA modes have been removed.
The settings script now checks single-sample rendering without FXAA/SMAA at every lighting preset, legacy migration, saving and cancellation.
Run it only when browser regression checks are requested, against a `scene-review` build with Playwright on `NODE_PATH`:

```sh
node tools/test-settings.cjs art/review/performance/unfiltered/interaction http://127.0.0.1:8080/
```

## Checks

```sh
cargo fmt --check
cargo check
cargo test
python3 -m unittest discover -s tools -p 'test_*.py'
```
