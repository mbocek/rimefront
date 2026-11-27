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
- Four fixed teams (`Team::Azure`, `Crimson`, `Amber`, `Violet`).
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

Recomputing one side's fog from 300 viewers (`tests/crowd_bench.rs`) once dimmed and compared the whole
grid, so with every chunk of a world opened it grew with the map: 0.15 ms on 256 × 256 cells, 0.49 ms on
1024 × 1024, 1.5 ms on 2048 × 2048, for every side five times a second. It now dims and compares only the
chunks that held a visible cell or hold one now, whole chunks at a time: 0.25, 0.21 and 0.19 ms. Painting the blurred fog texture, four
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
    themselves. `RIMEFRONT_CAPTURE_CLAIM_SITES=6` gives out sites two seconds into a capture run, to look
    at it.
11. **Sides as data.** Done. A `Team` is a side number; the four a level names keep their names
    (`Team::Azure` and the rest), and their look (name, colour, model tint) comes from a table that repeats
    past eight sides, the models having four tints. Saved text still names the four sides as before and
    numbers the others (`"Side4"`). The sides in a battle are the `Sides` resource; `sim::sides::admit_side`
    lets one more in, with fog and intel of its own, in a camp of its own: `Diplomacy` gives every side a
    camp, an alliance setting only fills in the four level sides, and `join_camp_of` makes allies.
    Economies, production, research, support power charge, match statistics and cadences keep a value for
    every side that needed one (`PerSide`), so a newcomer starts from the blank values (starting credits,
    uncharged powers); targets keep who sees them as a set of up to 256 sides. Simulation results are
    unchanged. Still four only: the seats of a network session and what a save stores per side.

### C. A world on a server

12. **The world runs on the server.** `rimefront-server` gains a world mode that runs `SimulationPlugin`
    headless (as the tests already do) and takes player commands from the network instead of relaying them.
    The simulation is a crate of its own (`crates/sim`, see [architecture.md](architecture.md)): it reaches
    into nothing of drawing, sound, the interface or the network, and the network session and saving add
    their systems through the steps of a tick and of a match's start. It builds on Bevy without rendering,
    windows or audio, and the network has a crate of its own on top of it (`crates/net`), so the dedicated
    server links no renderer.
    Done: `rimefront_net::world_host::WorldHost` runs a wilds world (`world_level`) without drawing, on the
    thread that made it, and takes its players through `WorldConnection`s: a network `Link`, or a
    `local_connection` within one process. A player sends `EnterWorld` and gets `Entered` with their side:
    the first takes the level's own side, everyone after a generated site of their own with the usual
    starting force (`match_flow::deploy_newcomer`); `Orders` from a player run for that player's side only,
    after the usual command delay. The level's opponents are the world's computer sides until step 16.
    `rimefront-server --world SEED` runs one beside the rooms, entered at `ws://host:port/world`; the server
    is 10 MB now that it links the simulation's systems, and with nobody connected and the computer sides
    building it used 0.16 s of processor time in 10 s. The game does not run a world yet: it needs the state
    a client is sent (step 13) to draw one.
13. **Areas of interest.** Each client is sent the state of the chunks its units and structures see: a full
    snapshot on entering a chunk, then changes per tick; the replay snapshots are the starting point of the
    format. Clients draw it with the existing interpolation; orders go to the server as today.
    Done in the protocol (`rimefront_net::world_view`): after every tick a world sends each player a
    `WorldView` of what changed in what their side sees, nothing when nothing did: the units and structures
    in the chunks within sight of their side's and allies' units and structures (`chunks_in_sight`) that the
    fog does not hide from them, each with its kind, side, position (hundredths of a cell), facing, health
    and a structure's origin; those gone from view; their credits; and the world's bounds when they widened.
    A player's `WorldMirror` applies them, and a test checks that after a minute it holds exactly what the
    world shows that side, the computer side's Command Center not among it. Measured in that test, as JSON
    at first: a minute of a quiet start cost 6 kB, and a tank driving 41 kB in 10 s, about 4 kB/s for every
    moving unit; the compact form below cut that.
    The game plays a world too (`net::world_client`): *Play a world alone* on the network page runs a
    `WorldHost` on a thread of the game's own, *Enter the server's world* connects to `ws://server/world`.
    Once the world lets the player in, the game starts a battle on the world's map and seed as a
    `MirroredBattle`: the simulation's rules do not run, only fog, intel and targets, and every tick the units
    and structures the world showed appear, move and go with the world's ids, so selection, orders, the fog
    and the minimap work as in any battle; the player's orders go to the world instead of the queue. A test
    (`tests/world_client.rs`) enters a world run on its own thread and drives a tank there;
    `RIMEFRONT_CAPTURE_WORLD=29` (or a server address) captures one.
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
    A side whose player has been away for `AbsenceRules::shelter_after` (2 minutes by default, so leaving does
    not get anyone out of a fight) is sheltered (`sim::sides::Shelter`) until the player returns: it is out
    of the fighting both ways. `Targets::hostile` is false between a sheltered side and anyone, so nothing
    picks its units or structures, splash and mines pass them by and its own weapons pick nothing; a target
    ordered before the shelter is dropped, a shell already flying at one does it no harm, and engineers
    cannot capture its structures. Vision is untouched, so others still see a sheltered base. Players are not
    yet shown which sides are sheltered.
    A sheltered side rests: when the shelter begins its units, harvesters aside, are sent to gather by its
    Command Center; its harvesters drop what they were mining, unload at the refinery and wait there; its
    production stops where it is. When the player returns the harvesters go back to mining on their own
    and production goes on; the units stay at home until ordered.
    Away past `decay_after` (3 days by default), every unit and structure of the side loses a share of its
    full health each tick, so that something unhurt is gone after `decay_over` (2 more days); the usual rules
    remove what reaches nothing. A player who returns stops the decay and can repair. Once nothing of a side
    stands, from decay or from a fight, it is retired: its key is forgotten, so its player starts anew, and
    the generated site it was deployed on is free for the next newcomer. Retiring a side (`sim::sides::
    retire_side`) takes away what is left of it (walls, mines) and its credits, production, research, support
    charge, statistics, intel and commander, and the next side to join takes the lowest retired number
    (`RetiredSides`, kept in a saved world), so a world holds 256 sides at once rather than over its life.
    The four sides a level names are not given out again, since the level put them in camps, and the
    level's own base is not given out again once its first player's side is gone.
15. **Persistence.** The world is saved by chunk, incrementally and often, so the server can stop and resume;
    the JSON save format gives way to a compact binary one.
    A world is kept in a directory (`world_store::WorldStore`): `world.bin` holds what is not kept by chunk
    (seed, tick, bounds, every side's credits, production lanes, research and support charge, the claimed
    starting sites, the computer commanders, and the host's players: keys, sides, sites, absences, shelter),
    and `chunks/` a file for every chunk with its crystal, units, structures, mines and neutral sites and
    which of its cells each side explored, one bit a cell. Units and structures are the records saved games
    use (`sim::records`). Everything is postcard, a compact binary serde format.
    A save writes only the chunks whose bytes changed since the last save, each to a new file named with
    the tick, then writes `world.bin` naming every chunk's file (to a staged file renamed over the old one),
    then removes the files no longer named; a save cut short at any point leaves the last whole save. Within
    a chunk, units and structures are in simulation order, so a chunk nothing happened in keeps its bytes.
    Measured in `world_host`'s test: a world widened by one newcomer, 36 chunks, saves in 43 kB; 5 seconds
    later, with a harvester working, the next save writes 3 chunks in 6 kB.
    `WorldHost::resume` starts a match on the saved seed without its forces (`RestoringSave`), widens it to
    the saved bounds, takes away what the seed put down, lays the saved crystal and spawns the saved things,
    sets the tick, and hands every player's side back to their key; nobody is connected, so everyone is
    away from the tick of the save on. Cells whose crystal differs from what the seed grows are told to
    players as mined, as their games grow the seed's fields. Shots in flight, the targets units had, and
    harvesters' errands are not kept: the shots are gone, the rest is taken up again.
    The dedicated server saves its world with `--world-dir DIR` (see [server.md](server.md)), and on Ctrl-C
    or a termination signal before it stops.
    Every message after the version check travels in postcard too (`link::encode`, see
    [determinism.md](determinism.md)), lockstep turns included. Measured in `world_host`'s test on the wire:
    the quiet first minute, with effects and side news, went from 16.8 kB as JSON to 2.1 kB, and a tank
    driving 10 s from 56 kB to 6.7 kB. A thing already shown that only moved, turned or was hurt then
    travels as the change from the last state sent (`Moved`: a step and a turn, a byte or two each, and its
    health), and credits only when they change: 1.9 kB and 5.1 kB. Of those 10 s, 191 views: 1.6 kB of
    moves, 1.5 kB of effects (each shot carries its flight, blast and sound), and about 8 bytes of header a
    view (tick, empty lists). The game keeps the last state of every thing shown to apply the moves to.
    Views then went out every second tick (`TICKS_PER_VIEW`), ten a second: effects, news and moves wait
    for the next one, and the game glides each unit from where it stands to where the view shows it over
    the ticks since the view before (at most four, so after a pause it catches up rather than trails), on top
    of the drawing between ticks every battle has. A quiet first minute costs 1.8 kB, a tank driving 10 s
    3.3 kB instead of 5.1 kB.
16. **The world's own inhabitants.** Neutral sites and computer sides that keep the world from standing
    empty between players.
    A world played alone is kept with the game's data, in `worlds/own` (`net::world_client::OwnWorld`), and
    entered with the game's own player key: *Play a world alone* resumes it when it is there and grows a new
    one otherwise. Its thread saves it every half minute and once more when the player leaves; leaving waits
    for that save, so the next game finds it (`tests/world_client.rs` drives a tank, leaves, comes back and
    finds it where it stopped).
    *New world alone* puts the kept world aside, in `worlds/own-previous` in place of the one put aside
    before, and grows a new one.
    Computer sides: the level's own opponent holds its base, and a newcomer with no computer side within
    80 cells of their site (a little more than the spacing of sites, so one site away at most) gets one on
    the free site nearest them: the usual starting force, a commander of normal difficulty and its income.
    A computer side with nothing left gives up its commander and its site. Every five minutes of world time
    each player present without a computer side near gets one, so a neighbour beaten is replaced after a
    while rather than at once; it is normal against a player with fewer than 8 structures that keep a side
    alive, hard up to 20 and brutal beyond. Commanders search for enemies nobody has found at the starting
    sites the world gave out as well as at the level's four bases, so in a world that has widened they find
    the players around them. The host keeps which sides are
    the computer's and where, and a saved world keeps that too. Computer commanders leave a sheltered side's
    structures out of their plans (attack waves, barrages, recon, engineers): `Targets` already keeps their
    shooters off them, and they would only stand there.
    Neutral sites: every square of sites holds one (`world::sites::neutral_site_in_square`), a Supply
    Depot, Field Hospital or Observation Post chosen from the map seed and the square, on the first of a few
    spots at least 24 cells from the square's starting site and clear of the level's bases whose ground is
    free. The host puts them down for the squares of the first world (its 64 cells hold none) and for every
    square the world widens by. A player is shown a neutral site, without a side, once their side has
    explored its ground, and keeps it shown until somebody takes it; computer engineers go for the ones
    their side explored, as in a battle. A saved world keeps them as any neutral site.
    What the player of a world sees of it: the objective is the world's own (`Objective::World`: nothing wins
    it), the clock shows the world's age from its tick, and once nothing of their side stands after it was
    shown standing, the battle is lost; the screen then offers only the main menu, since entering the world
    again gives them a new start. An attack or engineers ordered against a sheltered side are turned down
    with news to the one who ordered them (`SideNews::Sheltered`). Every player is also sent which sides are
    sheltered whenever that changes, and the game puts a glass dome over each one's structures it knows of,
    as wide as they spread and a little more (`view::shelter_dome`), gone when the side's player returns.
    New crystal: every ten minutes of world time a world grows a deposit for every four sides that mine (a
    refinery of theirs stands and they are not sheltered; at least one), while fewer deposits are fresh
    (crystal still lies within 6 cells of the middle) than one more than the sides that mine
    (`world::deposits::lay_new_deposit`). It is a field like those the world
    grew with, on open ground at least 16 cells from every structure and 4 from crystal already lying there,
    chosen from the map seed and the turn alone; on a fresh 64-cell world 57 of 60 turns found a spot. Every
    player present is told once a round that crystal surfaced (`SideNews::CrystalSurfaced`) but not where,
    and is shown it once their side sees its ground. A saved world keeps the deposits' middles.
    A side holds at most 120 units in a world (`UnitLimit`): a unit finished past that waits at its producer
    until the side loses one, and the side is told (`SideNews::UnitLimitReached`). Battles have no limit.
    A newcomer, or a player whose side fell, is given the nearest free site with nobody's units or
    structures within 30 cells, so they do not start again among the army that beat them.
    A unit destroyed in sight is sent as it falls (`Effect::Fell`: kind, side, spot, facing), so a world
    shows wrecks and fallen infantry as a battle does.
    The world saves in the background: the world's thread takes what it holds into chunk records, and a
    thread of its own encodes and writes them while the world goes on (`WorldStore::save_in_background`);
    the save when the server stops waits until it is written.

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

Taken on 2026-11-25, from the measurement below:

- **One simulation carries a world** of up to about 64 players; splitting it across several is not needed
  before the player counts outgrow what was measured.
- **A base starts to decay after three days away** and is gone two days later (`AbsenceRules`), long enough
  for a weekend away.

Taken on 2026-11-26:

- **A world goes round the year**, one season a day of world time, from summer (`Season::of_world_at`). The
  host and every player work the season out from the world's tick (the `f32` clock stops advancing after
  twelve days), so the protocol and the saved world
  carry nothing new. Snow and mud slow units on the host as in a battle, and the scenery, ground and sky
  turn while playing. A world's water never freezes: units could be standing on it when it thaws.
- **Players make alliances themselves.** A world is a free-for-all (it used to inherit a level's default
  camps, which made the second, third and fourth level sides allies). In the pause menu's *Game* page a
  player offers another player an alliance; once both have offered, they are allies (`sim::alliances`,
  `PlayerCommand::OfferAlliance`), and *Leave alliance* ends it for both. Allies see what each other sees
  and do not fight. Alliances and unanswered offers are saved with the world (layout 7, which still reads
  the worlds of layout 6 that release 2.0.0 saved), and each player is sent their allies and the sides
  players hold (protocol 8).
- **A player back after being away is told what happened**: how long they were gone and how many structures
  and units their side lost meanwhile (`SideNews::WelcomeBack`), once they were away long enough to be
  sheltered. The count starts when they leave and is not saved, so after a server restart they are told
  only how long they were away.

Taken on 2026-11-27 ([eras.md](eras.md#ages-of-a-world), from
[brainstorm-vekysveta-2026-11-26.md](../brainstorm-vekysveta-2026-11-26.md)):

- **A world goes through the ages**, from antiquity on, an age of a week by default turning at the twentieth
  hour of the world's day; every player is in the world's current age, a newcomer too. A side that arrives in the last quarter
  of an age has its first promotion for free, and an absent player's side is promoted from its own credits.

- **A player may give a nickname**, typed on the network page below the address (Tab moves between the two),
  sent as they enter a world (`EnterWorld::nickname`, which older games leave out) and kept with the world for
  its hall of fame. The world drops control characters and keeps at most 16 characters
  (`world_host::kept_nickname`); without one a player is "Player N". It is a name in a game, not an account:
  nobody checks it, and nothing else about the player is asked for or kept.

### Measured: a world with many players

`tests/world_bench.rs` lets players enter a world, each bringing a computer neighbour where none is near,
settles it for 30 s and measures 120 s of world time, all players connected and sent their views (Apple M5
Pro, release build). The tick budget is 50 ms.

| Players | Units | Structures | World | Tick, average | Tick, slowest | Sent to each player |
| --- | --- | --- | --- | --- | --- | --- |
| 4 | 35 | 40 | 320 × 320 | 0.05 ms | 0.24 ms | 67 B/s |
| 16 | 157 | 133 | 448 × 448 | 0.23 ms | 1.50 ms | 59 B/s |
| 32 | 294 | 236 | 576 × 576 | 0.44 ms | 2.83 ms | 49 B/s |
| 16, armies of 60 | 1237 | 128 | 448 × 448 | 1.84 ms | 3.57 ms | 239 B/s |
| 32, armies of 60 | 2393 | 222 | 576 × 576 | 3.73 ms | 8.24 ms | 256 B/s |
| 64, armies of 60 | 4692 | 463 | 832 × 832 | 8.51 ms | 36.53 ms | 434 B/s |

Sixty-four players were first measured at 19.1 ms a tick and 65.6 ms at worst, past the budget. Of that, the
views took 0.9 ms a tick and the simulation the rest: what grew with sides times the map or sides times
units. Four changes, with the battle's results unchanged:

- **The fog** dims only the chunks lit before (above) instead of the whole map, for each of 128 sides.
- **Intel** groups the eyes by side once instead of filtering every eye for every side.
- **Targets** ask only the sides with a visible cell in a target's chunk whether they see it
  (`Intel::viewers_seeing_into`), instead of every side's fog for every target.
- **Views** gather the world once a tick, by chunk (`WorldSnapshot`), and each player's view walks the
  chunks it sees into rather than every thing of the world.

Sixty-four players with 4700 units take a sixth of the budget on average; the slowest ticks come close to
it, which bounds a world's players for now. Views ten times a second and deflated long messages (step 13 above)
brought what each of them is sent from 803 B/s to 434 B/s: 64 players take about 28 kB/s of the server's
upload in all.

### Measured: days of a world

`rimefront-world-soak` runs a world as fast as it goes with 20 players who come and go: a computer commander
plays each one's side while they are connected; they stay one to six hours and are away for hours, half a
day to two days, or now and then three to six days, long enough for their base to decay; a player whose
side fell comes back ten minutes to two hours later. It writes a line of measurements for every hour of
world time (tick, units, structures, sides, crystal, what each player is sent, saves, memory). Six hours of
world 29 (Apple M5 Pro, release build), before and after the four fixes it led to:

| | Before | After |
| --- | --- | --- |
| Units after 5 hours | 2342 | 227 |
| Tick, average in hour 5 | 3.79 ms | 0.56 ms |
| Ticks over 50 ms in hour 5 | 20 | 0 |
| Save on the world's thread, average / slowest | 42.6 / 129 ms | 3.4 / 4.5 ms |
| Crystal cells after 3 hours, of about 2800 | 459 | 1030 |
| Real time for 5 hours of world | 544 s | 201 s |

- **Endless armies**: computer sides are paid an income and built without end; a side now holds at most 120
  units in a world.
- **Saves stalled the world**: a save took its records and wrote them on the world's thread; the records
  now read whole chunks at once and are written on a thread of their own.
- **Crystal ran out**: one deposit every ten minutes did not keep up with twenty players; a round now
  grows one for every four sides that mine, and the sides of players who are away no longer mine.
- **A side that fell started again among the army that beat it**: the first run counted 644 falls in five
  hours, the same sides falling every few minutes; a newcomer is now given a site nobody stands near.

Still open: one tick an hour or so takes 50 to 85 ms. Timing every phase of the slow ticks of six hours of
world 29 (2026-11-26) found only one cause: a newcomer for whom too few free sites are left widens the world
by a ring of squares on the spot (`claim_starting_site`), and generating the ring's chunks takes about 0.6 ms a
chunk: 17 ms for 32 chunks, 56 ms for 64, 46 ms for 96. The site search itself takes under half a
millisecond. The way out is to widen ahead of need and generate a few chunks a tick, which means a ring that
is not finished yet must stay closed, be saved half-made and be left out of the site search. Six hours of world take about four minutes, so ten days take about
three hours.

