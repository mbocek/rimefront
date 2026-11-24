mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::sim::catalog::{Team, UnitKind};
use iron_tide::sim::components::Health;
use iron_tide::world::map::CELL_SIZE;

const FIELD: IVec2 = IVec2::new(20, 45);

#[test]
fn a_sniper_drops_infantry_beyond_rifle_range_but_barely_scratches_armour() {
    let mut battle = Battle::sandbox();
    let sniper = battle.spawn_unit(UnitKind::Sniper, Team::Player, FIELD);
    let rifle_range = UnitKind::Rifleman.spec().weapon.unwrap().range;
    let gap_cells = (rifle_range / CELL_SIZE).ceil() as i32 + 1;
    let rifleman = battle.spawn_unit(
        UnitKind::Rifleman,
        Team::Crimson,
        FIELD + IVec2::new(gap_cells, 0),
    );
    battle.run_until(8.0, |battle| !battle.alive(rifleman));
    assert!(!battle.alive(rifleman), "the rifleman survived");
    let sniper_health = battle.world().get::<Health>(sniper).unwrap().current;
    assert_eq!(sniper_health, UnitKind::Sniper.spec().max_health);

    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD + IVec2::new(0, 6));
    battle.despawn_units(Team::Player);
    battle.spawn_unit(UnitKind::Sniper, Team::Player, FIELD);
    battle.run_for(6.0);
    let tank_health = battle
        .world()
        .get::<Health>(tank)
        .map_or(0.0, |h| h.current);
    assert!(
        tank_health > UnitKind::Tank.spec().max_health * 0.95,
        "a sniper should not hurt a tank much, left {tank_health}"
    );
}
