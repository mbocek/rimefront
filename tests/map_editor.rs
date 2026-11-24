mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use iron_tide::missions::levels::LevelDefinition;
use iron_tide::world::map::{GameMap, is_water, player_base_cell};
use iron_tide::world::map_edits::{Brush, MapEdit};

fn open_cells(count: usize) -> Vec<IVec2> {
    let level = LevelDefinition::default();
    let mut battle = Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(level.clone()),
    });
    let map = battle.world().resource::<GameMap>();
    (8..56)
        .flat_map(|y| (24..40).map(move |x| IVec2::new(x, y)))
        .filter(|&cell| map.is_buildable(cell) && !is_water(level.landscape, cell))
        .step_by(7)
        .take(count)
        .collect()
}

#[test]
fn painted_cells_hold_crystal_or_obstacles_when_the_battle_starts() {
    let cells = open_cells(3);
    let [crystal, forest, cleared] = [cells[0], cells[1], cells[2]];
    let level = LevelDefinition {
        map_edits: vec![
            MapEdit {
                cell: crystal,
                brush: Brush::Crystal,
            },
            MapEdit {
                cell: forest,
                brush: Brush::Forest,
            },
            MapEdit {
                cell: cleared,
                brush: Brush::Rock,
            },
            MapEdit {
                cell: cleared,
                brush: Brush::Clear,
            },
        ],
        ..default()
    };
    let mut battle = Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(level),
    });
    let map = battle.world().resource::<GameMap>();
    assert!(map.crystal_at(crystal) > 0);
    assert!(!map.is_passable(forest));
    assert!(map.is_buildable(cleared));
}

#[test]
fn obstacles_cannot_be_painted_over_a_base_site() {
    let base = player_base_cell();
    let level = LevelDefinition {
        map_edits: vec![MapEdit {
            cell: base,
            brush: Brush::Rock,
        }],
        ..default()
    };
    let mut battle = Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(level),
    });
    assert!(battle.world().resource::<GameMap>().is_passable(base));
}

#[test]
fn map_edits_survive_a_round_trip_through_json() {
    let level = LevelDefinition {
        map_edits: vec![MapEdit {
            cell: IVec2::new(4, 9),
            brush: Brush::Forest,
        }],
        ..default()
    };
    let json = serde_json::to_string(&level).unwrap();
    let back: LevelDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(back.map_edits, level.map_edits);
}
