use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::chunked_grid::ChunkedGrid;
use crate::world::map::{CELL_SIZE, MapBounds, chunk_of, map_bounds, world_to_cell};

static NEXT_LINEAGE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CellSight {
    #[default]
    Unexplored,
    Explored,
    Visible,
}

/// What one side knows about each map cell; `Intel` keeps one per team.
pub struct FogOfWar {
    enabled: bool,
    cells: ChunkedGrid<CellSight>,
    /// Bumped whenever any cell changes, so presentation systems can skip unchanged frames.
    pub revision: u64,
    /// The revision each chunk last changed at, so presentation can redraw only those.
    chunk_revisions: Vec<u64>,
    /// Tells this fog from every other one, copies included, so a painter that switches fogs starts afresh.
    lineage: u64,
    /// The chunks the last reveal made a cell visible in, so the next one dims only those rather than the
    /// whole map.
    lit_chunks: Vec<IVec2>,
}

impl Clone for FogOfWar {
    fn clone(&self) -> Self {
        Self {
            enabled: self.enabled,
            cells: self.cells.clone(),
            revision: self.revision,
            chunk_revisions: self.chunk_revisions.clone(),
            lineage: NEXT_LINEAGE.fetch_add(1, Ordering::Relaxed),
            lit_chunks: self.lit_chunks.clone(),
        }
    }
}

fn chunk_cells(bounds: MapBounds, chunk: IVec2) -> impl Iterator<Item = IVec2> {
    let (first, last) = bounds.cells_of_chunk(chunk);
    (first.y..=last.y).flat_map(move |y| (first.x..=last.x).map(move |x| IVec2::new(x, y)))
}

/// Radius in meters an entity reveals around itself.
#[derive(Component, Clone, Copy)]
pub struct Sight(pub f32);

impl FogOfWar {
    pub fn new(enabled: bool) -> Self {
        let state = if enabled {
            CellSight::Unexplored
        } else {
            CellSight::Visible
        };
        let revision = 1;
        Self {
            enabled,
            cells: ChunkedGrid::filled(map_bounds(), state),
            revision,
            chunk_revisions: vec![revision; map_bounds().chunk_count()],
            lineage: NEXT_LINEAGE.fetch_add(1, Ordering::Relaxed),
            lit_chunks: Vec::new(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn lineage(&self) -> u64 {
        self.lineage
    }

    /// Covers `wider`, which contains the map: known cells stay known, new ground is unexplored.
    pub fn widen(&mut self, wider: MapBounds) {
        let narrow = self.cells.bounds();
        self.cells.widen(wider);
        for index in 0..wider.chunk_count() {
            self.cells.open_chunk(wider.chunk_at(index));
        }
        self.revision += 1;
        let mut revisions = vec![self.revision; wider.chunk_count()];
        for (index, &revision) in self.chunk_revisions.iter().enumerate() {
            if let Some(wider_index) = wider.chunk_index(narrow.chunk_at(index)) {
                revisions[wider_index] = revision;
            }
        }
        self.chunk_revisions = revisions;
    }

    /// The chunks with a cell that changed after `revision`.
    pub fn chunks_changed_since(&self, revision: u64) -> impl Iterator<Item = IVec2> {
        let bounds = self.cells.bounds();
        self.chunk_revisions
            .iter()
            .enumerate()
            .filter(move |&(_, &changed)| changed > revision)
            .map(move |(index, _)| bounds.chunk_at(index))
    }

    /// Ground outside the playable map shares the sight of the nearest map edge, so the fog runs on past it.
    pub fn sight_at(&self, cell: IVec2) -> CellSight {
        let nearest_map_cell = self.cells.bounds().clamp(cell);
        self.cells
            .get(nearest_map_cell)
            .unwrap_or(CellSight::Explored)
    }

    pub fn is_visible(&self, position: Vec3) -> bool {
        self.sight_at(world_to_cell(position)) == CellSight::Visible
    }

    pub fn is_explored(&self, position: Vec3) -> bool {
        self.sight_at(world_to_cell(position)) != CellSight::Unexplored
    }

    /// Whether each cell of the bounds was ever seen, row by row, as saved games keep it.
    pub fn explored_cells(&self) -> Vec<bool> {
        self.cells
            .to_dense()
            .iter()
            .map(|&sight| sight != CellSight::Unexplored)
            .collect()
    }

    pub fn restore_explored(&mut self, explored: &[bool]) {
        let bounds = self.cells.bounds();
        if !self.enabled || explored.len() != bounds.area() {
            return;
        }
        for (cell, &was_explored) in bounds.cells().zip(explored) {
            if let Some(sight) = self.cells.get_mut(cell)
                && was_explored
                && *sight == CellSight::Unexplored
            {
                *sight = CellSight::Explored;
            }
        }
        self.revision += 1;
        self.chunk_revisions.fill(self.revision);
    }

    /// Which cells of `chunk` were ever seen, a bit each row by row from its first map cell; `None` when
    /// none were.
    pub fn explored_in_chunk(&self, chunk: IVec2) -> Option<Vec<u8>> {
        let mut bits = Vec::new();
        let mut any = false;
        for (index, cell) in chunk_cells(self.cells.bounds(), chunk).enumerate() {
            if index % 8 == 0 {
                bits.push(0);
            }
            if self
                .cells
                .get(cell)
                .is_some_and(|sight| sight != CellSight::Unexplored)
            {
                bits[index / 8] |= 1 << (index % 8);
                any = true;
            }
        }
        any.then_some(bits)
    }

    pub fn restore_explored_in_chunk(&mut self, chunk: IVec2, bits: &[u8]) {
        if !self.enabled {
            return;
        }
        for (index, cell) in chunk_cells(self.cells.bounds(), chunk).enumerate() {
            let was_explored = bits
                .get(index / 8)
                .is_some_and(|byte| byte & (1 << (index % 8)) != 0);
            if let Some(sight) = self.cells.get_mut(cell)
                && was_explored
                && *sight == CellSight::Unexplored
            {
                *sight = CellSight::Explored;
            }
        }
        self.revision += 1;
        self.chunk_revisions.fill(self.revision);
    }

    /// Recomputes visibility from the given viewers (position and sight radius): what they see is visible,
    /// what was visible before and is not any more is explored. It works on the chunks that held a visible
    /// cell before or do now, whole chunks at a time, so its cost follows what the viewers see rather than
    /// the size of the map.
    pub fn reveal_around(&mut self, viewers: impl IntoIterator<Item = (Vec3, f32)>) {
        if !self.enabled {
            return;
        }
        let lit_before = std::mem::take(&mut self.lit_chunks);
        let mut before = Vec::with_capacity(lit_before.len());
        for &chunk in &lit_before {
            if let Some(cells) = self.cells.chunk_cells_mut(chunk) {
                before.push((chunk, cells.to_vec()));
                for sight in cells.iter_mut() {
                    if *sight == CellSight::Visible {
                        *sight = CellSight::Explored;
                    }
                }
            }
        }
        let mut lit = HashSet::new();
        let mut last_lit = None;
        for (position, radius) in viewers {
            let center = world_to_cell(position);
            let reach = (radius / CELL_SIZE).ceil() as i32;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let cell = center + IVec2::new(dx, dy);
                    let cell_middle = (cell.as_vec2() + 0.5) * CELL_SIZE;
                    if cell_middle.distance(position.xz()) > radius {
                        continue;
                    }
                    if let Some(sight) = self.cells.get_mut(cell) {
                        *sight = CellSight::Visible;
                        let chunk = chunk_of(cell);
                        if last_lit != Some(chunk) {
                            last_lit = Some(chunk);
                            lit.insert(chunk);
                        }
                    }
                }
            }
        }
        let newly_lit = lit
            .iter()
            .copied()
            .filter(|chunk| !lit_before.contains(chunk));
        let redrawn = before
            .iter()
            .filter(|(chunk, cells)| self.cells.chunk_cells(*chunk) != Some(cells.as_slice()))
            .map(|(chunk, _)| *chunk);
        let changed: Vec<IVec2> = newly_lit.chain(redrawn).collect();
        self.lit_chunks = lit.into_iter().collect();
        if changed.is_empty() {
            return;
        }
        self.revision += 1;
        let bounds = self.cells.bounds();
        for chunk in changed {
            if let Some(index) = bounds.chunk_index(chunk) {
                self.chunk_revisions[index] = self.revision;
            }
        }
    }

    /// The chunks holding a cell the last reveal made visible.
    pub fn visible_chunks(&self) -> &[IVec2] {
        &self.lit_chunks
    }
}

impl Default for FogOfWar {
    fn default() -> Self {
        Self::new(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::{MapBounds, cell_center, set_map_bounds};

    #[test]
    fn sight_on_a_shifted_map_lands_on_the_cells_seen() {
        let bounds = MapBounds {
            min: IVec2::new(-30, 5),
            size: IVec2::new(40, 20),
        };
        set_map_bounds(bounds);
        let mut fog = FogOfWar::new(true);
        let center = bounds.min + IVec2::new(0, 3);
        fog.reveal_around([(cell_center(center), 3.0)]);
        assert_eq!(fog.sight_at(center), CellSight::Visible);
        assert_eq!(fog.sight_at(bounds.max()), CellSight::Unexplored);
        assert_eq!(
            fog.sight_at(center - IVec2::new(20, 0)),
            CellSight::Visible,
            "beyond the edge takes the nearest edge cell's sight"
        );
    }

    #[test]
    fn sight_reveals_a_circle_and_remembers_it() {
        let mut fog = FogOfWar::new(true);
        let center = IVec2::new(20, 20);
        fog.reveal_around([(cell_center(center), 6.0)]);
        assert_eq!(fog.sight_at(center), CellSight::Visible);
        assert_eq!(fog.sight_at(center + IVec2::new(2, 0)), CellSight::Visible);
        assert_eq!(
            fog.sight_at(center + IVec2::new(4, 4)),
            CellSight::Unexplored
        );

        fog.reveal_around([]);
        assert_eq!(
            fog.sight_at(center),
            CellSight::Explored,
            "explored ground stays known"
        );
    }

    #[test]
    fn disabled_fog_shows_everything() {
        let mut fog = FogOfWar::new(false);
        fog.reveal_around([]);
        assert!(fog.is_visible(cell_center(IVec2::new(60, 2))));
    }

    #[test]
    fn revision_changes_only_with_the_view() {
        let mut fog = FogOfWar::new(true);
        let viewer = [(cell_center(IVec2::new(10, 10)), 8.0)];
        fog.reveal_around(viewer);
        let after_first_look = fog.revision;
        fog.reveal_around(viewer);
        assert_eq!(fog.revision, after_first_look);
    }

    #[test]
    fn exploration_survives_a_save() {
        let mut fog = FogOfWar::new(true);
        fog.reveal_around([(cell_center(IVec2::new(5, 5)), 6.0)]);
        let saved = fog.explored_cells();
        let mut restored = FogOfWar::new(true);
        restored.restore_explored(&saved);
        assert_eq!(restored.sight_at(IVec2::new(5, 5)), CellSight::Explored);
        assert_eq!(restored.sight_at(IVec2::new(40, 40)), CellSight::Unexplored);
    }
}
