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
use crate::world_view::{Effect, MinedCrystal, SentView, WorldEffects, WorldNews};
use iron_tide_sim::levels::{ActiveLevel, LevelDefinition};
use iron_tide_sim::match_flow::{GameState, HostedWorld, RestoringSave, deploy_newcomer};
use iron_tide_sim::sim::catalog::Team;
use iron_tide_sim::sim::components::Health;
use iron_tide_sim::sim::determinism::run_schedules_on_one_thread;
use iron_tide_sim::sim::player_commands::{CommandQueue, SimulationTick};
use iron_tide_sim::sim::sides::{Shelter, side_stands};
use iron_tide_sim::world::map::{GameMap, base_sites};
use iron_tide_sim::world::sites::free_starting_site;
use iron_tide_sim::world::terrain::Landscape;
use iron_tide_sim::{SIMULATION_TICK, SimulationPlugin};

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
        Self::hosting(Self::started(map_seed, false))
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
        HostingRecord {
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
        Self {
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
        let effects = self.effects.gather(self.app.world_mut());
        self.unshown_effects.extend(effects);
        self.mined.note(self.app.world_mut());
        if self.tick() != before {
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
        };
        let world = self.app.world_mut();
        for player in &mut self.players {
            let Some(team) = player.team else { continue };
            if let Some(change) = player.sent.next(world, team, &news) {
                player
                    .connection
                    .send(&Message::WorldView(Box::new(change)));
            }
        }
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

    fn retire(&mut self, team: Team) {
        self.sides_of_players.retain(|_, &mut side| side != team);
        self.away_since.remove(&team);
        self.app
            .world_mut()
            .resource_mut::<Shelter>()
            .0
            .remove(team);
        if let Some(site) = self.sites.remove(&team) {
            free_starting_site(self.app.world_mut(), site);
        }
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
                    version,
                    player: key,
                },
                None,
            ) => {
                if version != env!("CARGO_PKG_VERSION") {
                    player.connection.send(&Message::Refused {
                        reason: format!("The world runs version {}", env!("CARGO_PKG_VERSION")),
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
        let (team, site) = deploy_newcomer(self.app.world_mut(), first_base)?;
        self.sites.insert(team, site);
        Some(team)
    }
}

/// The level a world grows from: a wilds landscape around the usual four bases.
pub fn world_level(map_seed: u64) -> LevelDefinition {
    LevelDefinition {
        name: "World".into(),
        map_seed,
        landscape: Landscape::Wilds,
        ..LevelDefinition::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_view::{Thing, WorldMirror, things_in_view};
    use bevy::ecs::system::SystemState;
    use iron_tide_sim::SIMULATION_TICK;
    use iron_tide_sim::sim::catalog::ProjectileStyle;
    use iron_tide_sim::sim::catalog::{BuildingKind, UnitKind};
    use iron_tide_sim::sim::components::Building;
    use iron_tide_sim::sim::components::Unit;
    use iron_tide_sim::sim::determinism::SimId;
    use iron_tide_sim::sim::economy::Economies;
    use iron_tide_sim::sim::intel::Intel;
    use iron_tide_sim::sim::notifications::SideNews;
    use iron_tide_sim::sim::player_commands::{GroundOrder, PlayerCommand};
    use iron_tide_sim::sim::production::{BuildItem, EnqueueRefusal, placement_context};
    use iron_tide_sim::sim::production::{Lane, Production};
    use iron_tide_sim::sim::spawn::{spawn_building, spawn_unit};
    use iron_tide_sim::world::map::GameMap;
    use iron_tide_sim::world::map::map_bounds;
    use iron_tide_sim::world::map::world_to_cell;
    use iron_tide_sim::world::sites::StartingSites;

    fn follow(mirror: &mut WorldMirror, player: &mut LocalConnection) -> usize {
        let mut bytes = 0;
        for arrival in player.drain() {
            if let Arrival::Message(message @ Message::WorldView(_)) = arrival {
                bytes += serde_json::to_vec(&message).unwrap().len();
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
            player: PlayerKey::new_random(),
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
        assert!(mirror.things.values().any(|thing| thing.team == team));
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
        println!("a minute of a world's view: {bytes} bytes of JSON");

        let tank = mirror
            .things
            .values()
            .find(|thing| thing.team == team && thing.thing == Thing::Unit(UnitKind::Tank))
            .copied()
            .expect("the player sees their tank");
        let goal = iron_tide_sim::world::map::world_to_cell(tank.position()) + IVec2::new(0, -12);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::Ground {
                units: vec![tank.id],
                goal,
                order: GroundOrder::Move,
                queued: false,
                refinery: None,
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
        println!("ten seconds of a tank driving: {moving_bytes} bytes of JSON");
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
            player: PlayerKey::new_random(),
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
        host.step(SIMULATION_TICK);
        host.step(SIMULATION_TICK);
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
            player: key.clone(),
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

        let (_back, _) = enter_as(&mut host, &key);
        host.step(SIMULATION_TICK);
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
        assert_ne!(new_team, team);
    }

    fn scratch_directory(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("iron_tide_{name}_{}", std::process::id()));
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
            player: key.clone(),
        });
        resumed.step(SIMULATION_TICK);
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
        let goal = iron_tide_sim::world::map::world_to_cell(start) + IVec2::new(0, 6);
        second.send(&Message::Orders {
            commands: vec![PlayerCommand::Ground {
                units: vec![id],
                goal,
                order: GroundOrder::Move,
                queued: false,
                refinery: None,
            }],
        });
        for _ in 0..100 {
            host.step(SIMULATION_TICK);
        }
        let moved = host.world().get::<Transform>(tank).unwrap().translation;
        assert!(moved.distance(start) > 3.0, "the tank stayed at {start}");
    }

    #[test]
    fn a_player_of_another_version_is_turned_away() {
        let mut host = WorldHost::new(29);
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: "0.0.1".into(),
            player: PlayerKey::new_random(),
        });
        host.step(SIMULATION_TICK);
        let refused = player
            .drain()
            .into_iter()
            .any(|arrival| matches!(arrival, Arrival::Message(Message::Refused { .. })));
        assert!(refused);
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
