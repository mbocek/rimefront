#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{RenderCreation, WgpuFeatures, WgpuSettings};
use rimefront::sim::determinism::run_schedules_on_one_thread;
use rimefront::{GamePlugin, app::capture};

fn game_window() -> Window {
    let window = Window {
        title: "Rimefront".into(),
        resolution: (1600_u32, 900_u32).into(),
        ..default()
    };
    if !capture::is_capture_run() {
        return window;
    }
    Window {
        focused: false,
        window_level: bevy::window::WindowLevel::AlwaysOnBottom,
        present_mode: bevy::window::PresentMode::AutoNoVsync,
        ..window
    }
}

/// Metal caps its GPU counter buffers, and Bevy's render diagnostics take a new one for every frame still in
/// flight: in a heavy battle the device refuses one and wgpu reports it as a lost device, which quits the game.
fn gpu_settings() -> WgpuSettings {
    let timestamps = WgpuFeatures::TIMESTAMP_QUERY
        | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES
        | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    WgpuSettings {
        disabled_features: cfg!(target_os = "macos").then_some(timestamps),
        ..default()
    }
}

fn main() {
    rimefront::app::settings::bring_over_data_from_before_the_rename();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(game_window()),
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(gpu_settings())),
                ..default()
            }),
    )
    .add_plugins(GamePlugin);
    run_schedules_on_one_thread(&mut app);
    app.run();
}
