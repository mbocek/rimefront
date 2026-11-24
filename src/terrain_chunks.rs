use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on};

use crate::camera::RtsCamera;
use crate::levels::ActiveLevel;
use crate::map::{CELL_SIZE, CHUNK_CELLS, GameMap, map_bounds};
use crate::terrain::{
    COARSE_SPACING, DETAILED_SPACING, FAR_RING_WIDTH, GroundLayers, LandscapeContext,
    OUTLANDS_WIDTH, build_terrain_patch,
};
use crate::terrain_material::TerrainSurface;

const PATCH_METRES: f32 = CHUNK_CELLS as f32 * CELL_SIZE;

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

/// The ground the corners of the view look at, flattened to sea level; the camera always looks down
/// steeply enough for every corner to reach the ground.
fn ground_in_view(camera: &Camera, transform: &GlobalTransform) -> Option<Rect> {
    let size = camera.logical_viewport_size()?;
    let corners = [
        Vec2::ZERO,
        Vec2::new(size.x, 0.0),
        size,
        Vec2::new(0.0, size.y),
    ];
    let mut seen = Rect::EMPTY;
    for corner in corners {
        let ray = camera.viewport_to_world(transform, corner).ok()?;
        let distance = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
        seen = seen.union_point(ray.get_point(distance).xz());
    }
    Some(seen)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapBounds, set_map_bounds};

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
