//! The dedicated network battle server:
//! `iron-tide-server [--port 47300] [--world SEED] [--world-dir DIR] [--save-every SECS]`. Players join
//! `ws://host:port/room/NAME`, and with `--world` or `--world-dir` enter a persistent world at
//! `ws://host:port/world`; with `--world-dir` the world is saved there and resumed from there. Ctrl-C or
//! a termination signal saves the world before the server stops.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use iron_tide_net::lobby::DEFAULT_PORT;
use iron_tide_net::server::Server;
use iron_tide_net::world_host::WorldHost;
use iron_tide_net::world_store::WorldStore;

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

/// The world to run: the one saved in `--world-dir` if there is one, otherwise a new one from `--world`.
fn world_to_run() -> std::io::Result<Option<WorldHost>> {
    let map_seed = argument_after::<u64>("--world");
    let Some(directory) = argument_after::<PathBuf>("--world-dir") else {
        return Ok(map_seed.map(WorldHost::new));
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
            WorldHost::new(map_seed)
        }
    };
    let every = Duration::from_secs(argument_after("--save-every").unwrap_or(DEFAULT_SAVE_SECS));
    host.keep_in(store, every)?;
    Ok(Some(host))
}

fn main() -> std::io::Result<()> {
    let port = argument_after("--port").unwrap_or(DEFAULT_PORT);
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    let mut server = match world_to_run()? {
        Some(world) => Server::hosting(listener, world)?,
        None => Server::new(listener)?,
    };
    let stopping = Arc::new(AtomicBool::new(false));
    let asked_to_stop = Arc::clone(&stopping);
    ctrlc::set_handler(move || asked_to_stop.store(true, Ordering::Relaxed))
        .map_err(std::io::Error::other)?;
    println!(
        "Iron Tide server {} on port {port}",
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
