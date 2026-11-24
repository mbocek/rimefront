mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::sim::catalog::{Team, UnitKind};
use iron_tide::sim::combat::Targets;
use iron_tide::sim::sides::admit_side;

#[test]
fn a_side_that_joins_sees_for_itself_and_fights_everyone() {
    let mut battle = Battle::sandbox();
    let newcomer = admit_side(battle.world());
    assert_eq!(newcomer, Team::from_index(4));
    let tank = battle.spawn_unit(UnitKind::Tank, newcomer, IVec2::new(30, 45));
    let target = battle.spawn_unit(UnitKind::Rifleman, Team::Player, IVec2::new(33, 45));
    battle.run_for(0.5);
    let seen = battle
        .world()
        .resource::<Targets>()
        .get(target)
        .is_some_and(|target| target.seen_by(newcomer));
    assert!(seen, "the newcomer's tank sees nothing");

    battle.run_for(10.0);
    assert!(battle.alive(tank));
    assert!(!battle.alive(target), "the newcomer never fought");
    assert!(battle.damage_taken(Team::Player) > 0.0);
}
