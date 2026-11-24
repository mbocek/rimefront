# Towards a persistent world

The goal: one world that runs permanently on a server, far larger than a match map, where players join, are
given a starting site, build up, leave and come back. This page is the series of steps that gets there. Each
step is one commit and is worth having on its own, for ordinary matches too.

## Where we start

- The map is a dense grid sized once per match: `MapSize` (64, 96, 128 or 256 cells of 2 m) sets the
  thread-local `map_cells()`, and `GameMap`, the fog of war, the minimap and the fog texture are each one
  `Vec` of `map_cells()²` entries.
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

## Why lockstep cannot carry it

A permanent world never ends, so its order history grows without limit, and a joining player cannot replay it.
Every client would also have to simulate the whole world, however far from their own units. The world therefore
moves to a simulation on the server that sends each client only the part of the world it can see. The
deterministic simulation stays: it keeps the tests, replays and the fingerprints honest, and lockstep remains
for ordinary network matches.

## Steps

### A. Cost follows activity, not area

1. **Vast map size (256 cells).** Done: `MapSize::Vast`, in the random battle and battle editor.
2. **Spatial index of units.** A grid of buckets (a few cells each) rebuilt once per tick, iterated in
   `SimId` order; target choice, separation and the other searches for nearby units ask it for neighbours instead of
   scanning every unit. Results stay identical, so the fingerprints must not change.
3. **Map bounds instead of a size.** Replace `map_cells()` with a `MapBounds` (minimum and maximum cell) that
   code asks for its extent, and stop assuming the map starts at cell zero or is square. The prerequisite for a
   world that can grow.
4. **Chunked grids.** `GameMap`, `FogOfWar` and the crystal amounts are stored in chunks of 32 × 32 cells
   behind the same per-cell API; the fog recomputes only chunks a viewer's sight touches, and remembers which
   chunks changed so the fog texture uploads only those.
5. **Terrain and scenery in chunks.** One terrain mesh per chunk with coarser levels of detail by distance,
   culled outside the view; scenery and crystal meshes merged per chunk instead of per map. The far ring stays
   as the backdrop beyond the world's edge.
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
