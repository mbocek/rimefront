# Code layout

The code is a Cargo workspace of three crates:

- **`crates/sim` (`rimefront_sim`)** — the battle simulation: `sim`, `world`, `levels`, `level_events`,
  `i18n`, `match_flow`, and in its `lib.rs` `SimulationPlugin` (battle logic, also used headless by the
  tests) with the steps of a tick (`TickStep`) and of starting a match (`MatchStart`). It knows nothing of
  drawing, sound, the interface or the network, and builds on Bevy without them (`cargo build -p
  rimefront_sim` compiles no renderer); the compiler keeps it so.
- **`crates/net` (`rimefront_net`)** — network battles: the lockstep session (`network`), one connection
  (`link`), the relay, the lobby, a persistent world (`world_host`) and the dedicated server with its binary
  `rimefront-server`. It depends on the simulation only, so `cargo build -p rimefront_net --bin
  rimefront-server` builds a 10 MB server with no renderer (a workspace build unifies Bevy's features and makes it as large as the game).
- **the game (`rimefront`, `src/`)** — everything around it, one directory per domain, and `GamePlugin`. It
  re-exports the simulation's and the network's modules where they always were (`crate::sim`,
  `crate::world`, `missions::levels`, `app::i18n`, `net::network`, …), and the lockstep session (`net::network::LockstepPlugin`) and saving
  (`app::savegame::SavePlugin`) slot their systems into the simulation's steps.

## `sim` — the rules of the battle

| Module | Responsibility |
| --- | --- |
| `catalog` | unit and building statistics, weapons, armor |
| `eras` | ages, the lines of units and defences across them |
| `promotion` | the age turning: refunds, the calm, promoting units and defences |
| `cues` | the sounds and blasts the simulation asks for; `audio` and `view` play and draw them |
| `spawn` | spawning units and buildings as simulation data; `view::dressing` gives them their models |
| `movement` | path following, unit separation |
| `spatial_index` | finding the units and structures near a point without checking all of them |
| `combat` | target acquisition, chasing, turrets, projectiles, damage |
| `economy` | credits, power, harvester state machine |
| `production` | production queues and the rules for where structures and walls may stand |
| `records` | what a saved game or a saved world keeps of every unit and structure, and how they come back |
| `structures` | repairing and selling structures |
| `ai` | computer opponents: build plan, saving up for structures, gathered attack waves, base defense |
| `army_mix` | what the computer has seen of the enemy army and which units counter it |
| `medical` | field medics healing infantry |
| `unit_orders` | move, attack-move, patrol and queued waypoints |
| `veterancy` | experience from kills, Veteran and Elite ranks and their bonuses |
| `player_commands` | player commands, their queue by tick, and carrying them out in the simulation |
| `sim_math` | the portable trigonometry the simulation uses so every processor computes the same battle |
| `determinism` | the seeded simulation randomness, simulation ids, single-threaded schedules and the state checksum |
| `neutral_sites` | Supply Depots, Observation Posts and Field Hospitals: where they stand, what they give their owner |
| `support_powers` | recon flights and artillery barrages: charging, calling and carrying them out |
| `transport` | Troop Carriers: boarding, carrying, unloading and escaping |
| `engineers` | engineers capturing enemy structures and repairing own ones |
| `fog` | fog of war: what each side knows of the map, with the chunks it last changed in |
| `sides` | the sides in a battle, letting one more in, and the per-side storage everything keeps |
| `intel` | what each side knows: the Syndicate's shared view and the enemy structures each side has spotted |
| `outcome`, `match_stats` | mission objectives, victory and defeat; the tally shown at the end of a match |
| `notifications` | news for every side (`SideNews`), and the local side's turned into status messages and sounds, shown by the sidebar |

## `world` — the map and its landscape

| Module | Responsibility |
| --- | --- |
| `map` | the cell grid and its bounds (64, 96, 128 or 256 cells a side), chunks, terrain with a river and fords, crystals, A* pathfinding that refuses routes between unconnected areas without searching |
| `map_regions` | the map's connected areas, labelled per chunk and joined across chunk edges |
| `chunked_grid` | a value per cell kept by chunk, so a map can widen without moving the chunks it has |
| `terrain` | landscapes (river, lake, plains): terrain height, water, roads, ground patches |
| `scenery` | forests, meadows, reeds, mountains with boulders as a layout of props by chunk |
| `wilds` | the `Wilds` landscape's crystal fields and scenery, generated chunk by chunk from the map seed; widening a wilds world |
| `sites` | starting sites across a wilds world and giving them out to newcomers, widening the world when they run low |
| `map_edits` | the map editor's brush strokes applied to a generated map |
| `season` | summer, autumn and winter: terrain tint, snow, foliage colours, light |
| `weather` | the forecast of a match: clouds and showers, and the sight they cost |

## `missions` — levels and campaigns (`levels` and `level_events` live in the simulation crate)

| Module | Responsibility |
| --- | --- |
| `levels` | level definitions, campaign and custom levels |
| `campaigns` | campaigns as chains of missions: unlocking, progress, campaign difficulty |
| `level_events`, `event_editing` | scripted mission events ([levels.md](levels.md)); editing them in the battle editor |
| `random_battle`, `mission_generator` | random battles and whole missions rolled from one seed |

## `net` — playing over the network (the `rimefront_net` crate)

| Module | Responsibility |
| --- | --- |
| `network` | the lockstep session with a player on another machine: turns, checksums, disconnection, catching up; the message protocol |
| `link` | one WebSocket connection, plain or over TLS, its threads, and the join address |
| `relay` | the hub of a network battle: passing messages on, the order history, late guests and rejoining players ([server.md](server.md)) |
| `server` | the dedicated server's rooms: gathering players around an owner's battle, then one relay per room |
| `world_view` | what a player of a world is shown: the things in sight, sent as changes, and the player's mirror of them |
| `world_client` (in the game, `src/net`) | playing a world that runs elsewhere: entering it, mirroring what it shows, sending orders |
| `world_host` | a persistent world run without drawing: players entering it and their orders, over the network or a channel |
| `world_store` | a world saved by chunk in a directory, in a compact binary form, and built again from it |
| `lobby` | hosting and joining a network battle from the main menu |

## `view` — drawing the battle

| Module | Responsibility |
| --- | --- |
| `camera` | the RTS camera and the ground it sees |
| `local_view` | the battle as this machine's player sees it, for drawing, sound and the interface |
| `interpolation` | drawing units and projectiles between simulation ticks |
| `terrain_material`, `terrain_chunks` | texture layers and the WESL shader; the ground and water, the ground as one patch per chunk, kept only around the camera, built on background threads and rebuilt when its crystal soil changes |
| `scenery_view`, `flat_scenery` | entities for the props around the camera, with their wind and trampling, redone when a chunk's props change; scenery models merged into one mesh each |
| `crystal_models`, `motion_material` | crystal deposits merged into one mesh per material; GPU vertex motion (wind, hovering shards) |
| `fog_view` | how the terrain, water and enemies show the fog of war; the fog texture, repainted and uploaded by chunk |
| `dressing` | models for the units, structures and sites the simulation spawns; turrets and harvester loads following the simulation; fallen infantry; the local side's mines |
| `sky` | the weather on screen: sunlight, sky light, wet ground, and telling the player when rain sets in |
| `overlay` | health bars, selection |
| `battlefield` | scorch marks, vehicle wrecks and rain |
| `wall_joints` | joins neighbouring wall segments of a side into one continuous wall |

## `audio` — sound

| Module | Responsibility |
| --- | --- |
| `sound` | sound cues from the simulation, playback with distance and fog, music director |
| `announcer` | the radio operator's spoken reports in the interface language |
| `battle_noise` | looping engines, rotors, footsteps, harvesting and construction, mixed from the units in view |
| `unit_voices` | units answering the player's selections and orders |

## `ui` — the interface

| Module | Responsibility |
| --- | --- |
| `sidebar/` | `mod.rs` fonts, colours and the sidebar's frame (tabs, tiles with icons); `build_menu` tabs, tiles and their tooltip; `resources` credits and power; `info_panel` the selection; `mission_status` objective, match time and notifications; `controls_help` the key overview; `outcome` the victory or defeat screen |
| `main_menu/` | `mod.rs` the menu's state, hub and screens; `campaign_map` the operations map and briefings; `battle_setup` the battle editor and random battle options; `network_page` hosting and joining; `commands` what each button does; `widgets` its buttons and fields; `update_banner` the update notice |
| `menu`, `perf_overlay` | in-game menu, performance overlay |
| `minimap` | minimap |
| `orders` | selection and player orders |
| `placement` | placing a structure or dragging a wall with the mouse: ghosts, previews and the resulting command |
| `structure_tools`, `support_tools` | the repair / sell mouse modes, buttons and hint; the support power buttons and aiming |
| `key_bindings` | rebindable hotkeys: actions, their default and bindable keys, swapping on conflict |
| `tutorial` | the step-by-step tutorial of levels with `"tutorial": true` |
| `map_editor`, `map_preview` | the map editor; the small picture of a level's map in the menus |
| `network_hud` | the ping and the banner shown while waiting for the other player |

## `app` — the game around the battle (`i18n` and `match_flow` live in the simulation crate)

| Module | Responsibility |
| --- | --- |
| `game_plugins` | the parts `GamePlugin` adds to the simulation: battle view, menus, sound, display, replays, map editor, asset loading, network battle |
| `match_flow` | match setup and states |
| `headless` | windowless battle harness shared by the tests and the balance tool |
| `settings`, `frame_pacing` | settings and their persistence, frame rate limit |
| `savegame` | saving and loading games (JSON) |
| `replay` | recording matches, storing and listing replays, watching them |
| `i18n` | interface languages: loads `assets/i18n/*.json`, `tr` / `tr_fmt` |
| `updates`, `self_update` | checking GitHub for a newer release and downloading its package; finding how this copy is installed, verifying a package's checksum and installing it in place |
| `catching_up` | a machine that joined a network battle late running it many times faster, silently, until it reaches the players |
| `capture`, `capture_pointer` | `RIMEFRONT_CAPTURE=shot.png` saves a screenshot and quits (for development; `_SAVE_SLOT` / `_LOAD_SLOT` test saves, `_REPLAY=1` watches the newest replay, `_SELECT_ALL=1` selects every own unit, `_ZOOM=150` sets the camera distance, `_MENU=keys` opens that menu page; `_MISSION` numbers past the campaign continue with the custom levels, `_CLAIM_SITES=6` gives out starting sites of a wilds level and so widens it); a scripted mouse ([development.md](development.md)) |
