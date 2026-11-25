# Dedicated server

`rimefront-server` (`crates/net/src/bin/server.rs`, `make server`) is a dedicated server for network battles: every player
connects to it, so nobody forwards a port and no player's computer is the hub. Its battle rooms run no
simulation. Hosting from the game on a local network works as before. With `--world SEED` it also runs a
persistent world, entered at `ws://server:47300/world` from the game's network page (see
[persistent-world.md](persistent-world.md)). With `--world-dir DIR` the world is saved in `DIR` every 30 seconds
of world time (`--save-every SECS`) and resumed from there when the server starts again; `--world SEED` then
only grows a world when `DIR` holds none. Ctrl-C or a termination signal (`SIGTERM`, as a service manager
sends) saves the world before the server stops; a crash loses at most the time since the last save. A game
of another version enters as long as it speaks the same world protocol; updating a server, and what may
change between versions, is in [compatibility.md](compatibility.md).

## Playing through it

1. Run `rimefront-server --port 47300` somewhere every player can reach, plain or behind a proxy that adds TLS
   (see below); players then use `ws://server:47300/room/NAME` or `wss://domain/room/NAME`.
2. One player types `ws://server:47300/room/NAME` in the network page's address field, sets the battle in the
   battle editor's settings (number of players included) and clicks *Open this room on the server*. They play
   the host's side.
3. The others type the same address and click *Join*; they get seats in order, then spectator places.
4. When every seat is ready, the player who opened the room clicks *Start the battle*.

Late spectators and players whose connection dropped join the same address during the battle and catch up, as
with a host in the game.

## How it works

The simulation is deterministic lockstep ([determinism.md](determinism.md)): every machine runs the whole battle
and they exchange only orders, so a hub only handles messages. `Relay` (`src/net/relay.rs`) is that hub, used both by
a host in the game and by the server:

- passing each machine's turns, checksums and chat on to every other machine, and answering pings;
- remembering every order (`TurnHistory`: each team's turns, complete through their latest tick, listing only
  those with orders);
- taking guests during the battle (`welcome`): a player whose connection dropped is offered their seat back,
  anyone else a place to watch; on accepting they get the whole history and catch up at 40x speed;
- holding the battle up to three minutes for a dropped player.

The server (`src/net/server.rs`) adds rooms. A room is named by the path of the WebSocket request
(`Link::requested_path`). Before its battle a room gathers: whoever sends it a battle (`OpenRoom`) owns it,
whatever order the handshakes finish in; guests waiting for it are then offered seats, the owner hears how many
are seated and ready (`RoomStatus`), and its `Start` begins the battle once all are. The owner plays
`Team::Player`, the guests their seats, and the room's `Relay` takes over. A room closes when its owner leaves
before the battle, or when its battle loses a player for good or everyone has gone. Refusals (`Refused`) carry a
reason the game shows translated: no room in the address, a full room, a room name already open, another version.

`tests/server.rs` runs a server on a thread and plays through it from real lobbies: a duel, two rooms kept
apart, a late spectator and a returning player catching up, an address without a room refused, a player entering the server's world.

## Still to do

- **Checksums.** Each machine compares the checksums it receives with its own; the server sees all of them and
  could decide alone.
- **Limits.** A link takes no more messages once more than 256 kB wait for a reader that does not keep up
  (`MOST_UNSENT_BYTES`), and a world lets go of a player whose link refuses their view; they enter again
  when their connection recovers. Still open: rooms per server and idle rooms. The binary is small (10 MB) when built on its own,
  `cargo build --release -p rimefront_net --bin rimefront-server`; a workspace build gives it the game's
  Bevy features and the game's size.
- **A world on its own thread.** A world lives on the thread that made it (map queries are per thread), so
  `Server::with_world` must be called on the thread that steps the server.
- **The owner changing the battle** after opening the room, as a host in the game can.

## Deployment sketch

The server is one binary listening on one port; a reverse proxy adds TLS and a certificate:

```dockerfile
FROM rust:1.100 AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p rimefront_net --bin rimefront-server

FROM debian:stable-slim
COPY --from=build /src/target/release/rimefront-server /usr/local/bin/
EXPOSE 47300
CMD ["rimefront-server", "--port", "47300"]
```

```caddyfile
play.example.org {
    reverse_proxy rimefront-server:47300
}
```

Caddy passes WebSocket upgrades through by default, so players join `wss://play.example.org/room/K7Q2`.

The game speaks TLS itself (rustls with the ring backend, already built for the update check) and trusts the
public web's certificate authorities (`webpki-roots`), so an ordinary certificate on the proxy is enough.
A `wss://` address without a port means 443. A TLS link is read and written by one thread with a 5 ms read
timeout, as TLS keeps one state for both directions, so a message may wait up to 5 ms; plain links read and
write on separate threads with no wait. `tests/tls.rs` talks to a local `wss` server with a test certificate
(`tests/fixtures/tls`, a CA whose key was thrown away) and checks that an untrusted certificate is refused.

## Decided so far

- Rooms are joined by name in the address; the server does not list them.
- The server relays and keeps the history; checksums are still compared by the players.
- Hosting from the game stays alongside the server.
- Where it runs and who may open rooms is still open.
