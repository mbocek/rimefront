use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rimefront::net::network::{Dialling, Link, LobbyNews};
use rimefront::sim::catalog::Team;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tls");

/// A WebSocket server behind TLS, as a dedicated server behind a proxy looks to the game: it sends every
/// message straight back.
fn secure_echo_server() -> u16 {
    let chain = vec![CertificateDer::from_pem_file(format!("{FIXTURES}/server.crt")).unwrap()];
    let key = PrivateKeyDer::from_pem_file(format!("{FIXTURES}/server.key")).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(chain, key)
    .unwrap();
    let config = Arc::new(config);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let config = Arc::clone(&config);
            std::thread::spawn(move || {
                let connection = rustls::ServerConnection::new(config).unwrap();
                let Ok(mut socket) =
                    tungstenite::accept(rustls::StreamOwned::new(connection, stream))
                else {
                    return;
                };
                while let Ok(message) = socket.read() {
                    if message.is_binary() && socket.send(message).is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

fn test_roots() -> Arc<rustls::RootCertStore> {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_file(format!("{FIXTURES}/ca.crt")).unwrap())
        .unwrap();
    Arc::new(roots)
}

fn news_within(link: &mut Link, wanted: impl Fn(&LobbyNews) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if link.lobby_news().iter().any(&wanted) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn a_link_talks_over_wss_to_a_server_with_a_trusted_certificate() {
    let port = secure_echo_server();
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut link = Link::guest_side_to(
        stream,
        Dialling {
            url: format!("wss://localhost:{port}/room/secure"),
            trusted: Some(test_roots()),
        },
    )
    .unwrap();
    link.say(Team::Player, "over TLS");
    assert!(news_within(&mut link, |news| {
        *news == LobbyNews::Chat(Team::Player, "over TLS".to_string())
    }));
}

#[test]
fn a_link_refuses_a_server_whose_certificate_it_cannot_trust() {
    let port = secure_echo_server();
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut link = Link::guest_side_to(
        stream,
        Dialling {
            url: format!("wss://localhost:{port}/room/secure"),
            trusted: None,
        },
    )
    .unwrap();
    assert!(news_within(&mut link, |news| *news == LobbyNews::Left));
}
