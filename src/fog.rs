use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::map::{CELL_SIZE, chunk_of, map_bounds, world_to_cell};

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
    cells: Vec<CellSight>,
    /// Bumped whenever any cell changes, so presentation systems can skip unchanged frames.
    pub revision: u64,
    /// The revision each chunk last changed at, so presentation can redraw only those.
    chunk_revisions: Vec<u64>,
    /// Tells this fog from every other one, copies included, so a painter that switches fogs starts afresh.
    lineage: u64,
}

impl Clone for FogOfWar {
    fn clone(&self) -> Self {
        Self {
            enabled: self.enabled,
            cells: self.cells.clone(),
            revision: self.revision,
            chunk_revisions: self.chunk_revisions.clone(),
            lineage: NEXT_LINEAGE.fetch_add(1, Ordering::Relaxed),
        }
    }
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
            cells: vec![state; map_bounds().area()],
            revision,
            chunk_revisions: vec![revision; map_bounds().chunk_count()],
            lineage: NEXT_LINEAGE.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn lineage(&self) -> u64 {
        self.lineage
    }

    /// The chunks with a cell that changed after `revision`.
    pub fn chunks_changed_since(&self, revision: u64) -> impl Iterator<Item = IVec2> {
        let bounds = map_bounds();
        self.chunk_revisions
            .iter()
            .enumerate()
            .filter(move |&(_, &changed)| changed > revision)
            .map(move |(index, _)| bounds.chunk_at(index))
    }

    fn index(cell: IVec2) -> Option<usize> {
        map_bounds().index(cell)
    }

    /// Ground outside the playable map shares the sight of the nearest map edge, so the fog runs on past it.
    pub fn sight_at(&self, cell: IVec2) -> CellSight {
        let nearest_map_cell = map_bounds().clamp(cell);
        Self::index(nearest_map_cell).map_or(CellSight::Explored, |i| self.cells[i])
    }

    pub fn is_visible(&self, position: Vec3) -> bool {
        self.sight_at(world_to_cell(position)) == CellSight::Visible
    }

    pub fn is_explored(&self, position: Vec3) -> bool {
        self.sight_at(world_to_cell(position)) != CellSight::Unexplored
    }

    pub fn explored_cells(&self) -> Vec<bool> {
        self.cells
            .iter()
            .map(|&sight| sight != CellSight::Unexplored)
            .collect()
    }

    pub fn restore_explored(&mut self, explored: &[bool]) {
        if !self.enabled || explored.len() != self.cells.len() {
            return;
        }
        for (cell, &was_explored) in self.cells.iter_mut().zip(explored) {
            if was_explored && *cell == CellSight::Unexplored {
                *cell = CellSight::Explored;
            }
        }
        self.revision += 1;
        self.chunk_revisions.fill(self.revision);
    }

    /// Recomputes visibility from the given viewers (position and sight radius).
    pub fn reveal_around(&mut self, viewers: impl IntoIterator<Item = (Vec3, f32)>) {
        if !self.enabled {
            return;
        }
        let mut next: Vec<CellSight> = self
            .cells
            .iter()
            .map(|&sight| {
                if sight == CellSight::Unexplored {
                    sight
                } else {
                    CellSight::Explored
                }
            })
            .collect();
        for (position, radius) in viewers {
            let center = world_to_cell(position);
            let reach = (radius / CELL_SIZE).ceil() as i32;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let cell = center + IVec2::new(dx, dy);
                    let Some(i) = Self::index(cell) else { continue };
                    let cell_middle = (cell.as_vec2() + 0.5) * CELL_SIZE;
                    if cell_middle.distance(position.xz()) <= radius {
                        next[i] = CellSight::Visible;
                    }
                }
            }
        }
        let bounds = map_bounds();
        let revision = self.revision + 1;
        let mut changed = false;
        for (index, (old, new)) in self.cells.iter().zip(&next).enumerate() {
            if old != new {
                changed = true;
                let chunk = chunk_of(bounds.cell_at(index));
                if let Some(chunk_index) = bounds.chunk_index(chunk) {
                    self.chunk_revisions[chunk_index] = revision;
                }
            }
        }
        if changed {
            self.cells = next;
            self.revision = revision;
        }
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
    use crate::map::{MapBounds, cell_center, set_map_bounds};

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
