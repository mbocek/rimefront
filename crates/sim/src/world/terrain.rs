use bevy::prelude::*;

use serde::{Deserialize, Serialize};

use crate::sim::sim_math;
use crate::world::map::{
    CELL_SIZE, MapBounds, base_sites, map_bounds, map_spread, relocate, set_map_bounds,
};

pub const WATER_LEVEL: f32 = -0.45;
/// How far the mountains surrounding the playable area reach beyond the map edge.
pub const OUTLANDS_WIDTH: f32 = 44.0;

/// How far beyond the detailed rim the coarse mountains reach, enough for the farthest zoom.
pub const FAR_RING_WIDTH: f32 = 110.0;
const RIVER_DEPTH: f32 = 1.5;
const RIVER_HALF_WIDTH_CELLS: f32 = 1.7;
/// Half the width of a ford, where the river can be crossed.
const FORD_HALF_WIDTH_CELLS: f32 = 2.5;
const STANDARD_MAP_CENTER_CELLS: f32 = 31.5;
const ROLLING_HILLS_HEIGHT: f32 = 1.1;
const MOUNTAIN_HEIGHT: f32 = 16.0;
const MOUNTAIN_RISE_DISTANCE: f32 = 26.0;
const BASE_FLAT_RADIUS_CELLS: f32 = 9.0;
const BASE_PAD_RADIUS_CELLS: f32 = 4.5;

/// The parts of a landscape that a level's terrain seed varies; everything stays point-symmetric around the
/// map centre, so neither corner is favoured.
#[derive(Clone, Copy, PartialEq, Debug)]
struct TerrainShape {
    /// Amplitude (cells) and frequency of two waves; sines are odd functions, which keeps the river symmetric.
    river_waves: [(f32, f32); 2],
    /// How far the outer fords lie from the middle one on the standard map.
    side_ford_offset: f32,
    lake_length: f32,
    lake_width: f32,
    hills_seed: u32,
    mountains_seed: u32,
    /// Ponds on the plains, as an offset from the map centre in standard cells and a radius; each has a twin
    /// on the other side of the centre.
    ponds: [Option<(Vec2, f32)>; 2],
}

/// The landscape every level had before terrain seeds; levels without one keep it.
const CLASSIC_SHAPE: TerrainShape = TerrainShape {
    river_waves: [(5.0, 0.13), (2.0, 0.31)],
    side_ford_offset: 20.0,
    lake_length: 13.0,
    lake_width: 5.5,
    hills_seed: 11,
    mountains_seed: 71,
    ponds: [None, None],
};

impl TerrainShape {
    fn rolled(seed: u64) -> TerrainShape {
        use rand::rngs::StdRng;
        use rand::{RngExt, SeedableRng};
        let mut rng = StdRng::seed_from_u64(seed);
        let sign = |rng: &mut StdRng| if rng.random_bool(0.5) { 1.0 } else { -1.0 };
        TerrainShape {
            river_waves: [
                (
                    sign(&mut rng) * rng.random_range(3.0..6.5),
                    rng.random_range(0.08..0.17),
                ),
                (
                    sign(&mut rng) * rng.random_range(0.5..3.0),
                    rng.random_range(0.24..0.42),
                ),
            ],
            side_ford_offset: rng.random_range(15.0..24.0),
            lake_length: rng.random_range(10.0..16.0),
            lake_width: rng.random_range(4.5..7.0),
            hills_seed: rng.random_range(1000..u32::MAX / 2),
            mountains_seed: rng.random_range(1000..u32::MAX / 2),
            ponds: std::array::from_fn(|_| {
                rng.random_bool(0.7).then(|| {
                    let angle = rng.random_range(0.0..std::f32::consts::TAU);
                    let reach = rng.random_range(6.0..14.0);
                    let offset = Vec2::new(sim_math::cos(angle), sim_math::sin(angle)) * reach;
                    (offset, rng.random_range(2.5..4.5))
                })
            }),
        }
    }
}

// Kept per thread like the map size, and set from the level together with it.
thread_local! {
    static ACTIVE_SHAPE: std::cell::Cell<TerrainShape> = const { std::cell::Cell::new(CLASSIC_SHAPE) };
}

/// Shapes the landscape for a level: `None` keeps the classic river, lake and hills.
pub fn set_terrain_seed(seed: Option<u64>) {
    ACTIVE_SHAPE.set(seed.map_or(CLASSIC_SHAPE, TerrainShape::rolled));
}

fn shape() -> TerrainShape {
    ACTIVE_SHAPE.get()
}
const CAUSEWAY_HALF_WIDTH_CELLS: f32 = 1.2;
const ROAD_HALF_WIDTH: f32 = 1.4;
const ROAD_SHOULDER: f32 = 1.6;

fn hash(cell: IVec2, seed: u32) -> f32 {
    let mut h = (cell.x as u32).wrapping_mul(0x8da6_b343)
        ^ (cell.y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

/// Smooth value noise in [-1, 1].
fn value_noise(point: Vec2, seed: u32) -> f32 {
    let cell = point.floor();
    let local = point - cell;
    let blend = local * local * (Vec2::splat(3.0) - 2.0 * local);
    let corner = |dx: i32, dy: i32| hash(cell.as_ivec2() + IVec2::new(dx, dy), seed);
    let bottom = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * blend.x;
    let top = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * blend.x;
    (bottom + (top - bottom) * blend.y) * 2.0 - 1.0
}

fn fractal_noise(point: Vec2, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amplitude, mut frequency, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..octaves {
        sum += value_noise(point * frequency, seed + octave) * amplitude;
        norm += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }
    sum / norm
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn river_centerline(column: f32) -> f32 {
    let middle = map_bounds().center_cells();
    let offset = column - middle.x;
    let [(first, first_frequency), (second, second_frequency)] = shape().river_waves;
    middle.y
        + first * sim_math::sin(offset * first_frequency)
        + second * sim_math::sin(offset * second_frequency)
}

/// The middle column of a ford authored at `standard_column` on the standard map: the middle ford, or an
/// outer one moved to where this terrain puts it.
fn ford_column(standard_column: f32) -> f32 {
    let authored = standard_column - STANDARD_MAP_CENTER_CELLS;
    let classic_side_offset = CLASSIC_SHAPE.side_ford_offset;
    let offset = authored * shape().side_ford_offset / classic_side_offset;
    let bounds = map_bounds();
    bounds.center_cells().x + offset * bounds.spread().x
}

fn ford_columns() -> [f32; 3] {
    let side = CLASSIC_SHAPE.side_ford_offset;
    [-side, 0.0, side].map(|offset| ford_column(STANDARD_MAP_CENTER_CELLS + offset))
}

fn is_ford(column: f32) -> bool {
    ford_columns()
        .iter()
        .any(|&middle| (column - middle).abs() <= FORD_HALF_WIDTH_CELLS)
}

/// Depth of the riverbed below the surrounding ground at a point in cell units; zero away from the river.
pub fn river_depth(column: f32, row: f32) -> f32 {
    let bounds = map_bounds();
    let inside_map = (bounds.min.x as f32..(bounds.min.x + bounds.size.x) as f32).contains(&column);
    if inside_map && is_ford(column) {
        return 0.0;
    }
    let distance = (row - river_centerline(column)).abs();
    let bank = 1.0 - (distance / RIVER_HALF_WIDTH_CELLS).clamp(0.0, 1.0);
    RIVER_DEPTH * sim_math::powf(bank, 0.6)
}

fn distance_to_river_cells(column: f32, row: f32) -> f32 {
    (row - river_centerline(column)).abs()
}

fn distance_outside_map(position: Vec2) -> f32 {
    let bounds = map_bounds();
    let outside = (bounds.world_min() - position)
        .max(position - bounds.world_max())
        .max(Vec2::ZERO);
    outside.length()
}

fn distance_to_bases_cells(column: f32, row: f32) -> f32 {
    base_sites()
        .iter()
        .map(|base| (Vec2::new(column, row) - base.as_vec2() - Vec2::splat(1.5)).length())
        .fold(f32::MAX, f32::min)
}

fn mountain_height(position: Vec2, gorge: f32) -> f32 {
    let outside = distance_outside_map(position);
    if outside <= 0.0 {
        return 0.0;
    }
    let rise = smoothstep(0.0, MOUNTAIN_RISE_DISTANCE, outside);
    let massif = 0.65 + 0.35 * fractal_noise(position * 0.03, 3, shape().mountains_seed);
    let crags = fractal_noise(position * 0.11, 3, 97).abs() * 0.25;
    MOUNTAIN_HEIGHT * rise * (massif + crags) * (1.0 - gorge)
}

fn rolling_hills(position: Vec2) -> f32 {
    let (column, row) = (position.x / CELL_SIZE, position.y / CELL_SIZE);
    let calm_near_bases = smoothstep(
        BASE_FLAT_RADIUS_CELLS * 0.6,
        BASE_FLAT_RADIUS_CELLS,
        distance_to_bases_cells(column, row),
    );
    let hill_shape = fractal_noise(position * 0.03, 3, shape().hills_seed) * 0.5 + 0.5;
    hill_shape * ROLLING_HILLS_HEIGHT * calm_near_bases
}

/// Ground height without any water, the same for every landscape; good enough to place things that settle
/// onto the real surface afterwards.
pub fn approximate_ground(position: Vec3) -> Vec3 {
    position.with_y(rolling_hills(position.xz()))
}

/// Signed distance in cells from the nearest pond's shore, negative inside; `f32::MAX` without ponds.
fn distance_to_pond_shore(column: f32, row: f32) -> f32 {
    let point = Vec2::new(column, row);
    let bounds = map_bounds();
    let center = bounds.center_cells();
    shape()
        .ponds
        .iter()
        .flatten()
        .flat_map(|&(offset, radius)| {
            let offset = offset * bounds.spread();
            [(center + offset, radius), (center - offset, radius)]
        })
        .map(|(middle, radius)| point.distance(middle) - radius)
        .fold(f32::MAX, f32::min)
}

/// Where the lake sits: `along` runs from the north-west to the south-east site, `across` from base to base.
fn lake_coordinates(column: f32, row: f32) -> (f32, f32) {
    let offset = Vec2::new(column, row) - map_bounds().center_cells();
    let along = (offset.x + offset.y) * std::f32::consts::FRAC_1_SQRT_2;
    let across = (offset.x - offset.y) * std::f32::consts::FRAC_1_SQRT_2;
    (along, across)
}

/// 0 at the lake's middle, 1 on its shore.
fn lake_ellipse(column: f32, row: f32) -> f32 {
    let (along, across) = lake_coordinates(column, row);
    let length = shape().lake_length * map_spread();
    let (along, across) = (along / length, across / shape().lake_width);
    (along * along + across * across).sqrt()
}

/// The shape of the battlefield. Every landscape is point-symmetric around the map centre.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Landscape {
    /// A winding river with three fords.
    #[default]
    River,
    /// A long lake between the bases with a narrow causeway across its middle.
    Lake,
    /// Open ground without water.
    Plains,
}

impl Landscape {
    pub const ALL: [Landscape; 3] = [Landscape::River, Landscape::Lake, Landscape::Plains];

    /// Depth of the water bed below the surrounding ground at a point in cell units; zero on dry land.
    pub fn water_depth(self, column: f32, row: f32) -> f32 {
        match self {
            Landscape::River => river_depth(column, row),
            Landscape::Lake => {
                let (along, _) = lake_coordinates(column, row);
                let causeway_bank = smoothstep(
                    CAUSEWAY_HALF_WIDTH_CELLS,
                    CAUSEWAY_HALF_WIDTH_CELLS + 1.0,
                    along.abs(),
                );
                RIVER_DEPTH * smoothstep(1.0, 0.7, lake_ellipse(column, row)) * causeway_bank
            }
            Landscape::Plains => {
                RIVER_DEPTH * smoothstep(0.0, 1.2, -distance_to_pond_shore(column, row).min(1.2))
            }
        }
    }

    /// Distance in cells from the middle of the nearest water; `f32::MAX` where there is none.
    fn distance_to_water(self, column: f32, row: f32) -> f32 {
        match self {
            Landscape::River => distance_to_river_cells(column, row),
            Landscape::Lake => (lake_ellipse(column, row) - 0.7).max(0.0) * shape().lake_width,
            Landscape::Plains => distance_to_pond_shore(column, row).max(0.0),
        }
    }

    /// Only the river leaves the map, cutting a gorge through the mountains.
    fn gorge(self, column: f32, row: f32) -> f32 {
        match self {
            Landscape::River => 1.0 - smoothstep(2.0, 7.0, distance_to_river_cells(column, row)),
            Landscape::Lake | Landscape::Plains => 0.0,
        }
    }

    /// Terrain surface height in meters at a world position.
    pub fn height(self, x: f32, z: f32) -> f32 {
        let position = Vec2::new(x, z);
        let (column, row) = (x / CELL_SIZE, z / CELL_SIZE);
        let calm_near_water = smoothstep(2.0, 5.0, self.distance_to_water(column, row));
        rolling_hills(position) * calm_near_water
            + mountain_height(position, self.gorge(column, row))
            - self.water_depth(column, row)
    }

    pub fn normal(self, x: f32, z: f32) -> Vec3 {
        let step = 0.4;
        let dx = self.height(x + step, z) - self.height(x - step, z);
        let dz = self.height(x, z + step) - self.height(x, z - step);
        Vec3::new(-dx, 2.0 * step, -dz).normalize()
    }

    pub fn on_ground(self, position: Vec3) -> Vec3 {
        position.with_y(self.height(position.x, position.z))
    }

    /// Ground beside the water where reeds grow.
    pub fn is_shore(self, column: f32, row: f32) -> bool {
        let shore_band = match self {
            Landscape::River => RIVER_HALF_WIDTH_CELLS + 0.2..RIVER_HALF_WIDTH_CELLS + 0.9,
            Landscape::Lake => 1.6..2.4,
            Landscape::Plains => 0.2..0.9,
        };
        self.water_depth(column, row) == 0.0
            && shore_band.contains(&self.distance_to_water(column, row))
    }

    fn roads(self) -> Vec<Vec<Vec2>> {
        match self {
            Landscape::River => river_roads(),
            Landscape::Lake | Landscape::Plains => open_roads(),
        }
    }
}

fn base_gates() -> ([Vec2; 4], impl Fn(Vec2) -> Vec2, impl Fn(Vec2) -> Vec2) {
    let sites = base_sites().map(|site| site.as_vec2() + Vec2::splat(1.5));
    let west_gate = |site: Vec2| site + Vec2::new(3.0, 0.0);
    let east_gate = |site: Vec2| site + Vec2::new(-3.0, 0.0);
    (sites, west_gate, east_gate)
}

fn in_world(points: &[Vec2]) -> Vec<Vec2> {
    points.iter().map(|p| *p * CELL_SIZE).collect()
}

/// Roads are authored on the standard map: their bends are relocated, their fords follow the spread river.
fn river_roads() -> Vec<Vec<Vec2>> {
    let ford = |standard_column: f32| {
        let column = ford_column(standard_column);
        Vec2::new(column, river_centerline(column) + 0.5)
    };
    let ([south_west, north_east, north_west, south_east], west_gate, east_gate) = base_gates();
    vec![
        in_world(&[
            west_gate(south_west),
            relocate(Vec2::new(12.0, 44.0)),
            ford(11.5),
            relocate(Vec2::new(14.0, 20.0)),
            west_gate(north_west),
        ]),
        in_world(&[
            west_gate(north_west),
            relocate(Vec2::new(30.0, 11.0)),
            east_gate(north_east),
        ]),
        in_world(&[
            west_gate(south_west),
            relocate(Vec2::new(28.0, 46.0)),
            ford(31.5),
            relocate(Vec2::new(40.0, 18.0)),
            east_gate(north_east),
        ]),
        in_world(&[
            west_gate(south_west),
            relocate(Vec2::new(32.0, 53.0)),
            east_gate(south_east),
        ]),
        in_world(&[
            east_gate(south_east),
            relocate(Vec2::new(54.0, 44.0)),
            ford(51.5),
            relocate(Vec2::new(55.0, 20.0)),
            east_gate(north_east),
        ]),
    ]
}

/// One road straight across the middle (over the causeway where there is a lake) and two along the edges.
fn open_roads() -> Vec<Vec<Vec2>> {
    let ([south_west, north_east, north_west, south_east], west_gate, east_gate) = base_gates();
    let middle = map_bounds().center_cells() + 0.5;
    vec![
        in_world(&[west_gate(south_west), middle, east_gate(north_east)]),
        in_world(&[
            west_gate(north_west),
            relocate(Vec2::new(30.0, 11.0)),
            east_gate(north_east),
        ]),
        in_world(&[
            west_gate(south_west),
            relocate(Vec2::new(32.0, 53.0)),
            east_gate(south_east),
        ]),
        in_world(&[
            west_gate(south_west),
            relocate(Vec2::new(10.0, 30.0)),
            west_gate(north_west),
        ]),
        in_world(&[
            east_gate(south_east),
            relocate(Vec2::new(53.0, 32.0)),
            east_gate(north_east),
        ]),
    ]
}

fn distance_to_polyline(point: Vec2, polyline: &[Vec2]) -> f32 {
    polyline
        .windows(2)
        .map(|segment| {
            let (start, end) = (segment[0], segment[1]);
            let along = (point - start).dot(end - start) / (end - start).length_squared();
            point.distance(start + (end - start) * along.clamp(0.0, 1.0))
        })
        .fold(f32::MAX, f32::min)
}

pub struct GroundLayers {
    landscape: Landscape,
    roads: Vec<Vec<Vec2>>,
}

impl GroundLayers {
    pub fn landscape(&self) -> Landscape {
        self.landscape
    }

    pub fn new(landscape: Landscape) -> Self {
        Self {
            landscape,
            roads: landscape.roads(),
        }
    }

    /// Blend weights of the terrain layers dirt, rock, riverbed and base gravel; grass fills the remainder.
    pub fn weights(&self, position: Vec2, crystal_soil: f32) -> Vec4 {
        layer_weights(self.landscape, position, &self.roads, crystal_soil)
    }

    pub fn minimap_color(&self, position: Vec2, crystal: bool, grass: Vec3) -> Vec3 {
        let weights = self.weights(position, if crystal { 1.0 } else { 0.0 });
        let dirt = Vec3::new(0.42, 0.34, 0.24);
        let rock = Vec3::new(0.48, 0.46, 0.43);
        let riverbed = Vec3::new(0.20, 0.36, 0.48);
        let pad = Vec3::new(0.52, 0.51, 0.48);
        let grass_weight = (1.0 - weights.element_sum()).max(0.0);
        grass * grass_weight
            + dirt * weights.x
            + rock * weights.y
            + riverbed * weights.z
            + pad * weights.w
    }
}

fn layer_weights(
    landscape: Landscape,
    position: Vec2,
    roads: &[Vec<Vec2>],
    crystal_soil: f32,
) -> Vec4 {
    let (column, row) = (position.x / CELL_SIZE, position.y / CELL_SIZE);
    let normal = landscape.normal(position.x, position.y);
    let slope = 1.0 - normal.y;
    let mountain = mountain_height(position, landscape.gorge(column, row));
    let patchiness = fractal_noise(position * 0.08, 3, 23);

    let rock = (smoothstep(0.08, 0.3, slope)
        + smoothstep(2.0, 7.0, mountain)
        + smoothstep(0.55, 0.8, patchiness) * 0.6)
        .min(1.0);
    let riverbed = smoothstep(0.05, 0.5, landscape.water_depth(column, row))
        .max(1.0 - smoothstep(1.4, 2.6, landscape.distance_to_water(column, row)));
    let road_distance = roads
        .iter()
        .map(|road| distance_to_polyline(position, road))
        .fold(f32::MAX, f32::min);
    let road_edge_noise = fractal_noise(position * 0.6, 2, 31) * 0.6;
    let road = 1.0
        - smoothstep(
            ROAD_HALF_WIDTH,
            ROAD_HALF_WIDTH + ROAD_SHOULDER,
            road_distance + road_edge_noise,
        );
    let worn_ground = smoothstep(0.2, 0.7, patchiness) * 0.5;
    let base_distance = distance_to_bases_cells(column, row);
    let base_wear =
        (1.0 - smoothstep(
            BASE_PAD_RADIUS_CELLS,
            BASE_FLAT_RADIUS_CELLS - 1.0,
            base_distance,
        )) * (0.6 + 0.4 * patchiness);
    let dirt = road
        .max(worn_ground)
        .max(base_wear * 0.8)
        .max(crystal_soil * 0.7);
    let pad = 1.0
        - smoothstep(
            BASE_PAD_RADIUS_CELLS - 1.5,
            BASE_PAD_RADIUS_CELLS + patchiness * 1.5,
            base_distance,
        );

    let mut remaining = 1.0;
    let mut claim = |wanted: f32| {
        let taken = wanted.min(remaining);
        remaining -= taken;
        taken
    };
    let riverbed = claim(riverbed);
    let pad = claim(pad);
    let rock = claim(rock);
    let dirt = claim(dirt);
    Vec4::new(dirt, rock, riverbed, pad)
}

/// The ambient state a landscape's height depends on, carried to a thread that builds ground for it.
#[derive(Clone, Copy)]
pub struct LandscapeContext {
    bounds: MapBounds,
    shape: TerrainShape,
}

impl LandscapeContext {
    pub fn current() -> Self {
        Self {
            bounds: map_bounds(),
            shape: shape(),
        }
    }

    pub fn enter(self) {
        set_map_bounds(self.bounds);
        ACTIVE_SHAPE.set(self.shape);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fords_are_dry_and_the_river_between_them_is_deep() {
        for column in [11.0, 32.0, 53.0] {
            assert_eq!(river_depth(column, river_centerline(column)), 0.0);
        }
        for column in [5.0, 22.0, 42.0, 60.0] {
            assert!(river_depth(column, river_centerline(column)) > 1.0);
        }
    }

    #[test]
    fn every_rolled_terrain_keeps_dry_fords_and_its_point_symmetry() {
        let mirror = |value: f32| 2.0 * map_bounds().center_cells().x - value;
        for seed in 0..40 {
            set_terrain_seed(Some(seed));
            for middle in ford_columns() {
                assert_eq!(
                    river_depth(middle, river_centerline(middle)),
                    0.0,
                    "seed {seed}"
                );
            }
            for landscape in Landscape::ALL {
                for (column, row) in [(7.3, 20.1), (25.0, 33.5), (40.2, 12.7), (31.0, 29.0)] {
                    let here = landscape.water_depth(column, row);
                    let there = landscape.water_depth(mirror(column), mirror(row));
                    assert!(
                        (here - there).abs() < 1e-3,
                        "seed {seed}: {landscape:?} at {column},{row}"
                    );
                }
            }
        }
        set_terrain_seed(None);
        assert_eq!(shape(), CLASSIC_SHAPE);
    }

    #[test]
    fn playable_ground_is_gentle_and_finite() {
        let step = CELL_SIZE * 1.5;
        let bounds = map_bounds();
        let mut x = bounds.world_min().x;
        while x < bounds.world_max().x {
            let mut z = bounds.world_min().y;
            while z < bounds.world_max().y {
                let height = Landscape::River.height(x, z);
                assert!(height.is_finite());
                assert!(
                    (-RIVER_DEPTH - 0.1..=ROLLING_HILLS_HEIGHT + 0.1).contains(&height),
                    "height {height} at {x},{z}"
                );
                z += step;
            }
            x += step;
        }
    }

    #[test]
    fn bases_sit_on_flat_ground() {
        for base in base_sites() {
            let center = (base.as_vec2() + Vec2::splat(1.5)) * CELL_SIZE;
            for offset in [Vec2::ZERO, Vec2::new(4.0, 0.0), Vec2::new(0.0, -4.0)] {
                let point = center + offset;
                for landscape in Landscape::ALL {
                    assert!(landscape.height(point.x, point.y).abs() < 0.05);
                }
            }
        }
    }

    #[test]
    fn mountains_rise_around_the_map() {
        let outside =
            Landscape::River.height(-OUTLANDS_WIDTH * 0.8, map_bounds().world_size().y * 0.2);
        assert!(outside > 4.0, "rim only {outside} m high");
    }

    #[test]
    fn terrain_normals_point_up() {
        for (x, z) in [(10.0, 10.0), (64.0, 64.0), (-20.0, 50.0)] {
            let normal = Landscape::River.normal(x, z);
            assert!(normal.y > 0.0 && (normal.length() - 1.0).abs() < 1e-4);
        }
    }
}
