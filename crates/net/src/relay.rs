use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use crate::network::{Arrival, Link, LobbyNews, MOST_SPECTATORS, Message};
use rimefront_sim::levels::LevelDefinition;
use rimefront_sim::sim::catalog::Team;
use rimefront_sim::sim::player_commands::PlayerCommand;

/// How long the hub holds a battle for a player whose connection dropped before giving up on them.
const REJOIN_WINDOW: Duration = Duration::from_secs(180);

/// The connections of one machine during a network battle, and on the hub everything a hub does: passing
/// each guest's messages on to the others, remembering every order, letting spectators in late and players
/// back after a dropped connection. It runs no simulation, so a dedicated server can run it as it is; see
/// docs/server.md.
pub struct Relay {
    links: Vec<Link>,
    /// The hub passes on what one guest says about the battle to every other guest.
    forwards: bool,
    /// Links to spectators; losing one does not stop the battle.
    watchers: BTreeSet<usize>,
    closed: BTreeSet<usize>,
    /// On the hub, which link carries which player's orders.
    link_teams: BTreeMap<usize, Team>,
    /// On the hub, players whose connection dropped, by team index, and since when the battle waits for them.
    away: BTreeMap<usize, (Team, Instant)>,
    door: Option<OpenDoor>,
    /// A player's connection dropped with no way back.
    broken: bool,
}

/// The hub keeps accepting guests during the battle and remembers every order to bring them up to date.
struct OpenDoor {
    level: LevelDefinition,
    players: Vec<Team>,
    /// Late guests considering the offer, with the seat they were offered; none to watch.
    newcomers: Vec<(Link, Option<Team>)>,
    history: TurnHistory,
}

#[derive(Default)]
struct TurnHistory {
    /// Keyed by team index; the latest tick of each team's turns.
    through: BTreeMap<usize, (Team, u64)>,
    turns: Vec<(Team, u64, Vec<PlayerCommand>)>,
}

impl TurnHistory {
    /// Each team's turns reach the hub in tick order, so the latest one closes every gap before it.
    fn record(&mut self, team: Team, tick: u64, commands: &[PlayerCommand]) {
        let latest = self.through.entry(team.index()).or_insert((team, tick));
        latest.1 = latest.1.max(tick);
        if !commands.is_empty() {
            self.turns.push((team, tick, commands.to_vec()));
        }
    }

    fn catch_up(&self) -> Message {
        Message::CatchUp {
            through: self.through.values().copied().collect(),
            turns: self.turns.clone(),
        }
    }
}

impl Relay {
    pub fn new(links: Vec<Link>, forwards: bool) -> Self {
        Self {
            links,
            forwards,
            watchers: BTreeSet::new(),
            closed: BTreeSet::new(),
            link_teams: BTreeMap::new(),
            away: BTreeMap::new(),
            door: None,
            broken: false,
        }
    }

    pub fn mark_watchers(&mut self, indices: impl IntoIterator<Item = usize>) {
        self.watchers.extend(indices);
    }

    /// Takes guests during the battle (see `welcome`). `remote_teams` are played over the links that do not
    /// lead to spectators, in order; `players` is every team people play, offered to late guests with `level`.
    pub fn open_door(&mut self, level: LevelDefinition, players: Vec<Team>, remote_teams: &[Team]) {
        let player_links = (0..self.links.len()).filter(|index| !self.watchers.contains(index));
        self.link_teams = player_links.zip(remote_teams.iter().copied()).collect();
        self.door = Some(OpenDoor {
            level,
            players,
            newcomers: Vec::new(),
            history: TurnHistory::default(),
        });
    }

    /// Whether the battle has lost a player for good.
    pub fn is_broken(&self) -> bool {
        self.broken
            || self
                .away
                .values()
                .any(|(_, since)| since.elapsed() > REJOIN_WINDOW)
    }

    /// Whether everyone has gone.
    pub fn is_deserted(&self) -> bool {
        self.closed.len() == self.links.len()
    }

    /// A player who lost the connection and how long the battle has waited for them.
    pub fn waiting_for_rejoin(&self) -> Option<(Team, Duration)> {
        self.away
            .values()
            .map(|&(team, since)| (team, since.elapsed()))
            .max_by_key(|&(_, waited)| waited)
    }

    pub fn record_turn(&mut self, team: Team, tick: u64, commands: &[PlayerCommand]) {
        if let Some(door) = &mut self.door {
            door.history.record(team, tick, commands);
        }
    }

    pub fn send_all(&mut self, message: &Message) {
        self.send_except(message, None);
    }

    fn send_except(&mut self, message: &Message, skipped: Option<usize>) {
        let lost: Vec<usize> = self
            .links
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != skipped && !self.closed.contains(index))
            .filter(|(_, link)| !link.send(message))
            .map(|(index, _)| index)
            .collect();
        for index in lost {
            self.lose_link(index);
        }
    }

    /// A spectator may go; a player's dropped connection holds the battle for a while on a hub that can
    /// take them back, and breaks it anywhere else.
    fn lose_link(&mut self, index: usize) {
        // A dropped connection is reported both by a failed send and by its reading thread.
        if self.closed.contains(&index) {
            return;
        }
        if self.watchers.contains(&index) {
            self.closed.insert(index);
            return;
        }
        match self
            .link_teams
            .remove(&index)
            .filter(|_| self.door.is_some())
        {
            Some(team) => {
                self.closed.insert(index);
                self.away.insert(team.index(), (team, Instant::now()));
            }
            None => self.broken = true,
        }
    }

    /// Reads every link and returns what concerns this machine; the hub passes what a guest says about the
    /// battle on to the other guests, remembers every turn, and answers pings itself.
    pub fn receive(&mut self) -> Vec<Message> {
        let mut for_this_machine = Vec::new();
        for index in 0..self.links.len() {
            for arrival in self.links[index].drain() {
                let message = match arrival {
                    Arrival::Message(message) => message,
                    Arrival::ConnectionLost => {
                        self.lose_link(index);
                        continue;
                    }
                };
                let shared = matches!(
                    message,
                    Message::Turn { .. }
                        | Message::Checksum { .. }
                        | Message::Chat { .. }
                        | Message::Paint { .. }
                );
                if self.forwards && shared {
                    self.send_except(&message, Some(index));
                }
                match &message {
                    Message::Turn {
                        team,
                        tick,
                        commands,
                    } => self.record_turn(*team, *tick, commands),
                    Message::Ping { nonce } => {
                        self.links[index].send(&Message::Pong { nonce: *nonce });
                        continue;
                    }
                    _ => {}
                }
                for_this_machine.push(message);
            }
        }
        for_this_machine
    }

    /// A guest who connected during the battle: a player whose connection dropped is offered their seat
    /// back, anyone else to watch while there is room; without room the link is dropped.
    pub fn welcome(&mut self, link: Link) {
        let watching = self.watchers.len() - self.closed.intersection(&self.watchers).count();
        let Some(door) = &mut self.door else { return };
        let free_seat = self.away.values().map(|&(team, _)| team).find(|&team| {
            door.newcomers
                .iter()
                .all(|(_, offered)| *offered != Some(team))
        });
        let spectator_room = watching + door.newcomers.len() < MOST_SPECTATORS;
        if free_seat.is_some() || spectator_room {
            link.offer_under_way(&door.level, free_seat, &door.players);
            door.newcomers.push((link, free_seat));
        }
    }

    /// Whether a guest arriving now would be let in.
    pub fn has_room(&self) -> bool {
        let watching = self.watchers.len() - self.closed.intersection(&self.watchers).count();
        self.door.as_ref().is_some_and(|door| {
            !self.away.is_empty() || watching + door.newcomers.len() < MOST_SPECTATORS
        })
    }

    /// Once a late guest accepts, they get every order so far and join the battle.
    pub fn admit_late_guests(&mut self) {
        let Some(door) = &mut self.door else { return };
        let mut admitted = Vec::new();
        let mut index = 0;
        while index < door.newcomers.len() {
            let news = door.newcomers[index].0.lobby_news();
            if news.contains(&LobbyNews::Left) {
                door.newcomers.remove(index);
            } else if news.contains(&LobbyNews::GuestReady) {
                let (link, seat) = door.newcomers.remove(index);
                link.send(&door.history.catch_up());
                link.go();
                admitted.push((link, seat));
            } else {
                index += 1;
            }
        }
        for (link, seat) in admitted {
            let index = self.links.len();
            match seat {
                Some(team) => {
                    self.link_teams.insert(index, team);
                    self.away.remove(&team.index());
                }
                None => {
                    self.watchers.insert(index);
                }
            }
            self.links.push(link);
        }
    }
}
