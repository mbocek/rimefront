//! The dedicated network battle server: `iron-tide-server [--port 47300]`; players join `ws://host:port/room/NAME`.

use std::net::TcpListener;
use std::time::Duration;

use iron_tide_net::lobby::DEFAULT_PORT;
use iron_tide_net::server::Server;

/// Short enough that relaying adds no noticeable delay to a turn, long enough to leave the processor idle.
const POLL_INTERVAL: Duration = Duration::from_millis(2);

fn port_from_arguments() -> u16 {
    let arguments: Vec<String> = std::env::args().collect();
    arguments
        .iter()
        .position(|argument| argument == "--port")
        .and_then(|at| arguments.get(at + 1))
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

fn main() -> std::io::Result<()> {
    let port = port_from_arguments();
    let mut server = Server::new(TcpListener::bind(("0.0.0.0", port))?)?;
    println!(
        "Iron Tide server {} on port {port}",
        env!("CARGO_PKG_VERSION")
    );
    loop {
        server.step();
        std::thread::sleep(POLL_INTERVAL);
    }
}
