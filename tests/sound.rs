mod common;
use iron_tide::sim::structures::StructureOrderKind;

use bevy::prelude::*;
use common::Battle;
use iron_tide::audio::sound::Sound;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::components::Health;
use iron_tide::sim::production::BuildItem;

const OPEN_FIELD: IVec2 = IVec2::new(30, 45);

#[test]
fn every_weapon_fires_with_its_own_report() {
    let mut battle = Battle::sandbox();
    battle.spawn_unit(UnitKind::Tank, Team::Player, OPEN_FIELD);
    battle.spawn_unit(
        UnitKind::Rifleman,
        Team::Player,
        OPEN_FIELD + IVec2::new(0, 2),
    );
    battle.spawn_unit(
        UnitKind::Rocketeer,
        Team::Player,
        OPEN_FIELD + IVec2::new(0, -2),
    );
    battle.spawn_unit(
        UnitKind::Bulwark,
        Team::Crimson,
        OPEN_FIELD + IVec2::new(4, 0),
    );
    battle.run_for(4.0);
    for report in [
        Sound::CannonShot,
        Sound::RifleShot,
        Sound::MissileLaunch,
        Sound::HeavyCannonShot,
    ] {
        assert!(battle.heard(report) > 0, "never heard {report:?}");
    }
    assert!(
        battle.heard(Sound::ShellExplosion) > 0,
        "shells landed silently"
    );
}

#[test]
fn destruction_is_heard() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, OPEN_FIELD);
    battle.construct(
        BuildingKind::PowerPlant,
        Team::Crimson,
        OPEN_FIELD + IVec2::new(6, 0),
    );
    let world = battle.world();
    world.get_mut::<Health>(tank).unwrap().current = 0.0;
    let plant = world
        .query_filtered::<Entity, With<iron_tide::sim::components::Building>>()
        .single(world)
        .unwrap();
    world.get_mut::<Health>(plant).unwrap().current = 0.0;
    battle.run_for(0.2);
    assert_eq!(battle.heard(Sound::VehicleExplosion), 1);
    assert_eq!(battle.heard(Sound::BuildingCollapse), 1);
}

#[test]
fn finished_production_is_announced_only_for_the_player() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::PowerPlant, Team::Player, IVec2::new(6, 47));
    battle.construct(BuildingKind::Barracks, Team::Player, IVec2::new(10, 52));
    battle.construct(BuildingKind::PowerPlant, Team::Crimson, IVec2::new(50, 10));
    battle.construct(BuildingKind::Barracks, Team::Crimson, IVec2::new(54, 12));
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Rifleman)));
    assert!(battle.enqueue(Team::Crimson, BuildItem::Unit(UnitKind::Rifleman)));
    battle.run_for(UnitKind::Rifleman.spec().build_secs + 1.0);
    assert_eq!(battle.heard(Sound::UnitReady), 1);
}

#[test]
fn harvester_deliveries_jingle() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::Refinery, Team::Player, IVec2::new(11, 51));
    let took = battle.run_until(180.0, |battle| battle.heard(Sound::Credits) > 0);
    assert!(took.is_some(), "a full load was delivered without a sound");
}

#[test]
fn selling_and_repairing_have_their_own_sounds() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::PowerPlant, Team::Player, IVec2::new(20, 44));
    let plant = battle
        .building(Team::Player, BuildingKind::PowerPlant)
        .unwrap();
    battle.world().get_mut::<Health>(plant).unwrap().current = 100.0;
    battle.set_credits(Team::Player, 5000);
    battle.order_structure(Team::Player, plant, StructureOrderKind::ToggleRepair);
    battle.run_for(0.2);
    assert_eq!(battle.heard(Sound::RepairStarted), 1);
    battle.order_structure(Team::Player, plant, StructureOrderKind::Sell);
    battle.run_for(0.2);
    assert_eq!(battle.heard(Sound::StructureSold), 1);
}
