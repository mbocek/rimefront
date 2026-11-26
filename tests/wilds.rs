mod common;

use common::{Battle, BattleSetup};
use rimefront::missions::levels::LevelDefinition;
use rimefront::sim::catalog::{BuildingKind, Team};
use rimefront::world::map::MapSize;
use rimefront::world::terrain::Landscape;

#[test]
fn the_computer_builds_a_base_in_the_wilds() {
    for (map_seed, map_size) in [(3, MapSize::Standard), (17, MapSize::Large)] {
        let level = LevelDefinition {
            landscape: Landscape::Wilds,
            map_seed,
            map_size,
            ..LevelDefinition::default()
        };
        let mut battle = Battle::new(BattleSetup {
            enemy_ai: true,
            starting_forces: true,
            level: Some(level),
        });
        battle.run_for(120.0);
        assert!(
            battle.buildings(Team::Crimson).len() > 1,
            "seed {map_seed}, {map_size:?}: the computer did not build"
        );
        assert!(
            battle
                .building(Team::Azure, BuildingKind::CommandCenter)
                .is_some()
        );
    }
}
