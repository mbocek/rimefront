use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;

use crate::app::settings::GameSettings;
use crate::sim::components::Unit;
use crate::ui::key_bindings::Action;
use crate::ui::sidebar::{FRAME, PANEL, SIDEBAR_WIDTH, TEXT_MAIN, UiFonts};

const REFRESH_SECS: f32 = 0.25;
const RENDER_ROOT: &str = "render";
const GPU_ELAPSED: &str = "elapsed_gpu";

#[derive(Component)]
pub struct PerformanceOverlay;

#[derive(Component)]
pub struct PerformanceText;

pub fn spawn_performance_overlay(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            PerformanceOverlay,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(SIDEBAR_WIDTH + 12.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL.with_alpha(0.85)),
            BorderColor::all(FRAME),
            Pickable::IGNORE,
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                PerformanceText,
                fonts.text("", 14.0, TEXT_MAIN),
                Pickable::IGNORE,
            ));
        });
}

pub fn toggle_performance_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<GameSettings>,
) {
    if settings
        .keys
        .just_pressed(&keys, Action::PerformanceOverlay)
    {
        settings.performance_overlay = !settings.performance_overlay;
    }
}

fn smoothed(
    diagnostics: &DiagnosticsStore,
    path: &bevy::diagnostic::DiagnosticPath,
) -> Option<f64> {
    diagnostics
        .get(path)
        .and_then(|diagnostic| diagnostic.smoothed())
}

/// Sums the GPU time of the top-level render passes; nested passes are already included in their parents.
fn gpu_frame_millis(diagnostics: &DiagnosticsStore) -> Option<f64> {
    let top_level_passes: Vec<f64> = diagnostics
        .iter()
        .filter(|diagnostic| {
            let components: Vec<&str> = diagnostic.path().components().collect();
            components.len() == 3 && components[0] == RENDER_ROOT && components[2] == GPU_ELAPSED
        })
        .filter_map(|diagnostic| diagnostic.smoothed())
        .collect();
    (!top_level_passes.is_empty()).then(|| top_level_passes.iter().sum())
}

pub fn refresh_performance_overlay(
    time: Res<Time<Real>>,
    settings: Res<GameSettings>,
    diagnostics: Res<DiagnosticsStore>,
    mut countdown: Local<f32>,
    units: Query<(), With<Unit>>,
    mut overlays: Query<&mut Visibility, With<PerformanceOverlay>>,
    mut texts: Query<&mut Text, With<PerformanceText>>,
) {
    for mut visibility in &mut overlays {
        *visibility = if settings.performance_overlay {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    *countdown -= time.delta_secs();
    if !settings.performance_overlay || *countdown > 0.0 {
        return;
    }
    *countdown = REFRESH_SECS;
    let Ok(mut text) = texts.single_mut() else {
        return;
    };

    let reading = |value: Option<f64>, format: &dyn Fn(f64) -> String| {
        value.map_or("n/a".to_string(), format)
    };
    let fps = smoothed(&diagnostics, &FrameTimeDiagnosticsPlugin::FPS);
    let frame_ms = smoothed(&diagnostics, &FrameTimeDiagnosticsPlugin::FRAME_TIME);
    let gpu_ms = gpu_frame_millis(&diagnostics);
    let process_cpu = smoothed(
        &diagnostics,
        &SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE,
    );
    let system_cpu = smoothed(
        &diagnostics,
        &SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE,
    );
    let process_memory = smoothed(
        &diagnostics,
        &SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE,
    );
    let entities = smoothed(&diagnostics, &EntityCountDiagnosticsPlugin::ENTITY_COUNT);

    text.0 = [
        format!("FPS          {}", reading(fps, &|v| format!("{v:.0}"))),
        format!(
            "Frame        {}",
            reading(frame_ms, &|v| format!("{v:.1} ms"))
        ),
        format!(
            "GPU          {}",
            reading(gpu_ms, &|v| format!("{v:.1} ms"))
        ),
        format!(
            "CPU game     {}",
            reading(process_cpu, &|v| format!("{v:.0} %"))
        ),
        format!(
            "CPU system   {}",
            reading(system_cpu, &|v| format!("{v:.0} %"))
        ),
        format!(
            "Memory       {}",
            reading(process_memory, &|v| format!("{v:.2} GiB"))
        ),
        format!("Entities     {}", reading(entities, &|v| format!("{v:.0}"))),
        format!("Units        {}", units.iter().count()),
    ]
    .join("\n");
}
