mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::missions::levels::{BaseSite, Difficulty};
use iron_tide::sim::catalog::{Team, UnitKind};
use iron_tide::sim::components::{Health, Mobile, Order, Orders};

#[test]
fn soldiers_at_home_rush_to_a_harvester_under_fire() {
    let mut battle = Battle::sandbox();
    let base = BaseSite::NorthEast.origin();
    battle.add_ai(Team::Crimson, base, Difficulty::Normal);
    let guard = battle.spawn_unit(UnitKind::Tank, Team::Crimson, base + IVec2::new(-3, 4));
    let harvester = battle.spawn_unit(
        UnitKind::Harvester,
        Team::Crimson,
        base + IVec2::new(-24, 12),
    );
    battle.spawn_unit(UnitKind::Rifleman, Team::Player, base + IVec2::new(-26, 12));
    battle.run_until(5.0, |battle| {
        let world = battle.world();
        let ordered = world
            .get::<Orders>(guard)
            .is_some_and(|orders| matches!(orders.current, Some(Order::AttackMove(_))));
        ordered
            || world
                .get::<Mobile>(guard)
                .is_some_and(|mobile| mobile.is_moving())
    });
    let world = battle.world();
    let harvester_hit = world
        .get::<Health>(harvester)
        .is_none_or(|health| health.current < health.max);
    assert!(harvester_hit, "nobody shot the harvester");
    assert!(
        world.get::<Mobile>(guard).unwrap().is_moving(),
        "the tank stayed home"
    );
}

#[test]
fn a_rocketeer_picks_the_tank_over_a_closer_rifleman() {
    let mut battle = Battle::sandbox();
    let field = IVec2::new(24, 45);
    let rocketeer = battle.spawn_unit(UnitKind::Rocketeer, Team::Player, field);
    battle.spawn_unit(UnitKind::Rifleman, Team::Crimson, field + IVec2::new(3, 0));
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, field + IVec2::new(4, 1));
    battle.run_for(1.0);
    let target = battle
        .world()
        .get::<iron_tide::sim::components::AttackTarget>(rocketeer)
        .and_then(|attack| attack.entity());
    assert_eq!(target, Some(tank));
}
