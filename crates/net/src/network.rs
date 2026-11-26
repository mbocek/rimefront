use std::collections::{BTreeMap, BTreeSet};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use bevy::prelude::*;

use rimefront_sim::match_flow::GameState;
use serde::{Deserialize, Serialize};

use crate::relay::Relay;
use crate::world_host::PlayerKey;
use crate::world_view::ViewChange;
use rimefront_sim::i18n::{tr, tr_fmt};
use rimefront_sim::levels::LevelDefinition;
use rimefront_sim::sim::ai::AiCommanders;
use rimefront_sim::sim::catalog::{SideColor, Team};
use rimefront_sim::sim::determinism::simulation_checksum;
use rimefront_sim::sim::notifications::Notifications;
use rimefront_sim::sim::player_commands::{
    CommandQueue, CommandTiming, LocalTeam, PlayerCommand, ScheduledCommand, SimulationTick,
};
use rimefront_sim::sim::sides::PerSide;

pub use crate::link::{Arrival, Dialling, JoinTarget, Link, Side};

/// Commands wait this many ticks (200 ms) before they run, so they reach every other machine in time.
pub const NETWORK_COMMAND_DELAY_TICKS: u64 = 4;
const CHECKSUM_EVERY_TICKS: u64 = 20;
/// A late spectator's catch-up carries every order of the battle so far.
pub(crate) const LARGEST_MESSAGE_BYTES: usize = 1 << 24;
/// At most this many people watch one battle.
pub const MOST_SPECTATORS: usize = 4;
const PING_INTERVAL: Duration = Duration::from_secs(1);
const PINGS_REMEMBERED: u64 = 10;
const ROUND_TRIP_SMOOTHING: f32 = 0.3;
const LONGEST_CHAT_LINE: usize = 120;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Message {
    /// What one team's player ordered for one tick, possibly nothing.
    Turn {
        team: Team,
        tick: u64,
        commands: Vec<PlayerCommand>,
    },
    /// The fingerprint of one machine's battle after a tick, sent as soon as it is known so a drift is
    /// noticed even when the machine that finds it first stops at once.
    Checksum {
        team: Team,
        tick: u64,
        sum: u64,
    },
    /// The host's battle, the team this guest will play (none for a spectator), and every team people play.
    Offer {
        version: String,
        level: Box<LevelDefinition>,
        your_team: Option<Team>,
        players: Vec<Team>,
        /// The battle has already begun: the guest can only watch, after catching up.
        #[serde(default)]
        under_way: bool,
    },
    /// Every order of a battle under way, for a spectator who joins late: each team's turns are complete
    /// through its tick in `through`, and only turns with orders are listed.
    CatchUp {
        through: Vec<(Team, u64)>,
        turns: Vec<(Team, u64, Vec<PlayerCommand>)>,
    },
    /// The guest accepts the offered battle.
    Ready,
    /// The host starts the countdown to the battle.
    Go,
    /// Asks the other machine to answer at once, to measure the round trip.
    Ping {
        nonce: u64,
    },
    Pong {
        nonce: u64,
    },
    /// The colour a player chose for their side's models; it changes nothing in the battle.
    Paint {
        team: Team,
        color: SideColor,
    },
    /// A line a player typed, in the lobby or during the battle.
    Chat {
        team: Team,
        text: String,
    },
    /// To a dedicated server: the first player in a room sets its battle for `players` people in all.
    OpenRoom {
        version: String,
        level: Box<LevelDefinition>,
        players: usize,
    },
    /// From a dedicated server to the player who opened the room: how many guests have a seat and are ready.
    RoomStatus {
        seated: usize,
        ready: usize,
    },
    /// To a dedicated server: the player who opened the room starts the battle.
    Start,
    /// To a world: this player wants to enter it.
    EnterWorld {
        /// The game's version, for the record; whether it may enter is the protocol's to say.
        version: String,
        /// `WORLD_PROTOCOL` of the game; 0 from a game that came before it.
        #[serde(default)]
        protocol: u32,
        player: PlayerKey,
        /// The colour the player chose for their side; `None` leaves it to the world.
        #[serde(default)]
        color: Option<SideColor>,
    },
    /// From a world: the side the player commands there, and the world's tick when they entered.
    Entered {
        team: Team,
        tick: u64,
        map_seed: u64,
    },
    /// From a world: what changed in what its player sees.
    WorldView(Box<ViewChange>),
    /// To a world: what its player ordered; the world decides when it runs.
    Orders {
        commands: Vec<PlayerCommand>,
    },
    /// The other end will not have this connection, and why.
    Refused {
        reason: String,
    },
}

impl Message {
    /// Whether this message is one that tells the other end which version sent it, or why it is refused.
    pub fn shows_the_version(&self) -> bool {
        matches!(
            self,
            Message::Offer { .. }
                | Message::OpenRoom { .. }
                | Message::EnterWorld { .. }
                | Message::Refused { .. }
        )
    }
}

/// The battle a guest was offered.
#[derive(Clone, PartialEq, Debug)]
pub struct Offer {
    pub level: LevelDefinition,
    /// None when every seat is taken and the guest may only watch.
    pub your_team: Option<Team>,
    pub players: Vec<Team>,
    pub under_way: bool,
}

/// What happened in the lobby on one connection since the last look.
#[derive(Clone, PartialEq, Debug)]
pub enum LobbyNews {
    /// The host's battle; `Err` carries the host's version when it differs from this one.
    Offered(Result<Box<Offer>, String>),
    GuestReady,
    Go,
    Chat(Team, String),
    /// From a dedicated server, to the player who opened the room: guests seated and ready.
    RoomStatus {
        seated: usize,
        ready: usize,
    },
    Refused(String),
    Left,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionState {
    InStep,
    /// The machines computed different battles at this tick; play cannot continue.
    Desynced(u64),
    Disconnected,
}

/// Which ticks' turns have arrived from one team, kept as the tick every turn up to which is known plus the few
/// that arrived ahead of a gap.
#[derive(Clone)]
struct ReceivedTurns {
    all_through: u64,
    ahead: BTreeSet<u64>,
}

impl Default for ReceivedTurns {
    fn default() -> Self {
        Self {
            all_through: NETWORK_COMMAND_DELAY_TICKS - 1,
            ahead: BTreeSet::new(),
        }
    }
}

impl ReceivedTurns {
    fn has(&self, tick: u64) -> bool {
        tick <= self.all_through || self.ahead.contains(&tick)
    }

    fn latest(&self) -> u64 {
        self.ahead.last().copied().unwrap_or(self.all_through)
    }

    fn record(&mut self, tick: u64) {
        if !self.has(tick) {
            self.ahead.insert(tick);
            self.close_gaps();
        }
    }

    fn record_all_through(&mut self, last: u64) {
        self.all_through = self.all_through.max(last);
        self.ahead.retain(|&tick| tick > last);
        self.close_gaps();
    }

    fn close_gaps(&mut self) {
        while self.ahead.remove(&(self.all_through + 1)) {
            self.all_through += 1;
        }
    }
}

/// A battle with players on other machines: every machine runs the whole simulation and they exchange only
/// commands. A guest has one link, to the hub; the hub (the host) has one per guest and passes messages
/// between them, see `Relay`.
#[derive(Resource)]
pub struct NetSession {
    relay: Relay,
    remote_teams: Vec<Team>,
    /// This machine only watches: it sends no orders and no fingerprints.
    spectating: bool,
    remote_turns: PerSide<ReceivedTurns>,
    local_checksums: BTreeMap<u64, u64>,
    remote_checksums: BTreeMap<(u64, usize), u64>,
    pub state: SessionState,
    reported: Option<SessionState>,
    pings_in_flight: BTreeMap<u64, Instant>,
    incoming_chat: Vec<(Team, String)>,
    incoming_paints: Vec<(Team, SideColor)>,
    next_ping: u64,
    last_ping_sent: Option<Instant>,
    round_trip: Option<Duration>,
    /// On a machine that joined late, the latest tick of each team's orders the hub already had; a returning
    /// player must not send those turns again.
    turns_known_through: BTreeMap<usize, u64>,
    /// On a host in the game, where guests knock during the battle.
    door: Option<TcpListener>,
}

impl NetSession {
    /// `relays` is true for the host, which passes each guest's messages on to the other guests.
    pub fn new(links: Vec<Link>, remote_teams: Vec<Team>, relays: bool) -> Self {
        Self {
            relay: Relay::new(links, relays),
            remote_teams,
            spectating: false,
            remote_turns: Default::default(),
            local_checksums: BTreeMap::new(),
            remote_checksums: BTreeMap::new(),
            state: SessionState::InStep,
            reported: None,
            pings_in_flight: BTreeMap::new(),
            incoming_paints: Vec::new(),
            incoming_chat: Vec::new(),
            next_ping: 0,
            last_ping_sent: None,
            round_trip: None,
            turns_known_through: BTreeMap::new(),
            door: None,
        }
    }

    /// Keeps `listener` open during the battle so guests can still join, offered `level`.
    pub fn with_open_door(mut self, listener: TcpListener, level: LevelDefinition) -> Self {
        let players = std::iter::once(Team::Azure)
            .chain(self.remote_teams.iter().copied())
            .collect();
        self.relay.open_door(level, players, &self.remote_teams);
        self.door = Some(listener);
        self
    }

    /// The last tick for which every other player's orders are known; a machine far behind it catches up.
    pub fn known_through(&self) -> Option<u64> {
        self.remote_teams
            .iter()
            .map(|team| self.remote_turns.of(*team).latest())
            .min()
    }

    fn admit_late_guests(&mut self) {
        if let Some(listener) = &self.door
            && self.relay.has_room()
            && let Ok((stream, _)) = listener.accept()
            && stream.set_nonblocking(false).is_ok()
            && let Ok(link) = Link::host_side(stream)
        {
            self.relay.welcome(link);
        }
        self.relay.admit_late_guests();
        self.notice_lost_players();
    }

    /// On the host: a player who lost the connection and how long the battle has waited for them.
    pub fn waiting_for_rejoin(&self) -> Option<(Team, Duration)> {
        self.relay.waiting_for_rejoin()
    }

    fn notice_lost_players(&mut self) {
        if self.relay.is_broken() && self.state == SessionState::InStep {
            self.state = SessionState::Disconnected;
        }
    }

    /// A battle against one other machine over `stream`.
    pub fn with_one_player(
        stream: TcpStream,
        side: Side,
        remote_team: Team,
    ) -> std::io::Result<Self> {
        Ok(Self::new(
            vec![Link::open(stream, side)?],
            vec![remote_team],
            false,
        ))
    }

    /// A spectator's session: it follows the orders of every team in `players` and gives none.
    pub fn watching(links: Vec<Link>, players: Vec<Team>) -> Self {
        Self {
            spectating: true,
            ..Self::new(links, players, false)
        }
    }

    /// Marks the links at `indices` as spectators' on the host.
    pub fn with_watchers(mut self, indices: impl IntoIterator<Item = usize>) -> Self {
        self.relay.mark_watchers(indices);
        self
    }

    pub fn is_spectating(&self) -> bool {
        self.spectating
    }

    pub fn command_timing(&self) -> CommandTiming {
        CommandTiming {
            delay_ticks: NETWORK_COMMAND_DELAY_TICKS,
            spectating: self.spectating,
        }
    }

    pub fn remote_teams(&self) -> &[Team] {
        &self.remote_teams
    }

    /// Sends a chat line in `team`'s name to every other player; returns the cleaned line.
    pub fn say(&mut self, team: Team, text: &str) -> Option<String> {
        let text = chat_line(text)?;
        self.broadcast(&Message::Chat {
            team,
            text: text.clone(),
        });
        Some(text)
    }

    /// Shows every other player `team`'s models in `color`.
    pub fn paint(&mut self, team: Team, color: SideColor) {
        self.broadcast(&Message::Paint { team, color });
    }

    /// The colours other players chose for their sides since the last call.
    pub fn take_paints(&mut self) -> Vec<(Team, SideColor)> {
        std::mem::take(&mut self.incoming_paints)
    }

    /// Chat lines other players sent since the last call, with the team that sent each.
    pub fn take_chat(&mut self) -> Vec<(Team, String)> {
        std::mem::take(&mut self.incoming_chat)
    }

    /// The smoothed time a message takes to another machine and back; `None` until first measured.
    pub fn round_trip(&self) -> Option<Duration> {
        self.round_trip
    }

    fn ping_if_due(&mut self) {
        let due = self
            .last_ping_sent
            .is_none_or(|sent| sent.elapsed() >= PING_INTERVAL);
        if !due {
            return;
        }
        let nonce = self.next_ping;
        self.next_ping += 1;
        let now = Instant::now();
        self.last_ping_sent = Some(now);
        self.pings_in_flight.insert(nonce, now);
        self.pings_in_flight
            .retain(|&sent, _| sent + PINGS_REMEMBERED > nonce);
        self.broadcast(&Message::Ping { nonce });
    }

    fn answered(&mut self, nonce: u64) {
        let Some(&sent) = self.pings_in_flight.get(&nonce) else {
            return;
        };
        let measured = sent.elapsed();
        self.round_trip = Some(match self.round_trip {
            Some(previous) => {
                previous.mul_f32(1.0 - ROUND_TRIP_SMOOTHING)
                    + measured.mul_f32(ROUND_TRIP_SMOOTHING)
            }
            None => measured,
        });
    }

    /// Whether every other player's commands for `tick` have arrived; the first ticks need none.
    fn has_turns_for(&self, tick: u64) -> bool {
        tick < NETWORK_COMMAND_DELAY_TICKS
            || self
                .remote_teams
                .iter()
                .all(|team| self.remote_turns.of(*team).has(tick))
    }

    fn broadcast(&mut self, message: &Message) {
        self.relay.send_all(message);
        self.notice_lost_players();
    }

    /// Once both fingerprints of a tick are known they are compared; the remote one is then forgotten.
    fn compare_checksums(&mut self, tick: u64, team: usize) {
        let (Some(&local), Some(&remote)) = (
            self.local_checksums.get(&tick),
            self.remote_checksums.get(&(tick, team)),
        ) else {
            return;
        };
        self.remote_checksums.remove(&(tick, team));
        if local != remote && self.state == SessionState::InStep {
            self.state = SessionState::Desynced(tick);
        }
    }

    /// Takes in what the other machines sent about the battle.
    fn receive(&mut self, queue: &mut CommandQueue) {
        for message in self.relay.receive() {
            match message {
                Message::Turn {
                    team,
                    tick,
                    commands,
                } => {
                    for command in commands {
                        queue.schedule(ScheduledCommand {
                            tick,
                            team,
                            command,
                        });
                    }
                    self.remote_turns.of_mut(team).record(tick);
                }
                Message::CatchUp { through, turns } => {
                    for (team, last) in through {
                        self.remote_turns.of_mut(team).record_all_through(last);
                        self.turns_known_through.insert(team.index(), last);
                    }
                    for (team, tick, commands) in turns {
                        for command in commands {
                            queue.schedule(ScheduledCommand {
                                tick,
                                team,
                                command,
                            });
                        }
                    }
                }
                Message::Checksum { team, tick, sum } => {
                    self.remote_checksums.insert((tick, team.index()), sum);
                    self.compare_checksums(tick, team.index());
                }
                Message::Chat { team, text } => {
                    self.incoming_chat
                        .extend(chat_line(&text).map(|text| (team, text)));
                }
                Message::Paint { team, color } => self.incoming_paints.push((team, color)),
                Message::Pong { nonce } => self.answered(nonce),
                Message::Ping { .. }
                | Message::EnterWorld { .. }
                | Message::Entered { .. }
                | Message::Orders { .. }
                | Message::WorldView(_)
                | Message::Offer { .. }
                | Message::Ready
                | Message::Go
                | Message::OpenRoom { .. }
                | Message::RoomStatus { .. }
                | Message::Start
                | Message::Refused { .. } => {}
            }
        }
        self.notice_lost_players();
    }
}

/// What may be shown of a chat line: printable characters, trimmed, at most `LONGEST_CHAT_LINE` of them.
pub(crate) fn chat_line(text: &str) -> Option<String> {
    let clean: String = text
        .chars()
        .filter(|character| !character.is_control())
        .take(LONGEST_CHAT_LINE)
        .collect();
    let clean = clean.trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

/// The simulation may take its next step only once every other player's commands for it are known.
pub fn simulation_may_advance(session: Option<Res<NetSession>>, tick: Res<SimulationTick>) -> bool {
    session.is_none_or(|session| {
        session.state == SessionState::InStep && session.has_turns_for(tick.0)
    })
}

pub fn receive_turns(session: Option<ResMut<NetSession>>, mut queue: ResMut<CommandQueue>) {
    if let Some(mut session) = session {
        session.receive(&mut queue);
    }
}

pub fn measure_latency(session: Option<ResMut<NetSession>>) {
    if let Some(mut session) = session
        && session.state == SessionState::InStep
    {
        session.ping_if_due();
    }
}

/// Sends this player's commands for the tick that is `NETWORK_COMMAND_DELAY_TICKS` ahead.
pub fn send_turn(
    session: Option<ResMut<NetSession>>,
    queue: Res<CommandQueue>,
    tick: Res<SimulationTick>,
    local: Res<LocalTeam>,
) {
    let Some(mut session) = session else { return };
    if session.spectating {
        return;
    }
    let turn = tick.0 + NETWORK_COMMAND_DELAY_TICKS;
    let already_sent = session
        .turns_known_through
        .get(&local.0.index())
        .is_some_and(|&known| turn <= known);
    if already_sent {
        return;
    }
    let commands = queue.scheduled_for(turn, local.0);
    session.relay.record_turn(local.0, turn, &commands);
    let message = Message::Turn {
        team: local.0,
        tick: turn,
        commands,
    };
    session.broadcast(&message);
}

pub fn admit_late_guests(session: Option<ResMut<NetSession>>) {
    if let Some(mut session) = session {
        session.admit_late_guests();
    }
}

/// How far a late spectator still has to run to reach the battle as it stands, as a share done; `None` when
/// it is caught up.
#[derive(Resource, Default)]
pub struct CatchingUp(pub Option<f32>);

pub fn fingerprint_tick(world: &mut World) {
    let tick = world.resource::<SimulationTick>().0;
    if !world.contains_resource::<NetSession>() || !tick.is_multiple_of(CHECKSUM_EVERY_TICKS) {
        return;
    }
    let sum = simulation_checksum(world);
    let team = world.resource::<LocalTeam>().0;
    let mut session = world.resource_mut::<NetSession>();
    session.local_checksums.insert(tick, sum);
    session
        .local_checksums
        .retain(|&checked, _| checked + CHECKSUM_EVERY_TICKS * 30 > tick);
    if !session.spectating {
        session.broadcast(&Message::Checksum { team, tick, sum });
    }
    let teams: Vec<usize> = session
        .remote_teams
        .iter()
        .map(|team| team.index())
        .collect();
    for team in teams {
        session.compare_checksums(tick, team);
    }
}

/// Tells the player once why the battle stopped.
pub fn report_session_trouble(
    session: Option<ResMut<NetSession>>,
    mut notifications: ResMut<Notifications>,
) {
    let Some(mut session) = session else { return };
    if session.state == SessionState::InStep || session.reported == Some(session.state) {
        return;
    }
    notifications.push(match session.state {
        SessionState::Disconnected => tr("Another player has left the battle").to_string(),
        SessionState::Desynced(tick) => {
            tr_fmt("Out of step with the other players at tick {}", &[&tick])
        }
        SessionState::InStep => return,
    });
    session.reported = Some(session.state);
}

/// Teams played by people on any machine get no computer commander.
pub fn retire_commanders_of_players(
    session: Option<Res<NetSession>>,
    local: Res<LocalTeam>,
    mut commanders: ResMut<AiCommanders>,
) {
    let Some(session) = session else { return };
    commanders.0.retain(|commander| {
        commander.team != local.0 && !session.remote_teams.contains(&commander.team)
    });
}

/// The lockstep session around the simulation: turns go out before a tick's rules, the state's fingerprint
/// after them, and the simulation waits for every player's turn. Without a session it does nothing.
pub struct LockstepPlugin;

impl Plugin for LockstepPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CatchingUp>()
            .configure_sets(
                FixedUpdate,
                rimefront_sim::FramePhase::Simulation.run_if(simulation_may_advance),
            )
            .add_systems(FixedFirst, receive_turns)
            .add_systems(
                PreUpdate,
                (receive_turns, measure_latency, admit_late_guests),
            )
            .add_systems(
                OnEnter(GameState::Playing),
                retire_commanders_of_players.in_set(rimefront_sim::MatchStart::Players),
            )
            .add_systems(
                FixedUpdate,
                send_turn.in_set(rimefront_sim::TickStep::SendOrders),
            )
            .add_systems(
                FixedUpdate,
                fingerprint_tick.in_set(rimefront_sim::TickStep::Fingerprint),
            );
    }
}

#[cfg(test)]
mod catching_up_tests {
    use super::*;

    #[test]
    fn turns_arriving_after_a_gap_count_once_the_gap_closes() {
        let mut turns = ReceivedTurns::default();
        let first = NETWORK_COMMAND_DELAY_TICKS;
        turns.record(first + 1);
        assert!(!turns.has(first));
        assert!(turns.has(first + 1));
        assert_eq!(turns.latest(), first + 1);
        turns.record(first);
        assert_eq!(turns.all_through, first + 1);
        assert!(turns.ahead.is_empty());
    }

    #[test]
    fn a_catch_up_marks_every_turn_through_its_last_tick() {
        let mut turns = ReceivedTurns::default();
        turns.record(900);
        turns.record_all_through(500);
        assert!(turns.has(0) && turns.has(500) && !turns.has(501));
        assert_eq!(turns.latest(), 900);
    }
}
