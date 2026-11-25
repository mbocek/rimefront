# Compatibility between versions

What a game of one version can share with a game, a server or a file of another, and what to do when a
change breaks that.

## Four kinds of compatibility

| What | Must match | Guarded by |
| --- | --- | --- |
| A network battle (lockstep) | the game version, exactly | the version in `Offer` and `OpenRoom`; `tests/fingerprints.txt` |
| A persistent world | `WORLD_PROTOCOL` | the protocol in `EnterWorld`; `crates/net/fixtures/world_protocol.txt` |
| A saved world | `STORE_VERSION`, or a way to read the one before | the layout at the head of `world.bin`; `crates/net/fixtures/world_layout.txt` |
| A saved game, a replay | `SAVE_VERSION`, `REPLAY_VERSION` | the version in the file ([determinism.md](determinism.md)) |

**A network battle** runs the whole simulation on every machine, so every rule has to be the same: any
two versions refuse each other, and a change to simulation results re-records the fingerprints.

**A persistent world** runs its simulation on the server alone; the game only mirrors what it is shown
(`MirroredBattle`). So the game and the world need not be the same version, only say the same things:
the messages after `EnterWorld` and everything they carry, the kinds of units, structures, sounds,
upgrades and news among them. That is the world protocol, `WORLD_PROTOCOL` in
`crates/net/src/world_host.rs`. A world lets in any game of its protocol, and turns away the others with a
reason the game shows on the network page: *update the game* when the game is older, *the server needs
updating* when it is newer. A patch release that changes rules, balance or the look of things leaves the
protocol alone and keeps playing on the same servers.

**A saved world** starts with its layout number, so a server recognises a world saved by another layout
before it reads anything else; it refuses a newer one, and an older one unless it has a way to read it.

## Changing the world protocol

The messages that show a version or a refusal (`Offer`, `OpenRoom`, `EnterWorld`, `Refused`) travel as JSON
and stay readable by every version: they only ever gain fields, each with `#[serde(default)]`, and keep the
ones they have. Everything after them travels in postcard, which writes fields by position and enum
variants by number, so any change to those messages or what they carry changes the protocol: a field added
or removed, a variant added anywhere, even at the end, or reordered.

`the_world_protocol_changes_only_with_its_number` (`crates/net/src/world_view.rs`) encodes one of every world
message and records the bytes, with how many kinds of units, structures, sounds, upgrades and stances there
are. When it fails:

1. Raise `WORLD_PROTOCOL` by one.
2. Re-record: `UPDATE_WORLD_PROTOCOL=1 cargo test -p rimefront_net the_world_protocol`.
3. Say in the release notes that servers and games of the release before cannot meet.

Variants added to `PlayerCommand`, `SideNews`, `Effect` or `BuildingKind`, whose counts the sample does not
record, need the same even when the test passes.

## Changing what a saved world holds

`the_layout_of_a_saved_world_changes_only_with_its_number` (`crates/net/src/world_store.rs`) does the same for
the world and chunk files. When it fails:

1. Raise `STORE_VERSION` by one.
2. Keep the structs of the layout before, renamed for it, and read them in `read_world_record`, turning
   them into the current ones, so worlds saved before still open.
3. Re-record: `UPDATE_WORLD_LAYOUT=1 cargo test -p rimefront_net the_layout`.

A saved world reads `SavedUnit` and `SavedBuilding` (`sim::records`), so their fields belong to the layout
too.

No release has saved a world yet: the layouts before 6 and the protocols before 5 were only ever on the
way to the first, so they have no reader.

## Updating a server

1. Stop the server with Ctrl-C (or `SIGTERM`); it saves its world.
2. Copy the world's directory aside, so the release before can run it again.
3. Start the new server on the same `--world-dir`; it reads the world, in an older layout if it knows how,
   and saves it in its own from then on.

Players with a game of the server's protocol go on where they were; the others are told which of the two
needs updating.
