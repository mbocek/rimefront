use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tungstenite::protocol::{Role, WebSocketConfig};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Connector, Message as WsMessage, WebSocket};

use crate::app::i18n::tr;
use crate::missions::levels::LevelDefinition;
use crate::net::network::{LARGEST_MESSAGE_BYTES, LobbyNews, Message, Offer, chat_line};
use crate::sim::catalog::Team;

pub enum Arrival {
    Message(Message),
    ConnectionLost,
}

/// Which end of the WebSocket handshake this machine plays: the host answers it, a guest opens it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Host,
    Guest,
}

/// Where a guest connects: the address to dial and the WebSocket URL to ask for there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct JoinTarget {
    pub address: String,
    pub url: String,
}

/// A server behind a TLS proxy listens where web browsers expect it.
const SECURE_PORT: u16 = 443;

impl JoinTarget {
    /// Accepts `host`, `host:port`, `ws://host[:port]/path` or `wss://host[:port]/path`; a path names a room
    /// on a dedicated server, and a host in the game ignores it.
    pub fn parse(text: &str, default_port: u16) -> Result<JoinTarget, String> {
        let text = text.trim();
        let (scheme, rest, port) = match text.strip_prefix("wss://") {
            Some(rest) => ("wss", rest, SECURE_PORT),
            None => (
                "ws",
                text.strip_prefix("ws://").unwrap_or(text),
                default_port,
            ),
        };
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if host.is_empty() {
            return Err(tr("Type the address of the host").to_string());
        }
        let address = if host.contains(':') {
            host.to_string()
        } else {
            format!("{host}:{port}")
        };
        let url = format!("{scheme}://{address}/{path}");
        Ok(JoinTarget { address, url })
    }
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

/// One WebSocket connection to another machine, plain or over TLS. Each message is one binary frame of JSON,
/// queued by `send` for the connection's own threads.
pub struct Link {
    outbox: Sender<Vec<u8>>,
    /// The connection's thread has ended: the handshake failed or the connection went.
    failed: Arc<AtomicBool>,
    /// The handshake is done; from then on dropping the link sends what is queued before closing.
    serving: Arc<AtomicBool>,
    /// On the answering side, the path the other end asked for, once its handshake has arrived.
    requested_path: Arc<Mutex<Option<String>>>,
    /// The underlying socket, to close it outright when the link is dropped.
    raw: TcpStream,
    inbox: Mutex<Receiver<Arrival>>,
    /// Battle messages that arrived while the lobby was still being set up.
    early: Vec<Arrival>,
}

/// How a guest's link opens: the URL to ask for, and for `wss://` the certificates to trust; `None` trusts
/// the public web's.
pub struct Dialling {
    pub url: String,
    pub trusted: Option<Arc<rustls::RootCertStore>>,
}

/// How often a TLS connection's thread stops waiting for messages to send queued ones.
const SOCKET_POLL: Duration = Duration::from_millis(5);

fn socket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(LARGEST_MESSAGE_BYTES))
        .max_frame_size(Some(LARGEST_MESSAGE_BYTES))
}

/// The certificates of the public web, which a dedicated server behind an ordinary TLS proxy presents.
fn public_roots() -> Arc<rustls::RootCertStore> {
    Arc::new(rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    })
}

fn tls_connector(trusted: Arc<rustls::RootCertStore>) -> Option<Connector> {
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .ok()?
    .with_root_certificates(trusted)
    .with_no_client_auth();
    Some(Connector::Rustls(Arc::new(config)))
}

// tungstenite's handshake callback must return its own `ErrorResponse`, however large clippy finds it.
#[allow(clippy::result_large_err)]
fn handshake(
    stream: TcpStream,
    side: Side,
    dialling: Option<Dialling>,
    requested_path: &Mutex<Option<String>>,
) -> Option<Socket> {
    match side {
        Side::Host => {
            let remember_path =
                |request: &tungstenite::handshake::server::Request,
                 response: tungstenite::handshake::server::Response| {
                    if let Ok(mut path) = requested_path.lock() {
                        *path = Some(request.uri().path().to_string());
                    }
                    Ok(response)
                };
            tungstenite::accept_hdr_with_config(
                MaybeTlsStream::Plain(stream),
                remember_path,
                Some(socket_config()),
            )
            .ok()
        }
        Side::Guest => {
            let Dialling { url, trusted } = match dialling {
                Some(dialling) => dialling,
                None => Dialling {
                    url: format!("ws://{}/", stream.peer_addr().ok()?),
                    trusted: None,
                },
            };
            let connector = if url.starts_with("wss://") {
                tls_connector(trusted.unwrap_or_else(public_roots))?
            } else {
                Connector::Plain
            };
            tungstenite::client_tls_with_config(url, stream, Some(socket_config()), Some(connector))
                .ok()
                .map(|(socket, _)| socket)
        }
    }
}

fn is_waiting(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(io)
        if matches!(io.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
}

fn read_arrival(socket: &mut Socket) -> Option<Arrival> {
    match socket.read() {
        Ok(WsMessage::Binary(bytes)) => {
            Some(serde_json::from_slice(&bytes).map_or(Arrival::ConnectionLost, Arrival::Message))
        }
        Ok(WsMessage::Close(_)) => Some(Arrival::ConnectionLost),
        Ok(_) => None,
        Err(error) if is_waiting(&error) => None,
        Err(_) => Some(Arrival::ConnectionLost),
    }
}

/// A plain connection is read on this thread and written on another, each through its own handle, so a
/// queued message leaves at once. Writing from a second handle is safe because the reading one only writes
/// when it answers a ping or a close, and these peers send neither while the battle runs.
fn serve_split(
    mut socket: Socket,
    writer: TcpStream,
    side: Side,
    outbox: Receiver<Vec<u8>>,
    inbox: &Sender<Arrival>,
) {
    let role = match side {
        Side::Host => Role::Server,
        Side::Guest => Role::Client,
    };
    let mut writer =
        WebSocket::from_raw_socket(MaybeTlsStream::Plain(writer), role, Some(socket_config()));
    std::thread::spawn(move || {
        for bytes in outbox {
            if writer.send(WsMessage::Binary(bytes.into())).is_err() {
                return;
            }
        }
        if let MaybeTlsStream::Plain(stream) = writer.get_mut() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    });
    loop {
        let Some(arrival) = read_arrival(&mut socket) else {
            continue;
        };
        let lost = matches!(arrival, Arrival::ConnectionLost);
        if inbox.send(arrival).is_err() || lost {
            return;
        }
    }
}

/// A TLS connection keeps its state in one place, so one thread owns it: it reads with a short timeout and
/// sends what was queued in between, at the cost of up to `SOCKET_POLL` before a message leaves.
fn serve_polling(mut socket: Socket, outbox: &Receiver<Vec<u8>>, inbox: &Sender<Arrival>) {
    loop {
        loop {
            match outbox.try_recv() {
                Ok(bytes) => {
                    if socket.send(WsMessage::Binary(bytes.into())).is_err() {
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = socket.close(None);
                    let _ = socket.flush();
                    return;
                }
            }
        }
        let arrival = match socket.read() {
            Ok(WsMessage::Binary(bytes)) => {
                serde_json::from_slice(&bytes).map_or(Arrival::ConnectionLost, Arrival::Message)
            }
            Ok(WsMessage::Close(_)) => return,
            Ok(_) => continue,
            Err(error) if is_waiting(&error) => continue,
            Err(_) => return,
        };
        let lost = matches!(arrival, Arrival::ConnectionLost);
        if inbox.send(arrival).is_err() || lost {
            return;
        }
    }
}

impl Link {
    pub fn host_side(stream: TcpStream) -> std::io::Result<Self> {
        Self::open_with(stream, Side::Host, None)
    }

    pub fn guest_side(stream: TcpStream) -> std::io::Result<Self> {
        Self::open_with(stream, Side::Guest, None)
    }

    /// A guest's link that asks the other end for `dialling.url`, which may name a room and may be `wss://`.
    pub fn guest_side_to(stream: TcpStream, dialling: Dialling) -> std::io::Result<Self> {
        Self::open_with(stream, Side::Guest, Some(dialling))
    }

    pub fn open(stream: TcpStream, side: Side) -> std::io::Result<Self> {
        Self::open_with(stream, side, None)
    }

    fn open_with(
        stream: TcpStream,
        side: Side,
        dialling: Option<Dialling>,
    ) -> std::io::Result<Self> {
        stream.set_nodelay(true)?;
        let raw = stream.try_clone()?;
        let second_handle = stream.try_clone()?;
        let failed = Arc::new(AtomicBool::new(false));
        let serving = Arc::new(AtomicBool::new(false));
        let requested_path = Arc::new(Mutex::new(None));
        let (outbox, queued) = channel::<Vec<u8>>();
        let (arrived, inbox) = channel();
        let thread_failed = Arc::clone(&failed);
        let thread_serving = Arc::clone(&serving);
        let thread_path = Arc::clone(&requested_path);
        std::thread::spawn(move || {
            if let Some(socket) = handshake(stream, side, dialling, &thread_path) {
                thread_serving.store(true, Ordering::Relaxed);
                let encrypted = matches!(socket.get_ref(), MaybeTlsStream::Rustls(_));
                if !encrypted {
                    serve_split(socket, second_handle, side, queued, &arrived);
                } else if second_handle.set_read_timeout(Some(SOCKET_POLL)).is_ok() {
                    serve_polling(socket, &queued, &arrived);
                }
            }
            thread_failed.store(true, Ordering::Relaxed);
            let _ = arrived.send(Arrival::ConnectionLost);
        });
        Ok(Self {
            outbox,
            failed,
            serving,
            requested_path,
            raw,
            inbox: Mutex::new(inbox),
            early: Vec::new(),
        })
    }

    /// The path the other end asked for in its handshake, e.g. `/room/K7Q2`; `None` until it has arrived.
    pub fn requested_path(&self) -> Option<String> {
        self.requested_path
            .lock()
            .ok()
            .and_then(|path| path.clone())
    }

    /// Whether the connection failed before or after its handshake.
    pub fn has_failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    /// Queues the message for the connection's thread; false once the connection has gone.
    pub fn send(&self, message: &Message) -> bool {
        let Ok(bytes) = serde_json::to_vec(message) else {
            return true;
        };
        !self.has_failed() && self.outbox.send(bytes).is_ok()
    }

    pub fn drain(&mut self) -> Vec<Arrival> {
        let mut arrivals = std::mem::take(&mut self.early);
        if let Ok(inbox) = self.inbox.lock() {
            arrivals.extend(inbox.try_iter());
        }
        arrivals
    }

    /// Shows a guest the host's battle.
    pub fn offer(&self, level: &LevelDefinition, your_team: Option<Team>, players: &[Team]) {
        self.send(&Message::Offer {
            version: env!("CARGO_PKG_VERSION").to_string(),
            level: Box::new(level.clone()),
            your_team,
            players: players.to_vec(),
            under_way: false,
        });
    }

    /// Offers a battle under way: to watch, or to take back the seat of a player who lost the connection.
    pub fn offer_under_way(
        &self,
        level: &LevelDefinition,
        your_team: Option<Team>,
        players: &[Team],
    ) {
        self.send(&Message::Offer {
            version: env!("CARGO_PKG_VERSION").to_string(),
            level: Box::new(level.clone()),
            your_team,
            players: players.to_vec(),
            under_way: true,
        });
    }

    pub fn accept_offer(&self) {
        self.send(&Message::Ready);
    }

    /// Sets the battle of the room this link leads to on a dedicated server.
    pub fn open_room(&self, level: &LevelDefinition, players: usize) {
        self.send(&Message::OpenRoom {
            version: env!("CARGO_PKG_VERSION").to_string(),
            level: Box::new(level.clone()),
            players,
        });
    }

    pub fn start_room(&self) {
        self.send(&Message::Start);
    }

    pub fn refuse(&self, reason: &str) {
        self.send(&Message::Refused {
            reason: reason.to_string(),
        });
    }

    pub fn report_room(&self, seated: usize, ready: usize) {
        self.send(&Message::RoomStatus { seated, ready });
    }

    pub fn go(&self) {
        self.send(&Message::Go);
    }

    /// Sends a chat line in `team`'s name; it is cleaned first, and `None` when nothing printable is left.
    pub fn say(&self, team: Team, text: &str) -> Option<String> {
        let text = chat_line(text)?;
        self.send(&Message::Chat {
            team,
            text: text.clone(),
        });
        Some(text)
    }

    /// Passes a chat line from one guest on to another.
    pub fn relay_chat(&self, team: Team, text: &str) {
        self.send(&Message::Chat {
            team,
            text: text.to_string(),
        });
    }

    /// Lobby messages since the last call; battle messages that arrive early are kept for the battle.
    pub fn lobby_news(&mut self) -> Vec<LobbyNews> {
        let mut news = Vec::new();
        for arrival in self.drain() {
            match arrival {
                Arrival::Message(Message::Offer {
                    version,
                    level,
                    your_team,
                    players,
                    under_way,
                }) => news.push(LobbyNews::Offered(
                    if version == env!("CARGO_PKG_VERSION") {
                        Ok(Box::new(Offer {
                            level: *level,
                            your_team,
                            players,
                            under_way,
                        }))
                    } else {
                        Err(version)
                    },
                )),
                Arrival::Message(Message::Ready) => news.push(LobbyNews::GuestReady),
                Arrival::Message(Message::Go) => news.push(LobbyNews::Go),
                Arrival::Message(Message::RoomStatus { seated, ready }) => {
                    news.push(LobbyNews::RoomStatus { seated, ready });
                }
                Arrival::Message(Message::Refused { reason }) => {
                    news.push(LobbyNews::Refused(reason))
                }
                Arrival::Message(Message::Ping { nonce }) => {
                    self.send(&Message::Pong { nonce });
                }
                Arrival::Message(Message::Chat { team, text }) => {
                    news.extend(chat_line(&text).map(|text| LobbyNews::Chat(team, text)));
                }
                Arrival::ConnectionLost => news.push(LobbyNews::Left),
                battle => self.early.push(battle),
            }
        }
        news
    }
}

/// Once connected, dropping the link lets its threads send what is still queued and then close the
/// connection, so a last message such as a refusal arrives; a link still in its handshake closes at once.
impl Drop for Link {
    fn drop(&mut self) {
        if !self.serving.load(Ordering::Relaxed) {
            let _ = self.raw.shutdown(std::net::Shutdown::Both);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_join_address_may_be_a_host_a_port_or_a_websocket_url_with_a_room() {
        let plain = JoinTarget::parse("192.168.1.5", 47300).unwrap();
        assert_eq!(plain.address, "192.168.1.5:47300");
        assert_eq!(plain.url, "ws://192.168.1.5:47300/");
        let with_port = JoinTarget::parse(" example.org:9000 ", 47300).unwrap();
        assert_eq!(with_port.url, "ws://example.org:9000/");
        let room = JoinTarget::parse("ws://play.example.org/room/K7Q2", 47300).unwrap();
        assert_eq!(room.address, "play.example.org:47300");
        assert_eq!(room.url, "ws://play.example.org:47300/room/K7Q2");
        let secure = JoinTarget::parse("wss://play.example.org/room/K7Q2", 47300).unwrap();
        assert_eq!(secure.address, "play.example.org:443");
        assert_eq!(secure.url, "wss://play.example.org:443/room/K7Q2");
        assert!(JoinTarget::parse("ws://", 47300).is_err());
    }
}
