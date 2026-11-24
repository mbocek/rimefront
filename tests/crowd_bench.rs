mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use iron_tide::catalog::{Team, UnitKind};
use iron_tide::levels::LevelDefinition;
use iron_tide::map::MapSize;

const UNITS_PER_SIDE_IN_A_FIGHT: i32 = 80;
const SECONDS_OF_FIGHTING: f32 = 20.0;

fn spread_fights(fights: i32) -> Battle {
    let mut battle = Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(LevelDefinition {
            map_size: MapSize::Vast,
            ..default()
        }),
    });
    for fight in 0..fights {
        let corner = IVec2::new(20 + (fight % 4) * 60, 20 + (fight / 4) * 70);
        for i in 0..UNITS_PER_SIDE_IN_A_FIGHT {
            let kind = if i % 5 == 0 {
                UnitKind::Tank
            } else {
                UnitKind::Rifleman
            };
            let cell = corner + IVec2::new(i % 10, i / 10);
            battle.spawn_unit(kind, Team::Player, cell);
            battle.spawn_unit(kind, Team::Crimson, cell + IVec2::new(0, 16));
        }
    }
    battle
}

/// How long the simulation takes for 1, 4 and 12 fights spread over a Vast map:
/// `cargo test --release --test crowd_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn simulation_time_of_spread_out_fights() {
    for fights in [1, 4, 12] {
        let mut battle = spread_fights(fights);
        let started = std::time::Instant::now();
        battle.run_for(SECONDS_OF_FIGHTING);
        let units = fights * UNITS_PER_SIDE_IN_A_FIGHT * 2;
        let survivors =
            battle.units(Team::Player, None).len() + battle.units(Team::Crimson, None).len();
        eprintln!(
            "{units} units: {:.2} s, {survivors} survive, {:.0} damage taken by the player",
            started.elapsed().as_secs_f32(),
            battle.damage_taken(Team::Player)
        );
    }
}
