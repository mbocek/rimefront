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
   the world must grow while it runs. Still whole: the fog texture's upload to the GPU (16 MB at 1024 cells,
   part of step 5) and the minimap's fog pass (step 7).
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
   - Still to do: upload only the changed chunks of the fog texture, and stream scenery the same way. Every
     prop is its own entity sharing a merged mesh, culled one by one, but a Vast map has about 123,000 of
     them and each is looked at every frame, so their cost grows with the area.
6. **Hierarchical pathfinding.** Regions labelled per chunk, chunks joined by portals on their borders; a route
   is planned over portals and refined by A* only inside the chunks it passes. A structure relabels only its own
   chunk.
7. **Minimap for a world.** The minimap shows the surroundings of the camera at a fixed scale, with a separate
   overview downsampled from the chunks.

### B. A world rather than a stretched map

8. **Generation by chunk.** The landscape (height, water, forests, crystal fields, roads) comes from a function
   of the world seed and the chunk coordinate, so any chunk can be generated on its own, in any order, with the
   same result. Replaces stretching the 64-cell landscapes for big maps; the authored ones stay for the campaign.
   Changes map generation, so the fingerprints and old replays.
9. **Starting sites across the world.** Generated sites, each with a crystal field and buildable ground nearby,
   spaced out; no mirror symmetry in world mode (duels keep it).
10. **Sides as data.** Replace the four fixed teams with a side id: name, colour, allies; the interface,
    intel, fog and AI keep one entry per side present.

### C. A world on a server

11. **The world runs on the server.** `iron-tide-server` gains a world mode that runs `SimulationPlugin`
    headless (as the tests already do) and takes player commands from the network instead of relaying them.
12. **Areas of interest.** Each client is sent the state of the chunks its units and structures see: a full
    snapshot on entering a chunk, then changes per tick; the replay snapshots are the starting point of the
    format. Clients draw it with the existing interpolation; orders go to the server as today.
13. **Joining and leaving.** A new player gets a free starting site and a starting force; a returning player
    gets their side back wherever it stands.
14. **Persistence.** The world is saved by chunk, incrementally and often, so the server can stop and resume;
    the JSON save format gives way to a compact binary one.
15. **The world's own inhabitants.** Computer sides and neutral sites that keep the world alive between
    players.

## Open questions

- What happens to a base while its player is offline: protected, slowly decaying, or fair game?
- A large but fixed world (for example 2048 × 2048 cells, 4 km a side), or one that grows as players arrive?
  Steps 3, 4 and 8 make both possible; the fixed one is far simpler to balance.
- Competitive, cooperative against computer sides, or both, and how many players at once? That sets the
  server's tick budget and whether the world must later be split across several simulations.
