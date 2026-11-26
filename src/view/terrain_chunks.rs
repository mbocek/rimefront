use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on};

use crate::missions::levels::ActiveLevel;
use crate::view::camera::{RtsCamera, ground_in_view};
use crate::view::fog_view::FogTexture;
use crate::view::terrain_material::{
    Ice, TerrainSurface, WaterMaterial, WaterSurface, water_material,
};
use crate::world::map::{CELL_SIZE, CHUNK_CELLS, GameMap, map_bounds};
use crate::world::season::Season;
use crate::world::terrain::WATER_LEVEL;
use crate::world::terrain::{FAR_RING_WIDTH, GroundLayers, LandscapeContext, OUTLANDS_WIDTH};
use crate::world::wilds::WorldWidened;

const PATCH_METRES: f32 = CHUNK_CELLS as f32 * CELL_SIZE;

/// How far a patch's skirt hangs below its edge, deep enough to hide the crack where it meets a coarser one.
const SKIRT_DEPTH: f32 = 3.0;

/// Ground with a vertex every `spacing` metres: the map and its rim, or the coarse mountains beyond.
pub const DETAILED_SPACING: f32 = 0.5;
pub const COARSE_SPACING: f32 = 2.0;

/// The ground over `area` with its layer weights as vertex colours. Normals come from the heights around each
/// vertex, so neighbouring patches meet without a lighting seam, and a skirt hangs from the edges to hide the
/// crack where a patch meets a coarser one.
pub fn build_terrain_patch(
    ground: &GroundLayers,
    area: Rect,
    spacing: f32,
    crystal_soil: impl Fn(IVec2) -> bool,
) -> Mesh {
    let landscape = ground.landscape();
    let vertices = (area.size() / spacing).round().as_ivec2() + IVec2::ONE;
    let point = |x: i32, z: i32| area.min + Vec2::new(x as f32, z as f32) * spacing;
    let with_apron = vertices + IVec2::splat(2);
    let heights: Vec<f32> = (0..with_apron.element_product())
        .map(|i| {
            let at = point(i % with_apron.x - 1, i / with_apron.x - 1);
            landscape.height(at.x, at.y)
        })
        .collect();
    let height = |x: i32, z: i32| heights[((z + 1) * with_apron.x + x + 1) as usize];

    let count = vertices.element_product() as usize;
    let mut positions = Vec::with_capacity(count);
    let mut normals = Vec::with_capacity(count);
    let mut colors = Vec::with_capacity(count);
    let mut uvs = Vec::with_capacity(count);
    for z in 0..vertices.y {
        for x in 0..vertices.x {
            let at = point(x, z);
            let soil = if crystal_soil((at / CELL_SIZE).floor().as_ivec2()) {
                1.0
            } else {
                0.0
            };
            let slope_x = height(x + 1, z) - height(x - 1, z);
            let slope_z = height(x, z + 1) - height(x, z - 1);
            positions.push([at.x, height(x, z), at.y]);
            normals.push(
                Vec3::new(-slope_x, 2.0 * spacing, -slope_z)
                    .normalize()
                    .to_array(),
            );
            colors.push(ground.weights(at, soil).to_array());
            uvs.push([at.x, at.y]);
        }
    }

    let index_of = |x: i32, z: i32| (z * vertices.x + x) as u32;
    let mut indices = Vec::with_capacity(((vertices - IVec2::ONE).element_product() * 6) as usize);
    for z in 0..vertices.y - 1 {
        for x in 0..vertices.x - 1 {
            let (top_left, top_right) = (index_of(x, z), index_of(x + 1, z));
            let (bottom_left, bottom_right) = (index_of(x, z + 1), index_of(x + 1, z + 1));
            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_right,
                top_right,
                bottom_left,
                bottom_right,
            ]);
        }
    }

    let last = vertices - IVec2::ONE;
    let rim: Vec<u32> = (0..last.x)
        .map(|x| index_of(x, 0))
        .chain((0..last.y).map(|z| index_of(last.x, z)))
        .chain((1..=last.x).rev().map(|x| index_of(x, last.y)))
        .chain((1..=last.y).rev().map(|z| index_of(0, z)))
        .collect();
    for (step, &edge) in rim.iter().enumerate() {
        let next_edge = rim[(step + 1) % rim.len()];
        let hanging = positions.len() as u32;
        for source in [edge, next_edge] {
            let [x, y, z] = positions[source as usize];
            positions.push([x, y - SKIRT_DEPTH, z]);
            normals.push(normals[source as usize]);
            colors.push(colors[source as usize]);
            uvs.push(uvs[source as usize]);
        }
        let (below, next_below) = (hanging, hanging + 1);
        indices.extend_from_slice(&[edge, below, next_edge, next_edge, below, next_below]);
        indices.extend_from_slice(&[edge, next_edge, below, next_edge, next_below, below]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// One chunk of ground, drawn only when in view; a crystal painted in the editor rebuilds only its chunk.
#[derive(Component)]
pub struct TerrainChunk {
    area: Rect,
    spacing: f32,
    /// The crystal cells whose soil it shows, in cell order; empty for the coarse mountains.
    crystal_soil: Vec<IVec2>,
}

/// Ground being built on another thread; the chunk gets its mesh when it is done.
#[derive(Component)]
pub struct GroundUnderConstruction(Task<Mesh>);

/// The ground of the map and its rim, detailed, and the coarse mountains beyond it, as chunk-sized patches.
fn ground_patches() -> Vec<(Rect, f32)> {
    let bounds = map_bounds();
    let detailed = Rect::from_corners(
        bounds.world_min() - OUTLANDS_WIDTH,
        bounds.world_max() + OUTLANDS_WIDTH,
    );
    let whole = Rect::from_corners(detailed.min - FAR_RING_WIDTH, detailed.max + FAR_RING_WIDTH);
    let first = (whole.min / PATCH_METRES).floor().as_ivec2();
    let past_last = (whole.max / PATCH_METRES).ceil().as_ivec2();
    let mut patches = Vec::new();
    for z in first.y..past_last.y {
        for x in first.x..past_last.x {
            let chunk = Rect::from_corners(
                Vec2::new(x as f32, z as f32) * PATCH_METRES,
                Vec2::new((x + 1) as f32, (z + 1) as f32) * PATCH_METRES,
            );
            let area = chunk.intersect(whole);
            if area.is_empty() {
                continue;
            }
            let shows_the_map = !area.intersect(detailed).is_empty();
            let spacing = if shows_the_map {
                DETAILED_SPACING
            } else {
                COARSE_SPACING
            };
            patches.push((area, spacing));
        }
    }
    patches
}

/// The crystal cells under a patch, including the row and column its far edge touches.
fn crystal_soil_under(map: &GameMap, area: Rect, spacing: f32) -> Vec<IVec2> {
    if spacing != DETAILED_SPACING {
        return Vec::new();
    }
    let first = (area.min / CELL_SIZE).floor().as_ivec2();
    let last = (area.max / CELL_SIZE).floor().as_ivec2();
    (first.y..=last.y)
        .flat_map(|y| (first.x..=last.x).map(move |x| IVec2::new(x, y)))
        .filter(|&cell| map.crystal_at(cell) > 0)
        .collect()
}

fn start_building(
    ground: &Arc<GroundLayers>,
    area: Rect,
    spacing: f32,
    crystal_soil: Vec<IVec2>,
) -> GroundUnderConstruction {
    let context = LandscapeContext::current();
    let ground = Arc::clone(ground);
    let task = AsyncComputeTaskPool::get().spawn(async move {
        context.enter();
        build_terrain_patch(&ground, area, spacing, |cell| {
            crystal_soil
                .binary_search_by_key(&(cell.y, cell.x), |c| (c.y, c.x))
                .is_ok()
        })
    });
    GroundUnderConstruction(task)
}

/// Every patch of ground the level has, and the entity showing it while it is near the camera.
#[derive(Resource)]
pub struct GroundPlan {
    ground: Arc<GroundLayers>,
    patches: Vec<(Rect, f32)>,
    shown: Vec<Option<Entity>>,
    placeholder: Handle<StandardMaterial>,
}

pub fn plan_terrain(
    commands: &mut Commands,
    level: &ActiveLevel,
    placeholder: Handle<StandardMaterial>,
) {
    let patches = ground_patches();
    commands.insert_resource(GroundPlan {
        ground: Arc::new(GroundLayers::new(level.0.landscape)),
        shown: vec![None; patches.len()],
        patches,
        placeholder,
    });
}

/// Ground beyond the view this far is built ahead, so panning does not reveal a patch still being built.
const BUILD_AHEAD: f32 = PATCH_METRES;
/// Ground stays this much farther than it is built, so a camera rocking on a patch's edge does not rebuild it.
const KEEP_BEYOND_BUILD: f32 = PATCH_METRES;

/// Shows the patches around what the camera sees and drops the ones it has left far behind.
pub fn stream_terrain(
    mut commands: Commands,
    plan: Option<ResMut<GroundPlan>>,
    map: Option<Res<GameMap>>,
    cameras: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    chunks: Query<(), With<TerrainChunk>>,
) {
    let (Some(mut plan), Some(map)) = (plan, map) else {
        return;
    };
    let Ok((camera, transform)) = cameras.single() else {
        return;
    };
    let Some(view) = ground_in_view(camera, transform) else {
        return;
    };
    let wanted = view.inflate(BUILD_AHEAD);
    let kept = wanted.inflate(KEEP_BEYOND_BUILD);
    let plan = &mut *plan;
    for (index, &(area, spacing)) in plan.patches.iter().enumerate() {
        let shown = plan.shown[index].filter(|&entity| chunks.contains(entity));
        plan.shown[index] = match shown {
            None if overlaps(area, wanted) => {
                let crystal_soil = crystal_soil_under(&map, area, spacing);
                let building = start_building(&plan.ground, area, spacing, crystal_soil.clone());
                let entity = commands
                    .spawn((
                        TerrainSurface,
                        TerrainChunk {
                            area,
                            spacing,
                            crystal_soil,
                        },
                        building,
                        MeshMaterial3d(plan.placeholder.clone()),
                        Transform::IDENTITY,
                    ))
                    .id();
                Some(entity)
            }
            Some(entity) if !overlaps(area, kept) => {
                commands.entity(entity).despawn();
                None
            }
            unchanged => unchanged,
        };
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    !a.intersect(b).is_empty()
}

pub fn finish_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut building: Query<(Entity, &mut GroundUnderConstruction)>,
) {
    for (entity, mut construction) in &mut building {
        if !construction.0.is_finished() {
            continue;
        }
        let mesh = block_on(&mut construction.0);
        commands
            .entity(entity)
            .remove::<GroundUnderConstruction>()
            .insert(Mesh3d(meshes.add(mesh)));
    }
}

/// Rebuilds the chunks whose crystal soil changed, once an editor stroke has painted or cleared crystals.
pub fn resoil_terrain_chunks(
    commands: &mut Commands,
    map: &GameMap,
    plan: Option<&GroundPlan>,
    chunks: &mut Query<(Entity, &mut TerrainChunk)>,
) {
    let Some(plan) = plan else { return };
    for (entity, mut chunk) in chunks.iter_mut() {
        let crystal_soil = crystal_soil_under(map, chunk.area, chunk.spacing);
        if crystal_soil == chunk.crystal_soil {
            continue;
        }
        let building = start_building(
            &plan.ground,
            chunk.area,
            chunk.spacing,
            crystal_soil.clone(),
        );
        chunk.crystal_soil = crystal_soil;
        commands.entity(entity).insert(building);
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
    spawn_water(
        &mut commands,
        &mut meshes,
        &mut images,
        &mut water_materials,
        &fog,
        level.0.season == Season::Winter,
    );
}

fn spawn_water(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    water_materials: &mut Assets<WaterMaterial>,
    fog: &FogTexture,
    frozen: bool,
) {
    let bounds = map_bounds();
    let water_span = bounds.world_size() + Vec2::splat(2.0 * (OUTLANDS_WIDTH + FAR_RING_WIDTH));
    let water_middle = bounds.world_center();
    let water_mesh = Plane3d::default()
        .mesh()
        .size(water_span.x, water_span.y)
        .build()
        .with_generated_tangents()
        .expect("plane mesh has normals and UVs");
    let mut water = commands.spawn((
        WaterSurface,
        Mesh3d(meshes.add(water_mesh)),
        MeshMaterial3d(water_materials.add(water_material(images, fog.0.clone(), frozen))),
        Transform::from_xyz(water_middle.x, WATER_LEVEL, water_middle.y),
    ));
    if frozen {
        water.insert(Ice);
    }
}

/// The ground and the water follow a world that widened during the match.
pub fn follow_widened_ground(
    mut widened: MessageReader<WorldWidened>,
    mut commands: Commands,
    plan: Option<Res<GroundPlan>>,
    level: Res<ActiveLevel>,
    chunks: Query<Entity, With<TerrainChunk>>,
    waters: Query<Entity, With<WaterSurface>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut water_materials: ResMut<Assets<WaterMaterial>>,
    fog: Res<FogTexture>,
) {
    let (Some(_), Some(plan)) = (widened.read().last(), plan) else {
        return;
    };
    for entity in chunks.iter().chain(&waters) {
        commands.entity(entity).despawn();
    }
    plan_terrain(&mut commands, &level, plan.placeholder.clone());
    spawn_water(
        &mut commands,
        &mut meshes,
        &mut images,
        &mut water_materials,
        &fog,
        level.0.season == Season::Winter,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::{MapBounds, set_map_bounds};

    #[test]
    fn patches_cover_the_ground_once_and_the_map_in_detail() {
        let bounds = MapBounds {
            min: IVec2::new(-10, 4),
            size: IVec2::new(70, 50),
        };
        set_map_bounds(bounds);
        let patches = ground_patches();
        let covered: f32 = patches
            .iter()
            .map(|(area, _)| area.size().element_product())
            .sum();
        let beyond_map = OUTLANDS_WIDTH + FAR_RING_WIDTH;
        let whole = bounds.world_size() + 2.0 * beyond_map;
        assert!((covered - whole.element_product()).abs() < 1.0);
        for cell in [bounds.min, bounds.max(), bounds.min + bounds.size / 2] {
            let middle = (cell.as_vec2() + 0.5) * CELL_SIZE;
            let (_, spacing) = patches
                .iter()
                .find(|(area, _)| area.contains(middle))
                .expect("every map cell lies under a patch");
            assert_eq!(*spacing, DETAILED_SPACING);
        }
        for &(area, spacing) in &patches {
            let steps = area.size() / spacing;
            assert_eq!(steps, steps.round(), "a patch must end on a vertex");
        }
    }
}
