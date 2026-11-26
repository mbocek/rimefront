mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use rimefront::missions::levels::LevelDefinition;
use rimefront::sim::catalog::{Team, UnitKind};
use rimefront::world::map::MapSize;

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
            battle.spawn_unit(kind, Team::Azure, cell);
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
            battle.units(Team::Azure, None).len() + battle.units(Team::Crimson, None).len();
        eprintln!(
            "{units} units: {:.2} s, {survivors} survive, {:.0} damage taken by the player",
            started.elapsed().as_secs_f32(),
            battle.damage_taken(Team::Azure)
        );
    }
}

/// How long one side's fog of war takes to follow 300 moving viewers on maps of growing size:
/// `cargo test --release --test crowd_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn fog_update_time_on_growing_maps() {
    use rimefront::sim::fog::FogOfWar;
    use rimefront::world::map::{MapBounds, set_map_bounds};
    const VIEWERS: i32 = 300;
    const UPDATES: i32 = 50;
    for cells in [256, 1024, 2048] {
        set_map_bounds(MapBounds::square(cells));
        let mut fog = FogOfWar::new(true);
        let started = std::time::Instant::now();
        for update in 0..UPDATES {
            let viewers = (0..VIEWERS).map(|i| {
                let spot = IVec2::new(20 + (i % 20) * 9 + update, 20 + (i / 20) * 9);
                (rimefront::world::map::cell_center(spot), 12.0)
            });
            fog.reveal_around(viewers);
        }
        let per_update = started.elapsed().as_secs_f32() / UPDATES as f32;
        eprintln!(
            "{cells} x {cells} cells: {:.3} ms per update",
            per_update * 1000.0
        );
    }
}
