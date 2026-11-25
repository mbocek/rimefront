use std::ops::RangeInclusive;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::levels::ActiveLevel;
use crate::sim::catalog::CRYSTAL_PER_CELL;
use crate::sim::intel::Intel;
use crate::world::map::{
    CELL_SIZE, CHUNK_CELLS, CrystalField, CrystalFields, GameMap, MapBounds, base_sites,
    block_water_among, is_water, level_bounds, map_bounds, near_base, spawn_crystal_visual,
    widen_map_bounds,
};
use crate::world::scenery::{
    BUSHES, CLUMP_SIZE, CLUMP_SPREAD, CONIFER_FOREST, DEAD_TREE_CHANCE, DECIDUOUS_FOREST,
    MAX_MEADOW_DIRT, MIXED_FOREST, ROCKS, SceneryLayout, block_cell_with_prop, cover_mountains,
    pick, place_decor,
};
use crate::world::sites::{sites_around, sites_within};
use crate::world::terrain::{GroundLayers, Landscape};

/// Share of chunks with a crystal field of their own, besides the one beside each base.
const FIELD_CHANCE: f64 = 0.4;
const HOME_FIELD_OFFSET: f32 = 7.5;
const HOME_FIELD_RADIUS: f32 = 3.6;
/// Own fields keep this far from a base or a starting site, and its surroundings stay open half as far.
const BASE_CLEARANCE: f32 = 18.0;
const FIELD_RADIUS: std::ops::Range<f32> = 2.6..4.2;
/// A field's centre stays this far inside its chunk, so it reaches no further than the next chunk.
const FIELD_MARGIN: i32 = 5;
/// How much of a field's edge a cell may fall short of or stick out past.
const FIELD_EDGE_JITTER: f32 = 0.3;
const FIELD_CLEARANCE: f32 = 2.0;
/// Scenery per chunk, as dense as on the standard map: 16 forest clumps, 40 rocks and 2,600 tries at a
/// meadow tuft on its 4,096 cells.
const FOREST_CLUMPS: RangeInclusive<u32> = 2..=6;
const ROCKS_PER_CHUNK: RangeInclusive<u32> = 6..=14;
const MEADOW_TRIES_PER_CHUNK: u32 = 650;
const SHORE_TRIES_PER_CHUNK: u32 = 2_500;
const REEDS_PER_CHUNK: u32 = 30;
const FOREST_MARGIN: i32 = 4;
const FIELDS_SALT: u64 = 0x6669_656c_6473;
const SCENERY_SALT: u64 = 0x7363_656e_6572;
const CRYSTAL_LOOK_SALT: u64 = 0x6c6f_6f6b;
const OUTLANDS_SALT: u64 = 0x6f75_746c;

/// Fills the map with crystal fields and scenery chunk by chunk. Each chunk comes from the map seed and its own
/// coordinate alone, so it is the same whichever chunks are generated with it, in whatever order. Returns
/// the crystal fields on the map.
pub fn generate_wilds(
    commands: &mut Commands,
    map: &mut GameMap,
    scenery: &mut SceneryLayout,
    map_seed: u64,
) -> Vec<CrystalField> {
    let bounds = map_bounds();
    for index in 0..bounds.chunk_count() {
        generate_chunk(commands, map, scenery, map_seed, bounds.chunk_at(index));
    }
    let mut outlands = StdRng::seed_from_u64(map_seed ^ OUTLANDS_SALT);
    cover_mountains(scenery, Landscape::Wilds, &mut outlands);
    let mut fields = home_fields();
    fields.extend(sites_within(map_seed, bounds).into_iter().map(site_field));
    fields.extend(own_fields_among(
        map_seed,
        (0..bounds.chunk_count()).map(|index| bounds.chunk_at(index)),
    ));
    fields
}

fn own_fields_among(
    map_seed: u64,
    chunks: impl Iterator<Item = IVec2>,
) -> impl Iterator<Item = CrystalField> {
    chunks.filter_map(move |chunk| own_field(map_seed, chunk))
}

/// The world widened during a match; whatever shows it covers the new ground.
#[derive(Message, Clone, Copy, Debug)]
pub struct WorldWidened {
    pub bounds: MapBounds,
}

/// Widens the world to `wider`, which contains it and like it starts and ends on chunk edges: the map and every
/// side's fog cover the new ground, and each new chunk is generated as `generate_chunk` makes it.
pub fn widen_world(world: &mut World, wider: MapBounds) {
    let narrow = map_bounds();
    let on_chunk_edges = |bounds: MapBounds| {
        bounds.min.rem_euclid(IVec2::splat(CHUNK_CELLS)) == IVec2::ZERO
            && bounds.size.rem_euclid(IVec2::splat(CHUNK_CELLS)) == IVec2::ZERO
    };
    assert!(
        on_chunk_edges(narrow) && on_chunk_edges(wider),
        "the wilds widen by whole chunks"
    );
    let map_seed = world.resource::<ActiveLevel>().0.map_seed;
    widen_map_bounds(wider);
    let new_chunks: Vec<IVec2> = (0..wider.chunk_count())
        .map(|index| wider.chunk_at(index))
        .filter(|&chunk| !narrow.contains(chunk * CHUNK_CELLS))
        .collect();
    world.resource_scope(|world, mut map: Mut<GameMap>| {
        world.resource_scope(|world, mut scenery: Mut<SceneryLayout>| {
            map.widen(wider);
            let mut commands = world.commands();
            for &chunk in &new_chunks {
                let (first, last) = wider.cells_of_chunk(chunk);
                scenery.clear_area(Rect::from_corners(
                    first.as_vec2() * CELL_SIZE,
                    (last + IVec2::ONE).as_vec2() * CELL_SIZE,
                ));
                map.open_chunk(chunk);
                block_water_among(&mut map, Landscape::Wilds, cells_between(first, last));
                generate_chunk(&mut commands, &mut map, &mut scenery, map_seed, chunk);
            }
            let mut outlands = coordinate_rng(map_seed ^ OUTLANDS_SALT, wider.size);
            cover_mountains(&mut scenery, Landscape::Wilds, &mut outlands);
        });
    });
    world.flush();
    world.resource_mut::<Intel>().widen(wider);
    let old_sites = sites_within(map_seed, narrow);
    let new_sites = sites_within(map_seed, wider)
        .into_iter()
        .filter(|site| !old_sites.contains(site));
    let mut new_fields: Vec<CrystalField> = new_sites.map(site_field).collect();
    new_fields.extend(own_fields_among(map_seed, new_chunks.into_iter()));
    world.resource_mut::<CrystalFields>().0.extend(new_fields);
    if let Some(mut widened) = world.get_resource_mut::<Messages<WorldWidened>>() {
        widened.write(WorldWidened { bounds: wider });
    }
}

/// The crystal and scenery of one chunk; it changes no cell outside the chunk.
pub fn generate_chunk(
    commands: &mut Commands,
    map: &mut GameMap,
    scenery: &mut SceneryLayout,
    map_seed: u64,
    chunk: IVec2,
) {
    let (first, last) = map_bounds().cells_of_chunk(chunk);
    let sites: Vec<IVec2> = sites_around(map_seed, (first + last) / 2).collect();
    let fields = fields_reaching(map_seed, chunk, &sites);
    for cell in cells_between(first, last) {
        let amount = crystal_at(map, &fields, cell);
        if amount > 0 {
            map.set_crystal(cell, amount);
            spawn_crystal_visual(commands, &mut crystal_look(map_seed, cell), cell, amount);
        }
    }
    let mut rng = coordinate_rng(map_seed ^ SCENERY_SALT, chunk);
    let mut chunk_scenery = ChunkScenery {
        scenery,
        map,
        fields: &fields,
        sites: &sites,
        first,
        last,
    };
    chunk_scenery.plant_forests(&mut rng, chunk);
    chunk_scenery.place_rocks(&mut rng);
    chunk_scenery.dress_meadows(&mut rng);
    chunk_scenery.line_shores(&mut rng);
}

fn cells_between(first: IVec2, last: IVec2) -> impl Iterator<Item = IVec2> {
    (first.y..=last.y).flat_map(move |y| (first.x..=last.x).map(move |x| IVec2::new(x, y)))
}

/// A random sequence fixed for each coordinate of a grid: a chunk, a cell or a square of sites.
pub(crate) fn coordinate_rng(seed: u64, chunk: IVec2) -> StdRng {
    let key = ((chunk.x as u32 as u64) << 32) | chunk.y as u32 as u64;
    StdRng::seed_from_u64(seed ^ key.wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

/// Each crystal looks the same however its chunk came to be generated.
pub fn crystal_look(map_seed: u64, cell: IVec2) -> StdRng {
    coordinate_rng(map_seed ^ CRYSTAL_LOOK_SALT, cell)
}

/// A field beside every base, where the standard map has the player's: diagonally towards the middle of the
/// map, so every side starts with crystal at hand.
fn home_fields() -> Vec<CrystalField> {
    let middle = level_bounds().center_cells();
    base_sites()
        .iter()
        .map(|site| {
            let base_middle = site.as_vec2() + Vec2::splat(1.5);
            let towards_middle = (middle - base_middle).signum() * HOME_FIELD_OFFSET;
            CrystalField {
                center: (base_middle + towards_middle).floor().as_ivec2(),
                radius: HOME_FIELD_RADIUS,
            }
        })
        .collect()
}

/// The field a chunk has of its own, if any; never beside a base, where the home fields are. It does not
/// depend on the map's bounds, so a chunk at the edge has its neighbour's crystal before the map widens there.
fn own_field(map_seed: u64, chunk: IVec2) -> Option<CrystalField> {
    let mut rng = coordinate_rng(map_seed ^ FIELDS_SALT, chunk);
    if !rng.random_bool(FIELD_CHANCE) {
        return None;
    }
    let inside = FIELD_MARGIN..CHUNK_CELLS - FIELD_MARGIN;
    let center = chunk * CHUNK_CELLS
        + IVec2::new(rng.random_range(inside.clone()), rng.random_range(inside));
    let radius = rng.random_range(FIELD_RADIUS);
    let clear_of_bases = base_sites()
        .into_iter()
        .chain(sites_around(map_seed, center))
        .all(|site| (center - site).as_vec2().length() > BASE_CLEARANCE);
    clear_of_bases.then_some(CrystalField { center, radius })
}

/// The home field beside a generated starting site, set like a level base's: towards the level's middle.
fn site_field(site: IVec2) -> CrystalField {
    let base_middle = site.as_vec2() + Vec2::splat(1.5);
    let towards_middle = (level_bounds().center_cells() - base_middle).signum() * HOME_FIELD_OFFSET;
    CrystalField {
        center: (base_middle + towards_middle).floor().as_ivec2(),
        radius: HOME_FIELD_RADIUS,
    }
}

fn fields_reaching(map_seed: u64, chunk: IVec2, sites: &[IVec2]) -> Vec<CrystalField> {
    let mut fields = home_fields();
    fields.extend(sites.iter().map(|&site| site_field(site)));
    for dy in -1..=1 {
        for dx in -1..=1 {
            fields.extend(own_field(map_seed, chunk + IVec2::new(dx, dy)));
        }
    }
    fields
}

/// Crystal a cell holds: rich in the middle of a field, thinner towards its ragged edge.
fn crystal_at(map: &GameMap, fields: &[CrystalField], cell: IVec2) -> u32 {
    if !map.is_passable(cell) {
        return 0;
    }
    fields
        .iter()
        .filter_map(|field| {
            let distance = (cell - field.center).as_vec2().length();
            let ragged = 1.0 - FIELD_EDGE_JITTER + 2.0 * FIELD_EDGE_JITTER * fixed_noise(cell);
            let edge = field.radius * ragged;
            (distance <= edge).then(|| {
                let richness = 1.0 - 0.5 * (distance / edge);
                (CRYSTAL_PER_CELL as f32 * richness) as u32
            })
        })
        .max()
        .unwrap_or(0)
}

/// A number in [0, 1] fixed for each coordinate.
fn fixed_noise(cell: IVec2) -> f32 {
    let mut h =
        (cell.x as u32).wrapping_mul(0x8da6_b343) ^ (cell.y as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 0xffff as f32
}

fn near_field(fields: &[CrystalField], cell: IVec2) -> bool {
    fields.iter().any(|field| {
        let outer = field.radius * (1.0 + FIELD_EDGE_JITTER) + FIELD_CLEARANCE;
        (cell - field.center).as_vec2().length() < outer
    })
}

struct ChunkScenery<'a> {
    scenery: &'a mut SceneryLayout,
    map: &'a mut GameMap,
    fields: &'a [CrystalField],
    /// Generated starting sites nearby, whose ground stays open.
    sites: &'a [IVec2],
    first: IVec2,
    last: IVec2,
}

impl ChunkScenery<'_> {
    fn inside(&self, cell: IVec2) -> bool {
        cell.cmpge(self.first).all() && cell.cmple(self.last).all()
    }

    fn open(&self, cell: IVec2) -> bool {
        self.inside(cell)
            && self.map.is_buildable(cell)
            && !near_base(cell)
            && !near_field(self.fields, cell)
            && self.sites.iter().all(|site| {
                (cell - (*site + IVec2::ONE)).as_vec2().length() >= BASE_CLEARANCE / 2.0
            })
    }

    fn random_cell(&self, rng: &mut StdRng, margin: i32) -> IVec2 {
        let low = self.first + IVec2::splat(margin);
        let high = (self.last - IVec2::splat(margin)).max(low);
        IVec2::new(
            rng.random_range(low.x..=high.x),
            rng.random_range(low.y..=high.y),
        )
    }

    fn random_point(&self, rng: &mut StdRng) -> Vec3 {
        let low = self.first.as_vec2() * CELL_SIZE;
        let high = (self.last + IVec2::ONE).as_vec2() * CELL_SIZE;
        Vec3::new(
            rng.random_range(low.x..high.x),
            0.0,
            rng.random_range(low.y..high.y),
        )
    }

    /// Some chunks are wooded, others open country.
    fn plant_forests(&mut self, rng: &mut StdRng, chunk: IVec2) {
        let clumps = rng.random_range(FOREST_CLUMPS);
        let wooded_share = fixed_noise(chunk);
        for _ in 0..clumps {
            if !rng.random_bool(0.4 + 0.6 * wooded_share as f64) {
                continue;
            }
            let center = self.random_cell(rng, FOREST_MARGIN);
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
                if self.open(cell) {
                    block_cell_with_prop(
                        self.scenery,
                        Landscape::Wilds,
                        self.map,
                        rng,
                        cell,
                        model,
                        0.8..1.25,
                    );
                }
            }
        }
    }

    fn place_rocks(&mut self, rng: &mut StdRng) {
        for _ in 0..rng.random_range(ROCKS_PER_CHUNK) {
            let cell = self.random_cell(rng, 0);
            let model = pick(rng, &ROCKS);
            if self.open(cell) {
                block_cell_with_prop(
                    self.scenery,
                    Landscape::Wilds,
                    self.map,
                    rng,
                    cell,
                    model,
                    0.6..1.3,
                );
            }
        }
    }

    fn dress_meadows(&mut self, rng: &mut StdRng) {
        let ground = GroundLayers::new(Landscape::Wilds);
        for _ in 0..MEADOW_TRIES_PER_CHUNK {
            let position = self.random_point(rng);
            let weights = ground.weights(position.xz(), 0.0);
            let grassy = weights.x < MAX_MEADOW_DIRT
                && weights.y < 0.3
                && weights.z < 0.05
                && weights.w < 0.05;
            let cell = (position.xz() / CELL_SIZE).floor().as_ivec2();
            if !grassy || !self.open(cell) {
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
                let spot = position + offset;
                if !self.inside((spot.xz() / CELL_SIZE).floor().as_ivec2()) {
                    continue;
                }
                place_decor(
                    self.scenery,
                    Landscape::Wilds,
                    rng,
                    model,
                    spot,
                    scale.clone(),
                    model == "pebbles",
                );
            }
        }
    }

    fn line_shores(&mut self, rng: &mut StdRng) {
        let mut planted = 0;
        for _ in 0..SHORE_TRIES_PER_CHUNK {
            if planted == REEDS_PER_CHUNK {
                break;
            }
            let position = self.random_point(rng);
            let (column, row) = (position.x / CELL_SIZE, position.z / CELL_SIZE);
            let cell = (position.xz() / CELL_SIZE).floor().as_ivec2();
            if !Landscape::Wilds.is_shore(column, row)
                || !self.open(cell)
                || is_water(Landscape::Wilds, cell)
            {
                continue;
            }
            place_decor(
                self.scenery,
                Landscape::Wilds,
                rng,
                "reeds",
                position,
                0.8..1.3,
                false,
            );
            planted += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::levels::{ActiveLevel, LevelDefinition};
    use crate::world::map::{MapSize, cell_center, chunk_of, generate_map, set_map_bounds};

    fn wilds(map_seed: u64, map_size: MapSize) -> World {
        let level = LevelDefinition {
            landscape: Landscape::Wilds,
            map_seed,
            map_size,
            ..LevelDefinition::default()
        };
        level.shape_the_world();
        let mut world = World::new();
        world.insert_resource(ActiveLevel(level));
        world.run_system_once(generate_map).unwrap();
        world
    }

    fn crystal_and_obstacles(
        map: &GameMap,
        cells: impl Iterator<Item = IVec2>,
    ) -> Vec<(u32, bool)> {
        cells
            .map(|cell| (map.crystal_at(cell), map.is_passable(cell)))
            .collect()
    }

    #[test]
    fn a_chunk_generated_alone_matches_the_same_chunk_of_the_whole_map() {
        let seed = 41;
        let whole = wilds(seed, MapSize::Vast);
        let chunk = IVec2::new(3, 2);
        let (first, last) = map_bounds().cells_of_chunk(chunk);

        let mut alone_world = World::new();
        let mut alone_map = GameMap::open_field();
        crate::world::map::block_water(&mut alone_map, Landscape::Wilds);
        let mut alone_scenery = SceneryLayout::default();
        let mut commands = alone_world.commands();
        generate_chunk(
            &mut commands,
            &mut alone_map,
            &mut alone_scenery,
            seed,
            chunk,
        );

        let whole_map = whole.resource::<GameMap>();
        assert_eq!(
            crystal_and_obstacles(whole_map, cells_between(first, last)),
            crystal_and_obstacles(&alone_map, cells_between(first, last))
        );
        let props = |layout: &SceneryLayout| {
            layout
                .chunk(chunk)
                .map(|props| {
                    props
                        .props
                        .iter()
                        .map(|prop| (prop.model, prop.transform.translation))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let whole_props = props(whole.resource::<SceneryLayout>());
        assert!(!whole_props.is_empty());
        assert_eq!(whole_props, props(&alone_scenery));
    }

    #[test]
    fn every_base_reaches_the_others_and_has_crystal_at_hand() {
        for map_size in [MapSize::Standard, MapSize::Large, MapSize::Vast] {
            for seed in 0..30 {
                let world = wilds(seed, map_size);
                let map = world.resource::<GameMap>();
                let sites = base_sites();
                for site in sites {
                    assert!(
                        map.find_path(sites[0], site).is_some(),
                        "seed {seed}, {map_size:?}: {site} cut off"
                    );
                    let crystal_nearby = crate::world::map::cells_by_distance(site, 14)
                        .any(|cell| map.crystal_at(cell) > 0);
                    assert!(
                        crystal_nearby,
                        "seed {seed}, {map_size:?}: no crystal by {site}"
                    );
                }
            }
        }
    }

    #[test]
    fn widening_keeps_the_old_ground_and_makes_the_new_as_if_alone() {
        let seed = 23;
        let mut world = wilds(seed, MapSize::Large);
        world.insert_resource(Intel::new(true));
        let narrow = map_bounds();
        let base = base_sites()[0];
        world
            .resource_mut::<Intel>()
            .vision_mut(crate::sim::catalog::Team::Player)
            .reveal_around([(cell_center(base), 10.0)]);
        let before = crystal_and_obstacles(world.resource::<GameMap>(), narrow.cells());
        let wider = MapBounds {
            min: narrow.min - IVec2::splat(CHUNK_CELLS),
            size: narrow.size + IVec2::splat(2 * CHUNK_CELLS),
        };

        widen_world(&mut world, wider);

        let map = world.resource::<GameMap>();
        assert_eq!(before, crystal_and_obstacles(map, narrow.cells()));
        assert_eq!(
            base_sites()[0],
            base,
            "the bases stay where the level put them"
        );
        let explored = world
            .resource::<Intel>()
            .vision(crate::sim::catalog::Team::Player)
            .is_explored(cell_center(base));
        assert!(explored, "known ground stays known");

        let corner = chunk_of(wider.min);
        let (first, last) = wider.cells_of_chunk(corner);
        let mut alone_world = World::new();
        let mut alone_map = GameMap::open_field();
        crate::world::map::block_water(&mut alone_map, Landscape::Wilds);
        let mut alone_scenery = SceneryLayout::default();
        let mut commands = alone_world.commands();
        generate_chunk(
            &mut commands,
            &mut alone_map,
            &mut alone_scenery,
            seed,
            corner,
        );
        assert_eq!(
            crystal_and_obstacles(map, cells_between(first, last)),
            crystal_and_obstacles(&alone_map, cells_between(first, last))
        );
        let out_there = map
            .nearest_passable(first + IVec2::splat(CHUNK_CELLS / 2))
            .unwrap();
        assert!(
            map.find_path(base, out_there).is_some(),
            "the new ground is reachable"
        );
    }

    #[test]
    fn the_same_seed_makes_the_same_wilds_and_another_seed_other_ones() {
        let cells = || MapBounds::square(MapSize::Large.cells()).cells();
        let first = wilds(5, MapSize::Large);
        let first_cells = crystal_and_obstacles(first.resource::<GameMap>(), cells());
        let again = wilds(5, MapSize::Large);
        assert_eq!(
            first_cells,
            crystal_and_obstacles(again.resource::<GameMap>(), cells())
        );
        let other = wilds(6, MapSize::Large);
        assert_ne!(
            first_cells,
            crystal_and_obstacles(other.resource::<GameMap>(), cells())
        );
        let water = cells()
            .filter(|&cell| crate::world::map::is_water(Landscape::Wilds, cell))
            .count();
        assert!(water > 0, "the wilds have lakes");
        set_map_bounds(MapBounds::square(MapSize::Standard.cells()));
    }
}
