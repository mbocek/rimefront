# Determinism and multiplayer

Multiplayer will keep every machine in step by exchanging only orders, so the simulation must play out
identically from the same orders. All chance in the simulation (artillery scatter, the computer's unit
choices, where new units gather) comes from one generator seeded by the level. Units and structures get a
simulation id in the order the simulation spawned them, because Bevy's entity ids also count effects, which
differ from machine to machine. Schedules run on one thread in a fixed order, in the game and in the tests.
The battle advances in fixed 50 ms ticks (20 per second, the same step the tests use) whatever the frame rate;
units and projectiles are drawn part way between their last two ticks, and get their exact simulated
transform back before each tick, so drawing never changes the battle.
Presentation adds its own components to units at moments that differ from machine to machine, which
changes the order in which Bevy's queries visit them; so every simulation step whose outcome depends on that
order (firing, projectile hits, destruction, harvesting, healing, repairs, engineers, the computer's army
and turret walls, choosing a producer, a refinery, a healer's patient, an aircraft's helipad, a service
station or the nearest known enemy structure) goes through `in_simulation_order` or a tie-break on
position. `determinism::simulation_checksum` fingerprints units, structures and credits. One test plays
the same computer-against-computer battle twice for eight minutes and compares the fingerprints every 15
seconds; another does the same while reshuffling the storage of units, structures and projectiles in one
of the two battles every second; a third replays the battle in a process that already played a short match,
since every rhythm of the simulation (fog and intel updates, the computer's periodic plans) lives in
`SimulationCadences`, which each match starts afresh.

Everything the player does to the battle (orders to units, rally points, engineers, production and its
cancelling, placing structures and walls, repairing and selling) becomes a `PlayerCommand`, which names
units and structures by simulation id and is carried out at the start of a later tick
(`COMMAND_DELAY_TICKS`). The simulation checks that a team only commands its own units and structures.
Selecting, the camera and the menus stay local. Feedback such as the acknowledgement sound and the order
marker is immediate; the change itself happens one tick (50 ms) later. These commands are what machines
exchange in a networked match.

In a networked match (`network`) both machines run the whole battle. For every tick each sends a turn with
its player's commands for the tick four ahead (200 ms, `NETWORK_COMMAND_DELAY_TICKS`), and the simulation
takes a tick only once the other player's turn for it has arrived, so neither machine can run ahead. Messages
travel over a WebSocket (`tungstenite`; plain `ws://` on the host's port, `wss://` to a server behind TLS): each message is one binary frame
(`link::encode`): JSON for the messages that show the sender's version or a refusal (`Offer`, `OpenRoom`,
`EnterWorld`, `Refused`), so a game of another version still reads why it is turned away, and postcard, a
compact binary form marked by a first byte of 0, for everything after; sides travel as their number there
and by name in JSON. A compact message of 256 bytes or more is deflated (first byte 1) when that makes it
smaller, as a late spectator's catch-up or a full first view; unpacking stops at the largest message a peer
may send, so a small message cannot unpack into a flood. The host answers the handshake and a guest opens it, both on the link's own
thread, and messages sent before it finishes wait for it. Every second each machine sends the checksum of its battle; differing
checksums end the match as out of step, and a lost connection stops it. Tests join two battles in one process
over a real local WebSocket connection: their orders reach each other, the battles stay identical, a deliberate
drift is noticed by both, and a dropped connection halts the other side.

Each machine shows the battle from its own player's side (`LocalTeam`, read through `LocalView`): the fog
of war, hidden enemies, the minimap, sounds, health bars, the sidebar's credits, power and production,
selection, notifications, the camera's home and the victory or defeat banner all follow the local team,
while the simulation keeps every team's view and stays the same on both machines. If the connection drops
or the battles drift apart, the player is told once and the battle stops.

Up to four people can play: the host sets *Network players* (2 to 4) in the battle editor, the guests
take the Syndicate corners in the order they join, and the computer plays any corner left over. A
four-player battle with the editor's default alliance becomes a free-for-all; *Two against two* pairs the
host with the Amber player. Every guest connects to the host, which passes the guests' turns, checksums and
chat on to each other; a tick runs once the turns of every player have arrived. To play, everyone opens
*Network battle* in the main menu. The host clicks *Host a battle*: the battle
editor's settings with one opponent are used, the game listens on port 47300 and shows this computer's
local address. The other player types that address (the port may be left out; a `ws://host:port/path` URL
works too, the path naming a room on the planned server, [server.md](server.md); the last address is
remembered) and clicks *Join*. Once connected, the host offers the battle and the guest reads it (objective,
bases, map, landscape, season, available units) before clicking *I'm ready*; both need the same game
version. When every guest is ready the host clicks *Start the battle*; all machines count down three seconds and
the battle begins, the host leading the faction chosen in the battle editor and the guests the other one.
Anyone who joins while every seat is taken is offered to watch instead: a spectator sees the whole map without
fog, follows everyone's orders and gives none, has no build menu or chat, and leaving does not stop the battle.
Spectators can also join a battle under way: the host keeps its port open, offers the battle to watch, and on
*I'm ready* sends every order so far (each team's turns, complete through their latest tick, listing only those
with orders). The spectator's machine then runs the battle silently at 40x speed (up to two seconds of battle per frame, past Bevy's default
250 ms limit that would otherwise hold it to about 15x), with a *Catching up* banner,
until it reaches the players; being deterministic, it arrives at exactly their battle. At most four people watch.
When a guest's connection drops, the host holds the battle for up to three minutes with a banner counting the
wait, and offers the next person who connects that player's seat back. Catching up works as for a spectator,
except that the returning player then sends orders again, starting after the last turn the host already had:
sending the earlier turns once more would run their orders twice on the host. If the host's own connection
drops, the battle ends, as the host is the hub every other machine talks through. Every machine also records the battle as a replay. Either side can leave the lobby: a host whose guest leaves goes back to waiting
for another player. While hosting, the network page shows the battle editor's settings (all but the number
of players), and the host can still change them: every guest at once sees the new battle, is told the host
changed it, and has to click *I'm ready* again. Leaving or quitting a battle closes the connection at once, so the other player is told
straight away. During a network battle the round trip to the other machine (measured every second
with a ping, smoothed) shows in the corner of the battlefield, and when the battle has waited more than half
a second for the other player's orders a banner says so and counts the seconds. The players can chat: in the lobby,
once connected, typing goes into a message line and Enter sends it; in the battle Enter opens a *Say:* line
(the keyboard then types instead of commanding), Enter sends and Esc drops it, and lines appear among the
notifications. Lines are trimmed to 120 printable characters. Restarting, saving and loading are disabled during a
network battle; pausing one machine makes the other wait. Over the internet the host's port 47300 must be
forwarded. Capture runs can automate this (`IRON_TIDE_CAPTURE_HOST=1`, `IRON_TIDE_CAPTURE_JOIN=127.0.0.1`).

Both machines must compute floating point identically, also across processors and operating systems. Basic
arithmetic and square roots are exact under IEEE 754 everywhere, but Rust's own `sin`, `atan2` and `powf`
call each platform's maths library, whose last bit differs, and SIMD quaternion maths may round differently
on x86 and ARM. The simulation therefore takes those functions from the portable `libm` crate
(`sim_math`) and keeps positions and directions in scalar `Vec3`; rotations it writes serve only the
drawing. `tests/cross_platform.rs` replays a fixed computer-against-computer battle and compares its
fingerprints with `tests/fingerprints.txt`, recorded on an Apple Silicon Mac; CI runs the test on x86
Linux. When the rules change on purpose, record them again with
`UPDATE_FINGERPRINTS=1 cargo test --test cross_platform`.

Replays are not re-simulated: they store snapshots of every unit and structure twice a second and redraw the
battlefield from the level. Changed combat or economy rules therefore leave old replays intact; what breaks
them is a change to map generation (terrain, crystals, scenery placed from the seed) or to the replay format
(`REPLAY_VERSION` in `src/app/replay.rs`). Saves and network battles, in contrast, need the same rules.
