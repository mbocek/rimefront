mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use rimefront::missions::levels::LevelDefinition;
use rimefront::sim::catalog::{BuildingKind, Team};
use rimefront::world::map::{GameMap, MapBounds, MapSize, base_sites, map_bounds, opposite_cell};

fn crystal_cells(map: &GameMap) -> Vec<IVec2> {
    map_bounds()
        .cells()
        .filter(|&cell| map.crystal_at(cell) > 0)
        .collect()
}

fn check_battlefield(size: MapSize) {
    let level = LevelDefinition {
        map_size: size,
        ..LevelDefinition::default()
    };
    let mut battle = Battle::new(BattleSetup {
        enemy_ai: true,
        starting_forces: true,
        level: Some(level),
    });
    assert_eq!(map_bounds(), MapBounds::square(size.cells()));
    let map = battle.world().resource::<GameMap>();
    let [player, enemy, ..] = base_sites();
    assert!(
        map.find_path(player, enemy).is_some(),
        "{size:?}: the bases are cut off from each other"
    );
    let crystals = crystal_cells(map);
    assert!(!crystals.is_empty(), "{size:?}: no crystals");
    for &cell in &crystals {
        assert!(
            map.crystal_at(opposite_cell(cell)) > 0,
            "{size:?}: crystal at {cell} has no mirror twin"
        );
    }
    let near_player = crystals
        .iter()
        .any(|cell| (*cell - player).as_vec2().length() < 14.0);
    assert!(
        near_player,
        "{size:?}: no crystal field near the player base"
    );

    battle.run_for(120.0);
    assert!(
        battle.buildings(Team::Crimson).len() > 1,
        "{size:?}: the computer did not build on a {} map",
        size.cells()
    );
    assert!(
        battle
            .building(Team::Player, BuildingKind::CommandCenter)
            .is_some()
    );
}

#[test]
fn every_map_size_is_a_fair_playable_battlefield() {
    for size in MapSize::ALL {
        check_battlefield(size);
    }
}
