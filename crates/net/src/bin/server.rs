//! The dedicated network battle server: `iron-tide-server [--port 47300] [--world SEED]`; players join
//! `ws://host:port/room/NAME`, and with `--world` enter a persistent world at `ws://host:port/world`.

use std::net::TcpListener;
use std::time::Duration;

use iron_tide_net::lobby::DEFAULT_PORT;
use iron_tide_net::server::Server;

/// Short enough that relaying adds no noticeable delay to a turn, long enough to leave the processor idle.
const POLL_INTERVAL: Duration = Duration::from_millis(2);

fn argument_after<T: std::str::FromStr>(flag: &str) -> Option<T> {
    let arguments: Vec<String> = std::env::args().collect();
    arguments
        .iter()
        .position(|argument| argument == flag)
        .and_then(|at| arguments.get(at + 1))
        .and_then(|value| value.parse().ok())
}

fn main() -> std::io::Result<()> {
    let port = argument_after("--port").unwrap_or(DEFAULT_PORT);
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    let mut server = match argument_after::<u64>("--world") {
        Some(map_seed) => Server::with_world(listener, map_seed)?,
        None => Server::new(listener)?,
    };
    println!(
        "Iron Tide server {} on port {port}",
        env!("CARGO_PKG_VERSION")
    );
    loop {
        server.step();
        std::thread::sleep(POLL_INTERVAL);
    }
}
