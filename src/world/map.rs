use std::cell::Cell;
use std::f32::consts::TAU;
use std::sync::{Mutex, PoisonError};

use bevy::prelude::*;
use pathfinding::prelude::astar;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::missions::levels::ActiveLevel;
use crate::sim::catalog::CRYSTAL_PER_CELL;
use crate::sim::sim_math;
use crate::view::crystal_models::{CRYSTAL_MODELS, CrystalModel};
use crate::view::fog_view::FogTexture;
use crate::view::terrain_chunks::plan_terrain;
use crate::view::terrain_material::{WaterMaterial, WaterSurface, water_material};
use crate::world::map_regions::Regions;
use crate::world::scenery::{self, SceneryLayout};
use crate::world::terrain::{
    FAR_RING_WIDTH, Landscape, OUTLANDS_WIDTH, WATER_LEVEL, approximate_ground,
};

pub const CELL_SIZE: f32 = 2.0;
/// The map every landscape, road and crystal field is authored for; bigger maps spread them out.
pub const STANDARD_MAP_CELLS: i32 = 64;
/// Base sites and crystal fields closer than this to a standard base site keep their offset from it.
const BASE_SURROUNDINGS_CELLS: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum MapSize {
    #[default]
    Standard,
    Large,
    Huge,
    Vast,
}

impl MapSize {
    pub const ALL: [MapSize; 4] = [
        MapSize::Standard,
        MapSize::Large,
        MapSize::Huge,
        MapSize::Vast,
    ];

    pub fn cells(self) -> i32 {
        match self {
            MapSize::Standard => STANDARD_MAP_CELLS,
            MapSize::Large => 96,
            MapSize::Huge => 128,
            MapSize::Vast => 256,
        }
    }
}

/// The cells of the map being played: `size` cells along each axis, starting at `min`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapBounds {
    pub min: IVec2,
    pub size: IVec2,
}

impl MapBounds {
    pub const fn square(cells: i32) -> Self {
        Self {
            min: IVec2::ZERO,
            size: IVec2::splat(cells),
        }
    }

    /// The last cell, inclusive.
    pub fn max(self) -> IVec2 {
        self.min + self.size - IVec2::ONE
    }

    pub fn contains(self, cell: IVec2) -> bool {
        cell.cmpge(self.min).all() && cell.cmple(self.max()).all()
    }

    /// Where a cell's entry is in a grid of the whole map stored row by row.
    pub fn index(self, cell: IVec2) -> Option<usize> {
        self.contains(cell).then(|| {
            let local = cell - self.min;
            (local.y * self.size.x + local.x) as usize
        })
    }

    pub fn cell_at(self, index: usize) -> IVec2 {
        let index = index as i32;
        self.min + IVec2::new(index % self.size.x, index / self.size.x)
    }

    pub fn area(self) -> usize {
        (self.size.x * self.size.y) as usize
    }

    /// Every cell, row by row, in the order of `index`.
    pub fn cells(self) -> impl Iterator<Item = IVec2> {
        (self.min.y..=self.max().y)
            .flat_map(move |y| (self.min.x..=self.max().x).map(move |x| IVec2::new(x, y)))
    }

    pub fn clamp(self, cell: IVec2) -> IVec2 {
        cell.clamp(self.min, self.max())
    }

    pub fn longest_side(self) -> i32 {
        self.size.max_element()
    }

    /// The middle in cell units, where a cell's centre is half a cell past its number.
    pub fn center_cells(self) -> Vec2 {
        self.min.as_vec2() + (self.size - IVec2::ONE).as_vec2() / 2.0
    }

    pub fn world_min(self) -> Vec2 {
        self.min.as_vec2() * CELL_SIZE
    }

    pub fn world_max(self) -> Vec2 {
        (self.min + self.size).as_vec2() * CELL_SIZE
    }

    pub fn world_size(self) -> Vec2 {
        self.size.as_vec2() * CELL_SIZE
    }

    pub fn world_center(self) -> Vec2 {
        self.world_min() + self.world_size() / 2.0
    }

    /// Whether a point on the ground, in metres, lies on the map.
    pub fn contains_point(self, point: Vec2) -> bool {
        point.cmpge(self.world_min()).all() && point.cmplt(self.world_max()).all()
    }

    /// How much farther apart things are than on the standard map, along each axis.
    pub fn spread(self) -> Vec2 {
        (self.size - IVec2::ONE).as_vec2() / (STANDARD_MAP_CELLS - 1) as f32
    }

    fn first_chunk(self) -> IVec2 {
        chunk_of(self.min)
    }

    /// How many chunks the map spans along each axis; the last ones may be cut short by the edge.
    pub fn chunk_span(self) -> IVec2 {
        chunk_of(self.max()) - self.first_chunk() + IVec2::ONE
    }

    pub fn chunk_count(self) -> usize {
        let span = self.chunk_span();
        (span.x * span.y) as usize
    }

    pub fn chunk_index(self, chunk: IVec2) -> Option<usize> {
        let local = chunk - self.first_chunk();
        let span = self.chunk_span();
        let inside = local.cmpge(IVec2::ZERO).all() && local.cmplt(span).all();
        inside.then(|| (local.y * span.x + local.x) as usize)
    }

    pub fn chunk_at(self, index: usize) -> IVec2 {
        let index = index as i32;
        let span = self.chunk_span();
        self.first_chunk() + IVec2::new(index % span.x, index / span.x)
    }

    /// The first and last cell of a chunk that lie on the map.
    pub fn cells_of_chunk(self, chunk: IVec2) -> (IVec2, IVec2) {
        let first = (chunk * CHUNK_CELLS).max(self.min);
        let last = (chunk * CHUNK_CELLS + IVec2::splat(CHUNK_CELLS - 1)).min(self.max());
        (first, last)
    }
}

/// Cells along each side of a chunk, the square of map that changes are tracked by.
pub const CHUNK_CELLS: i32 = 32;

pub fn chunk_of(cell: IVec2) -> IVec2 {
    cell.div_euclid(IVec2::splat(CHUNK_CELLS))
}

// The bounds of the map being played. Cell maths runs everywhere, so they are ambient rather than passed
// around; they are kept per thread because every schedule runs on the thread that updates the app, and tests
// that play several battles at once each run on their own thread.
thread_local! {
    static ACTIVE_MAP_BOUNDS: Cell<MapBounds> = const { Cell::new(MapBounds::square(STANDARD_MAP_CELLS)) };
}

pub fn set_map_size(size: MapSize) {
    set_map_bounds(MapBounds::square(size.cells()));
}

pub fn set_map_bounds(bounds: MapBounds) {
    ACTIVE_MAP_BOUNDS.set(bounds);
}

pub fn map_bounds() -> MapBounds {
    ACTIVE_MAP_BOUNDS.get()
}

/// How much farther apart things are than on the standard map; for shapes that do not follow one axis.
pub fn map_spread() -> f32 {
    map_bounds().spread().min_element()
}

/// Command Center origins of the four corner base sites; levels decide who starts where.
pub fn base_sites() -> [IVec2; 4] {
    let bounds = map_bounds();
    let (near, far) = (bounds.min, bounds.min + bounds.size);
    [
        IVec2::new(near.x + 6, far.y - 12),
        IVec2::new(far.x - 10, near.y + 8),
        IVec2::new(near.x + 6, near.y + 8),
        IVec2::new(far.x - 10, far.y - 12),
    ]
}

pub fn player_base_cell() -> IVec2 {
    base_sites()[0]
}

pub fn enemy_base_cell() -> IVec2 {
    base_sites()[1]
}

fn standard_base_sites() -> [IVec2; 4] {
    let far = STANDARD_MAP_CELLS;
    [
        IVec2::new(6, far - 12),
        IVec2::new(far - 10, 8),
        IVec2::new(6, 8),
        IVec2::new(far - 10, far - 12),
    ]
}

/// Moves a point in cell units, authored for the standard map, onto the map being played: near a base it
/// keeps its offset from that base, elsewhere it spreads out from the centre. Mirror twins stay twins.
pub fn relocate(standard: Vec2) -> Vec2 {
    let base_middle = |site: IVec2| site.as_vec2() + Vec2::splat(1.5);
    let nearest_base = standard_base_sites()
        .into_iter()
        .zip(base_sites())
        .map(|(old, new)| (base_middle(old), base_middle(new)))
        .min_by(|a, b| a.0.distance(standard).total_cmp(&b.0.distance(standard)));
    if let Some((old, new)) = nearest_base
        && old.distance(standard) < BASE_SURROUNDINGS_CELLS
    {
        return new + (standard - old);
    }
    let old_middle = MapBounds::square(STANDARD_MAP_CELLS).center_cells();
    let bounds = map_bounds();
    bounds.center_cells() + (standard - old_middle) * bounds.spread()
}

/// A cell and its mirror image always add up to this, so the map plays the same from opposite corners.
fn mirror_sum() -> IVec2 {
    let bounds = map_bounds();
    bounds.min * 2 + bounds.size - IVec2::splat(2)
}

pub fn opposite_cell(cell: IVec2) -> IVec2 {
    mirror_sum() - cell
}

/// Whether a cell lies past the diagonal the map is mirrored across, nearer the far corner.
fn beyond_mirror_diagonal(cell: IVec2) -> bool {
    2 * (cell.x + cell.y) > mirror_sum().element_sum()
}

/// The point the map is mirrored around, half a cell off the middle of the world.
pub fn mirror_centre() -> Vec3 {
    let first = map_bounds().min;
    (cell_center(first) + cell_center(opposite_cell(first))) / 2.0
}

/// The origin of a structure of `footprint` that is its own mirror image, in the middle of the map.
pub fn self_mirrored_origin(footprint: IVec2) -> IVec2 {
    (mirror_sum() - (footprint - IVec2::ONE)) / 2
}

/// Where a structure of `footprint` starting at `origin` lands when the map is mirrored.
pub fn opposite_origin(origin: IVec2, footprint: IVec2) -> IVec2 {
    opposite_cell(origin) - (footprint - IVec2::ONE)
}

/// Structures here are laid out and turned as the mirror image of their twin in the near half.
pub fn in_far_half(origin: IVec2, footprint: IVec2) -> bool {
    let opposite = opposite_origin(origin, footprint);
    origin.x + origin.y > opposite.x + opposite.y
}

const RIVER_BLOCKING_DEPTH: f32 = 0.25;

const CARDINAL_COST: u32 = 10;
const DIAGONAL_COST: u32 = 14;

#[derive(Resource)]
pub struct GameMap {
    pub(crate) terrain_blocked: Vec<bool>,
    building: Vec<Option<Entity>>,
    crystal: Vec<u32>,
    /// How many cells still hold crystal, so a search of the whole map can be skipped once it is mined out.
    crystal_cells: usize,
    regions: Mutex<Regions>,
}

#[derive(Component)]
pub struct CrystalVisual {
    pub cell: IVec2,
    /// This crystal's own size against others holding as much, so a field is not a row of equals.
    pub size: f32,
}

impl GameMap {
    fn empty() -> Self {
        let cells = map_bounds().area();
        Self {
            terrain_blocked: vec![false; cells],
            building: vec![None; cells],
            crystal: vec![0; cells],
            crystal_cells: 0,
            regions: Mutex::new(Regions::new()),
        }
    }

    /// A map with no river, crystals or obstacles, for tests.
    #[cfg(test)]
    pub(crate) fn open_field() -> Self {
        Self::empty()
    }

    pub(crate) fn index(cell: IVec2) -> Option<usize> {
        map_bounds().index(cell)
    }

    pub fn in_bounds(cell: IVec2) -> bool {
        Self::index(cell).is_some()
    }

    pub fn is_passable(&self, cell: IVec2) -> bool {
        Self::index(cell).is_some_and(|i| !self.terrain_blocked[i] && self.building[i].is_none())
    }

    pub fn building_at(&self, cell: IVec2) -> Option<Entity> {
        Self::index(cell).and_then(|i| self.building[i])
    }

    pub fn is_buildable(&self, cell: IVec2) -> bool {
        self.is_passable(cell) && self.crystal_at(cell) == 0
    }

    pub fn set_building(&mut self, origin: IVec2, footprint: IVec2, occupant: Option<Entity>) {
        for cell in footprint_cells(origin, footprint) {
            if let Some(i) = Self::index(cell) {
                self.building[i] = occupant;
            }
            self.regions_mut().forget_around(cell);
        }
    }

    pub fn crystal_amounts(&self) -> Vec<u32> {
        self.crystal.clone()
    }

    /// Restores a saved crystal grid and forgets every building footprint, ready for buildings to be respawned.
    pub fn reset_for_load(&mut self, crystal: Vec<u32>) {
        if crystal.len() == self.crystal.len() {
            self.crystal_cells = crystal.iter().filter(|&&amount| amount > 0).count();
            self.crystal = crystal;
        }
        self.building.fill(None);
        self.regions_mut().forget_all();
    }

    pub fn set_crystal(&mut self, cell: IVec2, amount: u32) {
        if let Some(i) = Self::index(cell) {
            let had = self.crystal[i] > 0;
            self.crystal[i] = amount;
            match (had, amount > 0) {
                (false, true) => self.crystal_cells += 1,
                (true, false) => self.crystal_cells -= 1,
                _ => {}
            }
        }
    }

    pub fn has_crystal_left(&self) -> bool {
        self.crystal_cells > 0
    }

    pub fn set_obstacle(&mut self, cell: IVec2, blocked: bool) {
        if let Some(i) = Self::index(cell) {
            self.terrain_blocked[i] = blocked;
        }
        self.regions_mut().forget_around(cell);
    }

    fn regions_mut(&mut self) -> &mut Regions {
        self.regions
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a unit at `from` could drive to the passable cell `to`. A unit stuck on a blocked cell, inside
    /// a new building for one, may be able to step out to several areas, so the route search decides for it.
    fn may_reach(&self, from: IVec2, to: IVec2) -> bool {
        if !self.is_passable(from) {
            return true;
        }
        let mut regions = self.regions.lock().unwrap_or_else(PoisonError::into_inner);
        regions.of(self, from) == regions.of(self, to)
    }

    pub fn crystal_at(&self, cell: IVec2) -> u32 {
        Self::index(cell).map_or(0, |i| self.crystal[i])
    }

    /// Returns the amount actually mined and whether the cell is now depleted.
    pub fn mine_crystal(&mut self, cell: IVec2, wanted: u32) -> (u32, bool) {
        let Some(i) = Self::index(cell) else {
            return (0, true);
        };
        let mined = wanted.min(self.crystal[i]);
        let depleted = mined > 0 && mined == self.crystal[i];
        self.crystal[i] -= mined;
        if depleted {
            self.crystal_cells -= 1;
        }
        (mined, self.crystal[i] == 0)
    }

    pub fn nearest_crystal(&self, from: IVec2, max_radius: i32) -> Option<IVec2> {
        cells_by_distance(from, max_radius).find(|&cell| self.crystal_at(cell) > 0)
    }

    pub fn nearest_passable(&self, from: IVec2) -> Option<IVec2> {
        cells_by_distance(from, map_bounds().longest_side()).find(|&cell| self.is_passable(cell))
    }

    /// Routes starting in the far half are planned in mirror image, so tie-breaks between equally short
    /// routes favour neither corner.
    pub fn find_path(&self, start: IVec2, goal: IVec2) -> Option<Vec<IVec2>> {
        let starts_in_far_half = beyond_mirror_diagonal(start);
        if !starts_in_far_half {
            return self.find_path_in_frame(start, goal, |cell| cell);
        }
        let path =
            self.find_path_in_frame(opposite_cell(start), opposite_cell(goal), opposite_cell)?;
        Some(path.into_iter().map(opposite_cell).collect())
    }

    /// A* over cells of a frame whose cell `c` is the real map cell `to_map(c)`.
    fn find_path_in_frame(
        &self,
        start: IVec2,
        goal: IVec2,
        to_map: impl Fn(IVec2) -> IVec2,
    ) -> Option<Vec<IVec2>> {
        let passable = |cell: IVec2| self.is_passable(to_map(cell));
        let goal = if passable(goal) {
            goal
        } else {
            cells_by_distance(goal, map_bounds().longest_side()).find(|&cell| passable(cell))?
        };
        if !self.may_reach(to_map(start), to_map(goal)) {
            return None;
        }
        let (path, _cost) = astar(
            &start,
            |&cell| walkable_neighbours(cell, passable),
            |&cell| octile_distance(cell, goal),
            |&cell| cell == goal,
        )?;
        Some(path)
    }
}

fn octile_distance(a: IVec2, b: IVec2) -> u32 {
    let d = (a - b).abs();
    let (short, long) = (d.x.min(d.y) as u32, d.x.max(d.y) as u32);
    DIAGONAL_COST * short + CARDINAL_COST * (long - short)
}

fn walkable_neighbours(cell: IVec2, passable: impl Fn(IVec2) -> bool) -> Vec<(IVec2, u32)> {
    let mut neighbours = Vec::with_capacity(8);
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let next = cell + IVec2::new(dx, dy);
            if !passable(next) {
                continue;
            }
            let diagonal = dx != 0 && dy != 0;
            let cuts_corner = diagonal
                && (!passable(cell + IVec2::new(dx, 0)) || !passable(cell + IVec2::new(0, dy)));
            if cuts_corner {
                continue;
            }
            neighbours.push((
                next,
                if diagonal {
                    DIAGONAL_COST
                } else {
                    CARDINAL_COST
                },
            ));
        }
    }
    neighbours
}

/// Ring by ring outward from `center`; around a centre in the far half each ring is walked in mirror
/// image, so the first fitting cell (a crystal, a free site, an exit) is the twin of the near half's choice.
pub fn cells_by_distance(center: IVec2, max_radius: i32) -> impl Iterator<Item = IVec2> {
    let mirrored = beyond_mirror_diagonal(center);
    let direction = if mirrored { -1 } else { 1 };
    (0..=max_radius).flat_map(move |radius| {
        (-radius..=radius).flat_map(move |dy| {
            let whole_row = dy.abs() == radius;
            let between_sides = if whole_row { 1 } else { 2 * radius };
            (-radius..=radius)
                .step_by(between_sides.max(1) as usize)
                .filter_map(move |dx| {
                    let cell = center + IVec2::new(dx, dy) * direction;
                    GameMap::in_bounds(cell).then_some(cell)
                })
        })
    })
}

pub fn footprint_cells(origin: IVec2, footprint: IVec2) -> impl Iterator<Item = IVec2> {
    (0..footprint.y).flat_map(move |dy| (0..footprint.x).map(move |dx| origin + IVec2::new(dx, dy)))
}

pub fn cell_center(cell: IVec2) -> Vec3 {
    approximate_ground(Vec3::new(
        (cell.x as f32 + 0.5) * CELL_SIZE,
        0.0,
        (cell.y as f32 + 0.5) * CELL_SIZE,
    ))
}

pub fn footprint_center(origin: IVec2, footprint: IVec2) -> Vec3 {
    approximate_ground(Vec3::new(
        (origin.x as f32 + footprint.x as f32 * 0.5) * CELL_SIZE,
        0.0,
        (origin.y as f32 + footprint.y as f32 * 0.5) * CELL_SIZE,
    ))
}

pub fn world_to_cell(position: Vec3) -> IVec2 {
    IVec2::new(
        (position.x / CELL_SIZE).floor() as i32,
        (position.z / CELL_SIZE).floor() as i32,
    )
}

pub struct CrystalField {
    pub center: IVec2,
    pub radius: f32,
}

/// On the standard map, in mirrored pairs: every even entry is followed by its opposite.
const STANDARD_CRYSTAL_FIELDS: [CrystalField; 6] = [
    CrystalField {
        center: IVec2::new(15, 46),
        radius: 3.6,
    },
    CrystalField {
        center: IVec2::new(47, 16),
        radius: 3.6,
    },
    CrystalField {
        center: IVec2::new(24, 38),
        radius: 2.6,
    },
    CrystalField {
        center: IVec2::new(38, 24),
        radius: 2.6,
    },
    CrystalField {
        center: IVec2::new(6, 22),
        radius: 3.2,
    },
    CrystalField {
        center: IVec2::new(56, 40),
        radius: 3.2,
    },
];

/// The crystal fields placed on the map being played, still in mirrored pairs.
pub fn crystal_fields() -> Vec<CrystalField> {
    STANDARD_CRYSTAL_FIELDS
        .iter()
        .map(|field| CrystalField {
            center: relocate(field.center.as_vec2() + Vec2::splat(0.5))
                .floor()
                .as_ivec2(),
            radius: field.radius,
        })
        .collect()
}

pub fn near_base(cell: IVec2) -> bool {
    let base_clearance = 9.0;
    base_sites()
        .iter()
        .any(|base| (cell - (*base + IVec2::ONE)).as_vec2().length() < base_clearance)
}

pub fn near_crystal_field(cell: IVec2) -> bool {
    crystal_fields().iter().any(|field| {
        let outer_edge = field.radius * OUTLINE_MAX_REACH + STRAY_REACH;
        (cell - field.center).as_vec2().length() < outer_edge + 1.0
    })
}

pub fn generate_map(mut commands: Commands, level: Res<ActiveLevel>) {
    let mut rng = StdRng::seed_from_u64(level.0.map_seed);
    let mut map = GameMap::empty();
    let landscape = level.0.landscape;
    block_water(&mut map, landscape);
    block_unmirrored_edge(&mut map);
    seed_crystal_fields(&mut commands, &mut map, &mut rng);
    let mut scenery = SceneryLayout::default();
    scenery::scatter(&mut scenery, landscape, &mut map, &mut rng);
    commands.insert_resource(map);
    commands.insert_resource(scenery);
}

pub fn is_water(landscape: Landscape, cell: IVec2) -> bool {
    landscape.water_depth(cell.x as f32 + 0.5, cell.y as f32 + 0.5) > RIVER_BLOCKING_DEPTH
}

fn block_water(map: &mut GameMap, landscape: Landscape) {
    for cell in map_bounds().cells() {
        if is_water(landscape, cell) {
            map.set_obstacle(cell, true);
        }
    }
}

/// The last row and column would mirror to cells beyond the first ones, so leaving them open gives the
/// north-east corner an extra lane behind its base.
fn block_unmirrored_edge(map: &mut GameMap) {
    let bounds = map_bounds();
    let last = bounds.max();
    for row in bounds.min.y..=last.y {
        map.set_obstacle(IVec2::new(last.x, row), true);
    }
    for column in bounds.min.x..=last.x {
        map.set_obstacle(IVec2::new(column, last.y), true);
    }
}

/// The rendered ground and water; the simulation itself only needs the grid from `generate_map`.
pub fn spawn_terrain_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    fog: Res<FogTexture>,
    level: Res<ActiveLevel>,
) {
    let placeholder = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.36, 0.20),
        perceptual_roughness: 0.95,
        ..default()
    });
    plan_terrain(&mut commands, &level, placeholder);

    let bounds = map_bounds();
    let water_span = bounds.world_size() + Vec2::splat(2.0 * (OUTLANDS_WIDTH + FAR_RING_WIDTH));
    let water_middle = bounds.world_center();
    let water_mesh = Plane3d::default()
        .mesh()
        .size(water_span.x, water_span.y)
        .build()
        .with_generated_tangents()
        .expect("plane mesh has normals and UVs");
    commands.spawn((
        WaterSurface,
        Mesh3d(meshes.add(water_mesh)),
        MeshMaterial3d(water_materials.add(water_material(&mut images, fog.0.clone()))),
        Transform::from_xyz(water_middle.x, WATER_LEVEL, water_middle.y),
    ));
}

/// A field's edge: a blob rather than a disc, with a few crystals scattered past it.
struct FieldOutline {
    /// (amplitude, phase) of the bulges going round the field twice and three times.
    lobes: [(f32, f32); 2],
}

/// The largest reach of an outline, as a share of the field's radius.
const OUTLINE_MAX_REACH: f32 = 1.0 + 0.3 + 0.15;
/// How far past the edge stray crystals lie, in cells, and how often a cell there has one.
const STRAY_REACH: f32 = 1.6;
const STRAY_CHANCE: f64 = 0.18;
const STRAY_RICHNESS: f32 = 0.3;
const EDGE_JITTER: f32 = 0.6;

impl FieldOutline {
    fn random(rng: &mut StdRng) -> Self {
        Self {
            lobes: [
                (rng.random_range(0.12..0.3), rng.random_range(0.0..TAU)),
                (rng.random_range(0.05..0.15), rng.random_range(0.0..TAU)),
            ],
        }
    }

    fn reach(&self, angle: f32) -> f32 {
        let [(twice, twice_phase), (thrice, thrice_phase)] = self.lobes;
        1.0 + twice * sim_math::sin(2.0 * angle + twice_phase)
            + thrice * sim_math::sin(3.0 * angle + thrice_phase)
    }
}

fn seed_crystal_fields(commands: &mut Commands, map: &mut GameMap, rng: &mut StdRng) {
    for field in crystal_fields().iter().step_by(2) {
        let outline = FieldOutline::random(rng);
        let reach = (field.radius * OUTLINE_MAX_REACH + STRAY_REACH).ceil() as i32;
        for cell in cells_by_distance(field.center, reach) {
            let offset = (cell - field.center).as_vec2();
            let distance = offset.length();
            let edge = field.radius * outline.reach(sim_math::atan2(offset.y, offset.x));
            let inside = distance + rng.random_range(-EDGE_JITTER..EDGE_JITTER) <= edge;
            let stray = !inside && distance <= edge + STRAY_REACH && rng.random_bool(STRAY_CHANCE);
            let Some(i) = GameMap::index(cell) else {
                continue;
            };
            if !(inside || stray) || map.terrain_blocked[i] {
                continue;
            }
            if GameMap::index(opposite_cell(cell)).is_none() {
                continue;
            }
            let richness = if inside {
                1.0 - 0.5 * (distance / edge).min(1.0)
            } else {
                STRAY_RICHNESS
            };
            let amount = (CRYSTAL_PER_CELL as f32 * richness) as u32;
            for at in [cell, opposite_cell(cell)] {
                map.set_crystal(at, amount);
                spawn_crystal_visual(commands, rng, at, amount);
            }
        }
    }
}

/// How far a crystal may sit from its cell's centre, in metres, and how far it may lean.
const CRYSTAL_SPREAD: f32 = 0.45;
const CRYSTAL_LEAN: f32 = 0.14;

pub fn spawn_crystal_visual(
    commands: &mut Commands,
    rng: &mut impl RngExt,
    cell: IVec2,
    amount: u32,
) {
    let model = CRYSTAL_MODELS[rng.random_range(0..CRYSTAL_MODELS.len())];
    let spread = Vec3::new(
        rng.random_range(-CRYSTAL_SPREAD..CRYSTAL_SPREAD),
        0.0,
        rng.random_range(-CRYSTAL_SPREAD..CRYSTAL_SPREAD),
    );
    let lean = Quat::from_rotation_x(rng.random_range(-CRYSTAL_LEAN..CRYSTAL_LEAN))
        * Quat::from_rotation_z(rng.random_range(-CRYSTAL_LEAN..CRYSTAL_LEAN));
    let heading = Quat::from_rotation_y(rng.random_range(0.0..TAU));
    let size = rng.random_range(0.8..1.2);
    commands.spawn((
        CrystalVisual { cell, size },
        CrystalModel(model),
        Transform::from_translation(approximate_ground(cell_center(cell) + spread))
            .with_rotation(lean * heading)
            .with_scale(Vec3::splat(size * crystal_scale(amount))),
        Visibility::default(),
    ));
}

pub fn cell_of_index(index: usize) -> IVec2 {
    map_bounds().cell_at(index)
}

pub fn crystal_scale(amount: u32) -> f32 {
    0.45 + 0.6 * (amount as f32 / CRYSTAL_PER_CELL as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::river_centerline;

    /// Off the origin and wider than tall, so nothing can lean on a square map starting at cell zero.
    const SHIFTED_BOUNDS: MapBounds = MapBounds {
        min: IVec2::new(-40, 12),
        size: IVec2::new(70, 30),
    };

    #[test]
    fn bounds_number_every_cell_once_row_by_row() {
        let cells: Vec<IVec2> = SHIFTED_BOUNDS.cells().collect();
        assert_eq!(cells.len(), SHIFTED_BOUNDS.area());
        assert_eq!(cells[0], SHIFTED_BOUNDS.min);
        assert_eq!(cells[cells.len() - 1], SHIFTED_BOUNDS.max());
        for (index, &cell) in cells.iter().enumerate() {
            assert_eq!(SHIFTED_BOUNDS.index(cell), Some(index));
            assert_eq!(SHIFTED_BOUNDS.cell_at(index), cell);
        }
        for outside in [IVec2::new(-41, 12), IVec2::new(30, 20), IVec2::new(0, 42)] {
            assert_eq!(SHIFTED_BOUNDS.index(outside), None);
        }
    }

    #[test]
    fn bounds_cover_the_same_ground_in_metres_as_in_cells() {
        let bounds = SHIFTED_BOUNDS;
        assert_eq!(bounds.world_min(), Vec2::new(-80.0, 24.0));
        assert_eq!(bounds.world_max(), Vec2::new(60.0, 84.0));
        assert!(bounds.contains_point(cell_center(bounds.min).xz()));
        assert!(bounds.contains_point(cell_center(bounds.max()).xz()));
        assert!(!bounds.contains_point(bounds.world_max()));
    }

    #[test]
    fn chunks_tile_a_shifted_map_without_gaps_or_overlaps() {
        let bounds = SHIFTED_BOUNDS;
        let mut covered = vec![0; bounds.area()];
        for index in 0..bounds.chunk_count() {
            let chunk = bounds.chunk_at(index);
            assert_eq!(bounds.chunk_index(chunk), Some(index));
            let (first, last) = bounds.cells_of_chunk(chunk);
            for cell in (MapBounds {
                min: first,
                size: last - first + IVec2::ONE,
            })
            .cells()
            {
                assert_eq!(chunk_of(cell), chunk);
                covered[bounds.index(cell).unwrap()] += 1;
            }
        }
        assert!(covered.iter().all(|&times| times == 1));
    }

    #[test]
    fn a_shifted_map_mirrors_onto_itself_and_routes_across_it() {
        set_map_bounds(SHIFTED_BOUNDS);
        let map = GameMap::open_field();
        for cell in [SHIFTED_BOUNDS.min, IVec2::new(-3, 20), IVec2::new(10, 30)] {
            let twin = opposite_cell(cell);
            assert!(SHIFTED_BOUNDS.contains(twin), "{cell} mirrors off the map");
            assert_eq!(opposite_cell(twin), cell);
        }
        let route = map.find_path(SHIFTED_BOUNDS.min, SHIFTED_BOUNDS.max() - IVec2::ONE);
        assert!(route.is_some_and(|route| route.iter().all(|&cell| map.is_passable(cell))));
        assert!(cells_by_distance(SHIFTED_BOUNDS.min, 3).all(|cell| SHIFTED_BOUNDS.contains(cell)));
    }

    fn map_with_river() -> GameMap {
        let mut map = GameMap::empty();
        block_water(&mut map, Landscape::River);
        map
    }

    fn is_ford_column(column: i32) -> bool {
        [(9, 13), (29, 33), (49, 53)]
            .iter()
            .any(|&(start, end)| column >= start && column <= end)
    }

    fn wall_in(map: &mut GameMap, center: IVec2) -> Vec<IVec2> {
        let ring: Vec<IVec2> = cells_by_distance(center, 1).skip(1).collect();
        for &cell in &ring {
            map.set_obstacle(cell, true);
        }
        ring
    }

    #[test]
    fn a_route_into_a_walled_in_pocket_fails_until_the_wall_opens() {
        let mut map = GameMap::empty();
        let pocket = IVec2::new(20, 20);
        let ring = wall_in(&mut map, pocket);
        assert_eq!(map.find_path(IVec2::new(2, 2), pocket), None);
        assert_eq!(map.find_path(pocket, IVec2::new(2, 2)), None);
        map.set_obstacle(ring[1], false);
        let route = map.find_path(IVec2::new(2, 2), pocket);
        assert_eq!(route.and_then(|route| route.last().copied()), Some(pocket));
    }

    #[test]
    fn a_unit_standing_on_a_blocked_cell_still_finds_its_way_out() {
        let mut map = GameMap::empty();
        let blocked = IVec2::new(20, 20);
        map.set_obstacle(blocked, true);
        assert!(map.find_path(blocked, IVec2::new(2, 2)).is_some());
    }

    #[test]
    fn bases_are_connected_and_the_river_is_crossed_only_at_fords() {
        let map = map_with_river();
        let path = map
            .find_path(player_base_cell(), enemy_base_cell())
            .expect("bases must be connected");
        assert!(path.iter().all(|&cell| map.is_passable(cell)));
        for cell in &path {
            let in_river_band = (cell.y as f32 - river_centerline(cell.x as f32 + 0.5)).abs() < 1.0;
            assert!(
                !in_river_band || is_ford_column(cell.x),
                "crossed the river outside a ford at {cell}"
            );
        }
    }

    #[test]
    fn opposite_base_sites_mirror_each_other() {
        let command_center = IVec2::splat(3);
        assert_eq!(
            opposite_origin(base_sites()[0], command_center),
            base_sites()[1]
        );
        assert_eq!(
            opposite_origin(base_sites()[2], command_center),
            base_sites()[3]
        );
    }

    #[test]
    fn the_river_and_crystal_fields_are_point_symmetric() {
        let map = map_with_river();
        for cell in map_bounds().cells() {
            let mirror = opposite_cell(cell);
            if GameMap::in_bounds(mirror) {
                assert_eq!(
                    map.is_passable(cell),
                    map.is_passable(mirror),
                    "{cell} vs {mirror}"
                );
            }
        }
        for pair in crystal_fields().chunks(2) {
            assert_eq!(opposite_cell(pair[0].center), pair[1].center);
            assert_eq!(pair[0].radius, pair[1].radius);
        }
    }

    #[test]
    fn every_landscape_connects_the_bases_and_mirrors_its_water() {
        for landscape in Landscape::ALL {
            let mut map = GameMap::empty();
            block_water(&mut map, landscape);
            for (from, to) in [
                (base_sites()[0], base_sites()[1]),
                (base_sites()[2], base_sites()[3]),
            ] {
                assert!(
                    map.find_path(from, to).is_some(),
                    "{landscape:?}: {from} cut off from {to}"
                );
            }
            for cell in map_bounds().cells() {
                let mirror = opposite_cell(cell);
                if GameMap::in_bounds(mirror) {
                    assert_eq!(
                        map.is_passable(cell),
                        map.is_passable(mirror),
                        "{landscape:?} at {cell}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_lake_blocks_the_middle_except_for_its_causeway() {
        let mut map = GameMap::empty();
        block_water(&mut map, Landscape::Lake);
        let middle = IVec2::splat(31);
        assert!(map.is_passable(middle), "the causeway is flooded");
        assert!(
            !map.is_passable(middle + IVec2::new(4, 4)),
            "no water beside the causeway"
        );
        for field in &crystal_fields() {
            assert!(
                map.is_passable(field.center),
                "crystals under water at {}",
                field.center
            );
        }
    }

    #[test]
    fn mirrored_trips_take_mirrored_routes() {
        let map = map_with_river();
        let start = IVec2::new(11, 53);
        for goal in [IVec2::new(55, 9), IVec2::new(7, 9), IVec2::new(40, 20)] {
            let route = map.find_path(start, goal).unwrap();
            let mirrored = map
                .find_path(opposite_cell(start), opposite_cell(goal))
                .unwrap();
            assert_eq!(
                route.into_iter().map(opposite_cell).collect::<Vec<_>>(),
                mirrored
            );
        }
    }

    #[test]
    fn path_never_cuts_diagonally_past_a_blocked_corner() {
        let mut map = GameMap::empty();
        map.set_building(IVec2::new(10, 10), IVec2::ONE, Some(Entity::PLACEHOLDER));
        let path = map
            .find_path(IVec2::new(9, 11), IVec2::new(11, 9))
            .expect("route around the obstacle");
        for step in path.windows(2) {
            let delta = step[1] - step[0];
            if delta.x != 0 && delta.y != 0 {
                assert!(map.is_passable(step[0] + IVec2::new(delta.x, 0)));
                assert!(map.is_passable(step[0] + IVec2::new(0, delta.y)));
            }
        }
    }

    #[test]
    fn path_to_a_blocked_goal_ends_next_to_it() {
        let mut map = GameMap::empty();
        let goal = IVec2::new(20, 20);
        map.set_building(goal, IVec2::splat(3), Some(Entity::PLACEHOLDER));
        let path = map
            .find_path(IVec2::new(5, 5), goal + IVec2::ONE)
            .expect("a reachable substitute goal");
        let end = *path.last().unwrap();
        assert!(map.is_passable(end));
        assert!((end - (goal + IVec2::ONE)).abs().max_element() <= 2);
    }

    #[test]
    fn buildings_block_cells_until_removed() {
        let mut map = GameMap::empty();
        let origin = IVec2::new(4, 4);
        map.set_building(origin, IVec2::splat(2), Some(Entity::PLACEHOLDER));
        assert!(footprint_cells(origin, IVec2::splat(2)).all(|cell| !map.is_passable(cell)));
        map.set_building(origin, IVec2::splat(2), None);
        assert!(footprint_cells(origin, IVec2::splat(2)).all(|cell| map.is_passable(cell)));
    }

    #[test]
    fn mining_takes_at_most_what_is_left_and_reports_depletion() {
        let mut map = GameMap::empty();
        let cell = IVec2::new(3, 3);
        map.set_crystal(cell, 30);
        assert_eq!(map.mine_crystal(cell, 25), (25, false));
        assert_eq!(map.mine_crystal(cell, 25), (5, true));
        assert_eq!(map.crystal_at(cell), 0);
        assert!(map.is_buildable(cell));
    }

    fn every_ring_cell_scanned_row_by_row(center: IVec2, max_radius: i32) -> Vec<IVec2> {
        let direction = if beyond_mirror_diagonal(center) {
            -1
        } else {
            1
        };
        (0..=max_radius)
            .flat_map(|radius| {
                (-radius..=radius).flat_map(move |dy| {
                    (-radius..=radius).filter_map(move |dx| {
                        let on_ring = dx.abs() == radius || dy.abs() == radius;
                        let cell = center + IVec2::new(dx, dy) * direction;
                        (on_ring && GameMap::in_bounds(cell)).then_some(cell)
                    })
                })
            })
            .collect()
    }

    #[test]
    fn walking_only_the_ring_visits_the_same_cells_in_the_same_order() {
        let far = map_bounds().max().x - 2;
        for center in [
            IVec2::new(0, 0),
            IVec2::new(10, 30),
            IVec2::new(far, far - 20),
        ] {
            assert_eq!(
                cells_by_distance(center, 12).collect::<Vec<_>>(),
                every_ring_cell_scanned_row_by_row(center, 12),
                "around {center}"
            );
        }
    }

    #[test]
    fn rings_are_visited_nearest_first_and_stay_on_the_map() {
        let cells: Vec<IVec2> = cells_by_distance(IVec2::ZERO, 3).collect();
        assert_eq!(cells[0], IVec2::ZERO);
        assert!(cells.iter().all(|&cell| GameMap::in_bounds(cell)));
        let rings: Vec<i32> = cells.iter().map(|cell| cell.abs().max_element()).collect();
        assert!(rings.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn world_and_cell_coordinates_round_trip() {
        for cell in [IVec2::ZERO, IVec2::new(17, 42), map_bounds().max()] {
            assert_eq!(world_to_cell(cell_center(cell)), cell);
            assert_eq!(cell_of_index(GameMap::index(cell).unwrap()), cell);
        }
    }
}
