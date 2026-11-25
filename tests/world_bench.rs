use std::time::{Duration, Instant};

use bevy::prelude::*;
use iron_tide::SIMULATION_TICK;
use iron_tide::net::link::encode;
use iron_tide::net::network::{Arrival, Message};
use iron_tide::net::world_host::{
    LocalConnection, PlayerKey, WORLD_PROTOCOL, WorldConnection, WorldHost, local_connection,
};
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::components::{Building, Unit};
use iron_tide::sim::spawn::spawn_unit;
use iron_tide::world::map::cell_center;

const SECONDS_TO_SETTLE: u64 = 30;
const SECONDS_MEASURED: u64 = 120;

fn ticks(seconds: u64) -> u64 {
    seconds * Duration::from_secs(1).as_millis() as u64 / SIMULATION_TICK.as_millis() as u64
}

fn enter(host: &mut WorldHost) -> LocalConnection {
    let (player, world_end) = local_connection();
    host.connect(Box::new(world_end));
    player.send(&Message::EnterWorld {
        version: env!("CARGO_PKG_VERSION").into(),
        protocol: WORLD_PROTOCOL,
        player: PlayerKey::new_random(),
        color: None,
    });
    host.step(SIMULATION_TICK);
    player
}

/// The bytes on the wire of what the world sent this player since the last look.
fn take_views(player: &mut LocalConnection) -> usize {
    player
        .drain()
        .into_iter()
        .filter_map(|arrival| match arrival {
            Arrival::Message(message) => encode(&message).map(|bytes| bytes.len()),
            Arrival::ConnectionLost => None,
        })
        .sum()
}

/// Gives every side with a Command Center an army of `units` beside it.
fn raise_armies(host: &mut WorldHost, units: i32) {
    let world = host.world();
    let bases: Vec<(Team, IVec2)> = world
        .query::<(&Building, &Team)>()
        .iter(world)
        .filter(|(building, _)| building.kind == BuildingKind::CommandCenter)
        .map(|(building, team)| (*team, building.origin))
        .collect();
    let mut commands = world.commands();
    for (team, origin) in bases {
        for index in 0..units {
            let kind = if index % 4 == 0 {
                UnitKind::Tank
            } else {
                UnitKind::Rifleman
            };
            let cell = origin + IVec2::new(-6 + index % 12, 5 + index / 12);
            spawn_unit(&mut commands, kind, team, cell_center(cell), 0.0);
        }
    }
    world.flush();
}

fn count<T: Component>(host: &mut WorldHost) -> usize {
    let world = host.world();
    world.query_filtered::<(), With<T>>().iter(world).count()
}

/// How long a world takes a tick, and what it sends, with 4 to 64 players connected, each with the
/// computer neighbour they bring, and again with an army of 60 for every side:
/// `cargo test --release --test world_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn a_world_with_many_players() {
    for (players, army) in [(4, 0), (16, 0), (32, 0), (16, 60), (32, 60), (64, 60)] {
        let mut host = WorldHost::new(29);
        let mut connections: Vec<LocalConnection> =
            (0..players).map(|_| enter(&mut host)).collect();
        raise_armies(&mut host, army);
        for _ in 0..ticks(SECONDS_TO_SETTLE) {
            host.step(SIMULATION_TICK);
            connections.iter_mut().for_each(|player| {
                take_views(player);
            });
        }
        let mut slowest = Duration::ZERO;
        let mut bytes = 0;
        let started = Instant::now();
        for _ in 0..ticks(SECONDS_MEASURED) {
            let step = Instant::now();
            host.step(SIMULATION_TICK);
            slowest = slowest.max(step.elapsed());
            bytes += connections.iter_mut().map(take_views).sum::<usize>();
        }
        let per_tick = started.elapsed() / ticks(SECONDS_MEASURED) as u32;
        let units = count::<Unit>(&mut host);
        let structures = count::<Building>(&mut host);
        let bounds = iron_tide::world::map::map_bounds().size;
        println!(
            "{players} players: {units} units, {structures} structures on {}x{} cells; a tick {:.2} ms \
             on average, {:.2} ms at most; {:.0} B/s to each player",
            bounds.x,
            bounds.y,
            per_tick.as_secs_f64() * 1000.0,
            slowest.as_secs_f64() * 1000.0,
            bytes as f64 / players as f64 / SECONDS_MEASURED as f64,
        );
    }
}
