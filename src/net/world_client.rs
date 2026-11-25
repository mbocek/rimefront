use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::app::match_flow::{GameState, MirroredBattle};
use crate::missions::levels::ActiveLevel;
use crate::net::lobby::DEFAULT_PORT;
use crate::net::network::{Arrival, Dialling, JoinTarget, Link, Message};
use crate::net::world_host::{WorldConnection, WorldHost, local_connection, world_level};
use crate::net::world_view::{Effect, SideState, Spot, Thing, ThingState, ViewChange};
use crate::sim::catalog::Team;
use crate::sim::components::{Building, Health};
use crate::sim::cues::{BlastCue, SoundCue};
use crate::sim::determinism::SimEntities;
use crate::sim::economy::Economies;
use crate::sim::intel::Intel;
use crate::sim::notifications::News;
use crate::sim::player_commands::{CommandQueue, LocalTeam, SimulationTick};
use crate::sim::sides::Sides;
use crate::sim::spawn::{spawn_building, spawn_unit};
use crate::view::effects::ShotFlight;
use crate::world::map::{CrystalVisual, GameMap, crystal_scale, map_bounds};
use crate::world::wilds::widen_world;

/// How long a world the game runs itself sleeps between steps; short enough to answer at once.
const LOCAL_WORLD_POLL: Duration = Duration::from_millis(2);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The world this game plays in, run elsewhere: on a server, or on a thread of the game's own.
#[derive(Resource)]
pub struct WorldClient {
    connection: Mutex<Box<dyn WorldConnection>>,
    team: Option<Team>,
    /// Views that arrived and are not yet shown.
    waiting: Vec<ViewChange>,
    side: SideState,
    /// How many ticks the world is ahead of the battle here.
    world_tick_lead: i64,
    lost: bool,
}

impl WorldClient {
    fn entering(connection: Box<dyn WorldConnection>) -> Self {
        connection.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
        });
        Self {
            connection: Mutex::new(connection),
            team: None,
            waiting: Vec::new(),
            side: SideState::default(),
            world_tick_lead: 0,
            lost: false,
        }
    }

    /// A world grown from `map_seed` on a thread of the game's own; it stops when this client goes.
    pub fn alone(map_seed: u64) -> Self {
        let (player, world_end) = local_connection();
        std::thread::spawn(move || {
            let mut host = WorldHost::new(map_seed);
            host.connect(Box::new(world_end));
            let mut last_step = Instant::now();
            while host.player_count() > 0 {
                host.step(last_step.elapsed());
                last_step = Instant::now();
                std::thread::sleep(LOCAL_WORLD_POLL);
            }
        });
        Self::entering(Box::new(player))
    }

    /// The world of the server at `address`, e.g. `ws://server:47300/world`.
    pub fn on_server(address: &str) -> Result<Self, String> {
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
        Ok(Self::entering(Box::new(link)))
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
                Arrival::Message(Message::Entered { team, map_seed, .. }) => {
                    self.team = Some(team);
                    entered = Some(map_seed);
                }
                Arrival::Message(Message::WorldView(change)) => self.waiting.push(*change),
                Arrival::Message(Message::Refused { reason }) => {
                    warn!("the world refused: {reason}");
                    self.lost = true;
                }
                Arrival::Message(_) => {}
                Arrival::ConnectionLost => self.lost = true,
            }
        }
        entered
    }
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
    commands.insert_resource(ActiveLevel(world_level(map_seed)));
    next_state.set(GameState::Playing);
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
        if let Some(bounds) = change.bounds.filter(|&bounds| bounds != map_bounds()) {
            widen_world(world, bounds);
        }
        world.resource_mut::<Economies>().of_mut(side).credits = change.credits;
        if let Some(state) = change.side {
            state.apply_to(world, side);
            world.resource_mut::<WorldClient>().side = state;
        }
        for id in change.hidden {
            if let Some(entity) = world.resource::<SimEntities>().get(id) {
                forget(world, entity);
            }
        }
        for state in change.shown {
            show(world, state);
        }
        for effect in change.effects {
            show_effect(world, side, effect);
        }
        show_crystal(world, &change.crystal);
    }
    show_support_recharge(world, side, local_tick);
}

fn show_support_recharge(world: &mut World, side: Team, local_tick: u64) {
    let client = world.resource::<WorldClient>();
    let world_tick = (local_tick as i64 + client.world_tick_lead).max(0) as u64;
    let state = client.side.clone();
    state.show_support_recharge(world, side, world_tick);
}

fn admit_own_side(world: &mut World, side: Team) {
    while world.resource::<Sides>().count() <= side.index() {
        world.resource_mut::<Sides>().join();
    }
    world.resource_mut::<Intel>().admit(side);
}

fn show(world: &mut World, state: ThingState) {
    let entity = match world.resource::<SimEntities>().get(state.id) {
        Some(entity) => entity,
        None => appear(world, state),
    };
    let Ok(mut thing) = world.get_entity_mut(entity) else {
        return;
    };
    if let Some(mut transform) = thing.get_mut::<Transform>()
        && matches!(state.thing, Thing::Unit(_))
    {
        transform.translation = state.position();
        transform.rotation = Quat::from_rotation_y(state.facing());
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
        Effect::Told { news, at, .. } => {
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
    for (entity, visual, mut transform) in visuals.iter_mut(world) {
        match left.get(&visual.cell) {
            Some(0) => mined_out.push(entity),
            Some(&amount) => transform.scale = Vec3::splat(visual.size * crystal_scale(amount)),
            None => {}
        }
    }
    for entity in mined_out {
        world.despawn(entity);
    }
}

fn appear(world: &mut World, state: ThingState) -> Entity {
    let entity = world.resource_scope(|world, mut map: Mut<GameMap>| {
        let mut commands = world.commands();
        match state.thing {
            Thing::Unit(kind) => spawn_unit(
                &mut commands,
                kind,
                state.team,
                state.position(),
                state.facing(),
            ),
            Thing::Building { kind, origin } => {
                spawn_building(&mut commands, &mut map, kind, state.team, origin)
            }
        }
    });
    world.flush();
    world.entity_mut(entity).insert(state.id);
    world.resource_mut::<SimEntities>().adopt(state.id, entity);
    entity
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

/// Leaving the battle leaves the world; a world the game ran itself stops with it.
pub fn leave_world(mut commands: Commands) {
    commands.remove_resource::<WorldClient>();
    commands.remove_resource::<MirroredBattle>();
}

/// Playing in a world that runs elsewhere.
pub struct WorldClientPlugin;

impl Plugin for WorldClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, enter_world)
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
