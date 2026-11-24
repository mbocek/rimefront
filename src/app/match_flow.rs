use bevy::prelude::*;

use crate::app::savegame::PendingLoad;
use crate::missions::levels::{ActiveLevel, ForceSetup};
use crate::sim::ai::{AiCommander, AiCommanders, find_building_site};
use crate::sim::catalog::{BuildingKind, Diplomacy, Team, UnitKind};
use crate::sim::combat::Targets;
use crate::sim::determinism::{SimEntities, SimIds, SimulationRng};
use crate::sim::economy::Economies;
use crate::sim::intel::Intel;
use crate::sim::match_stats::MatchStats;
use crate::sim::outcome::GameOutcome;
use crate::sim::player_commands::{CommandQueue, SimulationTick};
use crate::sim::production::{BuildItem, PlacementContext, Production, construct_building};
use crate::sim::spawn::{spawn_building, spawn_unit};
use crate::sim::veterancy::LastHits;
use crate::ui::control_groups::ControlGroups;
use crate::ui::notifications::{LastAlert, Notifications};
use crate::ui::placement::Placement;
use crate::view::assets::ModelLibrary;
use crate::world::map::{GameMap, cell_center, in_far_half};

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Bevy enters the initial state before `Startup`, so the menu waits here until assets are requested.
    #[default]
    Booting,
    MainMenu,
    Playing,
    /// Passes straight back to `Playing`; leaving `Playing` clears the old match first.
    Restarting,
    /// Watching a recorded match.
    Replay,
    /// Repainting a level's map by hand.
    MapEditor,
}

/// Entities that outlive matches: the camera and the sun.
#[derive(Component, Clone)]
pub struct Persistent;

/// How a match is assembled; tests switch parts off to build isolated scenarios.
#[derive(Resource, Clone, Copy)]
pub struct MatchRules {
    pub enemy_ai: bool,
    pub starting_forces: bool,
}

/// Game time elapsed in the current match; pauses with the game.
#[derive(Resource, Default)]
pub struct MatchClock {
    pub elapsed_secs: f32,
}

pub fn reset_match_state(mut commands: Commands, level: Res<ActiveLevel>) {
    level.0.shape_the_world();
    commands.insert_resource(SimulationRng::seeded(level.0.map_seed));
    commands.insert_resource(SimIds::default());
    commands.insert_resource(SimEntities::default());
    commands.insert_resource(CommandQueue::default());
    commands.insert_resource(SimulationTick::default());
    commands.insert_resource(crate::sim::cadence::SimulationCadences::default());
    commands.insert_resource(crate::sim::support_powers::SupportPowers::default());
    commands.insert_resource(Intel::new(level.0.fog_of_war));
    commands.insert_resource(Diplomacy {
        alliance: level.0.effective_alliance(),
    });
    commands.insert_resource(LastHits::default());
    commands.insert_resource(MatchStats::default());
    commands.insert_resource(Targets::default());
    commands.insert_resource(Economies::default());
    commands.insert_resource(Production::default());
    commands.insert_resource(Placement::default());
    commands.insert_resource(AiCommanders::default());
    commands.insert_resource(GameOutcome::default());
    commands.insert_resource(Notifications::default());
    commands.insert_resource(LastAlert::default());
    commands.insert_resource(ControlGroups::default());
    commands.insert_resource(MatchClock::default());
    commands.insert_resource(crate::world::weather::WeatherForecast::for_level(&level.0));
    commands.insert_resource(crate::missions::level_events::EventsFired(vec![
        false;
        level
            .0
            .events
            .len()
    ]));
}

pub fn unpause(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

pub fn advance_match_clock(time: Res<Time>, mut clock: ResMut<MatchClock>) {
    clock.elapsed_secs += time.delta_secs();
}

pub fn finish_booting(state: Res<State<GameState>>, mut next: ResMut<NextState<GameState>>) {
    if *state.get() == GameState::Booting {
        next.set(GameState::MainMenu);
    }
}

pub fn continue_after_restart(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Playing);
}

/// Everything spawned during a match, a replay or a map editing session goes away when it ends, except
/// entities marked `Persistent`.
pub fn scope_match_entities(
    mut commands: Commands,
    state: Res<State<GameState>>,
    unscoped: Query<
        Entity,
        (
            Or<(With<Transform>, With<Node>)>,
            Without<ChildOf>,
            Without<Persistent>,
            Without<DespawnOnExit<GameState>>,
        ),
    >,
) {
    for entity in &unscoped {
        commands.entity(entity).insert(DespawnOnExit(*state.get()));
    }
}

fn forbidden_items(level: &crate::missions::levels::LevelDefinition) -> Vec<BuildItem> {
    let units = UnitKind::ALL
        .into_iter()
        .filter(|&k| !level.allows_unit(k))
        .map(BuildItem::Unit);
    let buildings = BuildingKind::CONSTRUCTIBLE
        .into_iter()
        .filter(|&k| !level.allows_building(k))
        .map(BuildItem::Structure);
    units.chain(buildings).collect()
}

fn deploy_force(
    commands: &mut Commands,
    models: &ModelLibrary,
    map: &mut GameMap,
    team: Team,
    force: &ForceSetup,
) {
    let base = force.site.origin();
    spawn_building(
        commands,
        models,
        map,
        BuildingKind::CommandCenter,
        team,
        base,
    );
    let mut own_buildings = vec![(base, BuildingKind::CommandCenter.spec().footprint)];
    for &kind in &force.buildings {
        let context = PlacementContext {
            map,
            own_buildings: own_buildings.clone(),
            occupied_cells: Vec::new(),
            known_ground: None,
        };
        let Some(origin) = find_building_site(&context, base, kind) else {
            warn!("no room for a starting {kind:?} of {team:?}");
            continue;
        };
        construct_building(commands, models, map, kind, team, origin);
        own_buildings.push((origin, kind.spec().footprint));
    }

    let rally = base + force.site.rally_offset();
    let columns = 3;
    let lines_up_westward = in_far_half(base, BuildingKind::CommandCenter.spec().footprint);
    let direction = if lines_up_westward { -1 } else { 1 };
    for (i, &kind) in force.units.iter().enumerate() {
        let slot = IVec2::new(i as i32 % columns, i as i32 / columns);
        let wanted = rally + slot * direction;
        let cell = map.nearest_passable(wanted).unwrap_or(wanted);
        spawn_unit(
            commands,
            models,
            kind,
            team,
            cell_center(cell),
            force.site.facing(),
        );
    }
}

/// Builds the opening position of the active level: bases, armies, money, opponents and tech limits.
pub fn deploy_level_forces(
    mut commands: Commands,
    models: Res<ModelLibrary>,
    level: Res<ActiveLevel>,
    rules: Res<MatchRules>,
    pending_load: Option<Res<PendingLoad>>,
    mut map: ResMut<GameMap>,
    mut economies: ResMut<Economies>,
    mut production: ResMut<Production>,
    mut commanders: ResMut<AiCommanders>,
) {
    let level = &level.0;
    let forbidden = forbidden_items(level);
    for team in Team::ALL {
        production.forbid(team, forbidden.clone());
    }
    production.set_player_faction(level.player_faction);
    if pending_load.is_some() {
        return;
    }

    economies.of_mut(Team::Player).credits = level.player.credits;
    for (team, enemy) in level.opponents() {
        let economy = economies.of_mut(team);
        economy.credits = enemy.force.credits;
        economy.income_rate = enemy.difficulty.income_rate();
        if rules.enemy_ai {
            commanders.0.push(
                AiCommander::new(team, enemy.force.site.origin(), enemy.difficulty)
                    .with_personality(enemy.personality),
            );
        }
    }
    if level.neutral_sites {
        crate::sim::neutral_sites::place_neutral_sites(&mut commands, &models, &mut map);
    }
    if !rules.starting_forces {
        return;
    }
    deploy_force(
        &mut commands,
        &models,
        &mut map,
        Team::Player,
        &level.player,
    );
    for (team, enemy) in level.opponents() {
        deploy_force(&mut commands, &models, &mut map, team, &enemy.force);
    }
}
