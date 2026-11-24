mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::components::{Building, Health};
use iron_tide::sim::engineers::{EngineerJob, EngineerTask};
use iron_tide::sim::production::BuildItem;

const PLANT_SITE: IVec2 = IVec2::new(30, 46);

fn send(battle: &mut Battle, engineer: Entity, building: Entity, job: EngineerJob) {
    battle
        .world()
        .entity_mut(engineer)
        .insert(EngineerTask::new(building, job));
}

fn owner_of_plant(battle: &mut Battle) -> Option<Team> {
    let world = battle.world();
    world
        .query::<(&Building, &Team)>()
        .iter(world)
        .find(|(building, _)| building.origin == PLANT_SITE)
        .map(|(_, &team)| team)
}

#[test]
fn an_engineer_captures_an_enemy_structure_and_is_used_up() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::PowerPlant, Team::Crimson, PLANT_SITE);
    let plant = battle
        .building(Team::Crimson, BuildingKind::PowerPlant)
        .unwrap();
    let max = BuildingKind::PowerPlant.spec().max_health;
    battle.world().get_mut::<Health>(plant).unwrap().current = max * 0.4;
    let engineer = battle.spawn_unit(
        UnitKind::Engineer,
        Team::Player,
        PLANT_SITE + IVec2::new(-8, 0),
    );
    send(&mut battle, engineer, plant, EngineerJob::Capture);

    let took = battle.run_until(30.0, |battle| owner_of_plant(battle) == Some(Team::Player));
    assert!(took.is_some(), "the structure was never captured");
    assert!(!battle.alive(engineer));
    let captured = battle
        .building(Team::Player, BuildingKind::PowerPlant)
        .unwrap();
    let health = battle.world().get::<Health>(captured).unwrap().current;
    assert!(
        (health - max * 0.4).abs() < 1.0,
        "the damage was not kept: {health}"
    );
}

#[test]
fn an_engineer_fully_repairs_an_own_structure() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::PowerPlant, Team::Player, PLANT_SITE);
    let plant = battle
        .building(Team::Player, BuildingKind::PowerPlant)
        .unwrap();
    let max = BuildingKind::PowerPlant.spec().max_health;
    battle.world().get_mut::<Health>(plant).unwrap().current = max * 0.2;
    let engineer = battle.spawn_unit(
        UnitKind::Engineer,
        Team::Player,
        PLANT_SITE + IVec2::new(-8, 0),
    );
    send(&mut battle, engineer, plant, EngineerJob::Repair);

    battle.run_until(30.0, |battle| !battle.alive(engineer));
    assert!(!battle.alive(engineer), "the engineer never went in");
    assert_eq!(battle.world().get::<Health>(plant).unwrap().current, max);
}

#[test]
fn barracks_train_engineers() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::PowerPlant, Team::Player, IVec2::new(6, 47));
    battle.construct(BuildingKind::Barracks, Team::Player, IVec2::new(10, 52));
    battle.set_credits(Team::Player, 2000);
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Engineer)));
    battle.run_for(UnitKind::Engineer.spec().build_secs + 2.0);
    assert_eq!(
        battle.units(Team::Player, Some(UnitKind::Engineer)).len(),
        1
    );
}
