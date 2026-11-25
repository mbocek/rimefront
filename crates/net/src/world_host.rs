use std::collections::HashMap;
use std::io;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use crate::network::{Arrival, Link, Message};
use crate::world_store::{HostingRecord, SaveReport, SavedWorld, WorldStore, restore};
use crate::world_view::{Effect, MinedCrystal, SentView, WorldEffects, WorldNews, WorldSnapshot};
use rimefront_sim::levels::{ActiveLevel, Difficulty, LevelDefinition, Objective};
use rimefront_sim::match_flow::{GameState, HostedWorld, RestoringSave, deploy_newcomer};
use rimefront_sim::sim::ai::{AiCommander, AiCommanders};
use rimefront_sim::sim::catalog::{SIDE_PALETTE, SideColor, Team};
use rimefront_sim::sim::components::{Building, Health};
use rimefront_sim::sim::determinism::run_schedules_on_one_thread;
use rimefront_sim::sim::economy::Economies;
use rimefront_sim::sim::neutral_sites::{footprint_is_free, spawn_unclaimed_site};
use rimefront_sim::sim::notifications::{News, SideNews};
use rimefront_sim::sim::player_commands::{CommandQueue, SimulationTick};
use rimefront_sim::sim::sides::{Shelter, retire_side, side_stands};
use rimefront_sim::world::deposits::lay_new_deposit;
use rimefront_sim::world::map::{GameMap, base_sites, map_bounds};
use rimefront_sim::world::sites::{free_starting_site, neutral_site_in_square, squares_within};
use rimefront_sim::world::terrain::Landscape;
use rimefront_sim::{SIMULATION_TICK, SimulationPlugin};

/// A connection to one player of a world: a network link, or a channel when the game runs the world itself.
pub trait WorldConnection: Send {
    /// False once the connection has gone.
    fn send(&self, message: &Message) -> bool;
    fn drain(&mut self) -> Vec<Arrival>;
}

impl WorldConnection for Link {
    fn send(&self, message: &Message) -> bool {
        Link::send(self, message)
    }

    fn drain(&mut self) -> Vec<Arrival> {
        Link::drain(self)
    }
}

/// One end of a connection within this machine, for a world the game runs on a thread of its own.
pub struct LocalConnection {
    outbox: Sender<Message>,
    inbox: Receiver<Message>,
}

/// Two ends of a connection within this machine: one for the player, one for the world.
pub fn local_connection() -> (LocalConnection, LocalConnection) {
    let (to_world, from_player) = channel();
    let (to_player, from_world) = channel();
    (
        LocalConnection {
            outbox: to_world,
            inbox: from_world,
        },
        LocalConnection {
            outbox: to_player,
            inbox: from_player,
        },
    )
}

impl WorldConnection for LocalConnection {
    fn send(&self, message: &Message) -> bool {
        self.outbox.send(message.clone()).is_ok()
    }

    fn drain(&mut self) -> Vec<Arrival> {
        let mut arrivals: Vec<Arrival> = self.inbox.try_iter().map(Arrival::Message).collect();
        if matches!(
            self.inbox.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ) {
            arrivals.push(Arrival::ConnectionLost);
        }
        arrivals
    }
}

/// The secret a player comes back to a world with, to command their side again; their game keeps it.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PlayerKey(String);

impl PlayerKey {
    pub fn new_random() -> Self {
        let mut bytes = [0u8; 16];
        SystemRandom::new()
            .fill(&mut bytes)
            .expect("the system has randomness");
        Self(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn from_saved(text: &str) -> Option<Self> {
        let text = text.trim();
        let well_formed = text.len() == 32 && text.chars().all(|c| c.is_ascii_hexdigit());
        well_formed.then(|| Self(text.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How a world treats the sides of players who are away.
#[derive(Clone, Copy, Debug)]
pub struct AbsenceRules {
    /// Away this long, a side is sheltered; long enough that leaving does not get anyone out of a fight.
    pub shelter_after: Duration,
    /// Away this long, a side's units and structures start to decay.
    pub decay_after: Duration,
    /// How long decay takes to wear down something unhurt.
    pub decay_over: Duration,
}

impl Default for AbsenceRules {
    fn default() -> Self {
        let day = Duration::from_secs(24 * 60 * 60);
        Self {
            shelter_after: Duration::from_secs(120),
            decay_after: 3 * day,
            decay_over: 2 * day,
        }
    }
}

/// The version of what a world and its players say to each other after the game enters it: the messages and
/// everything they carry, the kinds of units, structures, sounds and news included. A world lets in only a
/// game of the same protocol, whatever either's version, since the game only mirrors the world's battle.
/// `fixtures/world_protocol.txt` changes with it; see docs/compatibility.md.
pub const WORLD_PROTOCOL: u32 = 5;

/// Why a game speaking `protocol` cannot enter this world; `None` when it can.
fn protocol_refusal(protocol: u32) -> Option<&'static str> {
    match protocol.cmp(&WORLD_PROTOCOL) {
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Less => {
            Some("The world is newer than this game: update the game to enter it")
        }
        std::cmp::Ordering::Greater => {
            Some("The world is older than this game: its server needs updating")
        }
    }
}

/// Two sides' colours closer than this, as `SideColor::distance` measures, are too alike to tell apart.
const ALIKE_COLORS: u32 = 3 * 45 * 45;

/// `wanted` when it is apart enough from every colour `taken`; otherwise the colour of the palette furthest
/// from all of them.
fn distinct_color(wanted: Option<SideColor>, taken: impl Iterator<Item = SideColor>) -> SideColor {
    let taken: Vec<SideColor> = taken.collect();
    let nearest = |color: SideColor| {
        taken
            .iter()
            .map(|&other| color.distance(other))
            .min()
            .unwrap_or(u32::MAX)
    };
    if let Some(wanted) = wanted.filter(|&wanted| nearest(wanted) >= ALIKE_COLORS) {
        return wanted;
    }
    SIDE_PALETTE
        .iter()
        .map(|&(_, color)| color)
        .max_by_key(|&color| nearest(color))
        .unwrap_or(SIDE_PALETTE[0].1)
}

/// Players are sent a view every this many ticks: ten a second, half the headers and steps of one a tick,
/// and the game glides its units between them.
pub const TICKS_PER_VIEW: u64 = 2;

/// A newcomer whose site has no computer side this many cells away gets one beside them: a little more than
/// the spacing of sites, so a computer side is one site away at most.
const COMPUTER_NEIGHBOUR_REACH: f32 = 80.0;
const TICKS_BETWEEN_LOOKS_AT_COMPUTER_SIDES: u64 = 20;
/// How often a world may grow a new crystal deposit.
const TICKS_BETWEEN_DEPOSITS: u64 = 10 * 60 * 20;
/// A deposit counts as fresh while crystal lies this close to its middle.
const DEPOSIT_REACH: i32 = 6;

/// How often a player present without a computer side near is given one: rarely enough that beating one
/// leaves the player some peace first.
const TICKS_BETWEEN_COMPUTER_NEIGHBOURS: u64 = 5 * 60 * 20;
/// A computer side gets harder beside a player with this many structures that keep a side alive, and harder
/// again past the second.
const STRUCTURES_FOR_A_HARD_NEIGHBOUR: usize = 8;
const STRUCTURES_FOR_A_BRUTAL_NEIGHBOUR: usize = 20;

fn ticks_in(duration: Duration) -> u64 {
    (duration.as_secs_f64() / SIMULATION_TICK.as_secs_f64()).round() as u64
}

struct WorldPlayer {
    connection: Box<dyn WorldConnection>,
    /// None until the player has entered.
    team: Option<Team>,
    /// When the player entered, counted in entries to the world, so the latest of two on one side stays.
    entry: u64,
    sent: SentView,
}

/// A persistent world: the simulation of a wilds world run without drawing anything, its players' orders
/// taken from their connections. The dedicated server runs one, and the game runs one on a thread of its own
/// for a world played alone. Map queries are per thread, so a world lives on the thread that made it.
pub struct WorldHost {
    app: App,
    players: Vec<WorldPlayer>,
    level_side_taken: bool,
    sides_of_players: HashMap<PlayerKey, Team>,
    entries: u64,
    absence: AbsenceRules,
    /// The tick each side of a player who is not connected was left at.
    away_since: HashMap<Team, u64>,
    /// The generated site each newcomer's side was deployed on.
    sites: HashMap<Team, IVec2>,
    /// The computer sides that keep the world from standing empty, and the sites they hold.
    computer_sides: HashMap<Team, IVec2>,
    /// Every side's colour, apart enough from the others' to tell them by.
    colors: HashMap<Team, SideColor>,
    /// The middles of the crystal deposits the world grew since it began.
    deposits: Vec<IVec2>,
    effects: WorldEffects,
    mined: MinedCrystal,
    /// Effects since the last views went out.
    unshown_effects: Vec<Effect>,
    store: Option<KeptStore>,
}

/// Where a world is saved, how often, and when it last was.
struct KeptStore {
    store: WorldStore,
    every_ticks: u64,
    last_saved_tick: u64,
}

impl WorldHost {
    pub fn new(map_seed: u64) -> Self {
        let mut host = Self::hosting(Self::started(map_seed, false));
        host.place_neutral_sites(squares_within(map_bounds()));
        host
    }

    /// Puts the neutral site of each square down on the first of its spots with free ground.
    fn place_neutral_sites(&mut self, squares: Vec<IVec2>) {
        let world = self.app.world_mut();
        let map_seed = world.resource::<ActiveLevel>().0.map_seed;
        world.resource_scope(|world, mut map: Mut<GameMap>| {
            let mut commands = world.commands();
            for square in squares {
                let (kind, spots) = neutral_site_in_square(map_seed, square);
                if let Some(origin) = spots
                    .into_iter()
                    .find(|&origin| footprint_is_free(&map, kind, origin))
                {
                    spawn_unclaimed_site(&mut commands, &mut map, kind, origin);
                }
            }
        });
        world.flush();
    }

    /// A new side on the free site nearest `near`; the neutral sites of any ground the world widened by
    /// for it go down with it.
    fn deploy_side_near(&mut self, near: IVec2) -> Option<(Team, IVec2)> {
        let before = map_bounds();
        let deployed = deploy_newcomer(self.app.world_mut(), near)?;
        let after = map_bounds();
        if after != before {
            let old = squares_within(before);
            let new = squares_within(after)
                .into_iter()
                .filter(|square| !old.contains(square))
                .collect();
            self.place_neutral_sites(new);
        }
        Some(deployed)
    }

    /// The world as it was saved, with its players' sides kept for them; nobody is connected yet.
    pub fn resume(saved: &SavedWorld) -> Self {
        let mut app = Self::started(saved.map_seed(), true);
        let world = app.world_mut();
        let mined = restore(world, saved);
        world.remove_resource::<RestoringSave>();
        world.resource_mut::<SimulationTick>().0 = saved.tick();
        let hosting = saved.hosting();
        world.resource_mut::<Shelter>().0 = hosting.sheltered.iter().copied().collect();
        let mut host = Self::hosting(app);
        host.mined = MinedCrystal::of_cells(mined);
        host.level_side_taken = hosting.level_side_taken;
        host.sides_of_players = hosting.players.iter().cloned().collect();
        host.sites = hosting.sites.iter().copied().collect();
        host.away_since = hosting.away_since.iter().copied().collect();
        host.computer_sides = hosting.computer_sides.iter().copied().collect();
        host.colors = hosting.colors.iter().copied().collect();
        host.deposits.clone_from(&hosting.deposits);
        for &team in host.sides_of_players.values() {
            host.away_since.entry(team).or_insert(saved.tick());
        }
        host
    }

    /// Saves the world into `store` every `every` of world time, and once at once.
    pub fn keep_in(&mut self, store: WorldStore, every: Duration) -> io::Result<SaveReport> {
        self.store = Some(KeptStore {
            store,
            every_ticks: ticks_in(every).max(1),
            last_saved_tick: self.tick(),
        });
        self.save()
    }

    pub fn is_kept(&self) -> bool {
        self.store.is_some()
    }

    pub fn save(&mut self) -> io::Result<SaveReport> {
        let tick = self.tick();
        let hosting = self.hosting_record();
        let Some(kept) = &mut self.store else {
            return Ok(SaveReport::default());
        };
        kept.last_saved_tick = tick;
        kept.store.save(self.app.world_mut(), tick, hosting)
    }

    fn save_when_due(&mut self) {
        let due = self
            .store
            .as_ref()
            .is_some_and(|kept| self.tick() - kept.last_saved_tick >= kept.every_ticks);
        if !due {
            return;
        }
        if let Err(error) = self.save() {
            warn!("could not save the world: {error}");
        }
    }

    fn hosting_record(&self) -> HostingRecord {
        let shelter = self.app.world().resource::<Shelter>();
        let mut players: Vec<(PlayerKey, Team)> = self
            .sides_of_players
            .iter()
            .map(|(key, &team)| (key.clone(), team))
            .collect();
        players.sort_by_key(|(_, team)| *team);
        let mut sites: Vec<(Team, IVec2)> = self.sites.iter().map(|(&t, &s)| (t, s)).collect();
        sites.sort_by_key(|(team, _)| *team);
        let mut away_since: Vec<(Team, u64)> =
            self.away_since.iter().map(|(&t, &s)| (t, s)).collect();
        away_since.sort_by_key(|(team, _)| *team);
        let mut computer_sides: Vec<(Team, IVec2)> =
            self.computer_sides.iter().map(|(&t, &s)| (t, s)).collect();
        computer_sides.sort_by_key(|(team, _)| *team);
        let mut colors: Vec<(Team, SideColor)> =
            self.colors.iter().map(|(&t, &c)| (t, c)).collect();
        colors.sort_by_key(|(team, _)| *team);
        HostingRecord {
            deposits: self.deposits.clone(),
            colors,
            computer_sides,
            level_side_taken: self.level_side_taken,
            sheltered: players
                .iter()
                .map(|(_, team)| *team)
                .filter(|&team| shelter.covers(team))
                .collect(),
            players,
            sites,
            away_since,
        }
    }

    /// The simulation of a world grown from `map_seed`, started; without the level's forces when a save
    /// is to take their place.
    fn started(map_seed: u64, restoring: bool) -> App {
        let level = world_level(map_seed);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .insert_resource(ActiveLevel(level))
            .insert_resource(HostedWorld)
            .add_plugins(SimulationPlugin::default());
        if restoring {
            app.insert_resource(RestoringSave);
        }
        run_schedules_on_one_thread(&mut app);
        let startup_frame_limit = 10;
        for _ in 0..startup_frame_limit {
            let state = *app.world().resource::<State<GameState>>().get();
            if state == GameState::Playing && app.world().contains_resource::<GameMap>() {
                break;
            }
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Playing);
            app.update();
        }
        assert!(
            app.world().contains_resource::<GameMap>(),
            "the world never started"
        );
        app
    }

    fn hosting(app: App) -> Self {
        let level = &app.world().resource::<ActiveLevel>().0;
        let computer_sides: HashMap<Team, IVec2> = level
            .opponents()
            .map(|(team, enemy)| (team, enemy.force.site.origin()))
            .collect();
        let mut colors = HashMap::new();
        let mut computers: Vec<Team> = computer_sides.keys().copied().collect();
        computers.sort();
        for team in computers {
            let color = distinct_color(None, colors.values().copied());
            colors.insert(team, color);
        }
        Self {
            computer_sides,
            colors,
            deposits: Vec::new(),
            app,
            players: Vec::new(),
            level_side_taken: false,
            sides_of_players: HashMap::new(),
            entries: 0,
            absence: AbsenceRules::default(),
            away_since: HashMap::new(),
            sites: HashMap::new(),
            effects: WorldEffects::default(),
            mined: MinedCrystal::default(),
            unshown_effects: Vec::new(),
            store: None,
        }
    }

    pub fn world(&mut self) -> &mut World {
        self.app.world_mut()
    }

    pub fn tick(&self) -> u64 {
        self.app.world().resource::<SimulationTick>().0
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn connect(&mut self, connection: Box<dyn WorldConnection>) {
        self.players.push(WorldPlayer {
            connection,
            team: None,
            entry: 0,
            sent: SentView::default(),
        });
    }

    /// Takes in what the players sent, lets `elapsed` of time pass in the world, then shows every player
    /// what changed in their view if a tick went by.
    pub fn set_absence_rules(&mut self, rules: AbsenceRules) {
        self.absence = rules;
    }

    pub fn step(&mut self, elapsed: Duration) {
        self.receive();
        self.shelter_sides_of_absent_players();
        let before = self.tick();
        self.app
            .insert_resource(TimeUpdateStrategy::ManualDuration(elapsed));
        self.app.update();
        let ticks_gone_by = self.tick() - before;
        self.wear_down_long_absent_sides(ticks_gone_by);
        let looks = |tick: u64| tick / TICKS_BETWEEN_LOOKS_AT_COMPUTER_SIDES;
        if looks(self.tick()) != looks(before) {
            self.retire_fallen_computer_sides();
        }
        let deposit_rounds = |tick: u64| tick / TICKS_BETWEEN_DEPOSITS;
        if deposit_rounds(self.tick()) != deposit_rounds(before) {
            self.grow_a_deposit(deposit_rounds(self.tick()));
        }
        let neighbour_rounds = |tick: u64| tick / TICKS_BETWEEN_COMPUTER_NEIGHBOURS;
        if neighbour_rounds(self.tick()) != neighbour_rounds(before) {
            self.bring_computer_sides_to_players();
        }
        let effects = self.effects.gather(self.app.world_mut());
        self.unshown_effects.extend(effects);
        self.mined.note(self.app.world_mut());
        if self.tick() / TICKS_PER_VIEW != before / TICKS_PER_VIEW {
            self.show_views();
        }
        self.save_when_due();
    }

    fn show_views(&mut self) {
        let effects = std::mem::take(&mut self.unshown_effects);
        let news = WorldNews {
            tick: self.tick(),
            effects: &effects,
            mined: &self.mined,
            colors: &self.colors,
        };
        let snapshot = WorldSnapshot::of(self.app.world_mut());
        let world = self.app.world();
        let mut kept_up = Vec::with_capacity(self.players.len());
        for player in &mut self.players {
            let Some(team) = player.team else {
                kept_up.push(true);
                continue;
            };
            let sent = match player.sent.next(world, &snapshot, team, &news) {
                Some(change) => player
                    .connection
                    .send(&Message::WorldView(Box::new(change))),
                None => true,
            };
            kept_up.push(sent);
        }
        let mut kept_up = kept_up.into_iter();
        self.players.retain(|_| kept_up.next().unwrap_or(true));
    }

    fn receive(&mut self) {
        let mut players = std::mem::take(&mut self.players);
        players.retain_mut(|player| {
            let mut connected = true;
            for arrival in player.connection.drain() {
                match arrival {
                    Arrival::Message(message) => self.answer(player, message),
                    Arrival::ConnectionLost => connected = false,
                }
            }
            connected
        });
        self.players = players;
        self.let_go_of_displaced_players();
    }

    fn shelter_sides_of_absent_players(&mut self) {
        let now = self.tick();
        let present: Vec<Team> = self
            .players
            .iter()
            .filter_map(|player| player.team)
            .collect();
        let shelter_after = ticks_in(self.absence.shelter_after);
        let mut shelter = *self.app.world().resource::<Shelter>();
        for &team in self.sides_of_players.values() {
            if present.contains(&team) {
                self.away_since.remove(&team);
                shelter.0.remove(team);
                continue;
            }
            let since = *self.away_since.entry(team).or_insert(now);
            if now - since >= shelter_after {
                shelter.0.insert(team);
            }
        }
        self.app.insert_resource(shelter);
    }

    /// The units and structures of a side away past `decay_after` lose health by the tick; once nothing of
    /// it stands, its player is forgotten and its site freed for the next newcomer.
    fn wear_down_long_absent_sides(&mut self, ticks: u64) {
        let now = self.tick();
        let decay_after = ticks_in(self.absence.decay_after);
        let decaying: Vec<Team> = self
            .away_since
            .iter()
            .filter(|&(_, &since)| now - since >= decay_after)
            .map(|(&team, _)| team)
            .collect();
        if decaying.is_empty() || ticks == 0 {
            return;
        }
        let share = ticks as f32 / ticks_in(self.absence.decay_over).max(1) as f32;
        let world = self.app.world_mut();
        let mut things = world.query::<(&Team, &mut Health)>();
        for (team, mut health) in things.iter_mut(world) {
            if decaying.contains(team) {
                health.current -= health.max * share;
            }
        }
        for team in decaying {
            if !side_stands(self.app.world_mut(), team) {
                self.retire(team);
            }
        }
    }

    /// A player's side with nothing left: its player is forgotten, its site freed and its number given out
    /// again.
    fn retire(&mut self, team: Team) {
        self.sides_of_players.retain(|_, &mut side| side != team);
        self.away_since.remove(&team);
        self.colors.remove(&team);
        if let Some(site) = self.sites.remove(&team) {
            free_starting_site(self.app.world_mut(), site);
        }
        retire_side(self.app.world_mut(), team);
    }

    /// A player who entered again elsewhere commands their side from there; the older connection goes.
    fn let_go_of_displaced_players(&mut self) {
        let mut latest: HashMap<Team, u64> = HashMap::new();
        for player in &self.players {
            if let Some(team) = player.team {
                let entry = latest.entry(team).or_default();
                *entry = (*entry).max(player.entry);
            }
        }
        self.players.retain(|player| {
            let displaced = player
                .team
                .is_some_and(|team| latest[&team] != player.entry);
            if displaced {
                player.connection.send(&Message::Refused {
                    reason: "You entered this world elsewhere".into(),
                });
            }
            !displaced
        });
    }

    fn answer(&mut self, player: &mut WorldPlayer, message: Message) {
        match (message, player.team) {
            (
                Message::EnterWorld {
                    protocol,
                    player: key,
                    color,
                    ..
                },
                None,
            ) => {
                if let Some(reason) = protocol_refusal(protocol) {
                    player.connection.send(&Message::Refused {
                        reason: reason.into(),
                    });
                    return;
                }
                let Some(team) = self.side_for(&key) else {
                    player.connection.send(&Message::Refused {
                        reason: "The world has no room for you".into(),
                    });
                    return;
                };
                self.entries += 1;
                player.entry = self.entries;
                player.team = Some(team);
                self.paint(team, color);
                let map_seed = self.app.world().resource::<ActiveLevel>().0.map_seed;
                player.connection.send(&Message::Entered {
                    team,
                    tick: self.tick(),
                    map_seed,
                });
            }
            (Message::Orders { commands }, Some(team)) => {
                let world = self.app.world_mut();
                let now = *world.resource::<SimulationTick>();
                let mut queue = world.resource_mut::<CommandQueue>();
                for command in commands {
                    queue.issue(now, team, command);
                }
            }
            _ => {}
        }
    }

    /// Gives `team` the colour wanted for it, or the one furthest from the other sides' when that is too
    /// close to one of theirs or none was wanted.
    fn paint(&mut self, team: Team, wanted: Option<SideColor>) {
        let others: Vec<SideColor> = self
            .colors
            .iter()
            .filter(|&(&other, _)| other != team)
            .map(|(_, &color)| color)
            .collect();
        let kept = self.colors.get(&team).copied().filter(|_| wanted.is_none());
        let color = kept.unwrap_or_else(|| distinct_color(wanted, others.into_iter()));
        self.colors.insert(team, color);
    }

    /// A returning player's side if anything of it still stands, otherwise a new one.
    fn side_for(&mut self, key: &PlayerKey) -> Option<Team> {
        if let Some(&team) = self.sides_of_players.get(key) {
            if side_stands(self.app.world_mut(), team) {
                return Some(team);
            }
            self.retire(team);
        }
        let team = self.side_for_newcomer()?;
        self.sides_of_players.insert(key.clone(), team);
        Some(team)
    }

    /// The first player takes the level's own side; everyone after a generated site of their own.
    fn side_for_newcomer(&mut self) -> Option<Team> {
        if !self.level_side_taken {
            self.level_side_taken = true;
            return Some(Team::Player);
        }
        let first_base = base_sites()[0];
        let (team, site) = self.deploy_side_near(first_base)?;
        self.sites.insert(team, site);
        self.give_a_computer_neighbour(site, Difficulty::Normal);
        Some(team)
    }

    /// A newcomer with no computer side near finds one on the free site nearest them.
    fn give_a_computer_neighbour(&mut self, site: IVec2, difficulty: Difficulty) {
        let has_one = self
            .computer_sides
            .values()
            .any(|&computer| (computer - site).as_vec2().length() <= COMPUTER_NEIGHBOUR_REACH);
        if has_one {
            return;
        }
        let Some((team, computer_site)) = self.deploy_side_near(site) else {
            return;
        };
        let world = self.app.world_mut();
        world
            .resource_mut::<AiCommanders>()
            .0
            .push(AiCommander::new(team, computer_site, difficulty));
        world.resource_mut::<Economies>().of_mut(team).income_rate = difficulty.income_rate();
        self.computer_sides.insert(team, computer_site);
        self.paint(team, None);
    }

    /// A new crystal deposit somewhere in the world, while fewer are fresh than one more than the players;
    /// every player present is told it surfaced, not where.
    fn grow_a_deposit(&mut self, round: u64) {
        let world = self.app.world();
        let map = world.resource::<GameMap>();
        let fresh = self
            .deposits
            .iter()
            .filter(|&&middle| {
                (-DEPOSIT_REACH..=DEPOSIT_REACH)
                    .flat_map(|dy| {
                        (-DEPOSIT_REACH..=DEPOSIT_REACH).map(move |dx| IVec2::new(dx, dy))
                    })
                    .any(|offset| map.crystal_at(middle + offset) > 0)
            })
            .count();
        if fresh > self.sides_of_players.len() {
            return;
        }
        let Some((middle, laid)) = lay_new_deposit(self.app.world_mut(), round) else {
            return;
        };
        self.deposits.push(middle);
        for (cell, amount) in laid {
            self.mined.record(cell, amount);
        }
        let mut present: Vec<Team> = self
            .players
            .iter()
            .filter_map(|player| player.team)
            .collect();
        present.dedup();
        for team in present {
            self.app
                .world_mut()
                .write_message(News::to(team, SideNews::CrystalSurfaced));
        }
    }

    /// Every player present whose computer neighbour fell, or who never had one, gets one, as hard as the
    /// player's base is big.
    fn bring_computer_sides_to_players(&mut self) {
        let mut present: Vec<Team> = self
            .players
            .iter()
            .filter_map(|player| player.team)
            .collect();
        present.sort();
        present.dedup();
        for team in present {
            let Some(site) = self.site_of(team) else {
                continue;
            };
            let difficulty = self.neighbour_difficulty_for(team);
            self.give_a_computer_neighbour(site, difficulty);
        }
    }

    /// Where a player's side started: a generated site, or the level's first base for the first player.
    fn site_of(&self, team: Team) -> Option<IVec2> {
        match self.sites.get(&team) {
            Some(&site) => Some(site),
            None => (team == Team::Player).then(|| base_sites()[0]),
        }
    }

    fn neighbour_difficulty_for(&mut self, team: Team) -> Difficulty {
        let world = self.app.world_mut();
        let structures = world
            .query::<(&Building, &Team)>()
            .iter(world)
            .filter(|(building, owner)| **owner == team && building.kind.keeps_a_side_alive())
            .count();
        if structures >= STRUCTURES_FOR_A_BRUTAL_NEIGHBOUR {
            Difficulty::Brutal
        } else if structures >= STRUCTURES_FOR_A_HARD_NEIGHBOUR {
            Difficulty::Hard
        } else {
            Difficulty::Normal
        }
    }

    /// A computer side with nothing left gives up its commander and its site.
    fn retire_fallen_computer_sides(&mut self) {
        let fallen: Vec<(Team, IVec2)> = self
            .computer_sides
            .iter()
            .map(|(&team, &site)| (team, site))
            .filter(|&(team, _)| !side_stands(self.app.world_mut(), team))
            .collect();
        for (team, site) in fallen {
            self.computer_sides.remove(&team);
            self.colors.remove(&team);
            let world = self.app.world_mut();
            free_starting_site(world, site);
            retire_side(world, team);
        }
    }
}

/// The level a world grows from: a wilds landscape around the usual four bases.
pub fn world_level(map_seed: u64) -> LevelDefinition {
    LevelDefinition {
        name: "World".into(),
        map_seed,
        landscape: Landscape::Wilds,
        objective: Objective::World,
        ..LevelDefinition::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_view::{Thing, WorldMirror, things_in_view};
    use bevy::ecs::system::SystemState;
    use rimefront_sim::SIMULATION_TICK;
    use rimefront_sim::sim::catalog::ProjectileStyle;
    use rimefront_sim::sim::catalog::{BuildingKind, UnitKind};
    use rimefront_sim::sim::components::Building;
    use rimefront_sim::sim::components::Unit;
    use rimefront_sim::sim::determinism::SimId;
    use rimefront_sim::sim::economy::Economies;
    use rimefront_sim::sim::intel::Intel;
    use rimefront_sim::sim::neutral_sites::UnclaimedSite;
    use rimefront_sim::sim::notifications::SideNews;
    use rimefront_sim::sim::player_commands::{GroundOrder, PlayerCommand};
    use rimefront_sim::sim::production::{BuildItem, EnqueueRefusal, placement_context};
    use rimefront_sim::sim::production::{Lane, Production};
    use rimefront_sim::sim::spawn::{spawn_building, spawn_unit};
    use rimefront_sim::world::map::GameMap;
    use rimefront_sim::world::map::cell_center;
    use rimefront_sim::world::map::chunk_of;
    use rimefront_sim::world::map::world_to_cell;
    use rimefront_sim::world::sites::StartingSites;

    fn follow(mirror: &mut WorldMirror, player: &mut LocalConnection) -> usize {
        let mut bytes = 0;
        for arrival in player.drain() {
            if let Arrival::Message(message @ Message::WorldView(_)) = arrival {
                bytes += crate::link::encode(&message).unwrap().len();
                let Message::WorldView(change) = message else {
                    unreachable!()
                };
                mirror.apply(*change);
            }
        }
        bytes
    }

    #[test]
    fn a_player_mirrors_what_their_side_sees_and_nothing_in_the_fog() {
        let mut host = WorldHost::new(29);
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            player: PlayerKey::new_random(),
            color: None,
        });
        let team = Team::Player;
        let mut mirror = WorldMirror::default();
        let mut bytes = 0;
        let minute_of_ticks = 60 * 20;
        for _ in 0..minute_of_ticks {
            host.step(SIMULATION_TICK);
            bytes += follow(&mut mirror, &mut player);
        }
        let tick = host.tick();
        let seen = things_in_view(host.world(), team);
        assert!(mirror.tick <= tick);
        assert_eq!(mirror.things, seen);
        assert_eq!(mirror.bounds, Some(map_bounds()));
        assert!(mirror.things.values().any(|thing| thing.team == Some(team)));
        let world = host.world();
        let enemy_base = world
            .query::<(&SimId, &Team, &Building)>()
            .iter(world)
            .find(|(_, team, building)| {
                **team == Team::Crimson && building.kind == BuildingKind::CommandCenter
            })
            .map(|(id, ..)| *id)
            .expect("the world has a computer side");
        assert!(
            !mirror.things.contains_key(&enemy_base),
            "seen through the fog"
        );
        println!("a minute of a world's view: {bytes} bytes");

        let tank = mirror
            .things
            .values()
            .find(|thing| thing.team == Some(team) && thing.thing == Thing::Unit(UnitKind::Tank))
            .copied()
            .expect("the player sees their tank");
        let goal = rimefront_sim::world::map::world_to_cell(tank.position()) + IVec2::new(0, -12);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::Ground {
                units: vec![tank.id],
                goal,
                order: GroundOrder::Move,
                queued: false,
                refinery: None,
                together: false,
            }],
        });
        let ten_seconds_of_ticks = 10 * 20;
        let mut moving_bytes = 0;
        for _ in 0..ten_seconds_of_ticks {
            host.step(SIMULATION_TICK);
            moving_bytes += follow(&mut mirror, &mut player);
        }
        assert_ne!(
            mirror.things[&tank.id], tank,
            "the mirrored tank never moved"
        );
        println!("ten seconds of a tank driving: {moving_bytes} bytes");
    }

    #[test]
    fn a_player_is_shown_the_shots_and_blasts_of_a_fight_in_sight() {
        let mut host = WorldHost::new(29);
        let (mut player, _) = enter(&mut host);
        let world = host.world();
        let tank_position = world
            .query::<(&Unit, &Team, &Transform)>()
            .iter(world)
            .find(|(unit, team, _)| **team == Team::Player && unit.0 == UnitKind::Tank)
            .map(|(.., transform)| transform.translation)
            .expect("the player has a tank");
        let mut commands = world.commands();
        spawn_unit(
            &mut commands,
            UnitKind::Tank,
            Team::Crimson,
            tank_position + Vec3::new(4.0, 0.0, 0.0),
            0.0,
        );
        world.flush();

        let mut effects = Vec::new();
        for _ in 0..3 * 20 {
            host.step(SIMULATION_TICK);
            for arrival in player.drain() {
                if let Arrival::Message(Message::WorldView(change)) = arrival {
                    effects.extend(change.effects);
                }
            }
        }
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Shot(shot) if shot.style == ProjectileStyle::Shell)),
            "no shell shown"
        );
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Blast { .. })),
            "no blast shown"
        );
    }

    fn own_unit(world: &mut World, team: Team, kind: UnitKind) -> (Entity, Vec3) {
        world
            .query::<(Entity, &Unit, &Team, &Transform)>()
            .iter(world)
            .find(|(_, unit, owner, _)| **owner == team && unit.0 == kind)
            .map(|(entity, .., transform)| (entity, transform.translation))
            .expect("the side has the unit")
    }

    fn entered_with_mirror(host: &mut WorldHost) -> (LocalConnection, WorldMirror) {
        let (mut player, _) = enter(host);
        let mut mirror = WorldMirror::default();
        host.step(SIMULATION_TICK);
        follow(&mut mirror, &mut player);
        (player, mirror)
    }

    fn run(
        host: &mut WorldHost,
        secs: usize,
        mirror: &mut WorldMirror,
        player: &mut LocalConnection,
    ) {
        for _ in 0..secs * 20 {
            host.step(SIMULATION_TICK);
            follow(mirror, player);
        }
    }

    #[test]
    fn a_player_is_shown_the_crystal_mined_in_sight() {
        let mut host = WorldHost::new(29);
        let (mut player, mut mirror) = entered_with_mirror(&mut host);
        let world = host.world();
        let (_, tank_position) = own_unit(world, Team::Player, UnitKind::Tank);
        let mut commands = world.commands();
        spawn_unit(
            &mut commands,
            UnitKind::Harvester,
            Team::Player,
            tank_position,
            0.0,
        );
        world.flush();
        run(&mut host, 30, &mut mirror, &mut player);

        assert!(!mirror.crystal.is_empty(), "no mining shown");
        let map = host.world().resource::<GameMap>();
        for (&(x, y), &amount) in &mirror.crystal {
            assert_eq!(map.crystal_at(IVec2::new(x, y)), amount);
        }
    }

    #[test]
    fn an_enemy_structure_stays_as_last_seen_once_out_of_sight() {
        let mut host = WorldHost::new(29);
        let (mut player, mut mirror) = entered_with_mirror(&mut host);
        let world = host.world();
        let (scout, scout_position) = own_unit(world, Team::Player, UnitKind::Scout);
        let far_off = scout_position + Vec3::new(0.0, 0.0, -30.0);
        world.get_mut::<Transform>(scout).unwrap().translation = far_off;
        let origin = world_to_cell(far_off) + IVec2::new(2, 0);
        let outpost = world.resource_scope(|world, mut map: Mut<GameMap>| {
            let mut commands = world.commands();
            spawn_building(
                &mut commands,
                &mut map,
                BuildingKind::PowerPlant,
                Team::Crimson,
                origin,
            )
        });
        world.flush();
        let outpost_shown = |mirror: &WorldMirror| {
            mirror.things.values().any(|thing| {
                thing.thing
                    == Thing::Building {
                        kind: BuildingKind::PowerPlant,
                        origin,
                    }
            })
        };
        run(&mut host, 1, &mut mirror, &mut player);
        assert!(outpost_shown(&mirror), "the scout never saw the outpost");

        host.world().despawn(scout);
        run(&mut host, 2, &mut mirror, &mut player);
        assert!(
            outpost_shown(&mirror),
            "the outpost went with the scout's sight"
        );
        host.world().despawn(outpost);
        run(&mut host, 1, &mut mirror, &mut player);
        assert!(!outpost_shown(&mirror), "a destroyed outpost still shown");
    }

    #[test]
    fn a_player_builds_in_a_world_from_what_their_side_is_shown() {
        let mut host = WorldHost::new(29);
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            player: PlayerKey::new_random(),
            color: None,
        });
        let mut mirror = WorldMirror::default();
        host.step(SIMULATION_TICK);
        follow(&mut mirror, &mut player);
        let power_plant = BuildItem::Structure(BuildingKind::PowerPlant);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::Enqueue {
                item: power_plant,
                copies: 1,
            }],
        });
        for _ in 0..1 + TICKS_PER_VIEW {
            host.step(SIMULATION_TICK);
        }
        follow(&mut mirror, &mut player);
        assert_eq!(mirror.side.lanes[0].queue, [power_plant]);

        let build_ticks = (power_plant.build_secs() * 20.0) as usize + 40;
        for _ in 0..build_ticks {
            host.step(SIMULATION_TICK);
            follow(&mut mirror, &mut player);
        }
        assert_eq!(mirror.side.lanes[0].ready, Some(BuildingKind::PowerPlant));
        assert!(mirror.told.contains(&SideNews::Finished(power_plant)));

        let origin = free_site_near_base(host.world(), Team::Player, BuildingKind::PowerPlant);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::PlaceStructure {
                kind: BuildingKind::PowerPlant,
                origin,
            }],
        });
        host.step(SIMULATION_TICK);
        host.step(SIMULATION_TICK);
        host.step(SIMULATION_TICK);
        follow(&mut mirror, &mut player);
        assert_eq!(mirror.side.lanes[0], Default::default());
        let placed = Thing::Building {
            kind: BuildingKind::PowerPlant,
            origin,
        };
        assert!(mirror.things.values().any(|thing| thing.thing == placed));
    }

    type PlacementParams = (
        Res<'static, GameMap>,
        Res<'static, Intel>,
        Query<'static, 'static, (&'static Building, &'static Team)>,
        Query<'static, 'static, &'static Transform, With<Unit>>,
    );

    fn free_site_near_base(world: &mut World, team: Team, kind: BuildingKind) -> IVec2 {
        let mut state: SystemState<PlacementParams> = SystemState::new(world);
        let (map, intel, buildings, units) = state.get(world).expect("the world has a map");
        let context = placement_context(&map, team, intel.vision(team), &buildings, &units);
        let base = context.own_buildings[0].0;
        (-8..=8)
            .flat_map(|y| (-8..=8).map(move |x| base + IVec2::new(x, y)))
            .find(|&origin| context.can_place(kind, origin))
            .expect("room to build by the base")
    }

    fn entered(player: &mut LocalConnection) -> Option<Team> {
        player
            .drain()
            .into_iter()
            .find_map(|arrival| match arrival {
                Arrival::Message(Message::Entered { team, .. }) => Some(team),
                _ => None,
            })
    }

    fn enter(host: &mut WorldHost) -> (LocalConnection, Team) {
        enter_as(host, &PlayerKey::new_random())
    }

    fn enter_as(host: &mut WorldHost, key: &PlayerKey) -> (LocalConnection, Team) {
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            player: key.clone(),
            color: None,
        });
        host.step(SIMULATION_TICK);
        let team = entered(&mut player).expect("the world let the player in");
        (player, team)
    }

    fn was_refused(player: &mut LocalConnection) -> bool {
        player
            .drain()
            .into_iter()
            .any(|arrival| matches!(arrival, Arrival::Message(Message::Refused { .. })))
    }

    #[test]
    fn a_returning_player_commands_their_side_again() {
        let mut host = WorldHost::new(29);
        let key = PlayerKey::new_random();
        let (_other, _) = enter(&mut host);
        let (first_visit, team) = enter_as(&mut host, &key);
        drop(first_visit);
        host.step(SIMULATION_TICK);
        assert_eq!(host.player_count(), 1);

        let (_second_visit, team_again) = enter_as(&mut host, &key);
        assert_eq!(team_again, team);
    }

    #[test]
    fn the_side_of_a_player_away_for_a_while_is_sheltered_until_they_return() {
        let mut host = WorldHost::new(29);
        host.set_absence_rules(AbsenceRules {
            shelter_after: Duration::from_secs(2),
            ..AbsenceRules::default()
        });
        let key = PlayerKey::new_random();
        let (visit, team) = enter_as(&mut host, &key);
        drop(visit);
        let sheltered = |host: &mut WorldHost| host.world().resource::<Shelter>().covers(team);
        for _ in 0..20 {
            host.step(SIMULATION_TICK);
        }
        assert!(!sheltered(&mut host), "sheltered at once");
        for _ in 0..30 {
            host.step(SIMULATION_TICK);
        }
        assert!(sheltered(&mut host), "never sheltered");

        let attacker_key = PlayerKey::new_random();
        let (mut attacker, world_end) = local_connection();
        host.connect(Box::new(world_end));
        attacker.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            player: attacker_key.clone(),
            color: None,
        });
        let mut mirror = WorldMirror::default();
        run(&mut host, 1, &mut mirror, &mut attacker);
        assert_eq!(
            mirror.sheltered,
            [team],
            "the newcomer was not shown the shelter"
        );
        let attacking = host.sides_of_players[&attacker_key];
        let world = host.world();
        let (_, target_id) = world
            .query::<(&Team, &SimId)>()
            .iter(world)
            .find(|(owner, _)| **owner == team)
            .map(|(owner, id)| (*owner, *id))
            .expect("the sheltered side has something");
        let (tank, _) = own_unit(world, attacking, UnitKind::Tank);
        let tank_id = *world.get::<SimId>(tank).unwrap();
        attacker.send(&Message::Orders {
            commands: vec![PlayerCommand::Attack {
                units: vec![tank_id],
                target: target_id,
            }],
        });
        run(&mut host, 1, &mut mirror, &mut attacker);
        assert!(
            mirror.told.contains(&SideNews::Sheltered),
            "the attacker was not told why"
        );

        let (_back, _) = enter_as(&mut host, &key);
        run(&mut host, 1, &mut mirror, &mut attacker);
        assert!(
            mirror.sheltered.is_empty(),
            "the shelter was not lifted for others to see"
        );
        assert!(
            !sheltered(&mut host),
            "still sheltered after the player came back"
        );
    }

    #[test]
    fn a_side_long_away_decays_and_frees_its_site() {
        let mut host = WorldHost::new(29);
        host.set_absence_rules(AbsenceRules {
            shelter_after: Duration::from_secs(1),
            decay_after: Duration::from_secs(2),
            decay_over: Duration::from_secs(3),
        });
        let (_first, _) = enter(&mut host);
        let key = PlayerKey::new_random();
        let (visit, team) = enter_as(&mut host, &key);
        let site = host.sites[&team];
        drop(visit);
        for _ in 0..3 * 20 {
            host.step(SIMULATION_TICK);
        }
        assert!(side_stands(host.world(), team), "decayed too soon");
        for _ in 0..4 * 20 {
            host.step(SIMULATION_TICK);
        }
        assert!(!side_stands(host.world(), team), "never decayed");
        let claimed = &host.world().resource::<StartingSites>().claimed;
        assert!(!claimed.contains(&site), "the site stayed claimed");
        assert!(!host.world().resource::<Shelter>().covers(team));

        let (_back, new_team) = enter_as(&mut host, &key);
        assert_eq!(new_team, team, "the side's number was not given out again");
        assert!(
            side_stands(host.world(), new_team),
            "the player was not given a new start"
        );
        assert!(host.sites.contains_key(&new_team));
    }

    fn scratch_directory(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("rimefront_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    fn standing(host: &mut WorldHost) -> Vec<(Team, String)> {
        let world = host.world();
        let mut things: Vec<(Team, String)> = world
            .query::<(&Team, Option<&Unit>, Option<&Building>)>()
            .iter(world)
            .filter_map(|(&team, unit, building)| match (unit, building) {
                (Some(unit), _) => Some((team, format!("{:?}", unit.0))),
                (None, Some(building)) => {
                    Some((team, format!("{:?} {}", building.kind, building.origin)))
                }
                (None, None) => None,
            })
            .collect();
        things.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        things
    }

    #[test]
    fn a_saved_world_resumes_with_its_sides_players_and_ground() {
        let directory = scratch_directory("resumed_world");
        let mut host = WorldHost::new(29);
        let key = PlayerKey::new_random();
        let (_first, _) = enter(&mut host);
        let (player, team) = enter_as(&mut host, &key);
        let world = host.world();
        let (_, tank_position) = own_unit(world, team, UnitKind::Tank);
        let mut commands = world.commands();
        spawn_unit(&mut commands, UnitKind::Harvester, team, tank_position, 0.0);
        world.flush();
        let power_plant = BuildItem::Structure(BuildingKind::PowerPlant);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::Enqueue {
                item: power_plant,
                copies: 1,
            }],
        });
        for _ in 0..20 * 20 {
            host.step(SIMULATION_TICK);
        }
        let first = host
            .keep_in(WorldStore::at(&directory), Duration::from_secs(60))
            .unwrap();
        assert_eq!(first.chunks_written, first.chunks);
        let quiet = host.save().unwrap();
        assert_eq!(
            quiet.chunks_written, 0,
            "an unchanged world wrote chunks again"
        );
        for _ in 0..5 * 20 {
            host.step(SIMULATION_TICK);
        }
        let later = host.save().unwrap();
        assert!(
            later.chunks_written < later.chunks,
            "every chunk written again"
        );
        println!(
            "a saved world: {} chunks in {} bytes; 5 s later {} chunks in {} bytes",
            first.chunks, first.bytes_written, later.chunks_written, later.bytes_written
        );

        let tick = host.tick();
        let before = standing(&mut host);
        let credits = host.world().resource::<Economies>().of(team).credits;
        let lane = host
            .world()
            .resource::<Production>()
            .lane(team, Lane::Structures)
            .queue
            .clone();
        let crystal = host.world().resource::<GameMap>().crystal_amounts();
        let explored = host
            .world()
            .resource::<Intel>()
            .vision(team)
            .explored_cells();

        let saved = WorldStore::at(&directory).load().unwrap().unwrap();
        let mut resumed = WorldHost::resume(&saved);
        assert_eq!(resumed.computer_sides, host.computer_sides);
        assert_eq!(resumed.tick(), tick);
        assert_eq!(standing(&mut resumed), before);
        let world = resumed.world();
        assert_eq!(world.resource::<Economies>().of(team).credits, credits);
        assert_eq!(
            world
                .resource::<Production>()
                .lane(team, Lane::Structures)
                .queue,
            lane
        );
        assert_eq!(world.resource::<GameMap>().crystal_amounts(), crystal);
        assert_eq!(
            world.resource::<Intel>().vision(team).explored_cells(),
            explored
        );

        let (mut back, world_end) = local_connection();
        resumed.connect(Box::new(world_end));
        back.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            player: key.clone(),
            color: None,
        });
        for _ in 0..TICKS_PER_VIEW {
            resumed.step(SIMULATION_TICK);
        }
        assert_eq!(resumed.sides_of_players[&key], team);
        let mut mirror = WorldMirror::default();
        follow(&mut mirror, &mut back);
        assert!(
            !mirror.crystal.is_empty(),
            "the mined crystal was not shown"
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn every_newcomer_has_a_computer_side_near() {
        let mut host = WorldHost::new(29);
        let level_computers = host.computer_sides.len();
        let _players: Vec<_> = (0..5).map(|_| enter(&mut host)).collect();
        assert!(
            host.computer_sides.len() > level_computers,
            "no computer side came"
        );
        for &site in host.sites.values() {
            let near = host
                .computer_sides
                .values()
                .any(|&computer| (computer - site).as_vec2().length() <= COMPUTER_NEIGHBOUR_REACH);
            assert!(near, "the newcomer at {site} has no computer side near");
        }
        let commanded: Vec<Team> = host
            .world()
            .resource::<AiCommanders>()
            .0
            .iter()
            .map(|ai| ai.team)
            .collect();
        for team in host.computer_sides.keys() {
            assert!(commanded.contains(team), "{team:?} has no commander");
        }
    }

    #[test]
    fn a_fallen_computer_side_gives_up_its_commander_and_site() {
        let mut host = WorldHost::new(29);
        let _players: Vec<_> = (0..5).map(|_| enter(&mut host)).collect();
        let level_sides = Team::ALL.len();
        let (&team, &site) = host
            .computer_sides
            .iter()
            .find(|(team, _)| team.index() >= level_sides)
            .expect("a computer side came");
        let world = host.world();
        let everything: Vec<Entity> = world
            .query::<(Entity, &Team)>()
            .iter(world)
            .filter(|(_, owner)| **owner == team)
            .map(|(entity, _)| entity)
            .collect();
        for entity in everything {
            world.despawn(entity);
        }
        for _ in 0..TICKS_BETWEEN_LOOKS_AT_COMPUTER_SIDES + 1 {
            host.step(SIMULATION_TICK);
        }
        assert!(!host.computer_sides.contains_key(&team));
        let world = host.world();
        assert!(
            world
                .resource::<AiCommanders>()
                .0
                .iter()
                .all(|ai| ai.team != team)
        );
        assert!(!world.resource::<StartingSites>().claimed.contains(&site));
    }

    fn neutral_sites(host: &mut WorldHost) -> Vec<(SimId, Thing)> {
        let world = host.world();
        world
            .query_filtered::<(&SimId, &Building), With<UnclaimedSite>>()
            .iter(world)
            .map(|(id, building)| {
                let site = Thing::Building {
                    kind: building.kind,
                    origin: building.origin,
                };
                (*id, site)
            })
            .collect()
    }

    fn origin_of(thing: Thing) -> IVec2 {
        match thing {
            Thing::Building { origin, .. } => origin,
            Thing::Unit(_) => unreachable!("a site is a structure"),
        }
    }

    #[test]
    fn neutral_sites_stand_in_the_ground_a_world_widens_by() {
        let mut host = WorldHost::new(29);
        let _players: Vec<_> = (0..5).map(|_| enter(&mut host)).collect();
        let sites = neutral_sites(&mut host);
        assert!(
            sites.len() >= 3,
            "the widened ground has {} neutral sites",
            sites.len()
        );
        let bases: Vec<IVec2> = host
            .sites
            .values()
            .chain(host.computer_sides.values())
            .copied()
            .collect();
        for &(_, site) in &sites {
            for base in &bases {
                let apart = (origin_of(site) - *base).as_vec2().length();
                assert!(apart > 12.0, "{site:?} stands {apart} cells from a base");
            }
        }
    }

    #[test]
    fn a_player_is_shown_a_neutral_site_once_their_side_explored_it() {
        let mut host = WorldHost::new(29);
        let (mut player, mut mirror) = entered_with_mirror(&mut host);
        let _newcomers: Vec<_> = (0..3).map(|_| enter(&mut host)).collect();
        let (id, site) = neutral_sites(&mut host).remove(0);
        assert!(
            !mirror.things.contains_key(&id),
            "shown before it was explored"
        );
        let world = host.world();
        let (scout, _) = own_unit(world, Team::Player, UnitKind::Scout);
        let beside = cell_center(origin_of(site) + IVec2::new(0, 4));
        world.get_mut::<Transform>(scout).unwrap().translation = beside;
        run(&mut host, 1, &mut mirror, &mut player);
        let shown = mirror.things.get(&id).expect("never shown");
        assert_eq!(shown.team, None);
        assert_eq!(shown.thing, site);
    }

    fn enter_in(host: &mut WorldHost, color: Option<SideColor>) -> (LocalConnection, Team) {
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            player: PlayerKey::new_random(),
            color,
        });
        host.step(SIMULATION_TICK);
        let team = entered(&mut player).expect("the world let the player in");
        (player, team)
    }

    #[test]
    fn players_get_the_colour_they_chose_unless_another_side_has_one_like_it() {
        let mut host = WorldHost::new(29);
        let teal = SideColor([20, 170, 180]);
        let nearly_teal = SideColor([25, 165, 185]);
        let (mut first, first_team) = enter_in(&mut host, Some(teal));
        let (_second, second_team) = enter_in(&mut host, Some(nearly_teal));
        assert_eq!(host.colors[&first_team], teal);
        let given = host.colors[&second_team];
        assert!(
            given.distance(teal) >= ALIKE_COLORS,
            "{given:?} is too like teal"
        );
        for (&team, &color) in &host.colors {
            for (&other, &other_color) in &host.colors {
                assert!(team == other || color.distance(other_color) >= ALIKE_COLORS);
            }
        }

        let mut mirror = WorldMirror::default();
        run(&mut host, 1, &mut mirror, &mut first);
        assert_eq!(mirror.colors.get(&first_team), Some(&teal));
        assert_eq!(mirror.colors.get(&second_team), Some(&given));
    }

    #[test]
    fn a_world_grows_crystal_deposits_away_from_bases_while_few_are_fresh() {
        let mut host = WorldHost::new(29);
        let (mut player, mut mirror) = entered_with_mirror(&mut host);
        for round in 1..=5 {
            host.grow_a_deposit(round);
        }
        assert_eq!(
            host.deposits.len(),
            2,
            "one more fresh deposit than players"
        );
        let deposits = host.deposits.clone();
        let recorded: Vec<bool> = deposits
            .iter()
            .map(|&middle| {
                host.mined
                    .in_chunk(chunk_of(middle))
                    .any(|(cell, _)| cell == middle)
            })
            .collect();
        assert!(
            recorded.iter().all(|&shown| shown),
            "not recorded for players"
        );
        let world = host.world();
        let structures: Vec<Vec3> = world
            .query_filtered::<&Transform, With<Building>>()
            .iter(world)
            .map(|transform| transform.translation)
            .collect();
        let map = world.resource::<GameMap>();
        for &middle in &deposits {
            assert!(
                map.crystal_at(middle) > 0,
                "no crystal at a deposit's middle"
            );
            let nearest = structures
                .iter()
                .map(|&at| (world_to_cell(at) - middle).as_vec2().length())
                .fold(f32::MAX, f32::min);
            assert!(
                nearest > 16.0,
                "a deposit grew {nearest} cells from a structure"
            );
        }
        run(&mut host, 1, &mut mirror, &mut player);
        assert!(mirror.told.contains(&SideNews::CrystalSurfaced));
    }

    #[test]
    fn the_same_world_grows_the_same_deposit_in_the_same_round() {
        let middle = || {
            let mut host = WorldHost::new(41);
            host.grow_a_deposit(3);
            host.deposits.clone()
        };
        let first = middle();
        assert_eq!(first.len(), 1);
        assert_eq!(first, middle());
    }

    #[test]
    fn a_player_whose_computer_neighbour_fell_gets_another_after_a_while() {
        let mut host = WorldHost::new(29);
        let (_first, _) = enter(&mut host);
        let (_player, team) = enter(&mut host);
        let site = host.sites[&team];
        let neighbour = |host: &WorldHost| {
            host.computer_sides
                .iter()
                .find(|(_, computer)| {
                    (**computer - site).as_vec2().length() <= COMPUTER_NEIGHBOUR_REACH
                })
                .map(|(&computer, _)| computer)
        };
        let fallen = neighbour(&host).expect("the newcomer has a computer neighbour");
        let world = host.world();
        let everything: Vec<Entity> = world
            .query::<(Entity, &Team)>()
            .iter(world)
            .filter(|(_, owner)| **owner == fallen)
            .map(|(entity, _)| entity)
            .collect();
        for entity in everything {
            world.despawn(entity);
        }
        for _ in 0..TICKS_BETWEEN_LOOKS_AT_COMPUTER_SIDES + 1 {
            host.step(SIMULATION_TICK);
        }
        assert_eq!(neighbour(&host), None, "the fallen neighbour stayed");
        for _ in 0..TICKS_BETWEEN_COMPUTER_NEIGHBOURS {
            host.step(SIMULATION_TICK);
        }
        assert!(neighbour(&host).is_some(), "no new neighbour came");
    }

    #[test]
    fn a_computer_neighbour_is_as_hard_as_the_player_is_big() {
        let mut host = WorldHost::new(29);
        let (_player, team) = enter(&mut host);
        assert_eq!(host.neighbour_difficulty_for(team), Difficulty::Normal);
        let site = host.site_of(team).unwrap();
        let world = host.world();
        world.resource_scope(|world, mut map: Mut<GameMap>| {
            let mut commands = world.commands();
            for index in 0..STRUCTURES_FOR_A_HARD_NEIGHBOUR as i32 {
                let origin = site + IVec2::new(-20 + index * 3, 12);
                spawn_building(
                    &mut commands,
                    &mut map,
                    BuildingKind::PowerPlant,
                    team,
                    origin,
                );
            }
        });
        world.flush();
        assert_eq!(host.neighbour_difficulty_for(team), Difficulty::Hard);
    }

    #[test]
    fn entering_again_elsewhere_lets_go_of_the_older_connection() {
        let mut host = WorldHost::new(29);
        let key = PlayerKey::new_random();
        let (mut older, team) = enter_as(&mut host, &key);
        let (mut newer, team_again) = enter_as(&mut host, &key);
        assert_eq!(team_again, team);
        assert!(was_refused(&mut older));
        assert!(!was_refused(&mut newer));
        assert_eq!(host.player_count(), 1);
    }

    #[test]
    fn a_player_whose_side_fell_starts_anew() {
        let mut host = WorldHost::new(29);
        let key = PlayerKey::new_random();
        let (_, team) = enter_as(&mut host, &key);
        let world = host.world();
        let everything: Vec<Entity> = world
            .query::<(Entity, &Team)>()
            .iter(world)
            .filter(|(_, owner)| **owner == team)
            .map(|(entity, _)| entity)
            .collect();
        for entity in everything {
            world.despawn(entity);
        }
        host.step(SIMULATION_TICK);

        let (_, new_team) = enter_as(&mut host, &key);
        assert_ne!(new_team, team);
    }

    #[test]
    fn players_enter_a_world_on_sides_of_their_own_and_command_them() {
        let mut host = WorldHost::new(29);
        let (mut first, first_team) = enter(&mut host);
        let (mut second, second_team) = enter(&mut host);
        assert_eq!(first_team, Team::Player);
        assert_eq!(second_team, Team::from_index(4));
        let mut second_mirror = WorldMirror::default();
        second.send(&Message::Orders {
            commands: vec![PlayerCommand::Enqueue {
                item: BuildItem::Structure(BuildingKind::TechCenter),
                copies: 1,
            }],
        });
        host.step(SIMULATION_TICK);
        host.step(SIMULATION_TICK);
        follow(&mut second_mirror, &mut second);
        assert!(
            matches!(
                second_mirror.told[..],
                [SideNews::Refused(EnqueueRefusal::Requires(_))]
            ),
            "the newcomer was not told why: {:?}",
            second_mirror.told
        );
        let mut first_mirror = WorldMirror::default();
        follow(&mut first_mirror, &mut first);
        assert!(first_mirror.told.is_empty(), "told another side's news");

        let world = host.world();
        let (tank, id, start) = world
            .query::<(Entity, &Unit, &Team, &SimId, &Transform)>()
            .iter(world)
            .find(|(_, unit, team, ..)| **team == second_team && unit.0 == UnitKind::Tank)
            .map(|(entity, _, _, id, transform)| (entity, *id, transform.translation))
            .expect("the newcomer has a starting tank");
        let goal = rimefront_sim::world::map::world_to_cell(start) + IVec2::new(0, 6);
        second.send(&Message::Orders {
            commands: vec![PlayerCommand::Ground {
                units: vec![id],
                goal,
                order: GroundOrder::Move,
                queued: false,
                refinery: None,
                together: false,
            }],
        });
        for _ in 0..100 {
            host.step(SIMULATION_TICK);
        }
        let moved = host.world().get::<Transform>(tank).unwrap().translation;
        assert!(moved.distance(start) > 3.0, "the tank stayed at {start}");
    }

    fn answer_to(host: &mut WorldHost, version: &str, protocol: u32) -> Option<Message> {
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: version.into(),
            protocol,
            player: PlayerKey::new_random(),
            color: None,
        });
        host.step(SIMULATION_TICK);
        player
            .drain()
            .into_iter()
            .find_map(|arrival| match arrival {
                Arrival::Message(message @ (Message::Entered { .. } | Message::Refused { .. })) => {
                    Some(message)
                }
                _ => None,
            })
    }

    #[test]
    fn a_game_of_another_protocol_is_turned_away_and_one_of_another_version_is_not() {
        let mut host = WorldHost::new(29);
        let refusal = |answer: Option<Message>| match answer {
            Some(Message::Refused { reason }) => reason,
            other => panic!("not refused: {other:?}"),
        };
        let older_game = refusal(answer_to(&mut host, "0.0.1", 0));
        assert!(older_game.contains("update the game"), "{older_game}");
        let newer_game = refusal(answer_to(&mut host, "99.0.0", WORLD_PROTOCOL + 1));
        assert!(newer_game.contains("server needs updating"), "{newer_game}");
        let same_protocol = answer_to(&mut host, "99.0.0", WORLD_PROTOCOL);
        assert!(matches!(same_protocol, Some(Message::Entered { .. })));
    }

    #[test]
    fn a_player_who_leaves_is_let_go() {
        let mut host = WorldHost::new(29);
        let (player, _) = enter(&mut host);
        drop(player);
        host.step(SIMULATION_TICK);
        assert_eq!(host.player_count(), 0);
    }
}
