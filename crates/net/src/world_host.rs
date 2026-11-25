use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;

use crate::network::{Arrival, Link, Message};
use crate::world_view::{Effect, MinedCrystal, SentView, WorldEffects, WorldNews};
use iron_tide_sim::SimulationPlugin;
use iron_tide_sim::levels::{ActiveLevel, LevelDefinition};
use iron_tide_sim::match_flow::{GameState, deploy_newcomer};
use iron_tide_sim::sim::catalog::Team;
use iron_tide_sim::sim::determinism::run_schedules_on_one_thread;
use iron_tide_sim::sim::player_commands::{CommandQueue, SimulationTick};
use iron_tide_sim::world::map::{GameMap, base_sites};
use iron_tide_sim::world::terrain::Landscape;

/// A connection to one player of a world: a network link, or a channel when the game runs the world itself.
pub trait WorldConnection: Send {
    /// False once the connection has gone.
    fn send(&self, message: &Message) -> bool;
    fn drain(&mut self) -> Vec<Arrival>;
}

impl WorldConnection for Link {
    fn send(&self, message: &Message) -> bool {
        Link::send(self, message)
    }

    fn drain(&mut self) -> Vec<Arrival> {
        Link::drain(self)
    }
}

/// One end of a connection within this machine, for a world the game runs on a thread of its own.
pub struct LocalConnection {
    outbox: Sender<Message>,
    inbox: Receiver<Message>,
}

/// Two ends of a connection within this machine: one for the player, one for the world.
pub fn local_connection() -> (LocalConnection, LocalConnection) {
    let (to_world, from_player) = channel();
    let (to_player, from_world) = channel();
    (
        LocalConnection {
            outbox: to_world,
            inbox: from_world,
        },
        LocalConnection {
            outbox: to_player,
            inbox: from_player,
        },
    )
}

impl WorldConnection for LocalConnection {
    fn send(&self, message: &Message) -> bool {
        self.outbox.send(message.clone()).is_ok()
    }

    fn drain(&mut self) -> Vec<Arrival> {
        let mut arrivals: Vec<Arrival> = self.inbox.try_iter().map(Arrival::Message).collect();
        if matches!(
            self.inbox.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ) {
            arrivals.push(Arrival::ConnectionLost);
        }
        arrivals
    }
}

struct WorldPlayer {
    connection: Box<dyn WorldConnection>,
    /// None until the player has asked to enter.
    team: Option<Team>,
    sent: SentView,
}

/// A persistent world: the simulation of a wilds world run without drawing anything, its players' orders
/// taken from their connections. The dedicated server runs one, and the game runs one on a thread of its own
/// for a world played alone. Map queries are per thread, so a world lives on the thread that made it.
pub struct WorldHost {
    app: App,
    players: Vec<WorldPlayer>,
    level_side_taken: bool,
    effects: WorldEffects,
    mined: MinedCrystal,
    /// Effects since the last views went out.
    unshown_effects: Vec<Effect>,
}

impl WorldHost {
    pub fn new(map_seed: u64) -> Self {
        let level = world_level(map_seed);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .insert_resource(ActiveLevel(level))
            .add_plugins(SimulationPlugin::default());
        run_schedules_on_one_thread(&mut app);
        let startup_frame_limit = 10;
        for _ in 0..startup_frame_limit {
            let state = *app.world().resource::<State<GameState>>().get();
            if state == GameState::Playing && app.world().contains_resource::<GameMap>() {
                break;
            }
            app.world_mut()
                .resource_mut::<NextState<GameState>>()
                .set(GameState::Playing);
            app.update();
        }
        assert!(
            app.world().contains_resource::<GameMap>(),
            "the world never started"
        );
        Self {
            app,
            players: Vec::new(),
            level_side_taken: false,
            effects: WorldEffects::default(),
            mined: MinedCrystal::default(),
            unshown_effects: Vec::new(),
        }
    }

    pub fn world(&mut self) -> &mut World {
        self.app.world_mut()
    }

    pub fn tick(&self) -> u64 {
        self.app.world().resource::<SimulationTick>().0
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn connect(&mut self, connection: Box<dyn WorldConnection>) {
        self.players.push(WorldPlayer {
            connection,
            team: None,
            sent: SentView::default(),
        });
    }

    /// Takes in what the players sent, lets `elapsed` of time pass in the world, then shows every player
    /// what changed in their view if a tick went by.
    pub fn step(&mut self, elapsed: Duration) {
        self.receive();
        let before = self.tick();
        self.app
            .insert_resource(TimeUpdateStrategy::ManualDuration(elapsed));
        self.app.update();
        let effects = self.effects.gather(self.app.world_mut());
        self.unshown_effects.extend(effects);
        self.mined.note(self.app.world_mut());
        if self.tick() != before {
            self.show_views();
        }
    }

    fn show_views(&mut self) {
        let effects = std::mem::take(&mut self.unshown_effects);
        let news = WorldNews {
            tick: self.tick(),
            effects: &effects,
            mined: &self.mined,
        };
        let world = self.app.world_mut();
        for player in &mut self.players {
            let Some(team) = player.team else { continue };
            if let Some(change) = player.sent.next(world, team, &news) {
                player
                    .connection
                    .send(&Message::WorldView(Box::new(change)));
            }
        }
    }

    fn receive(&mut self) {
        let mut players = std::mem::take(&mut self.players);
        players.retain_mut(|player| {
            let mut connected = true;
            for arrival in player.connection.drain() {
                match arrival {
                    Arrival::Message(message) => self.answer(player, message),
                    Arrival::ConnectionLost => connected = false,
                }
            }
            connected
        });
        self.players = players;
    }

    fn answer(&mut self, player: &mut WorldPlayer, message: Message) {
        match (message, player.team) {
            (Message::EnterWorld { version }, None) => {
                if version != env!("CARGO_PKG_VERSION") {
                    player.connection.send(&Message::Refused {
                        reason: format!("The world runs version {}", env!("CARGO_PKG_VERSION")),
                    });
                    return;
                }
                let Some(team) = self.side_for_newcomer() else {
                    player.connection.send(&Message::Refused {
                        reason: "The world has no room for you".into(),
                    });
                    return;
                };
                player.team = Some(team);
                let map_seed = self.app.world().resource::<ActiveLevel>().0.map_seed;
                player.connection.send(&Message::Entered {
                    team,
                    tick: self.tick(),
                    map_seed,
                });
            }
            (Message::Orders { commands }, Some(team)) => {
                let world = self.app.world_mut();
                let now = *world.resource::<SimulationTick>();
                let mut queue = world.resource_mut::<CommandQueue>();
                for command in commands {
                    queue.issue(now, team, command);
                }
            }
            _ => {}
        }
    }

    /// The first player takes the level's own side; everyone after a generated site of their own.
    fn side_for_newcomer(&mut self) -> Option<Team> {
        if !self.level_side_taken {
            self.level_side_taken = true;
            return Some(Team::Player);
        }
        let first_base = base_sites()[0];
        deploy_newcomer(self.app.world_mut(), first_base)
    }
}

/// The level a world grows from: a wilds landscape around the usual four bases.
pub fn world_level(map_seed: u64) -> LevelDefinition {
    LevelDefinition {
        name: "World".into(),
        map_seed,
        landscape: Landscape::Wilds,
        ..LevelDefinition::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_view::{Thing, WorldMirror, things_in_view};
    use bevy::ecs::system::SystemState;
    use iron_tide_sim::SIMULATION_TICK;
    use iron_tide_sim::sim::catalog::ProjectileStyle;
    use iron_tide_sim::sim::catalog::{BuildingKind, UnitKind};
    use iron_tide_sim::sim::components::Building;
    use iron_tide_sim::sim::components::Unit;
    use iron_tide_sim::sim::determinism::SimId;
    use iron_tide_sim::sim::intel::Intel;
    use iron_tide_sim::sim::player_commands::{GroundOrder, PlayerCommand};
    use iron_tide_sim::sim::production::{BuildItem, placement_context};
    use iron_tide_sim::sim::spawn::{spawn_building, spawn_unit};
    use iron_tide_sim::world::map::GameMap;
    use iron_tide_sim::world::map::map_bounds;
    use iron_tide_sim::world::map::world_to_cell;

    fn follow(mirror: &mut WorldMirror, player: &mut LocalConnection) -> usize {
        let mut bytes = 0;
        for arrival in player.drain() {
            if let Arrival::Message(message @ Message::WorldView(_)) = arrival {
                bytes += serde_json::to_vec(&message).unwrap().len();
                let Message::WorldView(change) = message else {
                    unreachable!()
                };
                mirror.apply(*change);
            }
        }
        bytes
    }

    #[test]
    fn a_player_mirrors_what_their_side_sees_and_nothing_in_the_fog() {
        let mut host = WorldHost::new(29);
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
        });
        let team = Team::Player;
        let mut mirror = WorldMirror::default();
        let mut bytes = 0;
        let minute_of_ticks = 60 * 20;
        for _ in 0..minute_of_ticks {
            host.step(SIMULATION_TICK);
            bytes += follow(&mut mirror, &mut player);
        }
        let tick = host.tick();
        let seen = things_in_view(host.world(), team);
        assert!(mirror.tick <= tick);
        assert_eq!(mirror.things, seen);
        assert_eq!(mirror.bounds, Some(map_bounds()));
        assert!(mirror.things.values().any(|thing| thing.team == team));
        let world = host.world();
        let enemy_base = world
            .query::<(&SimId, &Team, &Building)>()
            .iter(world)
            .find(|(_, team, building)| {
                **team == Team::Crimson && building.kind == BuildingKind::CommandCenter
            })
            .map(|(id, ..)| *id)
            .expect("the world has a computer side");
        assert!(
            !mirror.things.contains_key(&enemy_base),
            "seen through the fog"
        );
        println!("a minute of a world's view: {bytes} bytes of JSON");

        let tank = mirror
            .things
            .values()
            .find(|thing| thing.team == team && thing.thing == Thing::Unit(UnitKind::Tank))
            .copied()
            .expect("the player sees their tank");
        let goal = iron_tide_sim::world::map::world_to_cell(tank.position()) + IVec2::new(0, -12);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::Ground {
                units: vec![tank.id],
                goal,
                order: GroundOrder::Move,
                queued: false,
                refinery: None,
            }],
        });
        let ten_seconds_of_ticks = 10 * 20;
        let mut moving_bytes = 0;
        for _ in 0..ten_seconds_of_ticks {
            host.step(SIMULATION_TICK);
            moving_bytes += follow(&mut mirror, &mut player);
        }
        assert_ne!(
            mirror.things[&tank.id], tank,
            "the mirrored tank never moved"
        );
        println!("ten seconds of a tank driving: {moving_bytes} bytes of JSON");
    }

    #[test]
    fn a_player_is_shown_the_shots_and_blasts_of_a_fight_in_sight() {
        let mut host = WorldHost::new(29);
        let (mut player, _) = enter(&mut host);
        let world = host.world();
        let tank_position = world
            .query::<(&Unit, &Team, &Transform)>()
            .iter(world)
            .find(|(unit, team, _)| **team == Team::Player && unit.0 == UnitKind::Tank)
            .map(|(.., transform)| transform.translation)
            .expect("the player has a tank");
        let mut commands = world.commands();
        spawn_unit(
            &mut commands,
            UnitKind::Tank,
            Team::Crimson,
            tank_position + Vec3::new(4.0, 0.0, 0.0),
            0.0,
        );
        world.flush();

        let mut effects = Vec::new();
        for _ in 0..3 * 20 {
            host.step(SIMULATION_TICK);
            for arrival in player.drain() {
                if let Arrival::Message(Message::WorldView(change)) = arrival {
                    effects.extend(change.effects);
                }
            }
        }
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Shot(shot) if shot.style == ProjectileStyle::Shell)),
            "no shell shown"
        );
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Blast { .. })),
            "no blast shown"
        );
    }

    fn own_unit(world: &mut World, team: Team, kind: UnitKind) -> (Entity, Vec3) {
        world
            .query::<(Entity, &Unit, &Team, &Transform)>()
            .iter(world)
            .find(|(_, unit, owner, _)| **owner == team && unit.0 == kind)
            .map(|(entity, .., transform)| (entity, transform.translation))
            .expect("the side has the unit")
    }

    fn entered_with_mirror(host: &mut WorldHost) -> (LocalConnection, WorldMirror) {
        let (mut player, _) = enter(host);
        let mut mirror = WorldMirror::default();
        host.step(SIMULATION_TICK);
        follow(&mut mirror, &mut player);
        (player, mirror)
    }

    fn run(
        host: &mut WorldHost,
        secs: usize,
        mirror: &mut WorldMirror,
        player: &mut LocalConnection,
    ) {
        for _ in 0..secs * 20 {
            host.step(SIMULATION_TICK);
            follow(mirror, player);
        }
    }

    #[test]
    fn a_player_is_shown_the_crystal_mined_in_sight() {
        let mut host = WorldHost::new(29);
        let (mut player, mut mirror) = entered_with_mirror(&mut host);
        let world = host.world();
        let (_, tank_position) = own_unit(world, Team::Player, UnitKind::Tank);
        let mut commands = world.commands();
        spawn_unit(
            &mut commands,
            UnitKind::Harvester,
            Team::Player,
            tank_position,
            0.0,
        );
        world.flush();
        run(&mut host, 30, &mut mirror, &mut player);

        assert!(!mirror.crystal.is_empty(), "no mining shown");
        let map = host.world().resource::<GameMap>();
        for (&(x, y), &amount) in &mirror.crystal {
            assert_eq!(map.crystal_at(IVec2::new(x, y)), amount);
        }
    }

    #[test]
    fn an_enemy_structure_stays_as_last_seen_once_out_of_sight() {
        let mut host = WorldHost::new(29);
        let (mut player, mut mirror) = entered_with_mirror(&mut host);
        let world = host.world();
        let (scout, scout_position) = own_unit(world, Team::Player, UnitKind::Scout);
        let far_off = scout_position + Vec3::new(0.0, 0.0, -30.0);
        world.get_mut::<Transform>(scout).unwrap().translation = far_off;
        let origin = world_to_cell(far_off) + IVec2::new(2, 0);
        let outpost = world.resource_scope(|world, mut map: Mut<GameMap>| {
            let mut commands = world.commands();
            spawn_building(
                &mut commands,
                &mut map,
                BuildingKind::PowerPlant,
                Team::Crimson,
                origin,
            )
        });
        world.flush();
        let outpost_shown = |mirror: &WorldMirror| {
            mirror.things.values().any(|thing| {
                thing.thing
                    == Thing::Building {
                        kind: BuildingKind::PowerPlant,
                        origin,
                    }
            })
        };
        run(&mut host, 1, &mut mirror, &mut player);
        assert!(outpost_shown(&mirror), "the scout never saw the outpost");

        host.world().despawn(scout);
        run(&mut host, 2, &mut mirror, &mut player);
        assert!(
            outpost_shown(&mirror),
            "the outpost went with the scout's sight"
        );
        host.world().despawn(outpost);
        run(&mut host, 1, &mut mirror, &mut player);
        assert!(!outpost_shown(&mirror), "a destroyed outpost still shown");
    }

    #[test]
    fn a_player_builds_in_a_world_from_what_their_side_is_shown() {
        let mut host = WorldHost::new(29);
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
        });
        let mut mirror = WorldMirror::default();
        host.step(SIMULATION_TICK);
        follow(&mut mirror, &mut player);
        let power_plant = BuildItem::Structure(BuildingKind::PowerPlant);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::Enqueue {
                item: power_plant,
                copies: 1,
            }],
        });
        host.step(SIMULATION_TICK);
        host.step(SIMULATION_TICK);
        follow(&mut mirror, &mut player);
        assert_eq!(mirror.side.lanes[0].queue, [power_plant]);

        let build_ticks = (power_plant.build_secs() * 20.0) as usize + 40;
        for _ in 0..build_ticks {
            host.step(SIMULATION_TICK);
            follow(&mut mirror, &mut player);
        }
        assert_eq!(mirror.side.lanes[0].ready, Some(BuildingKind::PowerPlant));

        let origin = free_site_near_base(host.world(), Team::Player, BuildingKind::PowerPlant);
        player.send(&Message::Orders {
            commands: vec![PlayerCommand::PlaceStructure {
                kind: BuildingKind::PowerPlant,
                origin,
            }],
        });
        host.step(SIMULATION_TICK);
        host.step(SIMULATION_TICK);
        host.step(SIMULATION_TICK);
        follow(&mut mirror, &mut player);
        assert_eq!(mirror.side.lanes[0], Default::default());
        let placed = Thing::Building {
            kind: BuildingKind::PowerPlant,
            origin,
        };
        assert!(mirror.things.values().any(|thing| thing.thing == placed));
    }

    type PlacementParams = (
        Res<'static, GameMap>,
        Res<'static, Intel>,
        Query<'static, 'static, (&'static Building, &'static Team)>,
        Query<'static, 'static, &'static Transform, With<Unit>>,
    );

    fn free_site_near_base(world: &mut World, team: Team, kind: BuildingKind) -> IVec2 {
        let mut state: SystemState<PlacementParams> = SystemState::new(world);
        let (map, intel, buildings, units) = state.get(world).expect("the world has a map");
        let context = placement_context(&map, team, intel.vision(team), &buildings, &units);
        let base = context.own_buildings[0].0;
        (-8..=8)
            .flat_map(|y| (-8..=8).map(move |x| base + IVec2::new(x, y)))
            .find(|&origin| context.can_place(kind, origin))
            .expect("room to build by the base")
    }

    fn entered(player: &mut LocalConnection) -> Option<Team> {
        player
            .drain()
            .into_iter()
            .find_map(|arrival| match arrival {
                Arrival::Message(Message::Entered { team, .. }) => Some(team),
                _ => None,
            })
    }

    fn enter(host: &mut WorldHost) -> (LocalConnection, Team) {
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
        });
        host.step(SIMULATION_TICK);
        let team = entered(&mut player).expect("the world let the player in");
        (player, team)
    }

    #[test]
    fn players_enter_a_world_on_sides_of_their_own_and_command_them() {
        let mut host = WorldHost::new(29);
        let (_first, first_team) = enter(&mut host);
        let (second, second_team) = enter(&mut host);
        assert_eq!(first_team, Team::Player);
        assert_eq!(second_team, Team::from_index(4));

        let world = host.world();
        let (tank, id, start) = world
            .query::<(Entity, &Unit, &Team, &SimId, &Transform)>()
            .iter(world)
            .find(|(_, unit, team, ..)| **team == second_team && unit.0 == UnitKind::Tank)
            .map(|(entity, _, _, id, transform)| (entity, *id, transform.translation))
            .expect("the newcomer has a starting tank");
        let goal = iron_tide_sim::world::map::world_to_cell(start) + IVec2::new(0, 6);
        second.send(&Message::Orders {
            commands: vec![PlayerCommand::Ground {
                units: vec![id],
                goal,
                order: GroundOrder::Move,
                queued: false,
                refinery: None,
            }],
        });
        for _ in 0..100 {
            host.step(SIMULATION_TICK);
        }
        let moved = host.world().get::<Transform>(tank).unwrap().translation;
        assert!(moved.distance(start) > 3.0, "the tank stayed at {start}");
    }

    #[test]
    fn a_player_of_another_version_is_turned_away() {
        let mut host = WorldHost::new(29);
        let (mut player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: "0.0.1".into(),
        });
        host.step(SIMULATION_TICK);
        let refused = player
            .drain()
            .into_iter()
            .any(|arrival| matches!(arrival, Arrival::Message(Message::Refused { .. })));
        assert!(refused);
    }

    #[test]
    fn a_player_who_leaves_is_let_go() {
        let mut host = WorldHost::new(29);
        let (player, _) = enter(&mut host);
        drop(player);
        host.step(SIMULATION_TICK);
        assert_eq!(host.player_count(), 0);
    }
}
