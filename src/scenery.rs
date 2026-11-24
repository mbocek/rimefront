use std::f32::consts::TAU;
use std::ops::Range;

use bevy::prelude::*;
use rand::RngExt;
use rand::rngs::StdRng;

use crate::ambience::Trampleable;
use crate::assets::ModelLibrary;
use crate::components::Building;
use crate::graphics::FineDecor;
use crate::map::{
    CELL_SIZE, GameMap, cell_center, map_bounds, near_base, near_crystal_field, opposite_cell,
};
use crate::terrain::{GroundLayers, Landscape, OUTLANDS_WIDTH};

const FOREST_PAIRS: usize = 8;
const ROCK_PAIRS: usize = 20;
const CONIFER_FOREST: [&str; 3] = ["tree_pine", "tree_pine", "tree_pine_young"];
const DECIDUOUS_FOREST: [&str; 4] = ["tree_broadleaf", "tree", "tree_birch", "tree_broadleaf"];
const MIXED_FOREST: [&str; 5] = [
    "tree_pine",
    "tree_broadleaf",
    "tree_birch",
    "tree",
    "tree_pine_young",
];
const DEAD_TREE_CHANCE: f64 = 0.05;
const BUSHES: [&str; 3] = ["bush", "bush_round", "bush_flowering"];
const ROCKS: [&str; 3] = ["rock", "rock_a", "rock_b"];
const MEADOW_ATTEMPTS: usize = 2600;
const CLUMP_SIZE: std::ops::Range<usize> = 2..6;
const CLUMP_SPREAD: f32 = 0.9;
const SHORE_REEDS: usize = 420;
const SHORE_SAMPLES: usize = 40_000;
const OUTLAND_TREE_ATTEMPTS: usize = 1100;
const OUTLAND_BOULDER_COUNT: usize = 90;
const MAX_MEADOW_DIRT: f32 = 0.35;

/// Counts are authored for the standard map; a bigger map gets as much per square meter.
fn by_area(standard_count: usize) -> usize {
    let spread = map_bounds().spread();
    (standard_count as f32 * spread.x * spread.y).round() as usize
}

/// The outlands ring around the map grows with its edge, not its area.
fn by_border(standard_count: usize) -> usize {
    let spread = map_bounds().spread();
    (standard_count as f32 * (spread.x + spread.y) / 2.0).round() as usize
}

/// Any static piece of landscape: trees, rocks and decoration.
#[derive(Component)]
pub struct Scenery;

/// Scenery drawn as one merged mesh instead of a full glTF scene; `flat_scenery` dresses it.
#[derive(Component)]
pub struct FlatScenery(pub &'static str);

pub const FLAT_SCENERY_MODELS: [&str; 19] = [
    "boulder_large",
    "bush",
    "bush_flowering",
    "bush_round",
    "fallen_log",
    "flowers",
    "grass_tuft",
    "pebbles",
    "reeds",
    "rock",
    "rock_a",
    "rock_b",
    "stump",
    "tree",
    "tree_birch",
    "tree_broadleaf",
    "tree_dead",
    "tree_pine",
    "tree_pine_young",
];

/// Walk-through decoration that is cleared away when a structure is built on top of it.
#[derive(Component)]
pub struct Decor;

pub fn scatter(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    map: &mut GameMap,
    rng: &mut StdRng,
) {
    let ground = GroundLayers::new(landscape);
    plant_forests(commands, models, landscape, map, rng);
    place_rocks(commands, models, landscape, map, rng);
    dress_meadows(commands, models, landscape, map, rng, &ground);
    line_shores(commands, models, landscape, map, rng);
    cover_mountains(commands, models, landscape, rng);
}

fn pick<'a>(rng: &mut StdRng, models: &[&'a str]) -> &'a str {
    models[rng.random_range(0..models.len())]
}

fn plant_forests(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    map: &mut GameMap,
    rng: &mut StdRng,
) {
    let bounds = map_bounds();
    for _ in 0..by_area(FOREST_PAIRS) {
        let (first, past_last) = (bounds.min, bounds.min + bounds.size);
        let center = IVec2::new(
            rng.random_range(first.x + 2..past_last.x - 2),
            rng.random_range(first.y + 2..past_last.y - 2),
        );
        let species: &[&str] = match rng.random_range(0..3) {
            0 => &CONIFER_FOREST,
            1 => &DECIDUOUS_FOREST,
            _ => &MIXED_FOREST,
        };
        for _ in 0..rng.random_range(6..14) {
            let cell = center + IVec2::new(rng.random_range(-3..=3), rng.random_range(-3..=3));
            let model = if rng.random_bool(DEAD_TREE_CHANCE) {
                "tree_dead"
            } else {
                pick(rng, species)
            };
            block_with_prop(
                commands,
                models,
                landscape,
                map,
                rng,
                cell,
                model,
                0.8..1.25,
            );
        }
        for _ in 0..rng.random_range(4..9) {
            let offset = Vec3::new(
                rng.random_range(-8.0..8.0),
                0.0,
                rng.random_range(-8.0..8.0),
            );
            let position = cell_center(center) + offset;
            let undergrowth = match rng.random_range(0..5) {
                0 => "stump",
                1 => "fallen_log",
                _ => pick(rng, &BUSHES),
            };
            if is_open_ground(map, position) {
                place_decor(
                    commands,
                    models,
                    landscape,
                    rng,
                    undergrowth,
                    position,
                    0.8..1.2,
                    false,
                );
            }
        }
    }
}

fn place_rocks(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    map: &mut GameMap,
    rng: &mut StdRng,
) {
    let bounds = map_bounds();
    for _ in 0..by_area(ROCK_PAIRS) {
        let cell = IVec2::new(
            rng.random_range(bounds.min.x..bounds.min.x + bounds.size.x),
            rng.random_range(bounds.min.y..bounds.min.y + bounds.size.y),
        );
        let model = pick(rng, &ROCKS);
        block_with_prop(commands, models, landscape, map, rng, cell, model, 0.6..1.3);
    }
}

fn dress_meadows(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    map: &GameMap,
    rng: &mut StdRng,
    ground: &GroundLayers,
) {
    let bounds = map_bounds();
    for _ in 0..by_area(MEADOW_ATTEMPTS) {
        let position = Vec3::new(
            rng.random_range(bounds.world_min().x..bounds.world_max().x),
            0.0,
            rng.random_range(bounds.world_min().y..bounds.world_max().y),
        );
        let weights = ground.weights(position.xz(), 0.0);
        let grassy =
            weights.x < MAX_MEADOW_DIRT && weights.y < 0.3 && weights.z < 0.05 && weights.w < 0.05;
        if !grassy || !is_open_ground(map, position) {
            continue;
        }
        let (model, scale) = match rng.random_range(0..100) {
            0..=71 => ("grass_tuft", 0.8..1.5),
            72..=83 => ("flowers", 0.8..1.2),
            84..=92 => ("pebbles", 0.7..1.3),
            _ => (pick(rng, &BUSHES), 0.7..1.2),
        };
        let clump = if model == "grass_tuft" || model == "flowers" {
            rng.random_range(CLUMP_SIZE)
        } else {
            1
        };
        for _ in 0..clump {
            let offset = Vec3::new(
                rng.random_range(-CLUMP_SPREAD..CLUMP_SPREAD),
                0.0,
                rng.random_range(-CLUMP_SPREAD..CLUMP_SPREAD),
            );
            place_decor(
                commands,
                models,
                landscape,
                rng,
                model,
                position + offset,
                scale.clone(),
                model == "pebbles",
            );
        }
    }
}

fn line_shores(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    map: &GameMap,
    rng: &mut StdRng,
) {
    let bounds = map_bounds();
    let beyond_map = OUTLANDS_WIDTH / CELL_SIZE;
    let columns =
        bounds.min.x as f32 - beyond_map..(bounds.min.x + bounds.size.x) as f32 + beyond_map;
    let rows = bounds.min.y as f32 - beyond_map..(bounds.min.y + bounds.size.y) as f32 + beyond_map;
    let mut planted = 0;
    for _ in 0..by_area(SHORE_SAMPLES) {
        if planted == by_area(SHORE_REEDS) {
            break;
        }
        let (column, row) = (
            rng.random_range(columns.clone()),
            rng.random_range(rows.clone()),
        );
        if !landscape.is_shore(column, row) {
            continue;
        }
        let position = Vec3::new(column * CELL_SIZE, 0.0, row * CELL_SIZE);
        let inside_map = GameMap::in_bounds((position.xz() / CELL_SIZE).floor().as_ivec2());
        if inside_map && !is_open_ground(map, position) {
            continue;
        }
        place_decor(
            commands,
            models,
            landscape,
            rng,
            "reeds",
            position,
            0.8..1.3,
            false,
        );
        planted += 1;
    }
}

fn cover_mountains(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    rng: &mut StdRng,
) {
    let bounds = map_bounds();
    let (low, high) = (
        bounds.world_min() - OUTLANDS_WIDTH,
        bounds.world_max() + OUTLANDS_WIDTH,
    );
    let random_outland_point = |rng: &mut StdRng| loop {
        let position = Vec3::new(
            rng.random_range(low.x..high.x),
            0.0,
            rng.random_range(low.y..high.y),
        );
        if !bounds.contains_point(position.xz()) {
            return position;
        }
    };
    for _ in 0..by_border(OUTLAND_TREE_ATTEMPTS) {
        let position = random_outland_point(rng);
        let height = landscape.height(position.x, position.z);
        let too_steep = landscape.normal(position.x, position.z).y < 0.8;
        if too_steep || height < 0.2 || height > 13.0 {
            continue;
        }
        let model = if height > 5.0 {
            pick(rng, &CONIFER_FOREST)
        } else if rng.random_bool(0.1) {
            "tree_dead"
        } else {
            pick(rng, &MIXED_FOREST)
        };
        place_decor(
            commands,
            models,
            landscape,
            rng,
            model,
            position,
            0.9..1.5,
            false,
        );
    }
    for _ in 0..by_border(OUTLAND_BOULDER_COUNT) {
        let position = random_outland_point(rng);
        place_decor(
            commands,
            models,
            landscape,
            rng,
            "boulder_large",
            position,
            1.0..3.0,
            true,
        );
    }
}

fn is_open_ground(map: &GameMap, position: Vec3) -> bool {
    let cell = (position.xz() / CELL_SIZE).floor().as_ivec2();
    map.is_buildable(cell) && !near_base(cell) && !near_crystal_field(cell)
}

fn block_with_prop(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    map: &mut GameMap,
    rng: &mut StdRng,
    cell: IVec2,
    model_id: &str,
    scale: Range<f32>,
) {
    let free =
        |cell: IVec2| map.is_buildable(cell) && !near_base(cell) && !near_crystal_field(cell);
    let pair = [cell, opposite_cell(cell)];
    if !pair.iter().all(|&cell| free(cell)) {
        return;
    }
    for cell in pair {
        map.set_obstacle(cell, true);
        let jitter = Vec3::new(
            rng.random_range(-0.3..0.3),
            0.0,
            rng.random_range(-0.3..0.3),
        );
        spawn_scenery(
            commands,
            models,
            landscape,
            rng,
            model_id,
            cell_center(cell) + jitter,
            scale.clone(),
            false,
        );
    }
}

/// A blocking tree or rock standing on one cell, as the map editor places them.
pub fn spawn_obstacle(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    rng: &mut StdRng,
    cell: IVec2,
    obstacle: Obstacle,
) {
    let (model_id, scale) = match obstacle {
        Obstacle::Tree => (pick(rng, &MIXED_FOREST), 0.8..1.25),
        Obstacle::Rock => (pick(rng, &ROCKS), 0.6..1.3),
    };
    let jitter = Vec3::new(
        rng.random_range(-0.3..0.3),
        0.0,
        rng.random_range(-0.3..0.3),
    );
    spawn_scenery(
        commands,
        models,
        landscape,
        rng,
        model_id,
        cell_center(cell) + jitter,
        scale,
        false,
    );
}

#[derive(Clone, Copy)]
pub enum Obstacle {
    Tree,
    Rock,
}

fn place_decor(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    rng: &mut StdRng,
    model_id: &str,
    position: Vec3,
    scale: Range<f32>,
    follow_slope: bool,
) {
    let entity = spawn_scenery(
        commands,
        models,
        landscape,
        rng,
        model_id,
        position,
        scale,
        follow_slope,
    );
    commands.entity(entity).insert(Decor);
}

fn spawn_scenery(
    commands: &mut Commands,
    models: &ModelLibrary,
    landscape: Landscape,
    rng: &mut StdRng,
    model_id: &str,
    position: Vec3,
    scale: Range<f32>,
    follow_slope: bool,
) -> Entity {
    let ground = landscape.on_ground(position);
    let heading = Quat::from_rotation_y(rng.random_range(0.0..TAU));
    let rotation = if follow_slope {
        Quat::from_rotation_arc(Vec3::Y, landscape.normal(ground.x, ground.z)) * heading
    } else {
        heading
    };
    let mut scenery = commands.spawn((
        Scenery,
        Transform::from_translation(ground)
            .with_rotation(rotation)
            .with_scale(Vec3::splat(rng.random_range(scale))),
    ));
    match FLAT_SCENERY_MODELS.iter().find(|&&flat| flat == model_id) {
        Some(&flat) => scenery.insert(FlatScenery(flat)),
        None => scenery.insert(WorldAssetRoot(models.neutral(model_id))),
    };
    if matches!(model_id, "grass_tuft" | "flowers" | "pebbles") {
        scenery.insert(FineDecor);
    }
    if is_trampleable(model_id) {
        scenery.insert(Trampleable::default());
    }
    scenery.id()
}

fn is_trampleable(model_id: &str) -> bool {
    matches!(
        model_id,
        "grass_tuft" | "flowers" | "reeds" | "bush" | "bush_round" | "bush_flowering"
    )
}

/// How far each kind of vegetation leans in the wind, in radians; rigid scenery returns `None`.
pub fn sway_amplitude(model_id: &str) -> Option<f32> {
    match model_id {
        "reeds" => Some(0.09),
        "bush" | "bush_round" | "bush_flowering" => Some(0.035),
        "tree_birch" | "tree_pine_young" => Some(0.03),
        "tree" | "tree_pine" | "tree_broadleaf" => Some(0.018),
        _ => None,
    }
}

pub fn clear_decor_under_new_buildings(
    mut commands: Commands,
    new_buildings: Query<&Building, Added<Building>>,
    decor: Query<(Entity, &Transform), With<Decor>>,
) {
    let clearing_margin = 0.6;
    for building in &new_buildings {
        let min = building.origin.as_vec2() * CELL_SIZE - Vec2::splat(clearing_margin);
        let max = (building.origin + building.kind.spec().footprint).as_vec2() * CELL_SIZE
            + Vec2::splat(clearing_margin);
        for (entity, transform) in &decor {
            let point = transform.translation.xz();
            if point.cmpge(min).all() && point.cmple(max).all() {
                commands.entity(entity).despawn();
            }
        }
    }
}
