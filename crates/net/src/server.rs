use std::collections::HashMap;
use std::net::TcpListener;
use std::time::Instant;

use crate::lobby::MOST_PLAYERS;
use crate::network::{Arrival, Link, MOST_SPECTATORS, Message};
use crate::relay::Relay;
use crate::world_host::WorldHost;
use crate::world_store::SaveReport;
use rimefront_sim::levels::LevelDefinition;
use rimefront_sim::sim::catalog::Team;

const ROOM_PATH: &str = "/room/";
const WORLD_PATH: &str = "/world";

/// A dedicated server: every player connects to it and it runs one `Relay` per room, so nobody forwards a
/// port and no player's computer is the hub. A room is named by the path players connect to,
/// `ws://host/room/NAME`; whoever sends it a battle first owns it. It may also run a persistent world, which
/// players enter at `ws://host/world`. See docs/server.md.
pub struct Server {
    listener: TcpListener,
    /// Connections whose handshake has not yet said which room they want.
    arriving: Vec<Link>,
    rooms: HashMap<String, Room>,
    world: Option<WorldHost>,
    last_step: Instant,
}

enum Room {
    Gathering(Gathering),
    Playing(Relay),
}

/// A room before its battle. Who owns it is decided by who sends the battle, not by who connects first:
/// a guest's handshake may well finish before the owner's.
#[derive(Default)]
struct Gathering {
    owner: Option<Link>,
    battle: Option<(LevelDefinition, Vec<Team>)>,
    seats: Vec<Seat>,
    /// Connections that have not opened the room, waiting for a battle to be offered.
    waiting: Vec<Link>,
    start_asked: bool,
    /// The owner left, which closes the room.
    closed: bool,
}

struct Seat {
    link: Link,
    /// None for a spectator.
    team: Option<Team>,
    ready: bool,
}

impl Server {
    pub fn new(listener: TcpListener) -> std::io::Result<Self> {
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            arriving: Vec::new(),
            rooms: HashMap::new(),
            world: None,
            last_step: Instant::now(),
        })
    }

    /// Runs a world grown from `map_seed` beside the rooms.
    pub fn with_world(listener: TcpListener, map_seed: u64) -> std::io::Result<Self> {
        Self::hosting(listener, WorldHost::new(map_seed))
    }

    /// Runs `world` beside the rooms.
    pub fn hosting(listener: TcpListener, world: WorldHost) -> std::io::Result<Self> {
        let mut server = Self::new(listener)?;
        server.world = Some(world);
        Ok(server)
    }

    /// Saves the world, if the server runs one kept in a directory.
    pub fn save_world(&mut self) -> std::io::Result<Option<SaveReport>> {
        self.world
            .as_mut()
            .filter(|world| world.is_kept())
            .map(WorldHost::save)
            .transpose()
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Takes in new connections and moves every room along; call it often.
    pub fn step(&mut self) {
        let elapsed = self.last_step.elapsed();
        self.last_step = Instant::now();
        if let Some(world) = &mut self.world {
            world.step(elapsed);
        }
        while let Ok((stream, _)) = self.listener.accept() {
            if stream.set_nonblocking(false).is_ok()
                && let Ok(link) = Link::host_side(stream)
            {
                self.arriving.push(link);
            }
        }
        for link in std::mem::take(&mut self.arriving) {
            match link.requested_path() {
                Some(path) => self.route(link, &path),
                None if link.has_failed() => {}
                None => self.arriving.push(link),
            }
        }
        self.rooms.retain(|_, room| match room {
            Room::Gathering(gathering) => gathering.step(),
            Room::Playing(relay) => {
                relay.receive();
                relay.admit_late_guests();
                !relay.is_broken() && !relay.is_deserted()
            }
        });
        let starting: Vec<String> = self
            .rooms
            .iter()
            .filter(|(_, room)| matches!(room, Room::Gathering(gathering) if gathering.starts()))
            .map(|(name, _)| name.clone())
            .collect();
        for name in starting {
            if let Some(Room::Gathering(gathering)) = self.rooms.remove(&name)
                && let Some(relay) = gathering.begin()
            {
                self.rooms.insert(name, Room::Playing(relay));
            }
        }
    }

    fn route(&mut self, link: Link, path: &str) {
        if path == WORLD_PATH {
            match &mut self.world {
                Some(world) => world.connect(Box::new(link)),
                None => link.refuse("This server runs no world"),
            }
            return;
        }
        let Some(name) = path.strip_prefix(ROOM_PATH).filter(|name| !name.is_empty()) else {
            link.refuse("Join a room: ws://server/room/NAME");
            return;
        };
        match self
            .rooms
            .entry(name.to_string())
            .or_insert_with(|| Room::Gathering(Gathering::default()))
        {
            Room::Gathering(gathering) => gathering.arrive(link),
            Room::Playing(relay) if relay.has_room() => relay.welcome(link),
            Room::Playing(_) => link.refuse("The battle in this room has no room for you"),
        }
    }
}

impl Gathering {
    fn arrive(&mut self, link: Link) {
        if self.battle.is_some() {
            self.seat(link);
        } else {
            self.waiting.push(link);
        }
    }

    fn seat(&mut self, link: Link) {
        let Some((level, players)) = &self.battle else {
            self.waiting.push(link);
            return;
        };
        let free_team = players
            .iter()
            .skip(1)
            .copied()
            .find(|team| self.seats.iter().all(|seat| seat.team != Some(*team)));
        let spectators = self.seats.iter().filter(|seat| seat.team.is_none()).count();
        if free_team.is_none() && spectators >= MOST_SPECTATORS {
            link.refuse("This room is full");
            return;
        }
        link.offer(level, free_team, players);
        self.seats.push(Seat {
            link,
            team: free_team,
            ready: false,
        });
    }

    fn report(&self) {
        let Some(owner) = &self.owner else { return };
        let seated = self.seats.iter().filter(|seat| seat.team.is_some()).count();
        let ready = self
            .seats
            .iter()
            .filter(|seat| seat.team.is_some() && seat.ready)
            .count();
        owner.report_room(seated, ready);
    }

    /// Whether the room still has anyone in it worth keeping.
    fn step(&mut self) -> bool {
        self.read_waiting();
        self.read_owner();
        let changed = self.read_seats();
        if changed {
            self.report();
        }
        !self.closed && (self.owner.is_some() || !self.waiting.is_empty())
    }

    /// A waiting connection that sends a battle becomes the owner, and the others get it offered.
    fn read_waiting(&mut self) {
        let mut index = 0;
        while index < self.waiting.len() {
            let mut gone = false;
            let mut opened = None;
            for arrival in self.waiting[index].drain() {
                match arrival {
                    Arrival::ConnectionLost => gone = true,
                    Arrival::Message(Message::OpenRoom {
                        version,
                        level,
                        players,
                    }) => opened = Some((version, level, players)),
                    Arrival::Message(Message::Ping { nonce }) => {
                        self.waiting[index].send(&Message::Pong { nonce });
                    }
                    Arrival::Message(_) => {}
                }
            }
            if gone {
                self.waiting.remove(index);
                continue;
            }
            let Some((version, level, players)) = opened else {
                index += 1;
                continue;
            };
            let link = self.waiting.remove(index);
            if version != env!("CARGO_PKG_VERSION") {
                link.refuse(&format!(
                    "The server runs version {}; all players need the same version",
                    env!("CARGO_PKG_VERSION")
                ));
                continue;
            }
            if self.owner.is_some() {
                link.refuse("A room with this name is already open; join it or pick another name");
                continue;
            }
            let players = Team::ALL[..players.clamp(2, MOST_PLAYERS)].to_vec();
            self.owner = Some(link);
            self.battle = Some((*level, players));
            for link in std::mem::take(&mut self.waiting) {
                self.seat(link);
            }
            self.report();
            index = 0;
        }
    }

    fn read_owner(&mut self) {
        let Some(owner) = &mut self.owner else { return };
        let mut chat = Vec::new();
        for arrival in owner.drain() {
            match arrival {
                Arrival::ConnectionLost => self.closed = true,
                Arrival::Message(Message::Chat { team, text }) => chat.push((team, text)),
                Arrival::Message(Message::Ping { nonce }) => {
                    owner.send(&Message::Pong { nonce });
                }
                Arrival::Message(Message::Start) => self.start_asked = true,
                Arrival::Message(_) => {}
            }
        }
        for (team, text) in chat {
            for seat in &self.seats {
                seat.link.relay_chat(team, &text);
            }
        }
    }

    /// Reads the guests; true when a seat filled, emptied or got ready.
    fn read_seats(&mut self) -> bool {
        let mut changed = false;
        let mut index = 0;
        while index < self.seats.len() {
            let mut left = false;
            for arrival in self.seats[index].link.drain() {
                match arrival {
                    Arrival::ConnectionLost => left = true,
                    Arrival::Message(Message::Ready) => {
                        self.seats[index].ready = true;
                        changed = true;
                    }
                    Arrival::Message(Message::OpenRoom { .. }) => {
                        self.seats[index].link.refuse(
                            "A room with this name is already open; join it or pick another name",
                        );
                        left = true;
                    }
                    Arrival::Message(Message::Chat { team, text }) => {
                        if let Some(owner) = &self.owner {
                            owner.relay_chat(team, &text);
                        }
                        for (other, seat) in self.seats.iter().enumerate() {
                            if other != index {
                                seat.link.relay_chat(team, &text);
                            }
                        }
                    }
                    Arrival::Message(Message::Ping { nonce }) => {
                        self.seats[index].link.send(&Message::Pong { nonce });
                    }
                    Arrival::Message(_) => {}
                }
            }
            if left {
                self.seats.remove(index);
                changed = true;
            } else {
                index += 1;
            }
        }
        changed
    }

    /// The owner has asked to start and every seat is taken and ready.
    fn starts(&self) -> bool {
        let Some((_, players)) = &self.battle else {
            return false;
        };
        let seated: Vec<&Seat> = self
            .seats
            .iter()
            .filter(|seat| seat.team.is_some())
            .collect();
        self.start_asked
            && seated.len() + 1 == players.len()
            && seated.iter().all(|seat| seat.ready)
    }

    /// Starts the battle: the owner plays `Team::Player`, every guest their seat, and the room's relay takes over.
    fn begin(self) -> Option<Relay> {
        let (level, players) = self.battle?;
        let owner = self.owner?;
        owner.go();
        let mut links = vec![owner];
        let mut remote_teams = vec![Team::Player];
        let mut watchers = Vec::new();
        for seat in self.seats {
            seat.link.go();
            match seat.team {
                Some(team) => remote_teams.push(team),
                None => watchers.push(links.len()),
            }
            links.push(seat.link);
        }
        let mut relay = Relay::new(links, true);
        relay.mark_watchers(watchers);
        relay.open_door(level, players, &remote_teams);
        Some(relay)
    }
}
