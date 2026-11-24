# Code layout

`src/lib.rs` holds `SimulationPlugin` (battle logic, also used headless by the tests) and `GamePlugin` (the
full game); everything else lives in one directory per domain. A domain may use another's types, but the
simulation (`sim`, `world`) is meant not to reach into drawing, sound or the interface: where it still does,
it is listed in [persistent-world.md](persistent-world.md) as the work before it becomes a crate of its own.

## `sim` — the rules of the battle

| Module | Responsibility |
| --- | --- |
| `catalog` | unit and building statistics, weapons, armor |
| `spawn` | spawning units and buildings from the glTF models |
| `movement` | path following, unit separation |
| `spatial_index` | finding the units and structures near a point without checking all of them |
| `combat` | target acquisition, chasing, turrets, projectiles, damage |
| `economy` | credits, power, harvester state machine |
| `production` | production queues and the rules for where structures and walls may stand |
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
| `intel` | what each side knows: the Syndicate's shared view and the enemy structures each side has spotted |
| `outcome`, `match_stats` | mission objectives, victory and defeat; the tally shown at the end of a match |

## `world` — the map and its landscape

| Module | Responsibility |
| --- | --- |
| `map` | the cell grid and its bounds (64, 96, 128 or 256 cells a side), chunks, terrain with a river and fords, crystals, A* pathfinding that refuses routes between unconnected areas without searching |
| `map_regions` | the map's connected areas, labelled per chunk and joined across chunk edges |
| `terrain` | landscapes (river, lake, plains): terrain height, water, roads, ground patches |
| `scenery` | forests, meadows, reeds, mountains with boulders as a layout of props by chunk |
| `map_edits` | the map editor's brush strokes applied to a generated map |
| `season` | summer, autumn and winter: terrain tint, snow, foliage colours, light |
| `weather` | the sky changing during a match: clouds, showers, wet ground |

## `missions` — levels and campaigns

| Module | Responsibility |
| --- | --- |
| `levels` | level definitions, campaign and custom levels |
| `campaigns` | campaigns as chains of missions: unlocking, progress, campaign difficulty |
| `level_events`, `event_editing` | scripted mission events ([levels.md](levels.md)); editing them in the battle editor |
| `random_battle`, `mission_generator` | random battles and whole missions rolled from one seed |

## `net` — playing over the network

| Module | Responsibility |
| --- | --- |
| `network` | the lockstep session with a player on another machine: turns, checksums, disconnection, catching up; the message protocol |
| `link` | one WebSocket connection, plain or over TLS, its threads, and the join address |
| `relay` | the hub of a network battle: passing messages on, the order history, late guests and rejoining players ([server.md](server.md)) |
| `server` | the dedicated server's rooms: gathering players around an owner's battle, then one relay per room |
| `lobby` | hosting and joining a network battle from the main menu |

## `view` — drawing the battle

| Module | Responsibility |
| --- | --- |
| `camera` | the RTS camera and the ground it sees |
| `local_view` | the battle as this machine's player sees it, for drawing, sound and the interface |
| `interpolation` | drawing moving things between simulation ticks |
| `terrain_material`, `terrain_chunks` | texture layers and the WESL shader; the ground as one patch per chunk, kept only around the camera, built on background threads and rebuilt when its crystal soil changes |
| `scenery_view`, `flat_scenery` | entities for the props around the camera, redone when a chunk's props change; scenery models merged into one mesh each |
| `crystal_models`, `motion_material` | crystal deposits merged into one mesh per material; GPU vertex motion (wind, hovering shards) |
| `fog_view` | how the terrain, water and enemies show the fog of war; the fog texture, repainted and uploaded by chunk |
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
| `minimap`, `notifications` | minimap, status messages |
| `orders` | selection and player orders |
| `placement` | placing a structure or dragging a wall with the mouse: ghosts, previews and the resulting command |
| `structure_tools`, `support_tools` | the repair / sell mouse modes, buttons and hint; the support power buttons and aiming |
| `key_bindings` | rebindable hotkeys: actions, their default and bindable keys, swapping on conflict |
| `tutorial` | the step-by-step tutorial of levels with `"tutorial": true` |
| `map_editor`, `map_preview` | the map editor; the small picture of a level's map in the menus |
| `network_hud` | the ping and the banner shown while waiting for the other player |

## `app` — the game around the battle

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
| `capture`, `capture_pointer` | `IRON_TIDE_CAPTURE=shot.png` saves a screenshot and quits (for development; `_SAVE_SLOT` / `_LOAD_SLOT` test saves, `_REPLAY=1` watches the newest replay, `_SELECT_ALL=1` selects every own unit, `_ZOOM=150` sets the camera distance, `_MENU=keys` opens that menu page; `_MISSION` numbers past the campaign continue with the custom levels); a scripted mouse ([development.md](development.md)) |
