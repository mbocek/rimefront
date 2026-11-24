use std::collections::HashMap;
use std::f32::consts::TAU;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use rand::RngExt;
use rand::rngs::StdRng;

use crate::sim::components::Building;
use crate::world::map::{
    CELL_SIZE, GameMap, cell_center, chunk_of, map_bounds, near_base, near_crystal_field,
    opposite_cell, world_to_cell,
};
use crate::world::terrain::{GroundLayers, Landscape, OUTLANDS_WIDTH};

const FOREST_PAIRS: usize = 8;
const ROCK_PAIRS: usize = 20;
pub(crate) const CONIFER_FOREST: [&str; 3] = ["tree_pine", "tree_pine", "tree_pine_young"];
pub(crate) const DECIDUOUS_FOREST: [&str; 4] =
    ["tree_broadleaf", "tree", "tree_birch", "tree_broadleaf"];
pub(crate) const MIXED_FOREST: [&str; 5] = [
    "tree_pine",
    "tree_broadleaf",
    "tree_birch",
    "tree",
    "tree_pine_young",
];
pub(crate) const DEAD_TREE_CHANCE: f64 = 0.05;
pub(crate) const BUSHES: [&str; 3] = ["bush", "bush_round", "bush_flowering"];
pub(crate) const ROCKS: [&str; 3] = ["rock", "rock_a", "rock_b"];
const MEADOW_ATTEMPTS: usize = 2600;
pub(crate) const CLUMP_SIZE: std::ops::Range<usize> = 2..6;
pub(crate) const CLUMP_SPREAD: f32 = 0.9;
const SHORE_REEDS: usize = 420;
const SHORE_SAMPLES: usize = 40_000;
const OUTLAND_TREE_ATTEMPTS: usize = 1100;
const OUTLAND_BOULDER_COUNT: usize = 90;
pub(crate) const MAX_MEADOW_DIRT: f32 = 0.35;

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

pub fn scatter(
    layout: &mut SceneryLayout,
    landscape: Landscape,
    map: &mut GameMap,
    rng: &mut StdRng,
) {
    let ground = GroundLayers::new(landscape);
    plant_forests(layout, landscape, map, rng);
    place_rocks(layout, landscape, map, rng);
    dress_meadows(layout, landscape, map, rng, &ground);
    line_shores(layout, landscape, map, rng);
    cover_mountains(layout, landscape, rng);
}

pub(crate) fn pick<'a>(rng: &mut StdRng, models: &[&'a str]) -> &'a str {
    models[rng.random_range(0..models.len())]
}

fn plant_forests(
    layout: &mut SceneryLayout,
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
            block_with_prop(layout, landscape, map, rng, cell, model, 0.8..1.25);
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
                    layout,
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
    layout: &mut SceneryLayout,
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
        block_with_prop(layout, landscape, map, rng, cell, model, 0.6..1.3);
    }
}

fn dress_meadows(
    layout: &mut SceneryLayout,
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
                layout,
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

fn line_shores(layout: &mut SceneryLayout, landscape: Landscape, map: &GameMap, rng: &mut StdRng) {
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
        place_decor(layout, landscape, rng, "reeds", position, 0.8..1.3, false);
        planted += 1;
    }
}

pub(crate) fn cover_mountains(layout: &mut SceneryLayout, landscape: Landscape, rng: &mut StdRng) {
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
        place_decor(layout, landscape, rng, model, position, 0.9..1.5, false);
    }
    for _ in 0..by_border(OUTLAND_BOULDER_COUNT) {
        let position = random_outland_point(rng);
        place_decor(
            layout,
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
    layout: &mut SceneryLayout,
    landscape: Landscape,
    map: &mut GameMap,
    rng: &mut StdRng,
    cell: IVec2,
    model_id: &'static str,
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
        place_scenery(
            layout,
            landscape,
            rng,
            model_id,
            cell_center(cell) + jitter,
            scale.clone(),
        );
    }
}

/// A blocking tree or rock standing on one cell, as the map editor places them.
pub fn place_obstacle(
    layout: &mut SceneryLayout,
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
    place_scenery(
        layout,
        landscape,
        rng,
        model_id,
        cell_center(cell) + jitter,
        scale,
    );
}

#[derive(Clone, Copy)]
pub enum Obstacle {
    Tree,
    Rock,
}

pub(crate) fn place_decor(
    layout: &mut SceneryLayout,
    landscape: Landscape,
    rng: &mut StdRng,
    model_id: &'static str,
    position: Vec3,
    scale: Range<f32>,
    follow_slope: bool,
) {
    let transform = scenery_transform(landscape, rng, position, scale, follow_slope);
    layout.place(SceneryProp {
        model: model_id,
        transform,
        decor: true,
    });
}

/// A blocking tree or rock on one cell, with no twin across the map.
pub(crate) fn block_cell_with_prop(
    layout: &mut SceneryLayout,
    landscape: Landscape,
    map: &mut GameMap,
    rng: &mut StdRng,
    cell: IVec2,
    model_id: &'static str,
    scale: Range<f32>,
) {
    map.set_obstacle(cell, true);
    let jitter = Vec3::new(
        rng.random_range(-0.3..0.3),
        0.0,
        rng.random_range(-0.3..0.3),
    );
    place_scenery(
        layout,
        landscape,
        rng,
        model_id,
        cell_center(cell) + jitter,
        scale,
    );
}

fn place_scenery(
    layout: &mut SceneryLayout,
    landscape: Landscape,
    rng: &mut StdRng,
    model_id: &'static str,
    position: Vec3,
    scale: Range<f32>,
) {
    let transform = scenery_transform(landscape, rng, position, scale, false);
    layout.place(SceneryProp {
        model: model_id,
        transform,
        decor: false,
    });
}

fn scenery_transform(
    landscape: Landscape,
    rng: &mut StdRng,
    position: Vec3,
    scale: Range<f32>,
    follow_slope: bool,
) -> Transform {
    let ground = landscape.on_ground(position);
    let heading = Quat::from_rotation_y(rng.random_range(0.0..TAU));
    let rotation = if follow_slope {
        Quat::from_rotation_arc(Vec3::Y, landscape.normal(ground.x, ground.z)) * heading
    } else {
        heading
    };
    Transform::from_translation(ground)
        .with_rotation(rotation)
        .with_scale(Vec3::splat(rng.random_range(scale)))
}

/// One tree, rock or tuft of grass, kept whether or not it is near enough to the camera to be shown.
#[derive(Clone, Copy, Debug)]
pub struct SceneryProp {
    pub model: &'static str,
    pub transform: Transform,
    /// Walk-through decoration, cleared away when a structure is built on top of it.
    pub decor: bool,
}

/// Every prop of the map by the chunk it stands in, so only the chunks near the camera need entities and an
/// edit touches only its chunk. Props beyond the map's edge belong to chunks outside its bounds.
#[derive(Resource)]
pub struct SceneryLayout {
    chunks: HashMap<IVec2, SceneryChunk>,
    /// Tells one map's scenery from the next, so whatever shows it starts afresh on a new map.
    lineage: u64,
}

static NEXT_LAYOUT_LINEAGE: AtomicU64 = AtomicU64::new(1);

impl Default for SceneryLayout {
    fn default() -> Self {
        Self {
            chunks: HashMap::new(),
            lineage: NEXT_LAYOUT_LINEAGE.fetch_add(1, Ordering::Relaxed),
        }
    }
}

#[derive(Default)]
pub struct SceneryChunk {
    pub props: Vec<SceneryProp>,
    /// Bumped whenever a prop comes or goes, so whatever shows the chunk knows to redo it.
    pub revision: u64,
}

impl SceneryLayout {
    pub fn lineage(&self) -> u64 {
        self.lineage
    }

    pub fn place(&mut self, prop: SceneryProp) {
        let chunk = self.chunks.entry(chunk_of_prop(&prop)).or_default();
        chunk.props.push(prop);
        chunk.revision += 1;
    }

    pub fn chunk(&self, chunk: IVec2) -> Option<&SceneryChunk> {
        self.chunks.get(&chunk)
    }

    /// Removes the props standing in `area` that `doomed` picks.
    fn remove_in(&mut self, area: Rect, doomed: impl Fn(&SceneryProp) -> bool) {
        let first = chunk_of(world_to_cell(area.min.extend(0.0).xzy()));
        let last = chunk_of(world_to_cell(area.max.extend(0.0).xzy()));
        for y in first.y..=last.y {
            for x in first.x..=last.x {
                let Some(chunk) = self.chunks.get_mut(&IVec2::new(x, y)) else {
                    continue;
                };
                let before = chunk.props.len();
                chunk.props.retain(|prop| {
                    let point = prop.transform.translation.xz();
                    let inside = point.cmpge(area.min).all() && point.cmple(area.max).all();
                    !(inside && doomed(prop))
                });
                if chunk.props.len() != before {
                    chunk.revision += 1;
                }
            }
        }
    }

    /// Removes everything standing on `cell`, as the map editor's brush does before painting it.
    pub fn clear_cell(&mut self, cell: IVec2) {
        let corner = cell.as_vec2() * CELL_SIZE;
        let on_cell = |prop: &SceneryProp| world_to_cell(prop.transform.translation) == cell;
        self.remove_in(Rect::from_corners(corner, corner + CELL_SIZE), on_cell);
    }
}

fn chunk_of_prop(prop: &SceneryProp) -> IVec2 {
    chunk_of(world_to_cell(prop.transform.translation))
}

pub fn is_trampleable(model_id: &str) -> bool {
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
    layout: Option<ResMut<SceneryLayout>>,
    new_buildings: Query<&Building, Added<Building>>,
) {
    let Some(mut layout) = layout else { return };
    let clearing_margin = 0.6;
    for building in &new_buildings {
        let min = building.origin.as_vec2() * CELL_SIZE - Vec2::splat(clearing_margin);
        let max = (building.origin + building.kind.spec().footprint).as_vec2() * CELL_SIZE
            + Vec2::splat(clearing_margin);
        layout.remove_in(Rect::from_corners(min, max), |prop| prop.decor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(model: &'static str, cell: IVec2, decor: bool) -> SceneryProp {
        SceneryProp {
            model,
            transform: Transform::from_translation(cell_center(cell)),
            decor,
        }
    }

    #[test]
    fn props_are_kept_by_chunk_and_an_edit_marks_only_its_chunk() {
        let mut layout = SceneryLayout::default();
        let here = IVec2::new(5, 5);
        let elsewhere = IVec2::new(40, 5);
        layout.place(prop("tree", here, false));
        layout.place(prop("grass_tuft", here, true));
        layout.place(prop("rock", elsewhere, false));
        let before_here = layout.chunk(chunk_of(here)).unwrap().revision;
        let before_elsewhere = layout.chunk(chunk_of(elsewhere)).unwrap().revision;

        layout.clear_cell(here);

        let edited = layout.chunk(chunk_of(here)).unwrap();
        assert!(edited.props.is_empty());
        assert_ne!(edited.revision, before_here);
        let untouched = layout.chunk(chunk_of(elsewhere)).unwrap();
        assert_eq!(untouched.props.len(), 1);
        assert_eq!(untouched.revision, before_elsewhere);
    }

    #[test]
    fn a_structure_clears_only_decoration_under_it() {
        let mut layout = SceneryLayout::default();
        let site = IVec2::new(31, 31);
        layout.place(prop("grass_tuft", site, true));
        layout.place(prop("tree", site + IVec2::X, false));
        layout.place(prop("flowers", site + IVec2::new(8, 0), true));
        let footprint = Rect::from_corners(
            site.as_vec2() * CELL_SIZE,
            (site + IVec2::splat(2)).as_vec2() * CELL_SIZE,
        );

        layout.remove_in(footprint, |prop| prop.decor);

        let left: Vec<&str> = [chunk_of(site), chunk_of(site + IVec2::new(8, 0))]
            .iter()
            .flat_map(|&chunk| layout.chunk(chunk).unwrap().props.iter().map(|p| p.model))
            .collect();
        assert_eq!(left, vec!["tree", "flowers"]);
    }
}
