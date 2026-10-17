# Casino Game

A Rust / Bevy 0.19.1 project with a furnished first-person apartment and a dense urban exterior.

## Run

```sh
cargo run
```

The game starts inside the furnished apartment, two storeys above a narrow service court. Wait for loading, then **click** to capture the mouse.
Use **WASD** to walk, **Shift** to sprint, **Space** to jump, and the **mouse** to look.
**Escape** releases the mouse and opens the menu; **Enter** starts again at the interior spawn.
Losing window focus or browser pointer lock also opens the menu. The closed door, windows,
furniture, walls, floor, and ceiling are solid.
The first build compiles the engine and can take several minutes.
Install the native dependencies for your platform from the
[Bevy setup guide](https://bevy.org/learn/quick-start/getting-started/setup/).

## Browser

With `wasm-bindgen-cli` 0.2.129 on your PATH and the Rust
`wasm32-unknown-unknown` target installed:

```sh
tools/build-web.sh
python3 -m http.server 8080 --bind 127.0.0.1 --directory dist
```

Open <http://localhost:8080>. The build uses WebGL2 and copies the apartment assets
into `dist/`. On system Rust installations with `rust-src` but no precompiled
WASM target, the script builds the standard library from those sources instead.
The first build takes several minutes; subsequent builds reuse compiled crates.

## Layout

```text
src/
  main.rs             Entry point
  app.rs              Window, engine configuration, plugin registration
  state.rs            Application states
  game/
    mod.rs            Shared gameplay plugins
    camera.rs         Persistent 3D camera
    player.rs         First-person controls, static collision, and physics checks
  screens/
    mod.rs            Screen plugins and shared screen layout
    menu.rs           Menu setup and input
    playing.rs        Apartment scene, sunlight, and input
assets/
  audio/
  fonts/
  textures/
  shaders/
  models/             Runtime apartment_city.glb; original park conversion retained
```

Add gameplay features as plugins under `src/game/`, keeping their components,
resources, and systems together. Register them in `GamePlugin`. Run gameplay
systems with `in_state(GameState::Playing)` and attach `DespawnOnExit` to entities
that should be removed when a screen closes.

Asset paths are relative to `assets/`, for example
`asset_server.load("textures/cards.png")`.

## Apartment and city assets

The original downloads stay untouched in `base/`. The occupied Home interior is
retained; the other trailers, terrain, trees, background, porch, chassis, and trailer
roof are removed from the runtime scene. Its curtains are gathered to the sides,
and the solid window screens are clearer so the city is visible.

The entrance side is now a continuous opaque party wall, with a solid wood door
opening onto a 2.2 metre shared corridor. Neighbouring flats and a stair core fill
the rest of the host building; continuous floors connect it to street level.
Four original windows remain on the two exterior elevations.
The corridor retains the rough Round 1 materials requested during review.
The fridge is opaque, and plants use alpha cutouts so pots and leaves write depth
while the empty space around leaf silhouettes stays transparent.

Every new exterior model comes from `base/URBAN/`. Closely spaced apartment blocks,
balconies, rusted pipes, vents, roof machinery, graffiti, bins, litter, street lamps,
a bus shelter, fences, and roadworks surround the room. The street is 6 metres below
its floor. The player remains locked inside; the exterior is visual scenery and does
not create physics colliders. Interior walls, windows, doors and furniture still do.
Cool gray distance fog thickens over the city under dim, overcast lighting, while
nearby apartment details remain clear.

Bevy loads `assets/models/apartment_city.glb`. The editable, texture-packed Blender
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

## Temporary visual review

The current browser preview is built with `tools/build-web.sh --features scene-review`.
**F1** shows the closed entrance wall, **F2** the shared corridor, **F3–F4** the
living-room and bedroom windows, and **F5** the host building down to street level.
**F7** shows the balcony across the kitchen window, **F8** the service court,
**F9** the apartment interior, and **F10** the stair landing.
**F11** shows the retained apartment windows from the courtyard.
**F6** returns to the player camera. Review cameras work without mouse capture.
Normal builds do not include these review controls. Remove `src/game/review.rs`,
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

## Checks

```sh
cargo fmt --check
cargo check
cargo test
```
