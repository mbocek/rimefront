# Development

Building, testing and packaging Rimefront, and the contract between the Blender models and the game.

## Building and running

```sh
make run-release      # or: make run (dev build)
make models           # regenerate the models in Blender 5.x (ONLY=tank,scout for a subset)
make models-fast      # export only, without rendering previews (no GPU needed)
make check            # cargo fmt --check + clippy -D warnings + tests
make package          # release build bundled with its assets in dist/
make dmg              # macOS: Rimefront.app on a disk image in dist/
make capture CAPTURE=shot.png CAPTURE_AFTER=30 CAPTURE_FOCUS=52,10
make server           # the dedicated network battle server (PORT=47300), see docs/server.md
make help             # every target
```

Capture runs (`RIMEFRONT_CAPTURE=shot.png`, options in `src/app/capture.rs`) can also play a mouse script through
the real input path, interface picking included, without moving the desktop cursor. It is how menu clicks, box
selection, minimap orders, wall dragging and clicking aircraft in the air are checked without a person at the
mouse:

```sh
RIMEFRONT_CAPTURE=run.png RIMEFRONT_CAPTURE_MISSION=7 RIMEFRONT_CAPTURE_AFTER_SECS=10 \
RIMEFRONT_CAPTURE_POINTER="3 move 1355 760; 3.2 press left; 3.3 release left; 4 move 560 650; 4.3 press left; 5 move 960 700; 6 shot; 6.5 release left; 8 shot" \
  ./target/release/rimefront
```

Each step is seconds since start, then `move x y` (pixels of the 1600 x 900 capture), `press` / `release`
`left` or `right`, or `shot`, which saves `run-1.png`, `run-2.png` and so on.

## Packages and releases

```sh
make package          # dist/rimefront-<version>-<os>-<arch>.tar.gz: the release binary, its assets, the README and docs/
```

The game loads its assets from the folder next to the executable, so a package just needs unpacking.
CI (`.github/workflows/ci.yml`) runs `make check` on every push and pull request. Pushing a version tag (or
starting the workflow by hand) also packages Linux x86_64, macOS arm64, macOS x86_64 (Intel, cross-compiled on
the Apple Silicon runner) and Windows x86_64 (zip) builds; both macOS builds also come as a disk image with
`Rimefront.app` (`make dmg` builds one locally). A version tag then publishes a GitHub release with all the
packages:

```sh
git tag v1.6.0 && git push origin v1.6.0   # the tag should match the version in Cargo.toml
```

## Tests

The repository is a Cargo workspace (the game, `crates/sim` and `crates/net`, see
[architecture.md](architecture.md)); the `make` targets cover all three.

```sh
make test               # everything (about 2 s in release)
make test-unit          # game logic: pathfinding, terrain, production, placement, combat math, settings, saves
make test-playability   # headless matches on a fixed 50 ms clock
cargo test --release --test missions   # campaign setups, objectives, multiple enemies, new units
cargo test --release --test fog        # fog of war: hidden enemies, exploration, spotting, saves
cargo test --release --test sound      # the simulation asks for the right sounds at the right moments
cargo test --release --test orders     # attack-move, patrol, waypoint queues, rally points
cargo test --release --test crowd_bench -- --ignored --nocapture   # simulation time of up to 1920 fighting units
cargo test --release --test world_bench -- --ignored --nocapture   # a world's tick and views with up to 64 players
cargo run --release -p rimefront_net --bin rimefront-world-soak -- --days 0.25   # days of a world with 20 players who come and go
UPDATE_WORLD_PROTOCOL=1 cargo test -p rimefront_net the_world_protocol   # re-record after raising WORLD_PROTOCOL
UPDATE_WORLD_LAYOUT=1 cargo test -p rimefront_net the_layout             # re-record after raising STORE_VERSION
```

When to raise the world protocol or a saved world's layout, and how a server is updated, is in
[compatibility.md](compatibility.md).

The playability tests run the real simulation systems (`SimulationPlugin`) without a window: units cross the
river through a ford, a refinery's harvester earns credits, barracks train queued infantry, low power slows
production, a tank beats a rifle squad, a gun turret stops a scout, artillery splash hits a group, destroyed
buildings free their ground, the enemy AI builds an economy and attacks, a defenceless player loses, a strong
scripted army wins, and saving then loading restores the battle.

## Model contract (Blender → game)

1 Blender unit = 1 m, one cell = 2×2 m, the model's front faces Blender +X, the origin sits at the center of
its footprint on the ground. Rotating turrets are separate objects named `Turret` with their pivot at the origin.
A unit or structure is exported once, as `<id>.glb`, whatever side it is drawn for.

Units and structures are exported with a single material, `Palette` (`tools/blender/rimefront_models/palette.py`):
every surface of the palette in `materials.py` becomes a cell of three small textures (colour,
roughness/metallic, glow) and each face's UVs point at its cell, so a mesh is one glTF primitive and Bevy one
entity. The side's colour cell is painted the key colour, pure magenta (1, 0, 1), and the game's
`SideTintedMaterial` (`src/view/side_tint.rs`, `assets/shaders/side_tint.wesl`) puts the side's colour where
the palette shows the key, before lighting; the palette is sampled without filtering, so the key arrives
exact. Every side's units share one model and its textures, one material for every model and colour, and
any side can have any colour (`SideColors`). It replaced exporting every model in four tints: 174 files and
60 MB of models became 60 files and 16 MB. Previews and icons show the first side's blue. Measured over 40 s of a 210-unit battle on an
Apple M5 Pro: 24,800 → 18,800 entities and about 26 % less CPU time (≈69 s → 51 s); a large map at the start of
a mission has 7 % fewer entities and no measurable change in CPU time. Trees, rocks and crystals keep their named
materials, which the game merges itself (`flat_scenery.rs`, `crystal_models.rs`).

Writing a unit's `Transform` only when it changes (interpolation, settling on the terrain, separation, turret
nodes), so Bevy could skip propagating the transforms of units standing still, made no measurable difference:
over 40 s of a custom level with 120 idle units, 38.9 and 38.2 s of CPU time before, 38.2 and 38.5 s after
(Apple M5 Pro). It was left out. Not even attaching animation players at all (an upper bound for stopping
the clips of idle or hidden units) measured anything on the same level: 38.3 and 38.1 s.
