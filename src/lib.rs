#![allow(clippy::type_complexity, clippy::too_many_arguments)]

pub mod app;
pub mod audio;
pub mod missions;
pub mod net;
pub mod sim;
pub mod ui;
pub mod view;
pub mod world;

use app::{game_plugins, match_flow, savegame};
use missions::{level_events, levels};
use net::network;
use sim::{
    abilities, ai, cadence, catalog, combat, cues, determinism, economy, engineers, intel,
    match_stats, medical, movement, neutral_sites, notifications, outcome, player_commands,
    production, servicing, sorties, structures, support_powers, transport, unit_orders, veterancy,
};
use world::{map, map_edits, scenery};

use bevy::prelude::*;

/// Startup phases: assets must exist before the world is generated, and the interface reads the finished world.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum StartupPhase {
    Assets,
    World,
    Interface,
}

/// How long one step of the battle simulation is; every machine and every test advances by exactly this.
pub const SIMULATION_TICK: std::time::Duration = std::time::Duration::from_millis(50);

/// Per-frame phases: player input, then the battle simulation (on its own fixed tick), then everything that
/// only draws it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum FramePhase {
    Input,
    Simulation,
    Presentation,
}

/// The steps of one simulation tick. The battle's rules run in `Rules`; the network session and saving,
/// which the game adds around the simulation, take the other steps.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TickStep {
    SendOrders,
    Rules,
    Save,
    Fingerprint,
    Count,
}

/// The steps of starting a match: the world and its forces, then who plays which side, then a saved game
/// taking the place of the level's start.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchStart {
    Deploy,
    Players,
    Restore,
}

/// The battle itself: map, economy, production, enemy AI, movement, combat and the outcome.
pub struct SimulationPlugin {
    pub enemy_ai: bool,
    pub starting_forces: bool,
}

impl Default for SimulationPlugin {
    fn default() -> Self {
        Self {
            enemy_ai: true,
            starting_forces: true,
        }
    }
}

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::state::app::StatesPlugin>() {
            app.add_plugins(bevy::state::app::StatesPlugin);
        }
        app.init_state::<match_flow::GameState>()
            .configure_sets(
                Startup,
                (
                    StartupPhase::Assets,
                    StartupPhase::World,
                    StartupPhase::Interface,
                )
                    .chain(),
            )
            .configure_sets(
                Update,
                (
                    FramePhase::Input,
                    FramePhase::Simulation,
                    FramePhase::Presentation,
                )
                    .chain()
                    .run_if(in_state(match_flow::GameState::Playing)),
            )
            .configure_sets(
                FixedUpdate,
                FramePhase::Simulation.run_if(in_state(match_flow::GameState::Playing)),
            )
            .configure_sets(
                FixedUpdate,
                (
                    TickStep::SendOrders,
                    TickStep::Rules,
                    TickStep::Save,
                    TickStep::Fingerprint,
                    TickStep::Count,
                )
                    .chain()
                    .in_set(FramePhase::Simulation),
            )
            .configure_sets(
                OnEnter(match_flow::GameState::Playing),
                (MatchStart::Deploy, MatchStart::Players, MatchStart::Restore).chain(),
            )
            .insert_resource(Time::<Fixed>::from_duration(SIMULATION_TICK))
            .insert_resource(match_flow::MatchRules {
                enemy_ai: self.enemy_ai,
                starting_forces: self.starting_forces,
            })
            .init_resource::<levels::ActiveLevel>()
            .init_resource::<combat::Targets>()
            .init_resource::<determinism::SimulationRng>()
            .init_resource::<determinism::SimIds>()
            .init_resource::<determinism::SimEntities>()
            .init_resource::<player_commands::CommandQueue>()
            .init_resource::<player_commands::SimulationTick>()
            .init_resource::<player_commands::LocalTeam>()
            .init_resource::<player_commands::CommandTiming>()
            .add_observer(determinism::forget_removed_entity)
            .add_observer(determinism::number_new_unit)
            .add_observer(determinism::number_new_building)
            .add_observer(determinism::number_new_projectile)
            .init_resource::<economy::Economies>()
            .init_resource::<production::Production>()
            .init_resource::<ai::AiCommanders>()
            .init_resource::<outcome::GameOutcome>()
            .init_resource::<notifications::Notifications>()
            .init_resource::<notifications::LastAlert>()
            .init_resource::<match_flow::MatchClock>()
            .init_resource::<intel::Intel>()
            .init_resource::<support_powers::SupportPowers>()
            .init_resource::<cadence::SimulationCadences>()
            .init_resource::<catalog::Diplomacy>()
            .init_resource::<veterancy::LastHits>()
            .init_resource::<match_stats::MatchStats>()
            .add_message::<combat::DamageDealt>()
            .add_message::<combat::Destroyed>()
            .add_message::<combat::Impact>()
            .add_message::<cues::BlastCue>()
            .add_message::<cues::SoundCue>()
            .add_message::<structures::StructureOrder>()
            .add_message::<level_events::MissionMessage>()
            .init_resource::<level_events::EventsFired>()
            .add_systems(
                OnEnter(match_flow::GameState::Playing),
                (
                    match_flow::unpause,
                    match_flow::reset_match_state,
                    map::generate_map,
                    map_edits::apply_map_edits,
                    match_flow::deploy_level_forces,
                )
                    .chain()
                    .in_set(MatchStart::Deploy),
            )
            .add_systems(
                OnEnter(match_flow::GameState::Restarting),
                match_flow::continue_after_restart,
            )
            .add_systems(
                Last,
                match_flow::scope_match_entities.run_if(
                    in_state(match_flow::GameState::Playing)
                        .or_else(in_state(match_flow::GameState::Replay))
                        .or_else(in_state(match_flow::GameState::MapEditor)),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    (
                        player_commands::carry_out_player_commands,
                        (
                            match_flow::advance_match_clock,
                            level_events::run_level_events,
                        )
                            .chain(),
                        economy::tally_power,
                        production::advance_production,
                        ai::command_enemies,
                        ai::call_support_powers,
                        ai::repair_damaged_structures,
                        ai::fortify_turrets,
                        ai::employ_engineers,
                        ai::mine_the_approaches,
                        economy::run_harvesters,
                        structures::carry_out_structure_orders,
                        structures::repair_structures,
                        structures::finish_sales,
                        engineers::carry_out_engineer_tasks,
                        transport::board_carriers,
                        medical::treat_wounded,
                        neutral_sites::deliver_supplies,
                        neutral_sites::treat_at_field_hospitals,
                    )
                        .chain(),
                    (
                        (support_powers::run_support_powers, intel::update_intel).chain(),
                        abilities::update_camouflage,
                        combat::snapshot_targets,
                        combat::acquire_targets,
                        combat::chase_targets,
                        unit_orders::carry_out_orders,
                        (sorties::fly_home_to_rearm, sorties::climb_and_descend).chain(),
                        movement::follow_paths,
                        sorties::circle_while_waiting,
                        movement::separate_units,
                        movement::settle_on_terrain,
                        transport::carry_passengers,
                        movement::seat_new_buildings,
                        combat::aim_and_fire,
                        abilities::detonate_mines,
                        combat::fly_projectiles,
                        transport::unload_carriers,
                        combat::remove_destroyed,
                        economy::clear_depleted_crystals,
                        scenery::clear_decor_under_new_buildings,
                    )
                        .chain(),
                    (
                        match_stats::tally_damage,
                        match_stats::tally_losses,
                        match_stats::tally_new_forces,
                        ai::defend_harvesters,
                        veterancy::record_last_hits,
                        veterancy::credit_kills,
                        veterancy::promote_veterans,
                        veterancy::elite_field_repairs,
                        servicing::service_waiting_units,
                        notifications::watch_base,
                        outcome::decide_outcome,
                    )
                        .chain(),
                )
                    .chain()
                    .in_set(TickStep::Rules),
            )
            .add_systems(
                FixedUpdate,
                player_commands::count_ticks.in_set(TickStep::Count),
            );
    }
}

/// The full game: the simulation plus camera, interface, graphics, effects and animation.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            SimulationPlugin::default(),
            network::LockstepPlugin,
            savegame::SavePlugin,
        ))
        .add_plugins((
            game_plugins::InterpolationPlugin,
            game_plugins::BattleViewPlugin,
            game_plugins::MenusPlugin,
            game_plugins::SoundPlugin,
            game_plugins::DisplayPlugin,
            game_plugins::ReplayPlugin,
            game_plugins::MapEditorPlugin,
            game_plugins::AssetLoadingPlugin,
            game_plugins::NetworkBattlePlugin,
        ));
    }
}
