mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::match_stats::MatchStats;
use iron_tide::sim::production::BuildItem;

const FIELD: IVec2 = IVec2::new(20, 45);

#[test]
fn the_tally_counts_kills_losses_damage_and_new_units() {
    let mut battle = Battle::sandbox();
    battle.run_for(1.0);
    battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD);
    let prey = battle.spawn_unit(UnitKind::Harvester, Team::Crimson, FIELD + IVec2::new(4, 0));
    battle.run_until(120.0, |battle| !battle.alive(prey));
    battle.run_for(0.5);

    let stats = battle.world().resource::<MatchStats>();
    let player = *stats.of(Team::Player);
    let crimson = *stats.of(Team::Crimson);
    assert_eq!(player.enemies_destroyed, 1);
    assert_eq!(crimson.units_lost, 1);
    assert!(player.damage_dealt >= UnitKind::Harvester.spec().max_health);
    assert_eq!(crimson.damage_taken, player.damage_dealt);
    assert_eq!(
        player.units_trained, 1,
        "the tank spawned after the start counts as trained"
    );

    battle.construct(BuildingKind::PowerPlant, Team::Player, IVec2::new(6, 47));
    battle.construct(BuildingKind::Barracks, Team::Player, IVec2::new(10, 52));
    battle.set_credits(Team::Player, 1000);
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Rifleman)));
    battle.run_for(UnitKind::Rifleman.spec().build_secs + 1.0);
    let player = *battle.world().resource::<MatchStats>().of(Team::Player);
    assert_eq!(player.units_trained, 2);
    assert_eq!(player.structures_built, 2);
}

#[test]
fn an_attack_on_a_harvester_is_announced_with_its_place() {
    use iron_tide::sim::notifications::{LastAlert, Notifications};
    let mut battle = Battle::sandbox();
    let field = IVec2::new(24, 45);
    battle.spawn_unit(UnitKind::Harvester, Team::Player, field);
    battle.spawn_unit(UnitKind::Rifleman, Team::Crimson, field + IVec2::new(3, 0));
    battle.run_for(4.0);
    let world = battle.world();
    let spot = world
        .resource::<LastAlert>()
        .0
        .expect("no alert was recorded");
    let first_shots_land_near_the_spawn =
        iron_tide::world::map::cell_center(field).distance(spot) < 8.0;
    assert!(first_shots_land_near_the_spawn, "alert at {spot}");
    assert!(
        world
            .resource::<Notifications>()
            .visible()
            .any(|(line, _)| line == "Our harvester is under attack!")
    );
}
