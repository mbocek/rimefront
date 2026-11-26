mod common;

use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use common::Battle;
use rimefront::missions::levels::{BaseSite, EnemySetup, ForceSetup, LevelDefinition};
use rimefront::net::lobby::NetLobby;
use rimefront::net::network::{Link, NetSession, SessionState, Side};
use rimefront::sim::catalog::Alliance;
use rimefront::sim::catalog::SideColor;
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::determinism::simulation_checksum;
use rimefront::sim::economy::Economies;
use rimefront::sim::player_commands::{GroundOrder, PlayerCommand};
use rimefront::sim::production::BuildItem;
use rimefront::world::season::Season;

const TICKS_TO_PLAY: u64 = 1200;
const LIMIT_SECS: f32 = 0.05;
/// Real time a late guest's handshake may take on its own thread, however busy the machine is.
const LOBBY_PATIENCE: Duration = Duration::from_secs(30);

/// Two battles in one process, joined over a real TCP connection: the host plays the Coalition, the guest
/// the Syndicate.
fn connected_pair() -> (Battle, Battle) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let guest_end = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (host_end, _) = listener.accept().unwrap();
    let mut host = Battle::standard();
    let mut guest = Battle::standard();
    host.join_network(host_end, Side::Host, Team::Azure, Team::Crimson);
    guest.join_network(guest_end, Side::Guest, Team::Crimson, Team::Azure);
    (host, guest)
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
            together: false,
        },
    );
}

/// Steps both battles in turns until both have played `ticks`; neither may run ahead of the other's orders.
/// A round in which neither advanced waits a moment for the other's turns to cross the connection.
fn play_until(host: &mut Battle, guest: &mut Battle, ticks: u64) {
    let started = std::time::Instant::now();
    while host.tick() < ticks || guest.tick() < ticks {
        let before = (host.tick(), guest.tick());
        if host.tick() < ticks {
            host.run_for(LIMIT_SECS);
        }
        if guest.tick() < ticks {
            guest.run_for(LIMIT_SECS);
        }
        if (host.tick(), guest.tick()) == before {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            started.elapsed() < LOBBY_PATIENCE,
            "the battles stopped advancing"
        );
        assert_eq!(host.session_state(), Some(SessionState::InStep));
        assert_eq!(guest.session_state(), Some(SessionState::InStep));
    }
}

#[test]
fn two_machines_play_the_same_battle_from_each_others_orders() {
    let (mut host, mut guest) = connected_pair();
    play_until(&mut host, &mut guest, 60);
    attack_with_tanks(&mut host, Team::Azure, IVec2::new(40, 20));
    attack_with_tanks(&mut guest, Team::Crimson, IVec2::new(20, 44));
    play_until(&mut host, &mut guest, TICKS_TO_PLAY);
    while host.tick() > guest.tick() {
        guest.run_for(LIMIT_SECS);
    }
    while guest.tick() > host.tick() {
        host.run_for(LIMIT_SECS);
    }
    assert_eq!(
        simulation_checksum(host.world()),
        simulation_checksum(guest.world())
    );
    assert!(
        host.damage_taken(Team::Azure) > 0.0 && host.damage_taken(Team::Crimson) > 0.0,
        "the tanks never met, so the orders did not travel"
    );
    let round_trip = host
        .world()
        .resource::<NetSession>()
        .round_trip()
        .expect("the host measured the latency");
    assert!(
        round_trip.as_millis() < 500,
        "a local round trip took {round_trip:?}"
    );
}

#[test]
fn a_battle_that_drifts_apart_is_noticed() {
    let (mut host, mut guest) = connected_pair();
    play_until(&mut host, &mut guest, 40);
    guest
        .world()
        .resource_mut::<Economies>()
        .of_mut(Team::Crimson)
        .credits += 1;
    let mut rounds = 0;
    while host.session_state() == Some(SessionState::InStep)
        || guest.session_state() == Some(SessionState::InStep)
    {
        host.run_for(LIMIT_SECS);
        guest.run_for(LIMIT_SECS);
        rounds += 1;
        assert!(rounds < 2000, "nobody noticed the drift");
    }
    assert!(matches!(
        host.session_state(),
        Some(SessionState::Desynced(_))
    ));
    assert!(matches!(
        guest.session_state(),
        Some(SessionState::Desynced(_))
    ));
}

#[test]
fn a_lost_connection_stops_the_battle() {
    let (mut host, guest) = connected_pair();
    drop(guest);
    let mut rounds = 0;
    while host.session_state() == Some(SessionState::InStep) {
        host.run_for(LIMIT_SECS);
        rounds += 1;
        assert!(rounds < 2000, "the host never noticed");
    }
    assert_eq!(host.session_state(), Some(SessionState::Disconnected));
    let stopped_at = host.tick();
    host.run_for(2.0);
    assert_eq!(
        host.tick(),
        stopped_at,
        "the battle went on without the other player"
    );
}

#[test]
fn each_player_wins_or_loses_from_their_own_side() {
    use rimefront::sim::components::Building;
    use rimefront::sim::outcome::GameOutcome;
    use rimefront::sim::player_commands::LocalTeam;
    let mut guest = Battle::standard();
    guest.world().insert_resource(LocalTeam(Team::Crimson));
    guest.despawn_units(Team::Azure);
    let world = guest.world();
    let coalition_structures: Vec<Entity> = world
        .query::<(Entity, &Building, &Team)>()
        .iter(world)
        .filter(|(_, _, team)| **team == Team::Azure)
        .map(|(entity, ..)| entity)
        .collect();
    for structure in coalition_structures {
        world.despawn(structure);
    }
    guest.run_for(2.0);
    assert_eq!(guest.outcome(), GameOutcome::Victory);
}

fn lobby_pair() -> (NetLobby, NetLobby, LevelDefinition) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let level = LevelDefinition {
        name: "Duel".into(),
        ..LevelDefinition::default()
    };
    let mut host = NetLobby::default();
    host.host_on(listener, level.clone(), 2);
    let mut guest = NetLobby::default();
    guest.join(&address);
    (host, guest, level)
}

/// Advances both lobbies on a shared clock until `done` holds.
fn settle(
    host: &mut NetLobby,
    guest: &mut NetLobby,
    clock: &mut f32,
    mut done: impl FnMut(&NetLobby, &NetLobby) -> bool,
) {
    for _ in 0..1000 {
        if done(host, guest) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
        *clock += 0.002;
        assert!(host.advance(*clock).is_none() && guest.advance(*clock).is_none());
    }
    panic!(
        "the lobbies never got there: host {:?}, guest {:?}",
        host.status(*clock),
        guest.status(*clock)
    );
}

#[test]
fn the_guest_reads_the_offer_and_both_start_after_the_countdown() {
    let (mut host, mut guest, level) = lobby_pair();
    let mut clock = 0.0;
    settle(&mut host, &mut guest, &mut clock, |_, guest| {
        guest.can_accept_offer()
    });
    assert_eq!(guest.offered_level(), Some(&level));
    guest.accept_offer();
    let counting_down = |lobby: &NetLobby| {
        lobby
            .status(0.0)
            .is_some_and(|status| status.contains("begins"))
    };
    settle(&mut host, &mut guest, &mut clock, |host, _| {
        host.can_start()
    });
    assert!(
        !counting_down(&host),
        "the battle started before the host said so"
    );
    host.start_battle();
    settle(&mut host, &mut guest, &mut clock, |host, guest| {
        counting_down(host) && counting_down(guest)
    });
    let later = clock + 10.0;
    let hosted = host.advance(later).expect("the host starts");
    let joined = guest.advance(later).expect("the guest starts");
    assert_eq!(hosted.level, level);
    assert_eq!(joined.level, level);
    assert_eq!(
        (hosted.local_team, joined.local_team),
        (Team::Azure, Team::Crimson)
    );
}

#[test]
fn when_the_guest_leaves_the_host_waits_for_another() {
    let (mut host, mut guest, _) = lobby_pair();
    let mut clock = 0.0;
    settle(&mut host, &mut guest, &mut clock, |_, guest| {
        guest.can_accept_offer()
    });
    guest.cancel();
    settle(&mut host, &mut guest, &mut clock, |host, _| {
        host.status(0.0)
            .is_some_and(|status| status.contains("Players 1 of 2"))
    });
    assert!(host.is_busy());
}

#[test]
fn players_can_talk_in_the_lobby() {
    let (mut host, mut guest, _) = lobby_pair();
    let mut clock = 0.0;
    settle(&mut host, &mut guest, &mut clock, |_, guest| {
        guest.can_accept_offer()
    });
    guest.draft = "hello\u{7}  there ".to_string();
    guest.say();
    settle(&mut host, &mut guest, &mut clock, |host, _| {
        !host.chat().is_empty()
    });
    let heard = &host.chat()[0];
    assert!(!heard.mine);
    assert_eq!(heard.text, "hello  there");
    assert!(guest.chat()[0].mine);
    assert!(guest.draft.is_empty());
}

fn four_connected_battles() -> Vec<(Battle, Team)> {
    let level = LevelDefinition {
        alliance: Alliance::FreeForAll,
        enemies: vec![EnemySetup::default(); 3]
            .into_iter()
            .zip([
                BaseSite::NorthEast,
                BaseSite::NorthWest,
                BaseSite::SouthEast,
            ])
            .map(|(enemy, site)| EnemySetup {
                force: ForceSetup {
                    site,
                    ..enemy.force
                },
                ..enemy
            })
            .collect(),
        ..LevelDefinition::default()
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let guests = [Team::Crimson, Team::Amber, Team::Violet];
    let mut host_links = Vec::new();
    let mut battles = Vec::new();
    for &team in &guests {
        let guest_end = TcpStream::connect(address).unwrap();
        let (host_end, _) = listener.accept().unwrap();
        host_links.push(Link::host_side(host_end).unwrap());
        let others = Team::ALL
            .into_iter()
            .filter(|&other| other != team)
            .collect();
        let session = NetSession::new(vec![Link::guest_side(guest_end).unwrap()], others, false);
        let mut battle = Battle::on_level(level.clone());
        battle.join_session(session, team);
        battles.push((battle, team));
    }
    let mut host = Battle::on_level(level);
    host.join_session(
        NetSession::new(host_links, guests.to_vec(), true),
        Team::Azure,
    );
    battles.insert(0, (host, Team::Azure));
    battles
}

#[test]
fn four_machines_play_one_free_for_all() {
    let mut battles = four_connected_battles();
    let goal = IVec2::new(32, 32);
    let mut rounds = 0;
    while battles.iter_mut().any(|(battle, _)| battle.tick() < 900) {
        for (battle, team) in &mut battles {
            if battle.tick() == 60 && !battle.units(*team, Some(UnitKind::Tank)).is_empty() {
                attack_with_tanks(battle, *team, goal);
            }
            if battle.tick() < 900 {
                battle.run_for(LIMIT_SECS);
            }
            assert_eq!(battle.session_state(), Some(SessionState::InStep));
        }
        rounds += 1;
        assert!(rounds < 20_000, "the battles stopped advancing");
    }
    let sums: Vec<u64> = battles
        .iter_mut()
        .map(|(battle, _)| {
            while battle.tick() < 900 {
                battle.run_for(LIMIT_SECS);
            }
            simulation_checksum(battle.world())
        })
        .collect();
    assert!(sums.windows(2).all(|pair| pair[0] == pair[1]), "{sums:x?}");
}

#[test]
fn three_players_take_their_seats_and_start_together() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let mut host = NetLobby::default();
    host.host_on(listener, LevelDefinition::default(), 3);
    let mut guests = [NetLobby::default(), NetLobby::default()];
    for guest in &mut guests {
        guest.join(&address);
    }
    let mut clock = 0.0;
    let mut started = Vec::new();
    for _ in 0..3000 {
        std::thread::sleep(std::time::Duration::from_millis(2));
        clock += 0.002;
        for guest in &mut guests {
            if guest.can_accept_offer() {
                guest.accept_offer();
            }
        }
        host.start_battle();
        started.extend(host.advance(clock));
        for guest in &mut guests {
            started.extend(guest.advance(clock));
        }
        if started.len() == 3 {
            break;
        }
        let counting_down = std::iter::once(&host).chain(&guests).any(|lobby| {
            lobby
                .status(clock)
                .is_some_and(|status| status.contains("begins"))
        });
        if counting_down {
            clock += 0.5;
        }
    }
    let mut teams: Vec<Team> = started.iter().map(|start| start.local_team).collect();
    teams.sort_by_key(|team| team.index());
    assert_eq!(teams, vec![Team::Azure, Team::Crimson, Team::Amber]);
}

#[test]
fn a_change_by_the_host_reaches_the_guest_who_must_accept_again() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let level = LevelDefinition::default();
    let mut host = NetLobby::default();
    host.host_on(listener, level.clone(), 3);
    let mut guest = NetLobby::default();
    guest.join(&address);
    let mut clock = 0.0;
    settle(&mut host, &mut guest, &mut clock, |_, guest| {
        guest.can_accept_offer()
    });
    guest.accept_offer();
    settle(&mut host, &mut guest, &mut clock, |host, _| {
        host.status(0.0)
            .is_some_and(|status| status.contains("1 ready"))
    });
    let winter = LevelDefinition {
        season: Season::Winter,
        ..level
    };
    host.reoffer(winter.clone());
    settle(&mut host, &mut guest, &mut clock, |_, guest| {
        guest
            .offered_level()
            .is_some_and(|offered| offered.season == Season::Winter)
    });
    assert!(
        guest.can_accept_offer(),
        "the guest's earlier acceptance still counted"
    );
    assert!(
        host.status(0.0)
            .is_some_and(|status| status.contains("0 ready"))
    );
}

/// The host and one player, plus a spectator who follows both and gives no orders.
fn duel_with_a_spectator() -> (Battle, Battle, Battle) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let guest_end = TcpStream::connect(address).unwrap();
    let (host_to_guest, _) = listener.accept().unwrap();
    let watcher_end = TcpStream::connect(address).unwrap();
    let (host_to_watcher, _) = listener.accept().unwrap();

    let mut host = Battle::standard();
    host.join_session(
        NetSession::new(
            vec![
                Link::host_side(host_to_guest).unwrap(),
                Link::host_side(host_to_watcher).unwrap(),
            ],
            vec![Team::Crimson],
            true,
        )
        .with_watchers([1]),
        Team::Azure,
    );
    let mut guest = Battle::standard();
    guest.join_session(
        NetSession::new(
            vec![Link::guest_side(guest_end).unwrap()],
            vec![Team::Azure],
            false,
        ),
        Team::Crimson,
    );
    let mut watcher = Battle::standard();
    watcher.join_session(
        NetSession::watching(
            vec![Link::guest_side(watcher_end).unwrap()],
            vec![Team::Azure, Team::Crimson],
        ),
        Team::Azure,
    );
    (host, guest, watcher)
}

#[test]
fn a_spectator_follows_the_battle_without_holding_it_up() {
    let (mut host, mut guest, mut watcher) = duel_with_a_spectator();
    let ticks = 400;
    let mut rounds = 0;
    while host.tick() < ticks || guest.tick() < ticks {
        if host.tick() == 40 {
            attack_with_tanks(&mut host, Team::Azure, IVec2::new(48, 16));
        }
        for battle in [&mut host, &mut guest] {
            if battle.tick() < ticks {
                battle.run_for(LIMIT_SECS);
            }
        }
        rounds += 1;
        assert!(rounds < 20 * ticks, "the players stopped advancing");
    }
    for _ in 0..20 * ticks {
        if watcher.tick() >= ticks {
            break;
        }
        watcher.run_for(LIMIT_SECS);
    }
    assert_eq!(watcher.tick(), ticks);
    assert_eq!(watcher.session_state(), Some(SessionState::InStep));
    assert_eq!(
        simulation_checksum(watcher.world()),
        simulation_checksum(host.world())
    );
}

#[test]
fn a_spectator_leaving_does_not_stop_the_battle() {
    let (mut host, mut guest, watcher) = duel_with_a_spectator();
    drop(watcher);
    play_until(&mut host, &mut guest, 200);
}

#[test]
fn a_guest_beyond_the_seats_is_offered_to_watch() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let mut host = NetLobby::default();
    host.host_on(listener, LevelDefinition::default(), 2);
    let mut guest = NetLobby::default();
    guest.join(&address);
    let mut clock = 0.0;
    settle(&mut host, &mut guest, &mut clock, |_, guest| {
        guest.can_accept_offer()
    });
    let mut spectator = NetLobby::default();
    spectator.join(&address);
    settle(&mut host, &mut spectator, &mut clock, |_, spectator| {
        spectator.can_accept_offer()
    });
    assert!(
        spectator
            .status(0.0)
            .is_some_and(|status| status.contains("watch"))
    );

    guest.accept_offer();
    spectator.accept_offer();
    let counting_down = |lobby: &NetLobby| {
        lobby
            .status(0.0)
            .is_some_and(|status| status.contains("begins"))
    };
    for _ in 0..1000 {
        if counting_down(&host) && counting_down(&guest) && counting_down(&spectator) {
            break;
        }
        host.start_battle();
        std::thread::sleep(std::time::Duration::from_millis(2));
        clock += 0.002;
        for lobby in [&mut host, &mut guest, &mut spectator] {
            assert!(lobby.advance(clock).is_none());
        }
    }
    let later = clock + 10.0;
    let hosted = host.advance(later).expect("the host starts");
    let joined = guest.advance(later).expect("the guest starts");
    let watching = spectator.advance(later).expect("the spectator starts");
    assert!(!hosted.session.is_spectating() && !joined.session.is_spectating());
    assert!(watching.session.is_spectating());
    assert_eq!(hosted.session.remote_teams(), &[Team::Crimson]);
    assert_eq!(
        watching.session.remote_teams(),
        &[Team::Azure, Team::Crimson]
    );
}

#[test]
fn a_spectator_who_joins_late_catches_up_and_follows_the_battle() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let guest_end = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (host_to_guest, _) = listener.accept().unwrap();
    let door = TcpListener::bind("127.0.0.1:0").unwrap();
    door.set_nonblocking(true).unwrap();
    let door_address = door.local_addr().unwrap().to_string();

    let mut host = Battle::standard();
    let level = host
        .world()
        .resource::<rimefront::missions::levels::ActiveLevel>()
        .0
        .clone();
    host.join_session(
        NetSession::new(
            vec![Link::host_side(host_to_guest).unwrap()],
            vec![Team::Crimson],
            true,
        )
        .with_open_door(door, level),
        Team::Azure,
    );
    let mut guest = Battle::standard();
    guest.join_session(
        NetSession::new(
            vec![Link::guest_side(guest_end).unwrap()],
            vec![Team::Azure],
            false,
        ),
        Team::Crimson,
    );
    play_until(&mut host, &mut guest, 40);
    attack_with_tanks(&mut host, Team::Azure, IVec2::new(48, 16));
    attack_with_tanks(&mut guest, Team::Crimson, IVec2::new(16, 48));
    play_until(&mut host, &mut guest, 300);
    let teal = SideColor([20, 170, 180]);
    let gold = SideColor([230, 200, 40]);
    host.world()
        .resource_mut::<NetSession>()
        .paint(Team::Azure, teal);
    guest
        .world()
        .resource_mut::<NetSession>()
        .paint(Team::Crimson, gold);
    play_until(&mut host, &mut guest, 310);

    let mut lobby = NetLobby::default();
    lobby.join(&door_address);
    let mut clock = 0.0;
    let deadline = Instant::now() + LOBBY_PATIENCE;
    let start = loop {
        clock += 0.05;
        host.run_for(LIMIT_SECS);
        guest.run_for(LIMIT_SECS);
        if lobby.can_accept_offer() {
            lobby.accept_offer();
        }
        if let Some(start) = lobby.advance(clock) {
            break start;
        }
        assert!(
            Instant::now() < deadline,
            "the late spectator was never let in"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(start.session.is_spectating());
    let mut watcher = Battle::standard();
    watcher.join_session(start.session, start.local_team);

    let joined_at = host.tick();
    let ticks = joined_at + 200;
    play_until(&mut host, &mut guest, ticks);
    for _ in 0..40 * ticks {
        if watcher.tick() >= ticks {
            break;
        }
        watcher.run_for(LIMIT_SECS);
    }
    assert_eq!(watcher.tick(), ticks, "the spectator did not catch up");
    assert_eq!(watcher.session_state(), Some(SessionState::InStep));
    let mut seen = watcher.world().resource_mut::<NetSession>().take_paints();
    seen.sort_by_key(|(team, _)| *team);
    assert_eq!(
        seen,
        [(Team::Azure, teal), (Team::Crimson, gold)],
        "the late spectator sees the sides in other colours than the players chose"
    );
    assert_eq!(
        simulation_checksum(watcher.world()),
        simulation_checksum(host.world())
    );
}

#[test]
fn a_player_whose_connection_drops_rejoins_and_the_battle_goes_on() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let guest_end = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (host_to_guest, _) = listener.accept().unwrap();
    let door = TcpListener::bind("127.0.0.1:0").unwrap();
    door.set_nonblocking(true).unwrap();
    let door_address = door.local_addr().unwrap().to_string();

    let mut host = Battle::standard();
    let level = host
        .world()
        .resource::<rimefront::missions::levels::ActiveLevel>()
        .0
        .clone();
    host.join_session(
        NetSession::new(
            vec![Link::host_side(host_to_guest).unwrap()],
            vec![Team::Crimson],
            true,
        )
        .with_open_door(door, level),
        Team::Azure,
    );
    let mut guest = Battle::standard();
    guest.join_session(
        NetSession::new(
            vec![Link::guest_side(guest_end).unwrap()],
            vec![Team::Azure],
            false,
        ),
        Team::Crimson,
    );
    play_until(&mut host, &mut guest, 40);
    attack_with_tanks(&mut host, Team::Azure, IVec2::new(48, 16));
    attack_with_tanks(&mut guest, Team::Crimson, IVec2::new(16, 48));
    guest.issue(
        Team::Crimson,
        PlayerCommand::Enqueue {
            item: BuildItem::Structure(BuildingKind::PowerPlant),
            copies: 1,
        },
    );
    play_until(&mut host, &mut guest, 300);

    drop(guest);
    for _ in 0..40 {
        host.run_for(LIMIT_SECS);
    }
    assert_eq!(
        host.session_state(),
        Some(SessionState::InStep),
        "the host gave up at once"
    );
    let stalled_at = host.tick();
    for _ in 0..40 {
        host.run_for(LIMIT_SECS);
    }
    assert_eq!(
        host.tick(),
        stalled_at,
        "the battle went on without the player"
    );

    let mut lobby = NetLobby::default();
    lobby.join(&door_address);
    let mut clock = 0.0;
    let deadline = Instant::now() + LOBBY_PATIENCE;
    let start = loop {
        clock += 0.05;
        host.run_for(LIMIT_SECS);
        if lobby.can_accept_offer() {
            lobby.accept_offer();
        }
        if let Some(start) = lobby.advance(clock) {
            break start;
        }
        assert!(
            Instant::now() < deadline,
            "the player was never let back in"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(!start.session.is_spectating());
    assert_eq!(start.local_team, Team::Crimson);
    let mut rejoined = Battle::standard();
    rejoined.join_session(start.session, start.local_team);

    let ticks = stalled_at + 200;
    play_until(&mut host, &mut rejoined, stalled_at + 20);
    attack_with_tanks(&mut rejoined, Team::Crimson, IVec2::new(20, 40));
    play_until(&mut host, &mut rejoined, ticks);
    while host.tick() > rejoined.tick() {
        rejoined.run_for(LIMIT_SECS);
    }
    while rejoined.tick() > host.tick() {
        host.run_for(LIMIT_SECS);
    }
    assert_eq!(
        simulation_checksum(rejoined.world()),
        simulation_checksum(host.world())
    );
}
