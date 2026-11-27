//! The dedicated network battle server:
//! `rimefront-server [--port 47300] [--world SEED] [--world-dir DIR] [--save-every SECS] [--most-rooms N]
//! [--room-patience MINUTES] [--era-days 7] [--finale-days 3] [--era-hour 20] [--first-era antiquity]`.
//! Players join
//! `ws://host:port/room/NAME`, and with `--world` or `--world-dir` enter a persistent world at
//! `ws://host:port/world`; with `--world-dir` the world is saved there and resumed from there. Ctrl-C or
//! a termination signal saves the world before the server stops.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rimefront_net::lobby::DEFAULT_PORT;
use rimefront_net::server::{RoomLimits, Server};
use rimefront_net::world_host::WorldHost;
use rimefront_net::world_store::WorldStore;
use rimefront_sim::sim::eras::{AgeSchedule, Era};

/// Short enough that relaying adds no noticeable delay to a turn, long enough to leave the processor idle.
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const DEFAULT_SAVE_SECS: u64 = 30;

fn argument_after<T: std::str::FromStr>(flag: &str) -> Option<T> {
    let arguments: Vec<String> = std::env::args().collect();
    arguments
        .iter()
        .position(|argument| argument == flag)
        .and_then(|at| arguments.get(at + 1))
        .and_then(|value| value.parse().ok())
}

/// When a new world's ages turn; a world resumed keeps the schedule it was grown with.
fn age_schedule() -> std::io::Result<AgeSchedule> {
    let defaults = AgeSchedule::default();
    let first = match argument_after::<String>("--first-era") {
        Some(name) => Era::named(&name)
            .filter(|era| Era::playable().any(|playable| playable == *era))
            .ok_or_else(|| {
                std::io::Error::other(format!("no age called {name} has an army yet"))
            })?,
        None => defaults.age,
    };
    Ok(AgeSchedule::starting_in(
        first,
        argument_after("--era-days").unwrap_or(defaults.era_hours / 24),
        argument_after("--era-hour").unwrap_or(defaults.turn_hour),
    )
    .with_finale_days(argument_after("--finale-days").unwrap_or(defaults.finale_hours / 24)))
}

/// The world to run: the one saved in `--world-dir` if there is one, otherwise a new one from `--world`.
fn world_to_run() -> std::io::Result<Option<WorldHost>> {
    let map_seed = argument_after::<u64>("--world");
    let Some(directory) = argument_after::<PathBuf>("--world-dir") else {
        let Some(seed) = map_seed else {
            return Ok(None);
        };
        return Ok(Some(WorldHost::with_ages(seed, Some(age_schedule()?))));
    };
    let mut store = WorldStore::at(&directory);
    let mut host = match store.load()? {
        Some(saved) => {
            if map_seed.is_some_and(|seed| seed != saved.map_seed()) {
                println!(
                    "{} holds the world of seed {}",
                    directory.display(),
                    saved.map_seed()
                );
            }
            println!(
                "Resuming the world of {} at tick {}",
                directory.display(),
                saved.tick()
            );
            WorldHost::resume(&saved)
        }
        None => {
            let map_seed = map_seed.ok_or_else(|| {
                std::io::Error::other(format!(
                    "{} holds no world; give --world SEED to grow one",
                    directory.display()
                ))
            })?;
            WorldHost::with_ages(map_seed, Some(age_schedule()?))
        }
    };
    let every = Duration::from_secs(argument_after("--save-every").unwrap_or(DEFAULT_SAVE_SECS));
    host.keep_in(store, every)?;
    Ok(Some(host))
}

fn main() -> std::io::Result<()> {
    let port = argument_after("--port").unwrap_or(DEFAULT_PORT);
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    let defaults = RoomLimits::default();
    let limits = RoomLimits {
        most_rooms: argument_after("--most-rooms").unwrap_or(defaults.most_rooms),
        gathering_for: argument_after::<u64>("--room-patience")
            .map_or(defaults.gathering_for, |minutes| {
                Duration::from_secs(minutes * 60)
            }),
    };
    let mut server = match world_to_run()? {
        Some(world) => Server::hosting(listener, world)?,
        None => Server::new(listener)?,
    }
    .with_room_limits(limits);
    let stopping = Arc::new(AtomicBool::new(false));
    let asked_to_stop = Arc::clone(&stopping);
    ctrlc::set_handler(move || asked_to_stop.store(true, Ordering::Relaxed))
        .map_err(std::io::Error::other)?;
    println!(
        "Rimefront server {} on port {port}",
        env!("CARGO_PKG_VERSION")
    );
    while !stopping.load(Ordering::Relaxed) {
        server.step();
        std::thread::sleep(POLL_INTERVAL);
    }
    if let Some(report) = server.save_world()? {
        println!(
            "World saved: {} of {} chunks written",
            report.chunks_written, report.chunks
        );
    }
    println!("Stopped");
    Ok(())
}
