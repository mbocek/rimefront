use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};

use crate::missions::levels::ActiveLevel;
use crate::sim::catalog::CRYSTAL_PER_CELL;
use crate::world::map::{CrystalVisual, GameMap, is_water, near_base, spawn_crystal_visual};
use crate::world::scenery::{Obstacle, SceneryLayout, place_obstacle};
use crate::world::terrain::Landscape;

/// What the map editor paints onto one cell.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Brush {
    Crystal,
    Forest,
    Rock,
    Clear,
}

impl Brush {
    pub const ALL: [Brush; 4] = [Brush::Crystal, Brush::Forest, Brush::Rock, Brush::Clear];

    pub fn name(self) -> &'static str {
        match self {
            Brush::Crystal => "Crystal",
            Brush::Forest => "Forest",
            Brush::Rock => "Rock",
            Brush::Clear => "Clear ground",
        }
    }

    /// Base sites must stay open for the Command Center and the first structures.
    pub fn allowed_at(self, cell: IVec2) -> bool {
        self == Brush::Clear || !near_base(cell)
    }
}

/// One cell of a level's map repainted by hand on top of the generated landscape.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct MapEdit {
    pub cell: IVec2,
    pub brush: Brush,
}

/// Only the last edit of each cell counts, in the order the cells were first edited.
pub fn final_edits(edits: &[MapEdit]) -> Vec<MapEdit> {
    let mut last_brush: HashMap<IVec2, Brush> = HashMap::new();
    let mut order = Vec::new();
    for edit in edits {
        if last_brush.insert(edit.cell, edit.brush).is_none() {
            order.push(edit.cell);
        }
    }
    order
        .into_iter()
        .map(|cell| MapEdit {
            cell,
            brush: last_brush[&cell],
        })
        .collect()
}

pub fn apply_map_edits(level: Res<ActiveLevel>, mut painter: MapPainter) {
    let ground = Ground::of(&level);
    for edit in final_edits(&level.0.map_edits) {
        painter.paint(ground, edit);
    }
}

/// The parts of a level that decide what a cell can hold and how its props look.
#[derive(Clone, Copy)]
pub struct Ground {
    pub landscape: Landscape,
    pub map_seed: u64,
}

impl Ground {
    pub fn of(level: &ActiveLevel) -> Ground {
        Ground {
            landscape: level.0.landscape,
            map_seed: level.0.map_seed,
        }
    }
}

#[derive(SystemParam)]
pub struct MapPainter<'w, 's> {
    commands: Commands<'w, 's>,
    map: ResMut<'w, GameMap>,
    scenery: ResMut<'w, SceneryLayout>,
    crystals: Query<'w, 's, (Entity, &'static CrystalVisual)>,
}

impl MapPainter<'_, '_> {
    /// Returns whether the cell could take the brush; water and base sites refuse most brushes.
    pub fn paint(&mut self, ground: Ground, edit: MapEdit) -> bool {
        let MapEdit { cell, brush } = edit;
        let refused = !GameMap::in_bounds(cell)
            || is_water(ground.landscape, cell)
            || !brush.allowed_at(cell);
        if refused {
            return false;
        }
        self.clear(cell);
        let mut rng = visual_rng(ground.map_seed, cell);
        match brush {
            Brush::Clear => {}
            Brush::Crystal => {
                self.map.set_crystal(cell, CRYSTAL_PER_CELL);
                spawn_crystal_visual(&mut self.commands, &mut rng, cell, CRYSTAL_PER_CELL);
            }
            Brush::Forest | Brush::Rock => {
                let obstacle = if brush == Brush::Forest {
                    Obstacle::Tree
                } else {
                    Obstacle::Rock
                };
                self.map.set_obstacle(cell, true);
                place_obstacle(
                    &mut self.scenery,
                    ground.landscape,
                    &mut rng,
                    cell,
                    obstacle,
                );
            }
        }
        true
    }

    fn clear(&mut self, cell: IVec2) {
        self.map.set_crystal(cell, 0);
        self.map.set_obstacle(cell, false);
        self.scenery.clear_cell(cell);
        let crystals_here = self
            .crystals
            .iter()
            .filter(|(_, visual)| visual.cell == cell)
            .map(|(entity, _)| entity);
        for entity in crystals_here {
            self.commands.entity(entity).despawn();
        }
    }
}

/// Looks never touch the simulation, but seeding them by cell keeps a map looking the same each time.
fn visual_rng(map_seed: u64, cell: IVec2) -> StdRng {
    let cell_key = ((cell.x as u64) << 32) | cell.y as u32 as u64;
    StdRng::seed_from_u64(map_seed ^ cell_key.wrapping_mul(0x9e37_79b9_7f4a_7c15))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_edit_of_a_cell_wins_and_keeps_its_first_place() {
        let a = IVec2::new(3, 4);
        let b = IVec2::new(5, 6);
        let edits = [
            MapEdit {
                cell: a,
                brush: Brush::Rock,
            },
            MapEdit {
                cell: b,
                brush: Brush::Forest,
            },
            MapEdit {
                cell: a,
                brush: Brush::Clear,
            },
        ];
        assert_eq!(
            final_edits(&edits),
            vec![
                MapEdit {
                    cell: a,
                    brush: Brush::Clear
                },
                MapEdit {
                    cell: b,
                    brush: Brush::Forest
                },
            ]
        );
    }
}
