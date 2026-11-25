# Rimefront

[![CI](https://github.com/mbocek/rimefront/actions/workflows/ci.yml/badge.svg)](https://github.com/mbocek/rimefront/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/mbocek/rimefront)](https://github.com/mbocek/rimefront/releases/latest)
![Rust](https://img.shields.io/badge/rust-1.100%2B-orange?logo=rust)
![Bevy](https://img.shields.io/badge/bevy-0.20-232326)
![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-blue)
![Languages](https://img.shields.io/badge/languages-EN%20%7C%20SK%20%7C%20CS-green)

An original real-time strategy game written in Rust with [Bevy](https://bevyengine.org): build a base,
harvest crystals, keep the power up and destroy the enemy. Every line of code, every 3D model (generated
procedurally in Blender) and every piece of music is original; the sounds are synthesized, except gunfire
and explosions, which are built from public-domain recordings.

![A Coalition base under attack in winter](docs/screenshot.jpg)

## Features

- **Two factions:** the Coalition and the Syndicate, each with its own armoury, research and campaign.
- **Campaign:** ten Coalition missions and the Syndicate missions, starting with a step-by-step tutorial.
- **Skirmish:** random battles rolled from a seed, or your own levels made in the built-in battle editor.
- **Multiplayer:** up to four players over the network, with spectators, chat and replays.
- **Tactics:** fog of war, engineers that capture, repair and lay mines, camouflaged snipers, aircraft and flak.
- **Computer opponents** on three difficulties that build, expand, defend their harvesters and attack.
- **Sound that follows the camera:** positional gunfire, engines and footsteps, music that rises with the fight
  and a radio operator speaking English, Slovak or Czech.

## Play

Download the package for your system from the [latest release](https://github.com/mbocek/rimefront/releases/latest),
unpack it and run `rimefront` (on macOS open the disk image and start *Rimefront*).
Press **F1** in a battle to see the controls.

Release builds are not yet signed with a Developer ID, so macOS refuses the first start: open *System
Settings → Privacy & Security* and click *Open Anyway* next to Rimefront. For packages up to 1.3.0, which
macOS calls "damaged", run `xattr -dr com.apple.quarantine "/Applications/Rimefront.app"` once instead.

## Build from source

```sh
make run-release   # build and play
make check         # formatting, clippy and every test
make help          # every target
```

## Documentation

| Topic | |
| --- | --- |
| [Controls](docs/controls.md) | mouse, keyboard and rebindable keys |
| [Rules](docs/rules.md) | factions, units, structures, research, economy, veterancy |
| [Missions and custom levels](docs/levels.md) | campaign, random battles, battle editor, level file format |
| [Settings, saves and updates](docs/settings.md) | where settings and saves live, performance notes, update check |
| [Sound and music](docs/audio.md) | effects, recorded gunfire, music, positional sound, voice reports |
| [Languages](docs/languages.md) | translations and how to edit them |
| [Determinism and multiplayer](docs/determinism.md) | lockstep simulation, network play, replays |
| [Dedicated server](docs/server.md) | `rimefront-server`: rooms on an internet server, how to play through it, what is left |
| [Balance](docs/balance.md) | the balance tool and its latest results |
| [Development](docs/development.md) | building, tests, packages and releases, the Blender model contract |
| [Code layout](docs/architecture.md) | what each module is responsible for |
| [Persistent world](docs/persistent-world.md) | the steps towards a permanent world far larger than a match map |
| [Renaming to Rimefront](docs/rename.md) | why the game is being renamed and everything the rename touches |
| [Signing](docs/signing.md) | signing and notarizing release builds |

## Third-party assets

- Terrain textures (`assets/textures/terrain/`): [Poly Haven](https://polyhaven.com), CC0 -
  leafy_grass, forrest_ground_01, rocks_ground_05, river_small_rocks, gravel_concrete_02.
- Gunfire and explosion recordings (`tools/audio/recordings/`), CC0, from OpenGameArt:
  [The Free Firearm Sound Library](https://opengameart.org/content/the-free-firearm-sound-library),
  [25 CC0 bang / firework SFX](https://opengameart.org/content/25-cc0-bang-firework-sfx) and
  [Tiny Naval Battle Sounds Set](https://opengameart.org/content/tiny-naval-battle-sounds-set) (Iwan Gabovitch).
- Font Rajdhani (`assets/fonts/`): Indian Type Foundry, SIL Open Font License 1.1 (`assets/fonts/OFL.txt`).

(c) 2026 Rimefront
