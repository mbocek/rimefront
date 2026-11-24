mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::components::{AttackTarget, Health, Mobile};
use iron_tide::sim::production::BuildItem;
use iron_tide::world::map::{GameMap, cell_center, footprint_center};

const NORTH_BANK: IVec2 = IVec2::new(20, 24);
const SOUTH_BANK: IVec2 = IVec2::new(20, 40);
const FIELD: IVec2 = IVec2::new(20, 45);

fn health(battle: &mut Battle, entity: Entity) -> f32 {
    battle
        .world()
        .get::<Health>(entity)
        .map_or(0.0, |h| h.current)
}

#[test]
fn a_gunship_flies_straight_over_the_river_at_altitude() {
    let mut battle = Battle::sandbox();
    let crossing_blocked = (NORTH_BANK.y..=SOUTH_BANK.y).any(|row| {
        !battle
            .world()
            .resource::<GameMap>()
            .is_passable(IVec2::new(20, row))
    });
    assert!(
        crossing_blocked,
        "the test needs the river between the banks"
    );

    let gunship = battle.spawn_unit(UnitKind::Gunship, Team::Player, NORTH_BANK);
    battle.order_move(gunship, SOUTH_BANK);
    assert_eq!(battle.world().get::<Mobile>(gunship).unwrap().path.len(), 1);
    let flight_secs =
        cell_center(NORTH_BANK).distance(cell_center(SOUTH_BANK)) / UnitKind::Gunship.spec().speed;
    battle.run_for(flight_secs + 3.0);

    let position = battle.position(gunship);
    assert!(
        position.xz().distance(cell_center(SOUTH_BANK).xz()) < 1.5,
        "landed at {position}"
    );
    assert!(position.y > 5.0, "flying at only {} m", position.y);
}

#[test]
fn tanks_cannot_hit_a_gunship_but_rocketeers_can() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD);
    let gunship = battle.spawn_unit(UnitKind::Gunship, Team::Crimson, FIELD + IVec2::new(4, 0));
    battle.run_for(6.0);
    let tank_target = battle
        .world()
        .get::<AttackTarget>(tank)
        .and_then(|a| a.target);
    assert!(tank_target.is_none(), "the tank aimed at an aircraft");
    let full = UnitKind::Gunship.spec().max_health;
    assert_eq!(health(&mut battle, gunship), full);
    assert!(
        health(&mut battle, tank) < UnitKind::Tank.spec().max_health,
        "the gunship did not attack"
    );

    battle.spawn_unit(UnitKind::Rocketeer, Team::Player, FIELD + IVec2::new(1, 2));
    battle.run_for(6.0);
    assert!(
        health(&mut battle, gunship) < full,
        "rocketeers could not hit the gunship"
    );
}

#[test]
fn gunships_come_from_the_helipad_and_are_repaired_over_it() {
    let mut battle = Battle::sandbox();
    let pad_origin = IVec2::new(14, 50);
    battle.construct(BuildingKind::Helipad, Team::Player, pad_origin);
    battle.construct(
        BuildingKind::TechCenter,
        Team::Player,
        pad_origin + IVec2::new(0, 4),
    );
    battle.set_credits(Team::Player, 5000);
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Gunship)));
    battle.run_until(40.0, |battle| {
        !battle
            .units(Team::Player, Some(UnitKind::Gunship))
            .is_empty()
    });
    let gunship = battle.units(Team::Player, Some(UnitKind::Gunship))[0];
    let pad_centre = footprint_center(pad_origin, BuildingKind::Helipad.spec().footprint);
    let launch_distance = battle.position(gunship).xz().distance(pad_centre.xz());
    assert!(
        launch_distance < UnitKind::Gunship.spec().speed + 1.0,
        "took off {launch_distance} m away from the pad"
    );

    battle.order_move(gunship, pad_origin);
    battle.run_for(3.0);
    let full = UnitKind::Gunship.spec().max_health;
    battle.world().get_mut::<Health>(gunship).unwrap().current = full * 0.5;
    battle.run_for(5.0);
    let mended = health(&mut battle, gunship);
    assert!(
        mended > full * 0.6,
        "only {mended} of {full} after five seconds over the pad"
    );
}

#[test]
fn a_flak_cannon_shoots_down_gunships_and_ignores_ground_units() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::FlakCannon, Team::Player, FIELD);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD + IVec2::new(4, 0));
    if let Some(mut mobile) = battle.world().get_mut::<Mobile>(tank) {
        mobile.stop();
    }
    battle.run_for(4.0);
    assert_eq!(
        health(&mut battle, tank),
        UnitKind::Tank.spec().max_health,
        "the flak cannon fired at a tank"
    );
    battle.despawn_units(Team::Crimson);

    let gunship = battle.spawn_unit(UnitKind::Gunship, Team::Crimson, FIELD + IVec2::new(4, 2));
    battle.run_until(15.0, |battle| !battle.alive(gunship));
    assert!(!battle.alive(gunship), "the gunship survived the flak");
}

#[test]
fn a_flak_track_beats_a_gunship() {
    let mut battle = Battle::sandbox();
    let flak = battle.spawn_unit(UnitKind::FlakTrack, Team::Player, FIELD);
    let gunship = battle.spawn_unit(UnitKind::Gunship, Team::Crimson, FIELD + IVec2::new(5, 0));
    battle.run_until(40.0, |battle| !battle.alive(gunship) || !battle.alive(flak));
    assert!(battle.alive(flak), "the gunship won against its counter");
    assert!(!battle.alive(gunship));
}
