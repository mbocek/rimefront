use bevy::anti_alias::smaa::Smaa;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster};
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use crate::app::i18n::{tr_fmt, tr_owned};
use crate::app::settings::{GameSettings, QualityPreset, ShadowQuality};
use crate::ui::key_bindings::Action;
use crate::ui::notifications::Notifications;
use crate::view::camera::RtsCamera;
use crate::view::fog_view::Shrouded;

/// Camera distance the shadow settings were tuned for.
const SHADOW_REFERENCE_DISTANCE: f32 = 70.0;
const SHADOW_REFIT_STEP: f32 = 8.0;
const DECOR_CULL_INTERVAL_SECS: f32 = 0.3;

/// Small ground clutter (grass, flowers, pebbles): drawn only near the camera and never casting shadows.
#[derive(Component)]
pub struct FineDecor;

#[derive(Component)]
pub struct ShadowsStripped;

fn next_preset(preset: QualityPreset) -> QualityPreset {
    match preset {
        QualityPreset::Low => QualityPreset::Medium,
        QualityPreset::Medium => QualityPreset::High,
        QualityPreset::High | QualityPreset::Custom => QualityPreset::Low,
    }
}

pub fn cycle_quality_preset(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<GameSettings>,
    mut notifications: ResMut<Notifications>,
) {
    if settings.keys.just_pressed(&keys, Action::GraphicsQuality) {
        let preset = next_preset(settings.preset);
        settings.apply_preset(preset);
        notifications.push(tr_fmt(
            "Graphics quality: {}",
            &[&tr_owned(&format!("{preset:?}"))],
        ));
    }
}

pub fn apply_graphics_settings(
    mut commands: Commands,
    settings: Res<GameSettings>,
    cameras: Query<Entity, With<Camera3d>>,
    mut suns: Query<(Entity, &mut DirectionalLight)>,
) {
    if !settings.is_changed() {
        return;
    }
    for camera in &cameras {
        let mut camera = commands.entity(camera);
        if settings.ambient_occlusion {
            camera.insert(ScreenSpaceAmbientOcclusion::default());
        } else {
            camera.remove::<ScreenSpaceAmbientOcclusion>();
        }
        if settings.bloom {
            camera.insert(Bloom::NATURAL);
        } else {
            camera.remove::<Bloom>();
        }
        if settings.anti_aliasing {
            camera.insert(Smaa::default());
        } else {
            camera.remove::<Smaa>();
        }
    }
    commands.insert_resource(DirectionalLightShadowMap {
        size: settings.shadow_map_size(),
    });
    for (_, mut light) in &mut suns {
        light.shadow_maps_enabled = settings.shadows != ShadowQuality::Off;
    }
}

/// Shadow range is measured from the camera; when it pulls back, the range grows with it so the ground
/// around the focus keeps its shadows.
pub fn fit_shadows_to_zoom(
    mut commands: Commands,
    settings: Res<GameSettings>,
    mut fitted_distance: Local<Option<f32>>,
    cameras: Query<&RtsCamera>,
    suns: Query<Entity, With<DirectionalLight>>,
) {
    let Ok(camera) = cameras.single() else { return };
    let zoomed =
        fitted_distance.is_none_or(|fitted| (camera.distance - fitted).abs() > SHADOW_REFIT_STEP);
    if !zoomed && !settings.is_changed() {
        return;
    }
    *fitted_distance = Some(camera.distance);
    let pulled_back = (camera.distance - SHADOW_REFERENCE_DISTANCE).max(0.0);
    for sun in &suns {
        commands.entity(sun).insert(
            CascadeShadowConfigBuilder {
                num_cascades: settings.shadow_cascades(),
                maximum_distance: settings.shadow_distance() + pulled_back * 1.5,
                first_cascade_far_bound: 40.0 + pulled_back * 0.6,
                ..default()
            }
            .build(),
        );
    }
}

pub fn strip_fine_decor_shadows(
    mut commands: Commands,
    decor: Query<Entity, (With<FineDecor>, Without<ShadowsStripped>)>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh3d>>,
) {
    for root in &decor {
        let mesh_nodes: Vec<Entity> = std::iter::once(root)
            .chain(children.iter_descendants(root))
            .filter(|&node| meshes.contains(node))
            .collect();
        if mesh_nodes.is_empty() {
            continue;
        }
        for node in mesh_nodes {
            commands.entity(node).insert(NotShadowCaster);
        }
        commands.entity(root).insert(ShadowsStripped);
    }
}

pub fn cull_distant_decor(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut countdown: Local<f32>,
    cameras: Query<&RtsCamera>,
    mut decor: Query<(&Transform, &mut Visibility), (With<FineDecor>, Without<Shrouded>)>,
) {
    *countdown -= time.delta_secs();
    if *countdown > 0.0 && !settings.is_changed() {
        return;
    }
    *countdown = DECOR_CULL_INTERVAL_SECS;
    let Ok(camera) = cameras.single() else { return };
    let range = settings.fine_decor_range();
    for (transform, mut visibility) in &mut decor {
        let near = range
            .is_some_and(|range| transform.translation.xz().distance(camera.focus.xz()) < range);
        let wanted = if near {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}
