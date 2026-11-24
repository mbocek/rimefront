use std::collections::HashMap;

use bevy::prelude::*;

use rand::RngExt;

use crate::sim::catalog::ArmorClass;
use crate::sim::components::{Armor, BodyRadius, HarvestState, Harvester, Mobile};
use crate::sim::fog::CellSight;
use crate::view::effects::{EffectAssets, spawn_sparkle};
use crate::view::local_view::LocalView;
use crate::world::map::{GameMap, cell_center, cells_by_distance, crystal_fields, world_to_cell};

const CRYSTAL_GLOW_COLOR: Color = Color::srgb(0.35, 1.0, 0.9);
const CRYSTAL_GLOW_INTENSITY: f32 = 120_000.0;
const CRYSTAL_GLOW_HEIGHT: f32 = 2.2;
const CRYSTAL_GLOW_PULSE_SPEED: f32 = 1.3;
const CRYSTAL_SPARKLES_PER_SEC: f32 = 16.0;
const MINING_SPARKLES_PER_SEC: f32 = 10.0;
const HARVESTER_SCOOP_REACH: f32 = 1.9;
const TRAMPLE_REACH: f32 = 0.3;
const INFANTRY_TRAMPLE: f32 = 0.35;
/// Fraction of full height a plant recovers per second after being driven over.
const TRAMPLE_RECOVERY_PER_SEC: f32 = 0.12;
const FLATTENED_HEIGHT: f32 = 0.2;
const FLATTENED_SPREAD: f32 = 0.3;

#[derive(Component)]
pub struct CrystalGlow {
    field_index: usize,
    phase: f32,
    initial_crystal: u32,
}

fn field_crystal(map: &GameMap, field_index: usize) -> u32 {
    let field = &crystal_fields()[field_index];
    cells_by_distance(field.center, field.radius.ceil() as i32)
        .map(|cell| map.crystal_at(cell))
        .sum()
}

pub fn light_crystal_fields(mut commands: Commands, map: Res<GameMap>) {
    for (field_index, field) in crystal_fields().iter().enumerate() {
        commands.spawn((
            CrystalGlow {
                field_index,
                phase: field_index as f32 * 1.7,
                initial_crystal: field_crystal(&map, field_index).max(1),
            },
            PointLight {
                color: CRYSTAL_GLOW_COLOR,
                intensity: CRYSTAL_GLOW_INTENSITY,
                range: field.radius * 4.0 + 6.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(cell_center(field.center) + Vec3::Y * CRYSTAL_GLOW_HEIGHT),
        ));
    }
}

/// Pulses each field's glow and dims it as the field is mined out.
pub fn pulse_crystal_glow(
    time: Res<Time>,
    map: Res<GameMap>,
    view: LocalView,
    mut glows: Query<(&CrystalGlow, &mut PointLight)>,
) {
    let t = time.elapsed_secs();
    for (glow, mut light) in &mut glows {
        let remaining = field_crystal(&map, glow.field_index) as f32 / glow.initial_crystal as f32;
        let pulse = 0.8 + 0.2 * (t * CRYSTAL_GLOW_PULSE_SPEED + glow.phase).sin();
        let field_center = cell_center(crystal_fields()[glow.field_index].center);
        let known = if view.sight().is_explored(field_center) {
            1.0
        } else {
            0.0
        };
        light.intensity = CRYSTAL_GLOW_INTENSITY * pulse * remaining.sqrt() * known;
    }
}

/// Low vegetation that vehicles flatten instead of clipping through.
#[derive(Component, Default)]
pub struct Trampleable {
    rest_scale: Option<Vec3>,
    flattened: f32,
}

#[derive(Resource, Default)]
pub struct VegetationGrid(HashMap<IVec2, Vec<Entity>>);

pub fn index_trampleable_vegetation(
    mut commands: Commands,
    plants: Query<(Entity, &Transform), With<Trampleable>>,
) {
    let mut grid = VegetationGrid::default();
    for (entity, transform) in &plants {
        grid.0
            .entry(world_to_cell(transform.translation))
            .or_default()
            .push(entity);
    }
    commands.insert_resource(grid);
}

pub fn trample_vegetation(
    time: Res<Time>,
    grid: Option<Res<VegetationGrid>>,
    units: Query<(&Transform, &BodyRadius, &Armor), With<Mobile>>,
    mut plants: Query<(&mut Trampleable, &mut Transform), Without<Mobile>>,
) {
    let Some(grid) = grid else { return };
    for (unit, radius, armor) in &units {
        let crush = if armor.0 == ArmorClass::Infantry {
            INFANTRY_TRAMPLE
        } else {
            1.0
        };
        let reach = radius.0 + TRAMPLE_REACH;
        let home = world_to_cell(unit.translation);
        for cell in cells_by_distance(home, 1) {
            for &plant in grid.0.get(&cell).into_iter().flatten() {
                let Ok((mut trampled, transform)) = plants.get_mut(plant) else {
                    continue;
                };
                if transform.translation.xz().distance(unit.translation.xz()) < reach {
                    trampled.flattened = trampled.flattened.max(crush);
                }
            }
        }
    }

    let recovery = TRAMPLE_RECOVERY_PER_SEC * time.delta_secs();
    for (mut trampled, mut transform) in &mut plants {
        if trampled.flattened <= 0.0 {
            continue;
        }
        let rest = *trampled.rest_scale.get_or_insert(transform.scale);
        trampled.flattened = (trampled.flattened - recovery).max(0.0);
        let squash = trampled.flattened;
        let spread = 1.0 + FLATTENED_SPREAD * squash;
        transform.scale = rest * Vec3::new(spread, 1.0 - (1.0 - FLATTENED_HEIGHT) * squash, spread);
    }
}

/// Glowing motes rising from crystal fields, and dust thrown up by mining harvesters.
pub fn emit_crystal_sparkles(
    mut commands: Commands,
    time: Res<Time>,
    effects: Res<EffectAssets>,
    map: Res<GameMap>,
    view: LocalView,
    mut pending: Local<f32>,
    harvesters: Query<(&Transform, &Harvester)>,
) {
    let mut rng = rand::rng();
    let dt = time.delta_secs();
    *pending += CRYSTAL_SPARKLES_PER_SEC * dt;
    while *pending >= 1.0 {
        *pending -= 1.0;
        let field = &crystal_fields()[rng.random_range(0..crystal_fields().len())];
        let reach = field.radius.ceil() as i32;
        let cell = field.center
            + IVec2::new(
                rng.random_range(-reach..=reach),
                rng.random_range(-reach..=reach),
            );
        if map.crystal_at(cell) == 0 || view.sight().sight_at(cell) == CellSight::Unexplored {
            continue;
        }
        let jitter = Vec3::new(
            rng.random_range(-0.7..0.7),
            rng.random_range(0.3..1.1),
            rng.random_range(-0.7..0.7),
        );
        let drift = Vec3::new(
            rng.random_range(-0.1..0.1),
            rng.random_range(0.45..0.9),
            rng.random_range(-0.1..0.1),
        );
        spawn_sparkle(
            &mut commands,
            &effects,
            cell_center(cell) + jitter,
            drift,
            rng.random_range(0.035..0.06),
        );
    }

    for (transform, harvester) in &harvesters {
        if !view.sight().is_visible(transform.translation) {
            continue;
        }
        if !matches!(harvester.state, HarvestState::Mining { .. })
            || !rng.random_bool((MINING_SPARKLES_PER_SEC * dt).min(1.0) as f64)
        {
            continue;
        }
        let scoop = transform.translation
            + transform.rotation * Vec3::X * HARVESTER_SCOOP_REACH
            + Vec3::Y * 0.4;
        let spray = Vec3::new(
            rng.random_range(-0.6..0.6),
            rng.random_range(0.8..1.6),
            rng.random_range(-0.6..0.6),
        );
        spawn_sparkle(
            &mut commands,
            &effects,
            scoop,
            spray,
            rng.random_range(0.05..0.08),
        );
    }
}
