use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;
use rand::RngExt;

use crate::missions::levels::ActiveLevel;
use crate::sim::catalog::ArmorClass;
use crate::sim::combat::{DamageSource, Destroyed, Impact};
use crate::sim::cues::BlastSize;
use crate::view::assets::ModelLibrary;
use crate::view::camera::RtsCamera;
use crate::view::effects::{EffectAssets, spawn_blast};
use crate::view::local_view::LocalView;
use crate::view::sky::WeatherCycle;
use crate::world::season::Season;

const CRATER_LIFETIME_SECS: f32 = 40.0;
const MAX_CRATERS: usize = 80;
/// Crater opacity steps; a crater swaps to the next one as it fades instead of owning a material.
const CRATER_FADE_STEPS: usize = 6;
const CRATER_LIFT: f32 = 0.04;
const WRECK_LIFETIME_SECS: f32 = 45.0;
const WRECK_SINK_SECS: f32 = 3.0;
const WRECK_SMOKE_SECS: f32 = 15.0;
const WRECK_SMOKE_INTERVAL_SECS: f32 = 0.7;
const WRECK_SINK_DEPTH: f32 = 1.6;
const RAIN_DROPS: usize = 700;
const RAIN_AREA_HALF_SPAN: f32 = 40.0;
const RAIN_HEIGHT: f32 = 24.0;
const RAIN_FALL_SPEED: f32 = 18.0;
const RAIN_STREAK: Vec3 = Vec3::new(0.12, -0.9, 0.05);
const SNOW_FALL_SPEED: f32 = 2.2;
const SNOWFLAKE: Vec3 = Vec3::new(0.05, -0.12, 0.0);
const SNOW_FLUTTER: f32 = 0.5;

#[derive(Resource)]
pub struct BattlefieldAssets {
    crater_mesh: Handle<Mesh>,
    crater_materials: Vec<Handle<StandardMaterial>>,
    charred: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct Crater {
    age: f32,
}

#[derive(Component)]
pub struct Wreck {
    age: f32,
    smoke_timer: f32,
    rest_height: f32,
}

/// Set once every part of a wreck's model has been swapped to charred metal.
#[derive(Component)]
pub struct Charred;

pub fn create_battlefield_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let crater_materials = (0..CRATER_FADE_STEPS)
        .map(|step| {
            let opacity = 0.85 * (1.0 - step as f32 / CRATER_FADE_STEPS as f32);
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.09, 0.07, 0.05, opacity),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(BattlefieldAssets {
        crater_mesh: meshes.add(Circle::new(1.0)),
        crater_materials,
        charred: materials.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.065, 0.06),
            perceptual_roughness: 0.95,
            metallic: 0.3,
            ..default()
        }),
    });
}

pub fn scorch_the_ground(
    mut commands: Commands,
    mut impacts: MessageReader<Impact>,
    assets: Res<BattlefieldAssets>,
    view: LocalView,
    level: Res<ActiveLevel>,
    craters: Query<(Entity, &Crater)>,
) {
    let mut rng = rand::rng();
    let mut spawned = 0;
    for impact in impacts.read() {
        if !view.sight().is_visible(impact.position) {
            continue;
        }
        let ground = level.0.landscape.on_ground(impact.position) + Vec3::Y * CRATER_LIFT;
        let radius = impact.scorch_radius * rng.random_range(0.8..1.2);
        commands.spawn((
            Crater { age: 0.0 },
            Mesh3d(assets.crater_mesh.clone()),
            MeshMaterial3d(assets.crater_materials[0].clone()),
            Transform::from_translation(ground)
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
                .with_scale(Vec3::new(radius, radius * rng.random_range(0.75..1.0), 1.0)),
        ));
        spawned += 1;
    }
    let excess = (craters.iter().count() + spawned).saturating_sub(MAX_CRATERS);
    if excess > 0 {
        let mut oldest: Vec<(Entity, f32)> = craters.iter().map(|(e, c)| (e, c.age)).collect();
        oldest.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (entity, _) in oldest.into_iter().take(excess) {
            commands.entity(entity).despawn();
        }
    }
}

pub fn fade_craters(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<BattlefieldAssets>,
    mut craters: Query<(Entity, &mut Crater, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (entity, mut crater, mut material) in &mut craters {
        crater.age += time.delta_secs();
        let progress = crater.age / CRATER_LIFETIME_SECS;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let step = ((progress * CRATER_FADE_STEPS as f32) as usize).min(CRATER_FADE_STEPS - 1);
        if material.0 != assets.crater_materials[step] {
            material.0 = assets.crater_materials[step].clone();
        }
    }
}

/// A destroyed vehicle leaves its burnt-out hull behind for a while.
pub fn leave_wrecks(
    mut commands: Commands,
    mut losses: MessageReader<Destroyed>,
    models: Res<ModelLibrary>,
    view: LocalView,
    level: Res<ActiveLevel>,
) {
    let mut rng = rand::rng();
    for loss in losses.read() {
        let DamageSource::Unit(kind) = loss.what else {
            continue;
        };
        let leaves_hull = matches!(
            kind.spec().armor,
            ArmorClass::Vehicle | ArmorClass::Aircraft
        );
        if !leaves_hull || !view.sight().is_visible(loss.position) {
            continue;
        }
        let ground = level.0.landscape.on_ground(loss.position);
        let slump = Quat::from_rotation_z(rng.random_range(-0.06..0.06))
            * Quat::from_rotation_x(rng.random_range(-0.06..0.06));
        commands.spawn((
            Wreck {
                age: 0.0,
                smoke_timer: 0.0,
                rest_height: ground.y - 0.12,
            },
            WorldAssetRoot(models.unit(kind, loss.team)),
            Transform::from_translation(ground)
                .with_rotation(slump * Quat::from_rotation_y(rng.random_range(0.0..TAU))),
        ));
    }
}

pub fn char_wrecks(
    mut commands: Commands,
    assets: Res<BattlefieldAssets>,
    wrecks: Query<Entity, (With<Wreck>, Without<Charred>)>,
    children: Query<&Children>,
    mut parts: Query<&mut MeshMaterial3d<StandardMaterial>>,
) {
    for wreck in &wrecks {
        let mut charred_any = false;
        for part in children.iter_descendants(wreck) {
            if let Ok(mut material) = parts.get_mut(part) {
                material.0 = assets.charred.clone();
                charred_any = true;
            }
        }
        if charred_any {
            commands.entity(wreck).insert(Charred);
        }
    }
}

pub fn smoulder_and_sink_wrecks(
    mut commands: Commands,
    time: Res<Time>,
    effects: Res<EffectAssets>,
    mut wrecks: Query<(Entity, &mut Wreck, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut wreck, mut transform) in &mut wrecks {
        wreck.age += dt;
        if wreck.age >= WRECK_LIFETIME_SECS {
            commands.entity(entity).despawn();
            continue;
        }
        let sinking =
            (wreck.age - (WRECK_LIFETIME_SECS - WRECK_SINK_SECS)).max(0.0) / WRECK_SINK_SECS;
        transform.translation.y = wreck.rest_height - sinking * WRECK_SINK_DEPTH;
        if wreck.age > WRECK_SMOKE_SECS {
            continue;
        }
        wreck.smoke_timer -= dt;
        if wreck.smoke_timer <= 0.0 {
            wreck.smoke_timer = WRECK_SMOKE_INTERVAL_SECS;
            spawn_blast(
                &mut commands,
                &effects,
                transform.translation + Vec3::Y * 1.0,
                BlastSize::DamageSmoke,
            );
        }
    }
}

fn drop_hash(index: usize, salt: u32) -> f32 {
    let mut h = (index as u32).wrapping_mul(0x9e37_79b9) ^ salt.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

/// Streaks of rain, or in winter drifting snowflakes, falling around the point the camera looks at, as
/// thick as the weather says.
pub fn draw_rain(
    mut gizmos: Gizmos,
    time: Res<Time>,
    level: Res<ActiveLevel>,
    weather: Option<Res<WeatherCycle>>,
    view: Option<LocalView>,
    cameras: Query<(&RtsCamera, &GlobalTransform)>,
) {
    let rainfall = weather.map_or(0.0, |weather| weather.now.rainfall);
    let drops = (RAIN_DROPS as f32 * rainfall) as usize;
    if drops == 0 {
        return;
    }
    let Ok((camera, eye)) = cameras.single() else {
        return;
    };
    let eye = eye.translation();
    let in_the_dark = |drop: Vec3| {
        view.as_ref().is_some_and(|view| {
            !view
                .sight()
                .is_explored(ground_behind(eye, drop, camera.focus.y))
        })
    };
    let snowing = level.0.season == Season::Winter;
    let (color, fall_speed, streak) = if snowing {
        (
            Color::srgba(0.95, 0.97, 1.0, 0.8),
            SNOW_FALL_SPEED,
            SNOWFLAKE,
        )
    } else {
        (
            Color::srgba(0.72, 0.78, 0.86, 0.45),
            RAIN_FALL_SPEED,
            RAIN_STREAK,
        )
    };
    let elapsed = time.elapsed_secs();
    for index in 0..drops {
        let offset = Vec2::new(drop_hash(index, 1), drop_hash(index, 2)) * 2.0 - Vec2::ONE;
        let fallen = (elapsed * fall_speed + drop_hash(index, 3) * RAIN_HEIGHT) % RAIN_HEIGHT;
        let flutter = if snowing {
            (elapsed * 1.3 + drop_hash(index, 4) * 6.0).sin() * SNOW_FLUTTER
        } else {
            0.0
        };
        let top = camera.focus
            + Vec3::new(
                offset.x * RAIN_AREA_HALF_SPAN + flutter,
                RAIN_HEIGHT - fallen,
                offset.y * RAIN_AREA_HALF_SPAN,
            );
        if in_the_dark(top) {
            continue;
        }
        gizmos.line(top, top + streak, color);
    }
}

/// The ground a drop is seen against from the camera; under fog of war the drop would hang over the black.
fn ground_behind(eye: Vec3, drop: Vec3, ground_height: f32) -> Vec3 {
    let depth = (eye.y - ground_height) / (eye.y - drop.y).max(0.01);
    eye + (drop - eye) * depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drop_is_judged_by_the_ground_seen_behind_it_not_the_ground_below() {
        let eye = Vec3::new(0.0, 10.0, 10.0);
        let drop = Vec3::new(0.0, 5.0, 0.0);
        assert_eq!(ground_behind(eye, drop, 0.0), Vec3::new(0.0, 0.0, -10.0));
    }
}
