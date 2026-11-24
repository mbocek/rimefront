use std::collections::HashMap;

use bevy::prelude::*;

use crate::view::assets::ModelLibrary;
use crate::view::camera::{RtsCamera, ground_in_view};
use crate::world::map::{CELL_SIZE, CHUNK_CELLS};
use crate::world::scenery::{Scenery, SceneryLayout, spawn_scenery_entity};

const CHUNK_METRES: f32 = CHUNK_CELLS as f32 * CELL_SIZE;
/// Props this far beyond the view are shown ahead, so panning does not reveal a chunk still empty.
const SHOW_AHEAD: f32 = CHUNK_METRES;
/// Props stay this much farther than they are shown, so a camera rocking on a chunk's edge does not redo it.
const KEEP_BEYOND_SHOWN: f32 = CHUNK_METRES;

/// The entities showing each chunk's props near the camera, and the revision of the chunk they show.
#[derive(Resource, Default)]
pub struct SceneryShown {
    lineage: u64,
    chunks: HashMap<IVec2, ShownChunk>,
}

struct ShownChunk {
    revision: u64,
    entities: Vec<Entity>,
}

/// Gives the props around what the camera sees an entity each, drops those it has left far behind, and
/// redoes a shown chunk whose props changed.
pub fn stream_scenery(
    mut commands: Commands,
    layout: Option<Res<SceneryLayout>>,
    mut shown: ResMut<SceneryShown>,
    models: Res<ModelLibrary>,
    cameras: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    alive: Query<(), With<Scenery>>,
) {
    let Some(layout) = layout else { return };
    let mut drop_chunk = |chunk: ShownChunk| {
        for entity in chunk.entities {
            if alive.contains(entity) {
                commands.entity(entity).despawn();
            }
        }
    };
    if shown.lineage != layout.lineage() {
        shown.lineage = layout.lineage();
        for (_, chunk) in shown.chunks.drain() {
            drop_chunk(chunk);
        }
    }
    let Ok((camera, transform)) = cameras.single() else {
        return;
    };
    let Some(view) = ground_in_view(camera, transform) else {
        return;
    };
    let wanted = view.inflate(SHOW_AHEAD);
    let kept = wanted.inflate(KEEP_BEYOND_SHOWN);

    let stale: Vec<IVec2> = shown
        .chunks
        .iter()
        .filter(|&(&chunk, shown_chunk)| {
            let current = layout
                .chunk(chunk)
                .is_some_and(|props| props.revision == shown_chunk.revision);
            !current || !overlaps(chunk_area(chunk), kept)
        })
        .map(|(&chunk, _)| chunk)
        .collect();
    for chunk in stale {
        if let Some(shown_chunk) = shown.chunks.remove(&chunk) {
            drop_chunk(shown_chunk);
        }
    }

    let first = (wanted.min / CHUNK_METRES).floor().as_ivec2();
    let last = (wanted.max / CHUNK_METRES).floor().as_ivec2();
    for y in first.y..=last.y {
        for x in first.x..=last.x {
            let chunk = IVec2::new(x, y);
            if shown.chunks.contains_key(&chunk) {
                continue;
            }
            let Some(props) = layout.chunk(chunk) else {
                continue;
            };
            let entities = props
                .props
                .iter()
                .map(|prop| spawn_scenery_entity(&mut commands, &models, prop))
                .collect();
            shown.chunks.insert(
                chunk,
                ShownChunk {
                    revision: props.revision,
                    entities,
                },
            );
        }
    }
}

fn chunk_area(chunk: IVec2) -> Rect {
    let corner = chunk.as_vec2() * CHUNK_METRES;
    Rect::from_corners(corner, corner + CHUNK_METRES)
}

fn overlaps(a: Rect, b: Rect) -> bool {
    !a.intersect(b).is_empty()
}
