# Towards a persistent world

The goal: one world that runs permanently on a server, far larger than a match map, where players join, are
given a starting site, build up, leave and come back. This page is the series of steps that gets there. Each
step is one commit and is worth having on its own, for ordinary matches too.

## Where we start

- The map is a dense grid sized once per match: `MapSize` (64, 96, 128 or 256 cells of 2 m) sets the
  thread-local map bounds, and `GameMap`, the fog of war, the minimap and the fog texture are each one `Vec`
  covering the whole map.
- The terrain is one mesh with a vertex every 0.5 m (about 470 thousand vertices on a Huge map, 1.4 million on
  a Vast one); forests, rocks and crystals are merged into one mesh per model.
- Pathfinding is A* over cells across the whole map, after a region check that floods the whole map again
  whenever a structure appears or goes.
- Unit separation and target choice compare every unit with every other one.
- Landscapes, roads and crystal fields are authored for the 64-cell map and stretched; there are four base
  sites in the corners and every map is point-symmetric.
- Four fixed teams (`Team::Player`, `Crimson`, `Amber`, `Violet`).
- Network play is lockstep ([determinism.md](determinism.md), [server.md](server.md)): every machine runs the
  whole battle, the server only relays orders, and a late guest catches up by replaying the full order history.

### Measured: Vast against Huge

The first bridgehead mission as a custom level, 40 s capture runs on an Apple M5 Pro, sound off, two runs each:

| Map | CPU time per 40 s (user + sys) | Peak memory |
| --- | --- | --- |
| Huge (128 cells) | 33.5 s, 34.1 s | 1.63 GB, 1.60 GB |
| Vast (256 cells) | 37.6 s, 37.8 s | 1.92 GB, 1.93 GB |

Four times the area costs about 12 % more CPU and 300 MB more memory with a handful of units: playable, but
the cost grows with the area even where nothing happens, which is what the steps below remove.

### Measured: the spatial index

`tests/crowd_bench.rs` sets up fights of 80 against 80 units spread over a Vast map and simulates 20 s of
fighting (two runs each, Apple M5 Pro):

| Units | Before | With the index |
| --- | --- | --- |
| 160 (one fight) | 0.07 s, 0.05 s | 0.05 s, 0.04 s |
| 640 (four fights) | 0.37 s, 0.35 s | 0.24 s, 0.21 s |
| 1920 (twelve fights) | 2.71 s, 2.79 s | 0.91 s, 0.88 s |

Below 256 bodies the index checks every one, as before: searching buckets for a crowd that small made the
balance tool about 5 % slower; with the limit its CPU time is the same within noise (40.3 s against 40.0 s,
averaged over six runs of `balance --matches 20`).

### Measured: the fog of war

Recomputing one side's fog from 300 viewers stays cheap at any size (`tests/crowd_bench.rs`): 0.08 ms on
256 × 256 cells, 0.10 ms on 1024 × 1024, 0.17 ms on 2048 × 2048. Painting the blurred fog texture, four
texels a cell, did not: the whole texture was painted on every change, up to five times a second. With 300
viewers moving, per paint:

| Map | Whole texture | Changed chunks |
| --- | --- | --- |
| 128 × 128 cells | 1.8 ms | 0.97 ms |
| 256 × 256 (Vast) | 7.8 ms | 1.18 ms |
| 512 × 512 | 40.3 ms | 1.03 ms |
| 1024 × 1024 | 176.5 ms | 1.03 ms |

## Why lockstep cannot carry it

A permanent world never ends, so its order history grows without limit, and a joining player cannot replay it.
Every client would also have to simulate the whole world, however far from their own units. The world therefore
moves to a simulation on the server that sends each client only the part of the world it can see. The
deterministic simulation stays: it keeps the tests, replays and the fingerprints honest, and lockstep remains
for ordinary network matches.

## Steps

### A. Cost follows activity, not area

1. **Vast map size (256 cells).** Done: `MapSize::Vast`, in the random battle and battle editor.
2. **Spatial index of units.** Done: `SpatialIndex` sorts bodies by 8 m patches of ground once per tick and
   answers "who may be near" in simulation order. Target choice, splash damage, mines, spotting camouflage
   and unit separation ask it instead of checking every unit. It works without map bounds, and results are
   identical: the balance report matches byte for byte and the fingerprints are unchanged.
3. **Map bounds instead of a size.** Done: the thread-local `map_bounds()` gives the first cell and the size
   along each axis (`MapBounds`), and grids, fog, terrain and scenery placement, the minimap, the fog texture,
   the terrain and water shaders (`map_area`) and the camera all work from it, so a map need not start at cell
   zero nor be square. Unit tests play a shifted, oblong map. Levels still pick a square `MapSize` at the
   origin, and the landscapes are still authored for a square: generation by chunk (step 8) replaces that.
   Results are identical: same fingerprints, same balance report.
4. **Changes tracked by chunk.** Done: the map is divided into chunks of 32 × 32 cells (`CHUNK_CELLS`,
   `MapBounds::chunk_index`), the fog of war remembers the revision each chunk last changed at, and the fog
   texture repaints only the chunks that changed since it was last painted, exactly as a full repaint would.
   The grids themselves stay dense over the map bounds: recomputing one side's fog over 2048 × 2048 cells
   takes 0.17 ms, and a dense grid that size is a few tens of MB, so storing chunks separately pays only once
   the world must grow while it runs. Left whole here: the fog texture's upload to the GPU (done in step 5)
   and the minimap's fog pass (step 7).
5. **Terrain in chunks.**
   - Done: the ground is one patch per chunk (`terrain_chunks.rs`), detailed (0.5 m) where it shows the map
     or its rim and coarse (2 m) in the mountains beyond, each built on a background thread and drawn only
     when in view; an editor stroke rebuilds only the chunks whose crystal soil changed. Normals come from the
     heights around each vertex, so patches meet without a lighting seam, and a 3 m skirt hides the crack
     where a detailed patch meets a coarse one. On a Vast map the patches hold 1.9 million vertices against
     1.6 million in the two meshes before (detailed patches run to the edge of their chunk) and take the same
     0.6 s of CPU to build, now off the main thread; CPU time over a 50 s capture is unchanged within noise
     (47.1 s against 46.8 s, three runs each in rotating order).
   - Done: only the patches around what the camera sees exist (`stream_terrain`): the ground in view plus
     one patch, kept until the view is two patches away. On a Vast map that is 15 of the 196 patches at the
     usual zoom and 43 fully zoomed out, and the same on any larger map. Jumping across the map through the
     minimap shows complete ground in the first captured frame.
   - Done: the fog texture's changed chunks go to the GPU as regions written straight into the texture
     (`FogUploadPlugin`), about 72 KB a chunk, instead of re-uploading the whole image asset (4 MB on a
     Vast map, 64 MB on 1024 × 1024 cells); a new map or a switch to another side's fog still uploads it all.
   - Done: scenery is data first. Map generation fills a `SceneryLayout` of props by chunk, drawing the same
     random numbers as before, so maps and fingerprints are unchanged; `stream_scenery` gives the props
     around the view an entity each and drops them as the view moves on, and a chunk whose props changed
     (decoration cleared under a new structure, an editor brush) is shown afresh. A Vast map went from about
     123,000 entities to 10,000 at the usual zoom and 33,000 fully zoomed out. Over 40 s captures (two runs
     each, rotating order) a Vast map now costs what a Huge one does, 33.7 s of CPU against 33.4 s, where
     it cost 37.1 s before, and peaks at 1.52 GB instead of 1.87 GB.
6. **Pathfinding.** Measured, and left as it is for now. On River maps with their forests and rocks, one A*
   route cell by cell (Apple M5 Pro):

   | Map | Across the river away from a ford | Base to base |
   | --- | --- | --- |
   | 256 × 256 (Vast) | 0.2 ms | 0.3 ms |
   | 512 × 512 | 0.4 ms | 2.1 ms |
   | 1024 × 1024 | 1.9 ms | 9.3 ms |

   About 60 ns per cell searched: the time is the number of cells, not the bookkeeping. Two attempts made
   nothing better and were dropped. Portals between chunks (HPA*) cost 155 ms the first time across a
   1024-cell map, working out the chunks on the way, and gave routes 10 % longer. An A* with flat arrays
   instead of the `pathfinding` crate's hash map, popping cells in the crate's exact order so routes stay the
   same, ran at the same speed. Searching fewer cells means another algorithm (jump points, portals worked out
   ahead), and that changes routes and so the fingerprints; it becomes worth it when orders across a world
   larger than 1024 cells are common.

   What did grow with the area was labelling the map's connected regions, which the route search uses to
   refuse unreachable goals at once: the whole map again after every structure placed or removed. Done:
   each chunk now labels its own parts and the parts are joined across chunk edges (`map_regions.rs`), so a
   structure relabels its chunk and redoes the joins. Connections are exactly those of one flood fill (a test
   compares them over random maps and edits); fingerprints and the balance report are unchanged. After a
   structure, on River maps with forests:

   | Map | Whole map | After a structure |
   | --- | --- | --- |
   | 256 × 256 | 0.54 ms | 0.03 ms |
   | 512 × 512 | 1.83 ms | 0.10 ms |
   | 1024 × 1024 | 7.25 ms | 0.44 ms |
   | 2048 × 2048 | 29.6 ms | 2.1 ms |

   The joins still walk every chunk edge; joining only around the changed chunk is the next step if a
   world of 2048 cells sees many structures a second.
7. **Minimap for a world.** Done, as a bounded picture rather than a second view: the minimap is at most
   768 pixels a side, three pixels a cell up to a Vast map (so those look exactly as before) and less than a
   pixel a cell beyond, sampling the cell under each pixel's centre. A refresh, five a second with 300
   viewers revealing the fog, took 1.14 ms on a Vast map and 19.7 ms on 1024 × 1024 cells, uploading
   38 MB each time; now 0.84 ms, 1.5 ms on 1024 and 2.5 ms on 2048, at most 2.4 MB. A minimap showing the
   camera's surroundings at a fixed scale is still worth having once the world is large enough that a
   pixel covers several units' worth of ground.

### B. A world that grows

8. **Generation by chunk.** Done as a landscape of its own, `Wilds` (`world::wilds`), beside the authored
   ones, which stay as they were: fingerprints, the campaign and old replays are untouched. Height is the
   rolling hills every landscape has; lakes rise where a noise of the map seed passes a threshold, held down
   near the bases. Each chunk's crystal and scenery come from the map seed and the chunk's coordinate alone:
   a chunk has a field of its own with a 40 % chance, a field reaches at most into the next chunk, and a
   cell's crystal is a function of the cell and the fields around it, so a chunk generated alone equals the
   same chunk of the whole map (a test compares them, props included). Scenery is as dense as on the
   standard map but not mirrored, and every base has a home field. Over 30 seeds on Standard, Large and
   Vast maps every base reaches the others and has crystal at hand. Roads are left out for now. The
   crystal fields of a map are a resource (`CrystalFields`) rather than the authored list, for the glow and
   sparkles that mark them.
9. **Grids stored by chunk.** Done. `GameMap` (one `MapCell` per cell: blocked, building, crystal) and the fog
   of war keep their cells in a `ChunkedGrid`, a table of 32 × 32 chunks over the bounds; a chunk exists once
   it is opened, and widening re-lays only the table. Saves still get dense grids, row by row. The world's
   bounds (`map_bounds`) may now outgrow the level's (`level_bounds`), which the bases, the authored layout and
   the mirror stay with. `wilds::widen_world` widens a Wilds world by whole chunks: the map, every side's fog
   and the crystal fields cover the new ground, which is generated chunk by chunk exactly as a chunk generated
   alone, and known ground stays known (a test checks all of it, and a route into the new ground). The wilds
   no longer block the map's last row and column, which only evens out the mirror of authored maps.
   Simulation results are unchanged (fingerprints, balance report byte for byte) and so is the speed within
   noise: the balance tool took 42.8 s of CPU against 42.1 s (three runs each in rotating order), the crowd
   benchmark the same, one side's fog 0.11 ms instead of 0.15 ms on 256 × 256 cells and 1.6 ms instead of
   1.4 ms on 2048 × 2048. Still to do: saves of a widened world, whose grids are larger than the level's
   map (the view follows a widening since step 10).
10. **Starting sites across the world.** Done in the simulation (`world::sites`). A wilds world has at most
    one starting site in every square of 64 × 64 cells, from the map seed and the square's coordinate alone,
    on dry ground 40 cells or more from the level's own bases; each has a home field and open ground around
    it. `claim_starting_site` gives a newcomer the free site nearest a point and, when fewer than three would
    be left free, widens the world by a ring of squares first. The level's sides keep the bases it names. A
    test gives out ten sites on a standard wilds map: all different, apart, on open ground, with crystal at
    hand and reachable from the first base, the world having widened. A widening sends `WorldWidened`, and
    the view follows it during a match: the ground patches and the water are planned again, the minimap
    redrawn in the same image, the crystal fields lit, the outlands' trees cleared from the new ground and
    planted along the new edge; the fog texture, the ground material and the camera follow the bounds by
    themselves. `IRON_TIDE_CAPTURE_CLAIM_SITES=6` gives out sites two seconds into a capture run, to look
    at it.
11. **Sides as data.** Done. A `Team` is a side number; the four a level names keep their names
    (`Team::Player` and the rest), and their look (name, colour, model tint) comes from a table that repeats
    past eight sides, the models having four tints. Saved text still names the four sides as before and
    numbers the others (`"Side4"`). The sides in a battle are the `Sides` resource; `sim::sides::admit_side`
    lets one more in, with fog and intel of its own, in a camp of its own: `Diplomacy` gives every side a
    camp, an alliance setting only fills in the four level sides, and `join_camp_of` makes allies.
    Economies, production, research, support power charge, match statistics and cadences keep a value for
    every side that needed one (`PerSide`), so a newcomer starts from the blank values (starting credits,
    uncharged powers); targets keep who sees them as a set of up to 256 sides. Simulation results are
    unchanged. Still four only: the seats of a network session and what a save stores per side.

### C. A world on a server

12. **The world runs on the server.** `iron-tide-server` gains a world mode that runs `SimulationPlugin`
    headless (as the tests already do) and takes player commands from the network instead of relaying them.
    The simulation is a crate of its own (`crates/sim`, see [architecture.md](architecture.md)): it reaches
    into nothing of drawing, sound, the interface or the network, and the network session and saving add
    their systems through the steps of a tick and of a match's start. It builds on Bevy without rendering,
    windows or audio, and the network has a crate of its own on top of it (`crates/net`), so the dedicated
    server links no renderer.
    Done: `iron_tide_net::world_host::WorldHost` runs a wilds world (`world_level`) without drawing, on the
    thread that made it, and takes its players through `WorldConnection`s: a network `Link`, or a
    `local_connection` within one process. A player sends `EnterWorld` and gets `Entered` with their side:
    the first takes the level's own side, everyone after a generated site of their own with the usual
    starting force (`match_flow::deploy_newcomer`); `Orders` from a player run for that player's side only,
    after the usual command delay. The level's opponents are the world's computer sides until step 16.
    `iron-tide-server --world SEED` runs one beside the rooms, entered at `ws://host:port/world`; the server
    is 10 MB now that it links the simulation's systems, and with nobody connected and the computer sides
    building it used 0.16 s of processor time in 10 s. The game does not run a world yet: it needs the state
    a client is sent (step 13) to draw one.
13. **Areas of interest.** Each client is sent the state of the chunks its units and structures see: a full
    snapshot on entering a chunk, then changes per tick; the replay snapshots are the starting point of the
    format. Clients draw it with the existing interpolation; orders go to the server as today.
    Done in the protocol (`iron_tide_net::world_view`): after every tick a world sends each player a
    `WorldView` of what changed in what their side sees, nothing when nothing did: the units and structures
    in the chunks within sight of their side's and allies' units and structures (`chunks_in_sight`) that the
    fog does not hide from them, each with its kind, side, position (hundredths of a cell), facing, health
    and a structure's origin; those gone from view; their credits; and the world's bounds when they widened.
    A player's `WorldMirror` applies them, and a test checks that after a minute it holds exactly what the
    world shows that side, the computer side's Command Center not among it. Measured in that test, as JSON:
    a minute of a quiet start costs 6 kB, and a tank driving costs 41 kB in 10 s, about 4 kB/s for every
    moving unit, so a compact binary format (step 15) is due before crowds.
    The game plays a world too (`net::world_client`): *Play a world alone* on the network page runs a
    `WorldHost` on a thread of the game's own, *Enter the server's world* connects to `ws://server/world`.
    Once the world lets the player in, the game starts a battle on the world's map and seed as a
    `MirroredBattle`: the simulation's rules do not run, only fog, intel and targets, and every tick the units
    and structures the world showed appear, move and go with the world's ids, so selection, orders, the fog
    and the minimap work as in any battle; the player's orders go to the world instead of the queue. A test
    (`tests/world_client.rs`) enters a world run on its own thread and drives a tank there;
    `IRON_TIDE_CAPTURE_WORLD=29` (or a server address) captures one.
    The player's own side travels with the view whenever it changes (`SideState`): the production lanes
    with their queues, progress in percent and the structure ready to place, research done, and the tick
    each support power is charged at, a tick rather than a countdown so that a recharge sends nothing until
    a power is called. Power is tallied from the mirrored structures. So the sidebar builds, places,
    researches and calls support as in a battle (`tests/world_client.rs`, and in `world_host` a power plant
    queued, finished and placed from what the player is shown). What happens once travels as effects
    (`WorldEffects`): every shot fired with its start, end, arc and flight time, every blast and every
    sound with a place, each to the players who see into a chunk it touches. The game flies the shots as
    `view::effects::ShotFlight`, the same flight replays show, if an end is in its sight, and hands blasts and
    sounds to the usual presentation. A view that carries only effects goes out too; the quiet minute of the
    start, with 48 machine-gun shots in the player's sight, costs 17 kB.
    Crystal mined away travels as cells and what is left in them (`MinedCrystal`): after every update the
    host notes the cells harvesters mine, by chunk, and each player is sent the cells of the chunks they see
    into that differ from what they were last sent. A newcomer's game grows the same fields from the seed,
    so they are sent every cell mined in the chunks they come to see, and nothing else. Enemy structures
    stay as last seen once out of sight, as long as the side's intel still knows them, as in any battle.
    What a side is told is news for that side (`sim::notifications::SideNews`: finished production, a
    promotion, a capture, a refusal, an attack on the base, low power, the credits of a delivered load...),
    written by the rules for every side; `tell_local_side` turns the local side's news into lines and
    sounds, so in a battle nothing changed. A world host tells nobody itself (`HostedWorld`) and sends each
    player their own side's news, which their game puts into words in its own language.
14. **Joining and leaving.** A new player gets a free starting site and a starting force; a returning player
    gets their side back wherever it stands. While its player is away a side cannot be attacked, and after a
    long absence its structures decay and its site is freed.
    A player enters with a key (`world_host::PlayerKey`, 128 random bits): the game makes one the first
    time it enters a server's world and keeps it in its data directory as `world_player_key`; a world played
    alone gets a fresh one, since it stops with the game. The host remembers which side each key commands.
    A key it knows gets its side back as long as anything of it stands (a unit, or a structure that keeps a
    side in the battle), otherwise a new side on a new site, as an unknown key does. Entering again while
    still connected elsewhere lets go of the older connection, which is told why. The keys live as long as
    the server runs, until persistence (step 15) keeps them.
15. **Persistence.** The world is saved by chunk, incrementally and often, so the server can stop and resume;
    the JSON save format gives way to a compact binary one.
16. **The world's own inhabitants.** Neutral sites and computer sides that keep the world from standing
    empty between players.

## Decisions

Taken on 2026-11-24:

- **The world grows** as players arrive, rather than having a fixed size (steps 8 to 10).
- **An absent player's base is protected and slowly decays**: no one can attack it while its player is away,
  and after a long absence it falls apart and frees its site (step 14).
- **Players fight each other**, alone or in alliances; computer sides are only there to fill the world.
- **The game runs a world too**, for playing alone or hosting friends: the same `WorldHost` on a thread of
  its own, its player connected through a channel instead of the network, so playing alone goes through the
  server's code. A world played alone stops when the game does and is saved for next time, so protection
  and decay only matter for the computer sides there. Missions and skirmishes keep running the simulation
  in the game itself.

Still open: how many players at once, which sets the server's tick budget and whether the world must later
be split across several simulations, and how long an absence is before a base starts to decay.
