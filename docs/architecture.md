# Code layout

| Module | Responsibility |
| --- | --- |
| `catalog` | unit and building statistics, weapons, armor |
| `map` | the cell grid (64, 96, 128 or 256 cells a side), terrain with a river and fords, crystals, A* pathfinding that refuses routes between unconnected areas without searching |
| `spawn` | spawning units and buildings from the glTF models |
| `movement` | path following, unit separation |
| `spatial_index` | finding the units and structures near a point without checking all of them |
| `combat` | target acquisition, chasing, turrets, projectiles, damage |
| `economy` | credits, power, harvester state machine |
| `production` | production queues and the rules for where structures and walls may stand |
| `placement` | placing a structure or dragging a wall with the mouse: ghosts, previews and the resulting command |
| `structures`, `structure_tools` | repairing and selling structures; the repair / sell mouse modes, buttons and hint |
| `orders` | selection and player orders |
| `ai` | computer opponents: build plan, saving up for structures, gathered attack waves, base defense |
| `army_mix` | what the computer has seen of the enemy army and which units counter it |
| `headless` | windowless battle harness shared by the tests and the balance tool |
| `terrain`, `terrain_material` | landscapes (river, lake, plains): terrain height, water, roads, texture layers, WESL shader |
| `terrain_chunks` | the ground as one patch per chunk, built on background threads and rebuilt when its crystal soil changes |
| `scenery`, `flat_scenery` | forests, meadows, reeds, mountains with boulders; scenery models merged into one mesh each |
| `crystal_models`, `motion_material` | crystal deposits merged into one mesh per material; GPU vertex motion (wind, hovering shards) |
| `minimap`, `notifications` | minimap, status messages |
| `ui`, `overlay` | sidebar (tabs, tiles with icons), health bars, selection |
| `ui/` | `mod.rs` fonts, colours and the sidebar's frame; `build_menu` tabs, tiles and their tooltip; `resources` credits and power; `info_panel` the selection; `mission_status` objective, match time and notifications; `controls_help` the key overview; `outcome` the victory or defeat screen |
| `settings`, `menu`, `perf_overlay`, `frame_pacing` | settings and their persistence, in-game menu, performance overlay, frame rate limit |
| `savegame` | saving and loading games (JSON) |
| `lib.rs` | `SimulationPlugin` (battle logic, also used headless by the tests) and `GamePlugin` (the full game) |
| `game_plugins` | the parts `GamePlugin` adds to the simulation: battle view, menus, sound, display, replays, map editor, asset loading, network battle |
| `campaigns` | campaigns as chains of missions: unlocking, progress, campaign difficulty |
| `levels`, `match_flow`, `main_menu` | level definitions, campaign and custom levels; match setup and states; main menu and battle editor |
| `main_menu/` | `mod.rs` the menu's state, hub and screens; `campaign_map` the operations map and briefings; `battle_setup` the battle editor and random battle options; `network_page` hosting and joining; `commands` what each button does; `widgets` its buttons and fields; `update_banner` the update notice |
| `medical` | field medics healing infantry |
| `unit_orders` | move, attack-move, patrol and queued waypoints |
| `veterancy` | experience from kills, Veteran and Elite ranks and their bonuses |
| `i18n` | interface languages: loads `assets/i18n/*.json`, `tr` / `tr_fmt` |
| `interpolation` | drawing moving things between simulation ticks |
| `lobby` | hosting and joining a network battle from the main menu |
| `local_view` | the battle as this machine's player sees it, for drawing, sound and the interface |
| `network_hud` | the ping and the banner shown while waiting for the other player |
| `network` | the lockstep session with a player on another machine: turns, checksums, disconnection, catching up; the message protocol |
| `link` | one WebSocket connection, plain or over TLS, its threads, and the join address |
| `relay` | the hub of a network battle: passing messages on, the order history, late guests and rejoining players ([server.md](server.md)) |
| `server` | the dedicated server's rooms: gathering players around an owner's battle, then one relay per room |
| `player_commands` | player commands, their queue by tick, and carrying them out in the simulation |
| `sim_math` | the portable trigonometry the simulation uses so every processor computes the same battle |
| `determinism` | the seeded simulation randomness, simulation ids, single-threaded schedules and the state checksum |
| `updates` | checking GitHub for a newer release and downloading its package |
| `neutral_sites` | Supply Depots, Observation Posts and Field Hospitals: where they stand, what they give their owner |
| `support_powers`, `support_tools` | recon flights and artillery barrages: charging, calling and carrying them out; the sidebar buttons and aiming |
| `transport` | Troop Carriers: boarding, carrying, unloading and escaping |
| `self_update` | finding how this copy is installed, verifying a package's checksum and installing it in place |
| `season` | summer, autumn and winter: terrain tint, snow, foliage colours, light |
| `key_bindings` | rebindable hotkeys: actions, their default and bindable keys, swapping on conflict |
| `engineers` | engineers capturing enemy structures and repairing own ones |
| `tutorial` | the step-by-step tutorial of levels with `"tutorial": true` |
| `sound` | sound cues from the simulation, playback with distance and fog, music director |
| `announcer` | the radio operator's spoken reports in the interface language |
| `battle_noise` | looping engines, rotors, footsteps, harvesting and construction, mixed from the units in view |
| `fog`, `fog_view` | fog of war: what the player knows, and how the terrain, water, minimap and enemies show it |
| `intel` | what each side knows: the Syndicate's shared view and the enemy structures each side has spotted |
| `outcome`, `match_stats` | mission objectives, victory and defeat; the tally shown at the end of a match |
| `replay` | recording matches, storing and listing replays, watching them |
| `battlefield` | scorch marks, vehicle wrecks and rain |
| `wall_joints` | joins neighbouring wall segments of a side into one continuous wall |
| `weather` | the sky changing during a match: clouds, showers, wet ground |
| `capture` | `IRON_TIDE_CAPTURE=shot.png` saves a screenshot and quits (for development; `_SAVE_SLOT` / `_LOAD_SLOT` test saves, `_REPLAY=1` watches the newest replay, `_SELECT_ALL=1` selects every own unit, `_ZOOM=150` sets the camera distance, `_MENU=keys` opens that menu page; `_MISSION` numbers past the campaign
continue with the custom levels) |
