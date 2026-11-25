use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use iron_tide::SimulationPlugin;
use iron_tide::app::match_flow::{GameState, MirroredBattle};
use iron_tide::net::network::LockstepPlugin;
use iron_tide::net::world_client::{OwnWorld, WorldClient, WorldClientPlugin};
use iron_tide::net::world_host::PlayerKey;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::components::{Building, Unit};
use iron_tide::sim::determinism::{SimEntities, SimId, run_schedules_on_one_thread};
use iron_tide::sim::player_commands::{
    CommandQueue, GroundOrder, LocalTeam, PlayerCommand, SimulationTick,
};
use iron_tide::sim::production::{BuildItem, Lane, Production};
use iron_tide::world::map::world_to_cell;

const STEP: Duration = iron_tide::SIMULATION_TICK;

fn player_of_a_world_alone() -> App {
    player_of(WorldClient::alone(29))
}

fn player_of(client: WorldClient) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .add_plugins((
            SimulationPlugin::default(),
            LockstepPlugin,
            WorldClientPlugin,
        ))
        .insert_resource(client);
    run_schedules_on_one_thread(&mut app);
    app
}

/// Runs frames until `done` holds, giving the world on its thread real time to answer.
fn run_until(app: &mut App, mut done: impl FnMut(&mut World) -> bool) -> bool {
    for _ in 0..2_000 {
        app.update();
        if done(app.world_mut()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

fn own_tank(world: &mut World) -> Option<(Entity, SimId, Vec3)> {
    let side = world.resource::<LocalTeam>().0;
    world
        .query::<(Entity, &SimId, &Unit, &Team, &Transform)>()
        .iter(world)
        .find(|(_, _, unit, team, _)| **team == side && unit.0 == UnitKind::Tank)
        .map(|(entity, id, _, _, transform)| (entity, *id, transform.translation))
}

#[test]
fn the_game_plays_a_world_it_runs_itself() {
    let mut app = player_of_a_world_alone();
    let entered = run_until(&mut app, |world| {
        world.contains_resource::<MirroredBattle>()
            && *world.resource::<State<GameState>>().get() == GameState::Playing
            && own_tank(world).is_some()
    });
    assert!(entered, "the world never showed the player their forces");
    let world = app.world_mut();
    assert_eq!(world.resource::<LocalTeam>().0, Team::Player);
    let command_centers = world
        .query::<(&Building, &Team)>()
        .iter(world)
        .filter(|(building, team)| {
            building.kind == BuildingKind::CommandCenter && **team == Team::Player
        })
        .count();
    assert_eq!(command_centers, 1);

    let (tank, id, start) = own_tank(world).unwrap();
    assert_eq!(world.resource::<SimEntities>().get(id), Some(tank));
    let now = *world.resource::<SimulationTick>();
    world.resource_mut::<CommandQueue>().issue(
        now,
        Team::Player,
        PlayerCommand::Ground {
            units: vec![id],
            goal: world_to_cell(start) + IVec2::new(0, -8),
            order: GroundOrder::Move,
            queued: false,
            refinery: None,
        },
    );
    let drove = run_until(&mut app, |world| {
        world
            .get::<Transform>(tank)
            .is_some_and(|transform| transform.translation.distance(start) > 3.0)
    });
    assert!(drove, "the order never reached the world");

    let power_plant = BuildItem::Structure(BuildingKind::PowerPlant);
    let world = app.world_mut();
    let now = *world.resource::<SimulationTick>();
    world.resource_mut::<CommandQueue>().issue(
        now,
        Team::Player,
        PlayerCommand::Enqueue {
            item: power_plant,
            copies: 1,
        },
    );
    let building = run_until(&mut app, |world| {
        let lane = world
            .resource::<Production>()
            .lane(Team::Player, Lane::Structures);
        lane.queue.front() == Some(&power_plant) && lane.progress_fraction() > 0.0
    });
    assert!(
        building,
        "the side here never showed the world's production"
    );
}

fn entered_with_forces(app: &mut App) -> bool {
    run_until(app, |world| {
        *world.resource::<State<GameState>>().get() == GameState::Playing
            && own_tank(world).is_some()
    })
}

#[test]
fn a_world_played_alone_is_there_the_next_time() {
    let directory =
        std::env::temp_dir().join(format!("iron_tide_own_world_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let player = PlayerKey::new_random();
    let own = || OwnWorld {
        directory: directory.clone(),
        player: player.clone(),
    };

    let mut app = player_of(WorldClient::alone_in(own(), 29));
    assert!(
        entered_with_forces(&mut app),
        "the world never showed the player their forces"
    );
    let world = app.world_mut();
    let (tank, id, start) = own_tank(world).unwrap();
    let now = *world.resource::<SimulationTick>();
    let side = world.resource::<LocalTeam>().0;
    world.resource_mut::<CommandQueue>().issue(
        now,
        side,
        PlayerCommand::Ground {
            units: vec![id],
            goal: world_to_cell(start) + IVec2::new(0, -8),
            order: GroundOrder::Move,
            queued: false,
            refinery: None,
        },
    );
    let drove = run_until(&mut app, |world| {
        world
            .get::<Transform>(tank)
            .is_some_and(|transform| transform.translation.distance(start) > 6.0)
    });
    assert!(drove, "the tank never drove");
    let left_at = app.world().get::<Transform>(tank).unwrap().translation;
    drop(app);
    assert!(
        directory.join("world.bin").exists(),
        "the world was not kept"
    );

    let mut app = player_of(WorldClient::alone_in(own(), 77));
    assert!(
        entered_with_forces(&mut app),
        "the kept world never showed the player their forces"
    );
    let world = app.world_mut();
    assert_eq!(world.resource::<LocalTeam>().0, side);
    let (_, _, found_at) = own_tank(world).unwrap();
    assert!(
        found_at.distance(start) > 3.0 && found_at.distance(left_at) < 4.0,
        "the tank left at {left_at} is at {found_at}"
    );
    drop(app);
    std::fs::remove_dir_all(&directory).unwrap();
}
