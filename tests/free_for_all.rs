mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use iron_tide::missions::levels::{EnemySetup, LevelDefinition};
use iron_tide::sim::catalog::{Team, UnitKind};
use iron_tide::sim::components::Health;
use iron_tide::sim::intel::Intel;
use iron_tide::world::map::cell_center;

const FIELD: IVec2 = IVec2::new(30, 45);

fn empty_field(free_for_all: bool) -> Battle {
    let two_opponents = vec![EnemySetup::default(), EnemySetup::default()];
    Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(LevelDefinition {
            enemies: two_opponents,
            free_for_all,
            ..LevelDefinition::default()
        }),
    })
}

fn harvester_damage_after_a_tank_meets_it(free_for_all: bool) -> f32 {
    let mut battle = empty_field(free_for_all);
    battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD);
    let harvester = battle.spawn_unit(UnitKind::Harvester, Team::Amber, FIELD + IVec2::new(4, 0));
    battle.run_for(8.0);
    let health = battle.world().get::<Health>(harvester).unwrap();
    health.max - health.current
}

#[test]
fn opponents_fight_each_other_only_in_a_free_for_all() {
    assert_eq!(harvester_damage_after_a_tank_meets_it(false), 0.0);
    assert!(harvester_damage_after_a_tank_meets_it(true) > 0.0);
}

#[test]
fn in_a_free_for_all_every_opponent_sees_only_through_its_own_eyes() {
    for free_for_all in [false, true] {
        let mut battle = empty_field(free_for_all);
        battle.spawn_unit(UnitKind::Scout, Team::Crimson, FIELD);
        battle.run_for(0.5);
        let world = battle.world();
        let intel = world.resource::<Intel>();
        let seen_by_amber = intel.vision(Team::Amber).is_visible(cell_center(FIELD));
        assert_eq!(seen_by_amber, !free_for_all, "free for all: {free_for_all}");
    }
}
