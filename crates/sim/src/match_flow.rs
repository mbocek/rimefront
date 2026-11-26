use bevy::prelude::*;

use crate::levels::{ActiveLevel, BaseSite, ForceSetup, Objective};
use crate::sim::ai::{AiCommander, AiCommanders, find_building_site};
use crate::sim::catalog::{BuildingKind, Diplomacy, Team, UnitKind};
use crate::sim::combat::Targets;
use crate::sim::determinism::{SimEntities, SimIds, SimulationRng};
use crate::sim::economy::Economies;
use crate::sim::intel::Intel;
use crate::sim::match_stats::MatchStats;
use crate::sim::notifications::{LastAlert, Notifications};
use crate::sim::outcome::GameOutcome;
use crate::sim::player_commands::{CommandQueue, SimulationTick};
use crate::sim::production::{BuildItem, PlacementContext, Production, construct_building};
use crate::sim::sides::admit_side;
use crate::sim::spawn::{spawn_building, spawn_unit};
use crate::sim::veterancy::LastHits;
use crate::world::map::{GameMap, cell_center, in_far_half, level_bounds};
use crate::world::season::Season;
use crate::world::sites::claim_starting_site;

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

/// The battle is a copy of a world that runs elsewhere: its rules do not run here, and its units and
/// structures come and go as the world shows them, with the world's ids.
#[derive(Resource)]
pub struct MirroredBattle;

/// A world run for players whose games are elsewhere: nobody plays here, so nobody here is told anything.
#[derive(Resource)]
pub struct HostedWorld;

/// A saved game is about to replace the level's starting forces, so they are not deployed.
#[derive(Resource)]
pub struct RestoringSave;

/// Game time elapsed in the current match; pauses with the game.
#[derive(Resource, Default)]
pub struct MatchClock {
    pub elapsed_secs: f32,
}

/// A persistent world's season follows its clock.
pub fn turn_the_world_seasons(clock: Res<MatchClock>, mut level: ResMut<ActiveLevel>) {
    if level.0.objective != Objective::World {
        return;
    }
    let season = Season::of_world_at(clock.elapsed_secs);
    if level.0.season != season {
        level.0.season = season;
    }
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
    commands.insert_resource(crate::sim::sides::Sides::default());
    commands.insert_resource(Diplomacy::of(level.0.effective_alliance()));
    commands.insert_resource(LastHits::default());
    commands.insert_resource(MatchStats::default());
    commands.insert_resource(Targets::default());
    commands.insert_resource(Economies::default());
    commands.insert_resource(Production::default());
    commands.insert_resource(AiCommanders::default());
    commands.insert_resource(GameOutcome::default());
    commands.insert_resource(Notifications::default());
    commands.insert_resource(LastAlert::default());
    commands.insert_resource(MatchClock::default());
    commands.insert_resource(crate::world::weather::WeatherForecast::for_level(&level.0));
    commands.insert_resource(crate::level_events::EventsFired(vec![
        false;
        level.0.events.len()
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
/// entities marked `Persistent`: the simulation scopes what has a `Transform`, the interface its nodes.
pub fn scope_match_entities<Kind: Component>(
    mut commands: Commands,
    state: Res<State<GameState>>,
    unscoped: Query<
        Entity,
        (
            With<Kind>,
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

fn forbidden_items(level: &crate::levels::LevelDefinition) -> Vec<BuildItem> {
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

/// Where a force stands: its Command Center, where its units line up, and which way they face.
struct Deployment {
    base: IVec2,
    rally_offset: IVec2,
    facing: f32,
}

impl Deployment {
    fn on_level_site(force: &ForceSetup) -> Self {
        Self {
            base: force.site.origin(),
            rally_offset: force.site.rally_offset(),
            facing: force.site.facing(),
        }
    }

    /// On a generated site, lined up like the level base on the same side of the level's centre.
    fn on_generated_site(base: IVec2) -> Self {
        let west_of_centre = (base.x as f32) < level_bounds().center_cells().x;
        let like = if west_of_centre {
            BaseSite::SouthWest
        } else {
            BaseSite::NorthEast
        };
        Self {
            base,
            rally_offset: like.rally_offset(),
            facing: like.facing(),
        }
    }
}

fn deploy_force(commands: &mut Commands, map: &mut GameMap, team: Team, force: &ForceSetup) {
    deploy_force_at(commands, map, team, force, Deployment::on_level_site(force));
}

fn deploy_force_at(
    commands: &mut Commands,
    map: &mut GameMap,
    team: Team,
    force: &ForceSetup,
    deployment: Deployment,
) {
    let base = deployment.base;
    spawn_building(commands, map, BuildingKind::CommandCenter, team, base);
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
        construct_building(commands, map, kind, team, origin);
        own_buildings.push((origin, kind.spec().footprint));
    }

    let rally = base + deployment.rally_offset;
    let columns = 3;
    let lines_up_westward = in_far_half(base, BuildingKind::CommandCenter.spec().footprint);
    let direction = if lines_up_westward { -1 } else { 1 };
    for (i, &kind) in force.units.iter().enumerate() {
        let slot = IVec2::new(i as i32 % columns, i as i32 / columns);
        let wanted = rally + slot * direction;
        let cell = map.nearest_passable(wanted).unwrap_or(wanted);
        spawn_unit(commands, kind, team, cell_center(cell), deployment.facing);
    }
}

/// A newcomer to a wilds world: a side of its own, in a camp of its own, on the free starting site nearest
/// `near`, with the usual starting force and credits. The side and its site; `None` when the world is not
/// a wilds world.
pub fn deploy_newcomer(world: &mut World, near: IVec2) -> Option<(Team, IVec2)> {
    let site = claim_starting_site(world, near)?;
    let team = admit_side(world);
    let force = ForceSetup::default();
    world.resource_mut::<Economies>().of_mut(team).credits = force.credits;
    world.resource_scope(|world, mut map: Mut<GameMap>| {
        let mut commands = world.commands();
        let deployment = Deployment::on_generated_site(site);
        deploy_force_at(&mut commands, &mut map, team, &force, deployment);
    });
    world.flush();
    Some((team, site))
}

/// Builds the opening position of the active level: bases, armies, money, opponents and tech limits.
pub fn deploy_level_forces(
    mut commands: Commands,
    level: Res<ActiveLevel>,
    rules: Res<MatchRules>,
    restoring: Option<Res<RestoringSave>>,
    mirrored: Option<Res<MirroredBattle>>,
    mut map: ResMut<GameMap>,
    mut economies: ResMut<Economies>,
    mut production: ResMut<Production>,
    mut commanders: ResMut<AiCommanders>,
) {
    let level = &level.0;
    let forbidden = forbidden_items(level);
    production.forbid_to_every_side(forbidden);
    production.set_player_faction(level.player_faction);
    if restoring.is_some() || mirrored.is_some() {
        return;
    }

    economies.of_mut(Team::Azure).credits = level.player.credits;
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
        crate::sim::neutral_sites::place_neutral_sites(&mut commands, &mut map);
    }
    if !rules.starting_forces {
        return;
    }
    deploy_force(&mut commands, &mut map, Team::Azure, &level.player);
    for (team, enemy) in level.opponents() {
        deploy_force(&mut commands, &mut map, team, &enemy.force);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::levels::LevelDefinition;

    fn season_after(objective: Objective, secs: f32) -> LevelDefinition {
        let mut world = World::new();
        world.insert_resource(ActiveLevel(LevelDefinition {
            objective,
            ..LevelDefinition::default()
        }));
        world.insert_resource(MatchClock { elapsed_secs: secs });
        world.run_system_once(turn_the_world_seasons).unwrap();
        world.resource::<ActiveLevel>().0.clone()
    }

    #[test]
    fn a_world_reaches_winter_on_its_third_day_without_freezing_its_water() {
        let winter_day = 2.5 * 24.0 * 3600.0;
        let world = season_after(Objective::World, winter_day);
        assert_eq!(world.season, Season::Winter);
        assert!(!world.water_frozen());
        let battle = season_after(Objective::DestroyAllEnemies, winter_day);
        assert_eq!(battle.season, Season::Summer);
    }
}
