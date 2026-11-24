#![allow(clippy::type_complexity, clippy::too_many_arguments)]

pub mod app;
pub mod audio;
pub mod missions;
pub mod net;
pub mod ui;
pub mod view;

pub use iron_tide_sim::{
    FramePhase, MatchStart, SIMULATION_TICK, SimulationPlugin, StartupPhase, TickStep, sim, world,
};

use app::{game_plugins, savegame};
use bevy::prelude::*;
use net::network;

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
