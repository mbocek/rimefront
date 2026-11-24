mod common;

use common::Battle;
use iron_tide::missions::random_battle::{DifficultyChoice, RandomBattleOptions, generate};
use iron_tide::sim::catalog::{BuildingKind, Team};
use iron_tide::world::map::MapSize;

#[test]
fn rolled_battles_of_every_size_start_with_a_base_for_every_side() {
    for (seed, map_size) in MapSize::ALL.into_iter().enumerate() {
        let options = RandomBattleOptions {
            opponents: 3,
            difficulty: DifficultyChoice::Mixed,
            map_size,
            ..RandomBattleOptions::default()
        };
        let mut battle = Battle::on_level(generate(&options, seed as u64 * 977));
        battle.run_for(30.0);
        for team in Team::ALL {
            assert!(
                battle
                    .buildings(team)
                    .contains(&BuildingKind::CommandCenter),
                "{map_size:?}: {team:?} has no Command Center"
            );
        }
    }
}

#[test]
fn rolled_forests_and_rocks_never_cut_a_base_off() {
    use iron_tide::world::map::GameMap;
    for seed in 0..30u64 {
        let options = RandomBattleOptions {
            opponents: 3,
            map_size: MapSize::ALL[seed as usize % 3],
            ..RandomBattleOptions::default()
        };
        let level = generate(&options, seed * 7919 + 1);
        let sites: Vec<_> = std::iter::once(level.player.site)
            .chain(level.enemies.iter().map(|enemy| enemy.force.site))
            .collect();
        let battle = &mut Battle::on_level(level);
        let map = battle.world().resource::<GameMap>();
        let gate = |site: iron_tide::missions::levels::BaseSite| {
            map.nearest_passable(site.origin() + bevy::prelude::IVec2::new(4, 4))
                .expect("open ground near the base")
        };
        for pair in sites.windows(2) {
            let path = map.find_path(gate(pair[0]), gate(pair[1]));
            let arrives = path.as_ref().and_then(|path| path.last()) == Some(&gate(pair[1]));
            assert!(
                arrives,
                "seed {seed}: no way from {:?} to {:?}",
                pair[0], pair[1]
            );
        }
    }
}
