mod common;

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bevy::prelude::*;
use common::Battle;
use iron_tide::missions::levels::{ActiveLevel, LevelDefinition};
use iron_tide::net::lobby::{BattleStart, NetLobby};
use iron_tide::net::network::{Arrival, Dialling, Link, Message, SessionState};
use iron_tide::net::server::Server;
use iron_tide::net::world_host::{PlayerKey, WORLD_PROTOCOL};
use iron_tide::sim::catalog::{Team, UnitKind};
use iron_tide::sim::determinism::simulation_checksum;
use iron_tide::sim::player_commands::{GroundOrder, PlayerCommand};

const STEP_SECS: f32 = 0.05;

/// A dedicated server on its own thread, as it would run on the internet; it stops when dropped.
struct RunningServer {
    address: String,
    stop: Arc<AtomicBool>,
}

impl RunningServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut server = Server::new(listener).unwrap();
            while !stopping.load(Ordering::Relaxed) {
                server.step();
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        Self { address, stop }
    }

    fn room(&self, name: &str) -> String {
        format!("ws://{}/room/{name}", self.address)
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn standard_level() -> LevelDefinition {
    Battle::standard()
        .world()
        .resource::<ActiveLevel>()
        .0
        .clone()
}

/// Moves lobbies along on a shared clock, accepting offers and starting when it can, until all have begun;
/// `battles` keep running meanwhile, as a battle under way would.
fn start_through_lobbies(
    lobbies: &mut [&mut NetLobby],
    battles: &mut [&mut Battle],
) -> Vec<BattleStart> {
    let mut clock = 0.0;
    let mut started: Vec<Option<BattleStart>> = lobbies.iter().map(|_| None).collect();
    for _ in 0..20_000 {
        clock += 0.002;
        std::thread::sleep(Duration::from_millis(1));
        for battle in battles.iter_mut() {
            battle.run_for(STEP_SECS);
        }
        for (lobby, start) in lobbies.iter_mut().zip(started.iter_mut()) {
            if start.is_some() {
                continue;
            }
            if lobby.can_accept_offer() {
                lobby.accept_offer();
            }
            if lobby.can_start() {
                lobby.start_battle();
            }
            *start = lobby.advance(clock);
        }
        if started.iter().all(Option::is_some) {
            return started.into_iter().flatten().collect();
        }
    }
    let statuses: Vec<_> = lobbies.iter().map(|lobby| lobby.status(clock)).collect();
    panic!("the lobbies never started: {statuses:?}");
}

fn battle_for(start: BattleStart) -> Battle {
    let mut battle = Battle::standard();
    battle.join_session(start.session, start.local_team);
    battle
}

fn attack_with_tanks(battle: &mut Battle, team: Team, goal: IVec2) {
    let tanks = battle.units(team, Some(UnitKind::Tank));
    let units = tanks.iter().map(|&tank| battle.sim_id(tank)).collect();
    battle.issue(
        team,
        PlayerCommand::Ground {
            units,
            goal,
            order: GroundOrder::AttackMove,
            queued: false,
            refinery: None,
        },
    );
}

/// Steps both battles until both have played `ticks`; neither may run ahead of the other's orders.
fn play_until(first: &mut Battle, second: &mut Battle, ticks: u64) {
    for _ in 0..40 * ticks {
        if first.tick() >= ticks && second.tick() >= ticks {
            return;
        }
        for battle in [&mut *first, &mut *second] {
            if battle.tick() < ticks {
                battle.run_for(STEP_SECS);
            }
            assert_eq!(battle.session_state(), Some(SessionState::InStep));
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    panic!(
        "the battles stopped at {} and {}",
        first.tick(),
        second.tick()
    );
}

fn level_up_to(battle: &mut Battle, other: &mut Battle) {
    while battle.tick() > other.tick() {
        other.run_for(STEP_SECS);
    }
    while other.tick() > battle.tick() {
        battle.run_for(STEP_SECS);
    }
}

/// Two players meet in a room on the server and play one battle through it.
fn duel_on_a_server(server: &RunningServer, room: &str) -> (Battle, Battle) {
    let mut owner = NetLobby::default();
    owner.open_room(&server.room(room), standard_level(), 2);
    let mut guest = NetLobby::default();
    guest.join(&server.room(room));
    let starts = start_through_lobbies(&mut [&mut owner, &mut guest], &mut []);
    let mut starts = starts.into_iter();
    let owner_start = starts.next().unwrap();
    let guest_start = starts.next().unwrap();
    assert_eq!(
        (owner_start.local_team, guest_start.local_team),
        (Team::Player, Team::Crimson)
    );
    (battle_for(owner_start), battle_for(guest_start))
}

#[test]
fn two_players_meet_in_a_room_and_play_one_battle_through_the_server() {
    let server = RunningServer::start();
    let (mut owner, mut guest) = duel_on_a_server(&server, "duel");
    play_until(&mut owner, &mut guest, 40);
    attack_with_tanks(&mut owner, Team::Player, IVec2::new(48, 16));
    attack_with_tanks(&mut guest, Team::Crimson, IVec2::new(16, 48));
    play_until(&mut owner, &mut guest, 400);
    level_up_to(&mut owner, &mut guest);
    assert_eq!(
        simulation_checksum(owner.world()),
        simulation_checksum(guest.world())
    );
}

#[test]
fn rooms_on_one_server_keep_their_battles_apart() {
    let server = RunningServer::start();
    let (mut first_owner, mut first_guest) = duel_on_a_server(&server, "north");
    let (mut second_owner, mut second_guest) = duel_on_a_server(&server, "south");
    attack_with_tanks(&mut first_owner, Team::Player, IVec2::new(48, 16));
    play_until(&mut first_owner, &mut first_guest, 200);
    play_until(&mut second_owner, &mut second_guest, 200);
    level_up_to(&mut first_owner, &mut first_guest);
    level_up_to(&mut second_owner, &mut second_guest);
    assert_eq!(
        simulation_checksum(first_owner.world()),
        simulation_checksum(first_guest.world())
    );
    assert_eq!(
        simulation_checksum(second_owner.world()),
        simulation_checksum(second_guest.world())
    );
    assert_ne!(
        simulation_checksum(first_owner.world()),
        simulation_checksum(second_owner.world()),
        "the orders in one room reached the other"
    );
}

#[test]
fn a_late_spectator_and_a_returning_player_catch_up_through_the_server() {
    let server = RunningServer::start();
    let (mut owner, mut guest) = duel_on_a_server(&server, "long");
    play_until(&mut owner, &mut guest, 40);
    attack_with_tanks(&mut owner, Team::Player, IVec2::new(48, 16));
    attack_with_tanks(&mut guest, Team::Crimson, IVec2::new(16, 48));
    play_until(&mut owner, &mut guest, 300);

    let mut watching = NetLobby::default();
    watching.join(&server.room("long"));
    let start =
        start_through_lobbies(&mut [&mut watching], &mut [&mut owner, &mut guest]).remove(0);
    assert!(start.session.is_spectating());
    let mut watcher = battle_for(start);

    drop(guest);
    let mut returning = NetLobby::default();
    returning.join(&server.room("long"));
    let start =
        start_through_lobbies(&mut [&mut returning], &mut [&mut owner, &mut watcher]).remove(0);
    assert_eq!(start.local_team, Team::Crimson);
    let mut guest = battle_for(start);

    let ticks = owner.tick() + 200;
    play_until(&mut owner, &mut guest, ticks);
    attack_with_tanks(&mut guest, Team::Crimson, IVec2::new(20, 40));
    play_until(&mut owner, &mut guest, ticks + 100);
    play_until(&mut owner, &mut watcher, ticks + 100);
    level_up_to(&mut owner, &mut guest);
    level_up_to(&mut owner, &mut watcher);
    let checksum = simulation_checksum(owner.world());
    assert_eq!(simulation_checksum(guest.world()), checksum);
    assert_eq!(simulation_checksum(watcher.world()), checksum);
}

#[test]
fn an_address_without_a_room_is_refused_with_a_reason() {
    let server = RunningServer::start();
    let mut lobby = NetLobby::default();
    lobby.join(&format!("ws://{}/", server.address));
    let mut clock = 0.0;
    for _ in 0..2000 {
        clock += 0.002;
        std::thread::sleep(Duration::from_millis(1));
        assert!(lobby.advance(clock).is_none());
        if lobby
            .status(clock)
            .is_some_and(|status| status.contains("room"))
        {
            return;
        }
    }
    panic!("no refusal: {:?}", lobby.status(clock));
}

#[test]
fn a_player_enters_the_world_a_server_runs() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = Arc::clone(&stop);
    std::thread::spawn(move || {
        let mut server = Server::with_world(listener, 29).unwrap();
        while !stopping.load(Ordering::Relaxed) {
            server.step();
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let dialling = Dialling {
        url: format!("ws://{address}/world"),
        trusted: None,
    };
    let mut link = Link::guest_side_to(TcpStream::connect(&address).unwrap(), dialling).unwrap();
    link.send(&Message::EnterWorld {
        version: env!("CARGO_PKG_VERSION").into(),
        protocol: WORLD_PROTOCOL,
        player: PlayerKey::new_random(),
    });
    let mut entered = None;
    for _ in 0..5_000 {
        entered = link.drain().into_iter().find_map(|arrival| match arrival {
            Arrival::Message(Message::Entered { team, .. }) => Some(team),
            _ => None,
        });
        if entered.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    stop.store(true, Ordering::Relaxed);
    assert_eq!(entered, Some(Team::Player));
}
