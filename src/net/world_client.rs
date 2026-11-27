use std::collections::{BTreeSet, HashMap, HashSet};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::SIMULATION_TICK;
use crate::app::i18n::tr_owned;
use crate::app::match_flow::{GameState, MatchClock, MirroredBattle};
use crate::app::settings::game_data_directory;
use crate::missions::levels::ActiveLevel;
use crate::net::lobby::{DEFAULT_PORT, NetLobby};
use crate::net::network::{Arrival, Dialling, JoinTarget, Link, Message};
use crate::net::world_host::{
    PlayerKey, WORLD_PROTOCOL, WorldConnection, WorldHost, local_connection, world_level,
};
use crate::net::world_store::WorldStore;
use crate::net::world_view::{
    Effect, FACING_STEPS, SideState, Spot, Thing, ThingState, ViewChange,
};
use crate::sim::catalog::{Diplomacy, SideColor, Team};
use crate::sim::combat::{DamageSource, Destroyed};
use crate::sim::components::{Building, Health};
use crate::sim::cues::{BlastCue, SoundCue};
use crate::sim::determinism::{SimEntities, SimId};
use crate::sim::economy::Economies;
use crate::sim::eras::AgeSchedule;
use crate::sim::intel::Intel;
use crate::sim::neutral_sites::spawn_unclaimed_site;
use crate::sim::notifications::{News, SideNews};
use crate::sim::outcome::GameOutcome;
use crate::sim::player_commands::{CommandQueue, LocalTeam, SimulationTick};
use crate::sim::production::Production;
use crate::sim::sides::{Shelter, Sides, side_stands};
use crate::sim::spawn::{spawn_building, spawn_unit};
use crate::view::effects::ShotFlight;
use crate::view::side_tint::SideColors;
use crate::world::map::{CrystalVisual, GameMap, crystal_scale, map_bounds, spawn_crystal_visual};
use crate::world::season::Season;
use crate::world::wilds::{crystal_look, widen_world};

/// How long a world the game runs itself sleeps between steps; short enough to answer at once.
const LOCAL_WORLD_POLL: Duration = Duration::from_millis(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// A unit glides to where a view showed it over the ticks since the view before, but never longer than this,
/// so after a pause in the views it catches up at once instead of trailing behind.
const LONGEST_GLIDE_TICKS: u64 = 4;
const PLAYER_KEY_FILE: &str = "world_player_key";
const OWN_WORLD_DIRECTORY: &str = "worlds/own";
const PUT_ASIDE_SUFFIX: &str = "-previous";
const OWN_WORLD_SAVE_EVERY: Duration = Duration::from_secs(30);

/// The world this game plays in, run elsewhere: on a server, or on a thread of the game's own.
#[derive(Resource)]
pub struct WorldClient {
    connection: Mutex<Box<dyn WorldConnection>>,
    team: Option<Team>,
    /// Views that arrived and are not yet shown.
    waiting: Vec<ViewChange>,
    side: SideState,
    /// Every thing shown, as last sent, for the changes the world sends of them.
    shown: HashMap<SimId, ThingState>,
    /// How many ticks the world is ahead of the battle here.
    world_tick_lead: i64,
    /// The world's tick of the last view shown.
    last_view_tick: u64,
    lost: bool,
    /// Why the world turned the player away, in its words.
    refusal: Option<String>,
    /// The thread of a world the game runs itself.
    world_thread: Option<JoinHandle<()>>,
    /// Whether anything of the player's side was shown standing yet.
    side_was_shown: bool,
    /// When the world's ages turn, and its tick when the player entered.
    ages: Option<AgeSchedule>,
    entered_at_tick: u64,
}

impl WorldClient {
    fn entering(
        connection: Box<dyn WorldConnection>,
        player: PlayerKey,
        color: Option<SideColor>,
    ) -> Self {
        connection.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            color,
            player,
        });
        Self {
            connection: Mutex::new(connection),
            team: None,
            waiting: Vec::new(),
            side: SideState::default(),
            shown: HashMap::new(),
            world_tick_lead: 0,
            last_view_tick: 0,
            lost: false,
            refusal: None,
            world_thread: None,
            side_was_shown: false,
            ages: None,
            entered_at_tick: 0,
        }
    }

    /// A world grown from `map_seed` on a thread of the game's own, kept nowhere; it stops when this client
    /// goes.
    pub fn alone(map_seed: u64) -> Self {
        let (player, world_end) = local_connection();
        let thread = std::thread::spawn(move || {
            let mut host = WorldHost::with_ages(map_seed, Some(AgeSchedule::default()));
            host.connect(Box::new(world_end));
            run_until_left(&mut host);
        });
        Self::entering(Box::new(player), PlayerKey::new_random(), None).running(thread)
    }

    /// The world kept in `own`, or a new one grown from `map_seed` when it holds none, on a thread of the
    /// game's own. It is saved every half minute and when this client goes, as it stops then.
    pub fn alone_in(own: OwnWorld, map_seed: u64) -> Self {
        let (player, world_end) = local_connection();
        let directory = own.directory.clone();
        let thread = std::thread::spawn(move || {
            let mut store = WorldStore::at(&directory);
            let mut host = match store.load() {
                Ok(Some(saved)) => WorldHost::resume(&saved),
                Ok(None) => WorldHost::with_ages(map_seed, Some(AgeSchedule::default())),
                Err(error) => {
                    warn!(
                        "could not read the world in {}: {error}",
                        directory.display()
                    );
                    return;
                }
            };
            if let Err(error) = host.keep_in(store, OWN_WORLD_SAVE_EVERY) {
                warn!(
                    "could not save the world in {}: {error}",
                    directory.display()
                );
            }
            host.connect(Box::new(world_end));
            run_until_left(&mut host);
            if let Err(error) = host.save() {
                warn!(
                    "could not save the world in {}: {error}",
                    directory.display()
                );
            }
        });
        Self::entering(Box::new(player), own.player, own.color).running(thread)
    }

    fn running(mut self, thread: JoinHandle<()>) -> Self {
        self.world_thread = Some(thread);
        self
    }

    /// The world of the server at `address`, e.g. `ws://server:47300/world`, asking for `color` for the
    /// player's side.
    pub fn on_server(address: &str, color: Option<SideColor>) -> Result<Self, String> {
        let target = JoinTarget::parse(address, DEFAULT_PORT)?;
        let socket_address = std::net::ToSocketAddrs::to_socket_addrs(&target.address)
            .map_err(|error| error.to_string())?
            .next()
            .ok_or_else(|| format!("{} has no address", target.address))?;
        let stream = TcpStream::connect_timeout(&socket_address, CONNECT_TIMEOUT)
            .map_err(|error| error.to_string())?;
        let dialling = Dialling {
            url: target.url,
            trusted: None,
        };
        let link = Link::guest_side_to(stream, dialling).map_err(|error| error.to_string())?;
        Ok(Self::entering(Box::new(link), saved_player_key(), color))
    }

    pub fn team(&self) -> Option<Team> {
        self.team
    }

    pub fn is_lost(&self) -> bool {
        self.lost
    }

    fn send(&self, message: &Message) {
        if let Ok(connection) = self.connection.lock() {
            connection.send(message);
        }
    }

    /// Takes in what the world sent; returns the map seed once the world has let the player in.
    fn receive(&mut self) -> Option<u64> {
        let arrivals = match self.connection.get_mut() {
            Ok(connection) => connection.drain(),
            Err(_) => Vec::new(),
        };
        let mut entered = None;
        for arrival in arrivals {
            match arrival {
                Arrival::Message(Message::Entered {
                    team,
                    map_seed,
                    tick,
                    ages,
                }) => {
                    self.team = Some(team);
                    self.ages = ages;
                    self.entered_at_tick = tick;
                    entered = Some(map_seed);
                }
                Arrival::Message(Message::WorldView(change)) => self.waiting.push(*change),
                Arrival::Message(Message::Refused { reason }) => {
                    self.refusal = Some(reason);
                    self.lost = true;
                }
                Arrival::Message(_) => {}
                Arrival::ConnectionLost => self.lost = true,
            }
        }
        entered
    }
}

fn run_until_left(host: &mut WorldHost) {
    let mut last_step = Instant::now();
    while host.player_count() > 0 {
        host.step(last_step.elapsed());
        last_step = Instant::now();
        std::thread::sleep(LOCAL_WORLD_POLL);
    }
}

/// A world the game keeps for playing alone, and the player who plays it.
pub struct OwnWorld {
    pub directory: PathBuf,
    pub player: PlayerKey,
    /// The colour the player chose for their side; `None` leaves it to the world.
    pub color: Option<SideColor>,
}

impl OwnWorld {
    /// Puts the kept world aside, in place of the one put aside before, so the next one starts anew.
    pub fn put_aside(&self) -> std::io::Result<()> {
        if !self.directory.exists() {
            return Ok(());
        }
        let mut aside = self.directory.clone().into_os_string();
        aside.push(PUT_ASIDE_SUFFIX);
        let aside = PathBuf::from(aside);
        if aside.exists() {
            std::fs::remove_dir_all(&aside)?;
        }
        std::fs::rename(&self.directory, aside)
    }

    /// The one kept with the game's data.
    pub fn kept_with_the_game() -> Self {
        Self {
            directory: game_data_directory().join(OWN_WORLD_DIRECTORY),
            player: saved_player_key(),
            color: None,
        }
    }
}

/// Leaving lets the world on the game's own thread see the player go, save and stop, before the game goes on.
impl Drop for WorldClient {
    fn drop(&mut self) {
        if let Ok(connection) = self.connection.get_mut() {
            *connection = Box::new(Closed);
        }
        if let Some(thread) = self.world_thread.take() {
            let _ = thread.join();
        }
    }
}

/// The end of a connection that was let go.
struct Closed;

impl WorldConnection for Closed {
    fn send(&self, _: &Message) -> bool {
        false
    }

    fn drain(&mut self) -> Vec<Arrival> {
        vec![Arrival::ConnectionLost]
    }
}

/// The key this game comes back to the worlds of servers with, made the first time and kept with the game's
/// data; a world played alone stops with the game, so it needs none.
fn saved_player_key() -> PlayerKey {
    let path = game_data_directory().join(PLAYER_KEY_FILE);
    if let Some(key) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| PlayerKey::from_saved(&text))
    {
        return key;
    }
    let key = PlayerKey::new_random();
    let saved = std::fs::create_dir_all(game_data_directory())
        .and_then(|()| std::fs::write(&path, key.as_str()));
    if let Err(error) = saved {
        warn!("could not keep the world player key: {error}");
    }
    key
}

/// Once the world lets the player in, the battle starts as a copy of it, on the player's side.
pub fn enter_world(
    mut commands: Commands,
    client: Option<ResMut<WorldClient>>,
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(mut client) = client else { return };
    let Some(map_seed) = client.receive() else {
        return;
    };
    let Some(team) = client.team else { return };
    if *state.get() == GameState::Playing {
        return;
    }
    commands.insert_resource(MirroredBattle);
    commands.insert_resource(LocalTeam(team));
    let mut level = world_level(map_seed, client.ages);
    if let Some(ages) = client.ages {
        level.era = ages.era_at(client.entered_at_tick);
    }
    commands.insert_resource(ActiveLevel(level));
    commands.insert_resource(WorldPolitics::default());
    next_state.set(GameState::Playing);
}

/// Who the player may ally with in a world, and how their offers stand, for the pause menu.
#[derive(Resource, Default, Debug)]
pub struct WorldPolitics {
    /// The sides players hold, the player's own among them.
    pub players: Vec<Team>,
    pub allies: Vec<Team>,
    /// Sides that offered the player an alliance not answered yet.
    pub offered_us: BTreeSet<Team>,
    /// Sides the player offered an alliance, which did not answer yet.
    pub we_offered: BTreeSet<Team>,
}

impl WorldPolitics {
    fn allied_with(&mut self, allies: Vec<Team>) {
        for ally in &allies {
            self.offered_us.remove(ally);
            self.we_offered.remove(ally);
        }
        self.allies = allies;
    }
}

/// Every tick, shows what the world sent since the last: units and structures with the world's ids where
/// it placed them, the player's credits, and the world's new ground when it widened.
pub fn follow_world(world: &mut World) {
    let Some(mut client) = world.get_resource_mut::<WorldClient>() else {
        return;
    };
    client.receive();
    let changes = std::mem::take(&mut client.waiting);
    let Some(team) = client.team else { return };
    let side = world.resource::<LocalTeam>().0;
    admit_own_side(world, side);
    debug_assert_eq!(side, team);
    let local_tick = world.resource::<SimulationTick>().0;
    for change in changes {
        let mut client = world.resource_mut::<WorldClient>();
        client.world_tick_lead = change.tick as i64 - local_tick as i64;
        let ticks_since_last_view = change.tick.saturating_sub(client.last_view_tick);
        client.last_view_tick = change.tick;
        let glide_ticks = ticks_since_last_view.clamp(1, LONGEST_GLIDE_TICKS) as u32;
        if let Some(bounds) = change.bounds.filter(|&bounds| bounds != map_bounds()) {
            widen_world(world, bounds);
        }
        if let Some(credits) = change.credits {
            world.resource_mut::<Economies>().of_mut(side).credits = credits;
        }
        if let Some(state) = change.side {
            state.apply_to(world, side);
            world.resource_mut::<WorldClient>().side = state;
        }
        for id in change.hidden {
            world.resource_mut::<WorldClient>().shown.remove(&id);
            if let Some(entity) = world.resource::<SimEntities>().get(id) {
                forget(world, entity);
            }
        }
        let mut client = world.resource_mut::<WorldClient>();
        let mut states = change.shown;
        states.extend(
            change
                .moved
                .iter()
                .filter_map(|moved| client.shown.get(&moved.id).map(|state| state.after(moved))),
        );
        for state in &states {
            client.shown.insert(state.id, *state);
        }
        for state in states {
            show(world, state, glide_ticks);
        }
        for effect in change.effects {
            show_effect(world, side, effect);
        }
        show_crystal(world, &change.crystal);
        let mut colors = world.resource_mut::<SideColors>();
        for (team, color) in change.colors {
            colors.choose(team, color.color());
        }
        if let Some(sheltered) = change.sheltered {
            world.insert_resource(Shelter(sheltered.into_iter().collect()));
        }
        if let Some(allies) = change.allies {
            world.insert_resource(Diplomacy::with_allies(side, &allies));
            if let Some(mut politics) = world.get_resource_mut::<WorldPolitics>() {
                politics.allied_with(allies);
            }
        }
        if let (Some(players), Some(mut politics)) =
            (change.players, world.get_resource_mut::<WorldPolitics>())
        {
            politics.players = players;
        }
    }
    glide(world);
    let world_tick = world_tick_now(world, local_tick);
    show_support_recharge(world, side, world_tick);
    let world_secs = world_tick as f32 * SIMULATION_TICK.as_secs_f32();
    world.resource_mut::<MatchClock>().elapsed_secs = world_secs;
    let season = Season::of_world_at(world_secs);
    if world.resource::<ActiveLevel>().0.season != season {
        world.resource_mut::<ActiveLevel>().0.season = season;
    }
    follow_the_ages(world, world_tick);
    see_whether_the_side_fell(world, side);
}

/// The world turns the age itself, refunds and all; here the battle only takes it on, so that the build menu,
/// the models and the countdown follow without the age turning a second time.
fn follow_the_ages(world: &mut World, world_tick: u64) {
    let Some(ages) = world.resource::<WorldClient>().ages else {
        return;
    };
    let era = ages.era_at(world_tick);
    if world.resource::<ActiveLevel>().0.era == era {
        return;
    }
    world.resource_mut::<ActiveLevel>().0.era = era;
    world.resource_mut::<Production>().set_era(era);
}

fn world_tick_now(world: &World, local_tick: u64) -> u64 {
    let lead = world.resource::<WorldClient>().world_tick_lead;
    (local_tick as i64 + lead).max(0) as u64
}

fn show_support_recharge(world: &mut World, side: Team, world_tick: u64) {
    let state = world.resource::<WorldClient>().side.clone();
    state.show_support_recharge(world, side, world_tick);
}

/// The battle here is lost once nothing of the player's side stands any more, after it was shown standing.
fn see_whether_the_side_fell(world: &mut World, side: Team) {
    let stands = side_stands(world, side);
    let mut client = world.resource_mut::<WorldClient>();
    if stands {
        client.side_was_shown = true;
        return;
    }
    if client.side_was_shown && *world.resource::<GameOutcome>() == GameOutcome::Ongoing {
        *world.resource_mut::<GameOutcome>() = GameOutcome::Defeat;
    }
}

fn admit_own_side(world: &mut World, side: Team) {
    while world.resource::<Sides>().count() <= side.index() {
        world.resource_mut::<Sides>().join();
    }
    world.resource_mut::<Intel>().admit(side);
}

/// A unit on its way to where the world last showed it, a share of the way each tick until the next view.
#[derive(Component)]
struct Glide {
    from: Transform,
    to: Transform,
    ticks: u32,
    done: u32,
}

/// Moves every gliding unit a tick further along.
fn glide(world: &mut World) {
    let mut gliding = world.query::<(Entity, &mut Transform, &mut Glide)>();
    let mut arrived = Vec::new();
    for (entity, mut transform, mut glide) in gliding.iter_mut(world) {
        glide.done += 1;
        let share = glide.done as f32 / glide.ticks as f32;
        transform.translation = glide.from.translation.lerp(glide.to.translation, share);
        transform.rotation = glide.from.rotation.slerp(glide.to.rotation, share);
        if glide.done >= glide.ticks {
            arrived.push(entity);
        }
    }
    for entity in arrived {
        world.entity_mut(entity).remove::<Glide>();
    }
}

/// Shows a thing as the world last sent it; a unit already here glides there over `ticks`.
fn show(world: &mut World, state: ThingState, ticks: u32) {
    let known = world.resource::<SimEntities>().get(state.id);
    let Some(entity) = known.or_else(|| appear(world, state)) else {
        return;
    };
    let Ok(mut thing) = world.get_entity_mut(entity) else {
        return;
    };
    if let Some(mut transform) = thing.get_mut::<Transform>()
        && matches!(state.thing, Thing::Unit(_))
    {
        let to = Transform {
            translation: state.position(),
            rotation: Quat::from_rotation_y(state.facing()),
            ..*transform
        };
        match known {
            Some(_) => {
                let from = *transform;
                thing.insert(Glide {
                    from,
                    to,
                    ticks,
                    done: 0,
                });
            }
            None => *transform = to,
        }
    }
    if let Some(mut health) = thing.get_mut::<Health>() {
        health.current = health.max * f32::from(state.health_percent) / 100.0;
    }
}

fn show_effect(world: &mut World, side: Team, effect: Effect) {
    match effect {
        Effect::Shot(shot) => {
            let (start, end) = (shot.start.position(), shot.end.position());
            let sight = world.resource::<Intel>().vision(side);
            if !sight.is_visible(start) && !sight.is_visible(end) {
                return;
            }
            world.spawn((
                ShotFlight {
                    style: shot.style,
                    start,
                    end,
                    arc_height: shot.arc_height(),
                    duration: shot.duration(),
                    elapsed: 0.0,
                },
                Transform::from_translation(start),
                DespawnOnExit(GameState::Playing),
            ));
        }
        Effect::Blast { size, at } => {
            world.write_message(BlastCue::at(at.position(), size));
        }
        Effect::Sound { sound, at } => {
            world.write_message(SoundCue::at(sound, at.position()));
        }
        Effect::Fell {
            unit,
            team,
            at,
            facing,
        } => {
            world.write_message(Destroyed {
                entity: Entity::PLACEHOLDER,
                team,
                what: DamageSource::Unit(unit),
                position: at.position(),
                rotation: Quat::from_rotation_y(facing as f32 / FACING_STEPS),
            });
        }
        Effect::Told { news, at, .. } => {
            if let (SideNews::AllianceOffered(from), Some(mut politics)) =
                (news, world.get_resource_mut::<WorldPolitics>())
            {
                politics.offered_us.insert(from);
            }
            world.write_message(News {
                team: side,
                news,
                at: at.map(Spot::position),
            });
        }
    }
}

/// Fields shrink and mined-out cells clear as the world says.
fn show_crystal(world: &mut World, changes: &[(IVec2, u32)]) {
    if changes.is_empty() {
        return;
    }
    let mut map = world.resource_mut::<GameMap>();
    for &(cell, amount) in changes {
        map.set_crystal(cell, amount);
    }
    let left: HashMap<IVec2, u32> = changes.iter().copied().collect();
    let mut visuals = world.query::<(Entity, &CrystalVisual, &mut Transform)>();
    let mut mined_out = Vec::new();
    let mut shown = HashSet::new();
    for (entity, visual, mut transform) in visuals.iter_mut(world) {
        shown.insert(visual.cell);
        match left.get(&visual.cell) {
            Some(0) => mined_out.push(entity),
            Some(&amount) => transform.scale = Vec3::splat(visual.size * crystal_scale(amount)),
            None => {}
        }
    }
    for entity in mined_out {
        world.despawn(entity);
    }
    let map_seed = world.resource::<ActiveLevel>().0.map_seed;
    let mut commands = world.commands();
    for (&cell, &amount) in &left {
        if amount > 0 && !shown.contains(&cell) {
            spawn_crystal_visual(
                &mut commands,
                &mut crystal_look(map_seed, cell),
                cell,
                amount,
            );
        }
    }
    world.flush();
}

/// `None` for a thing of nobody's that is not a neutral site, which the world never sends.
fn appear(world: &mut World, state: ThingState) -> Option<Entity> {
    let entity = world.resource_scope(|world, mut map: Mut<GameMap>| {
        let mut commands = world.commands();
        match (state.thing, state.team) {
            (Thing::Unit(kind), Some(team)) => Some(spawn_unit(
                &mut commands,
                kind,
                team,
                state.position(),
                state.facing(),
            )),
            (Thing::Building { kind, origin }, Some(team)) => {
                Some(spawn_building(&mut commands, &mut map, kind, team, origin))
            }
            (Thing::Building { kind, origin }, None) => {
                Some(spawn_unclaimed_site(&mut commands, &mut map, kind, origin))
            }
            (Thing::Unit(_), None) => None,
        }
    })?;
    world.flush();
    world.entity_mut(entity).insert(state.id);
    world.resource_mut::<SimEntities>().adopt(state.id, entity);
    Some(entity)
}

fn forget(world: &mut World, entity: Entity) {
    if let Some(building) = world.get::<Building>(entity) {
        let (origin, footprint) = (building.origin, building.kind.spec().footprint);
        world
            .resource_mut::<GameMap>()
            .set_building(origin, footprint, None);
    }
    world.despawn(entity);
}

/// Orders the player gives go to the world instead of the battle here.
pub fn send_orders_to_world(client: Option<Res<WorldClient>>, mut queue: ResMut<CommandQueue>) {
    let Some(client) = client else { return };
    let commands: Vec<_> = queue
        .take_all()
        .into_iter()
        .map(|scheduled| scheduled.command)
        .collect();
    if !commands.is_empty() {
        client.send(&Message::Orders { commands });
    }
}

/// A world that turned the player away or whose connection went is given up: the network page says why,
/// and a battle in it ends.
pub fn give_up_a_lost_world(
    mut commands: Commands,
    client: Option<Res<WorldClient>>,
    lobby: Option<ResMut<NetLobby>>,
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(client) = client.filter(|client| client.is_lost()) else {
        return;
    };
    let reason = client
        .refusal
        .as_deref()
        .unwrap_or("The connection to the world was lost");
    warn!("left the world: {reason}");
    if let Some(mut lobby) = lobby {
        lobby.fail(tr_owned(reason));
    }
    commands.remove_resource::<WorldClient>();
    commands.remove_resource::<WorldPolitics>();
    if *state.get() == GameState::Playing {
        next_state.set(GameState::MainMenu);
    }
}

/// Leaving the battle leaves the world; a world the game ran itself stops with it.
pub fn leave_world(mut commands: Commands) {
    commands.remove_resource::<WorldClient>();
    commands.remove_resource::<WorldPolitics>();
    commands.remove_resource::<MirroredBattle>();
}

/// Playing in a world that runs elsewhere.
pub struct WorldClientPlugin;

impl Plugin for WorldClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SideColors>()
            .add_systems(PreUpdate, (enter_world, give_up_a_lost_world).chain())
            .add_systems(
                FixedUpdate,
                (follow_world, send_orders_to_world)
                    .chain()
                    .in_set(crate::TickStep::SendOrders)
                    .run_if(resource_exists::<MirroredBattle>),
            )
            .add_systems(OnExit(GameState::Playing), leave_world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_glides_to_where_the_world_showed_it_over_the_ticks_between_views() {
        let mut world = World::new();
        let to = Transform::from_xyz(4.0, 0.0, 0.0);
        let unit = world
            .spawn((
                Transform::default(),
                Glide {
                    from: Transform::default(),
                    to,
                    ticks: 2,
                    done: 0,
                },
            ))
            .id();
        glide(&mut world);
        assert_eq!(world.get::<Transform>(unit).unwrap().translation.x, 2.0);
        glide(&mut world);
        assert_eq!(world.get::<Transform>(unit).unwrap().translation.x, 4.0);
        assert!(
            world.get::<Glide>(unit).is_none(),
            "still gliding once there"
        );
    }
}
