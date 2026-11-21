//! A windowless match driven on a fixed clock, shared by the tests and the balance tool.

use std::path::PathBuf;
use std::time::Duration;

use crate::SimulationPlugin;
use crate::ai::{AiCommander, AiCommanders};
use crate::assets::ModelLibrary;
use crate::catalog::{BuildingKind, Diplomacy, Team, UnitKind};
use crate::combat::DamageDealt;
use crate::combat::{DamageSource, Destroyed};
use crate::components::*;
use crate::determinism::SimId;
use crate::economy::Economies;
use crate::levels::Difficulty;
use crate::levels::{ActiveLevel, LevelDefinition, Personality};
use crate::map::{GameMap, cell_center};
use crate::match_flow::GameState;
use crate::movement::order_move;
use crate::network::{NETWORK_COMMAND_DELAY_TICKS, NetSession, SessionState, Side};
use crate::outcome::GameOutcome;
use crate::player_commands::{
    COMMAND_DELAY_TICKS, CommandQueue, LocalTeam, PlayerCommand, SimulationTick,
};
use crate::production::{BuildItem, Production, construct_building};
use crate::savegame::SaveDirectory;
use crate::sound::{Sound, SoundCue};
use crate::spawn::spawn_unit;
use crate::structures::{StructureOrder, StructureOrderKind};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use std::collections::HashMap;

/// Simulation step; small enough that the fastest projectiles and units behave as in the real game.
pub const STEP: Duration = crate::SIMULATION_TICK;

/// Who hurt whom since the battle started, collected from `DamageDealt` and `Destroyed` messages.
#[derive(Resource, Default)]
pub struct DamageLedger {
    pub taken: [f32; 4],
    /// Damage dealt, keyed by the attacking side and what fired.
    pub dealt: HashMap<(Team, DamageSource), f32>,
    /// Units and structures destroyed, keyed by their side.
    pub lost: HashMap<(Team, DamageSource), u32>,
    /// Units that entered the battle, keyed by side, starting forces included.
    pub recruited: HashMap<(Team, UnitKind), u32>,
}

/// Every sound the simulation asked for, in order.
#[derive(Resource, Default)]
pub struct SoundLedger {
    pub heard: Vec<SoundCue>,
}

fn record_sounds(mut cues: MessageReader<SoundCue>, mut ledger: ResMut<SoundLedger>) {
    ledger.heard.extend(cues.read().copied());
}

fn record_damage(mut hits: MessageReader<DamageDealt>, mut ledger: ResMut<DamageLedger>) {
    for hit in hits.read() {
        ledger.taken[hit.team.index()] += hit.amount;
        *ledger.dealt.entry((hit.attacker, hit.source)).or_default() += hit.amount;
    }
}

fn record_losses(mut losses: MessageReader<Destroyed>, mut ledger: ResMut<DamageLedger>) {
    for loss in losses.read() {
        *ledger.lost.entry((loss.team, loss.what)).or_default() += 1;
    }
}

fn record_recruits(recruits: Query<(&Unit, &Team), Added<Unit>>, mut ledger: ResMut<DamageLedger>) {
    for (unit, &team) in &recruits {
        *ledger.recruited.entry((team, unit.0)).or_default() += 1;
    }
}

/// A headless match running the real simulation systems on a fixed clock.
pub struct Battle {
    pub app: App,
    pub elapsed_secs: f32,
    save_directory: PathBuf,
}

pub struct BattleSetup {
    pub enemy_ai: bool,
    pub starting_forces: bool,
    /// The default skirmish (one Normal opponent) when `None`.
    pub level: Option<LevelDefinition>,
}

impl Battle {
    pub fn new(setup: BattleSetup) -> Self {
        let save_directory = std::env::temp_dir().join(format!(
            "iron_tide_test_{}_{}",
            std::process::id(),
            rand_suffix()
        ));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
            .insert_resource(ModelLibrary::placeholder())
            .insert_resource(SaveDirectory(save_directory.clone()))
            .init_resource::<DamageLedger>()
            .init_resource::<SoundLedger>()
            .add_plugins(SimulationPlugin {
                enemy_ai: setup.enemy_ai,
                starting_forces: setup.starting_forces,
            })
            .add_systems(
                FixedUpdate,
                (record_damage, record_sounds, record_losses, record_recruits)
                    .after(crate::FramePhase::Simulation),
            );
        crate::determinism::run_schedules_on_one_thread(&mut app);
        if let Some(level) = setup.level {
            app.insert_resource(ActiveLevel(level));
        }
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
            "the match never started"
        );
        Self {
            app,
            elapsed_secs: 0.0,
            save_directory,
        }
    }

    pub fn standard() -> Self {
        Self::new(BattleSetup {
            enemy_ai: true,
            starting_forces: true,
            level: None,
        })
    }

    pub fn sandbox() -> Self {
        Self::new(BattleSetup {
            enemy_ai: false,
            starting_forces: false,
            level: None,
        })
    }

    pub fn world(&mut self) -> &mut World {
        self.app.world_mut()
    }

    pub fn run_for(&mut self, secs: f32) {
        let steps = (secs / STEP.as_secs_f32()).ceil() as usize;
        for _ in 0..steps {
            self.app.update();
            self.elapsed_secs += STEP.as_secs_f32();
        }
    }

    /// Runs until `done` holds or `limit_secs` of game time pass; returns the game time it took.
    pub fn run_until(
        &mut self,
        limit_secs: f32,
        mut done: impl FnMut(&mut Battle) -> bool,
    ) -> Option<f32> {
        let start = self.elapsed_secs;
        let check_every_secs = 1.0;
        while self.elapsed_secs - start < limit_secs {
            self.run_for(check_every_secs);
            if done(self) {
                return Some(self.elapsed_secs - start);
            }
        }
        None
    }

    pub fn spawn_unit(&mut self, kind: UnitKind, team: Team, cell: IVec2) -> Entity {
        let world = self.world();
        let entity = world.resource_scope(|world, models: Mut<ModelLibrary>| {
            let mut commands = world.commands();
            spawn_unit(&mut commands, &models, kind, team, cell_center(cell), 0.0)
        });
        world.flush();
        entity
    }

    /// The first structure of `kind` that `team` owns.
    pub fn building(&mut self, team: Team, kind: BuildingKind) -> Option<Entity> {
        let world = self.world();
        world
            .query::<(Entity, &Building, &Team)>()
            .iter(world)
            .find(|(_, building, owner)| building.kind == kind && **owner == team)
            .map(|(entity, ..)| entity)
    }

    pub fn order_structure(&mut self, team: Team, building: Entity, kind: StructureOrderKind) {
        self.world().write_message(StructureOrder {
            team,
            building,
            kind,
        });
    }

    pub fn construct(&mut self, kind: BuildingKind, team: Team, origin: IVec2) {
        let world = self.world();
        world.resource_scope(|world, models: Mut<ModelLibrary>| {
            world.resource_scope(|world, mut map: Mut<GameMap>| {
                let mut commands = world.commands();
                construct_building(&mut commands, &models, &mut map, kind, team, origin);
            });
        });
        world.flush();
    }

    pub fn order_move(&mut self, unit: Entity, goal: IVec2) {
        let world = self.world();
        world.resource_scope(|world, map: Mut<GameMap>| {
            let position = world
                .get::<Transform>(unit)
                .expect("unit exists")
                .translation;
            let mut mobile = world.get_mut::<Mobile>(unit).expect("unit is mobile");
            assert!(
                order_move(&mut mobile, &map, position, goal, true),
                "no path to {goal}"
            );
        });
    }

    pub fn enqueue(&mut self, team: Team, item: BuildItem) -> bool {
        let world = self.world();
        let owned: Vec<BuildingKind> = world
            .query::<(&Building, &Team)>()
            .iter(world)
            .filter(|(_, owner)| **owner == team)
            .map(|(building, _)| building.kind)
            .collect();
        world.resource_scope(|world, mut production: Mut<Production>| {
            let mut economies = world.resource_mut::<Economies>();
            production
                .enqueue(&mut economies, team, item, &owned)
                .is_ok()
        })
    }

    pub fn units(&mut self, team: Team, kind: Option<UnitKind>) -> Vec<Entity> {
        let world = self.world();
        world
            .query::<(Entity, &Unit, &Team)>()
            .iter(world)
            .filter(|(_, unit, owner)| **owner == team && kind.is_none_or(|k| unit.0 == k))
            .map(|(entity, ..)| entity)
            .collect()
    }

    pub fn buildings(&mut self, team: Team) -> Vec<BuildingKind> {
        let world = self.world();
        world
            .query::<(&Building, &Team)>()
            .iter(world)
            .filter(|(_, owner)| **owner == team)
            .map(|(building, _)| building.kind)
            .collect()
    }

    pub fn alive(&mut self, entity: Entity) -> bool {
        self.world().get_entity(entity).is_ok()
    }

    pub fn position(&mut self, entity: Entity) -> Vec3 {
        self.world()
            .get::<Transform>(entity)
            .expect("entity exists")
            .translation
    }

    pub fn credits(&mut self, team: Team) -> i32 {
        self.world().resource::<Economies>().of(team).credits
    }

    pub fn set_credits(&mut self, team: Team, credits: i32) {
        self.world()
            .resource_mut::<Economies>()
            .of_mut(team)
            .credits = credits;
    }

    pub fn outcome(&mut self) -> GameOutcome {
        *self.world().resource::<GameOutcome>()
    }

    pub fn damage_taken(&mut self, team: Team) -> f32 {
        self.world().resource::<DamageLedger>().taken[team.index()]
    }

    pub fn despawn_units(&mut self, team: Team) {
        for unit in self.units(team, None) {
            self.world().despawn(unit);
        }
    }
}

impl Drop for Battle {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.save_directory);
    }
}

fn rand_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Battle {
    pub fn on_level(level: LevelDefinition) -> Self {
        Self::new(BattleSetup {
            enemy_ai: true,
            starting_forces: true,
            level: Some(level),
        })
    }

    pub fn state(&mut self) -> GameState {
        *self.world().resource::<State<GameState>>().get()
    }

    pub fn hostile_units(&mut self, team: Team) -> usize {
        let world = self.world();
        let diplomacy = *world.resource::<Diplomacy>();
        world
            .query::<&Team>()
            .iter(world)
            .filter(|owner| diplomacy.hostile(**owner, team))
            .count()
    }
}

impl Battle {
    /// Lets the computer play `team` too, for AI-against-AI matches.
    pub fn add_ai(&mut self, team: Team, base: IVec2, difficulty: Difficulty) {
        self.add_ai_with(team, base, difficulty, Personality::Balanced);
    }

    pub fn add_ai_with(
        &mut self,
        team: Team,
        base: IVec2,
        difficulty: Difficulty,
        personality: Personality,
    ) {
        self.world()
            .resource_mut::<AiCommanders>()
            .0
            .push(AiCommander::new(team, base, difficulty).with_personality(personality));
    }

    /// Gives a command as a player of `team` would; it is carried out on a later tick.
    pub fn issue(&mut self, team: Team, command: PlayerCommand) {
        let tick = *self.world().resource::<SimulationTick>();
        let delay = if self.world().contains_resource::<NetSession>() {
            NETWORK_COMMAND_DELAY_TICKS
        } else {
            COMMAND_DELAY_TICKS
        };
        self.world()
            .resource_mut::<CommandQueue>()
            .issue_after(tick, delay, team, command);
    }

    /// Hands `remote_team` to a player at the other end of `stream`; this battle plays `local_team`.
    pub fn join_network(
        &mut self,
        stream: std::net::TcpStream,
        side: Side,
        local_team: Team,
        remote_team: Team,
    ) {
        let session =
            NetSession::with_one_player(stream, side, remote_team).expect("a working connection");
        let world = self.world();
        world.insert_resource(LocalTeam(local_team));
        world.insert_resource(session);
        world
            .resource_mut::<AiCommanders>()
            .0
            .retain(|commander| commander.team != local_team && commander.team != remote_team);
    }

    /// Joins a battle with several other machines; this battle plays `local_team`.
    pub fn join_session(&mut self, session: NetSession, local_team: Team) {
        let humans: Vec<Team> = session.remote_teams().to_vec();
        let world = self.world();
        world.insert_resource(LocalTeam(local_team));
        world.insert_resource(session);
        world
            .resource_mut::<AiCommanders>()
            .0
            .retain(|commander| commander.team != local_team && !humans.contains(&commander.team));
    }

    pub fn tick(&mut self) -> u64 {
        self.world().resource::<SimulationTick>().0
    }

    pub fn session_state(&mut self) -> Option<SessionState> {
        self.world()
            .get_resource::<NetSession>()
            .map(|session| session.state)
    }

    pub fn sim_id(&mut self, entity: Entity) -> SimId {
        *self
            .world()
            .get::<SimId>(entity)
            .expect("a simulated unit or structure")
    }

    pub fn ledger(&mut self) -> &DamageLedger {
        self.app.world().resource::<DamageLedger>()
    }

    pub fn heard(&mut self, sound: Sound) -> usize {
        self.world()
            .resource::<SoundLedger>()
            .heard
            .iter()
            .filter(|cue| cue.sound == sound)
            .count()
    }
}
