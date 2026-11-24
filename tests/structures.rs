mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::app::savegame::SaveRequests;
use iron_tide::missions::levels::LevelDefinition;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::components::Health;
use iron_tide::sim::structures::{Repairing, StructureOrderKind};
use iron_tide::world::map::{GameMap, footprint_cells};

const FACTORY_SITE: IVec2 = IVec2::new(20, 44);

fn damaged_factory(battle: &mut Battle, health_fraction: f32) -> Entity {
    battle.construct(BuildingKind::PowerPlant, Team::Player, IVec2::new(14, 44));
    battle.construct(BuildingKind::Factory, Team::Player, FACTORY_SITE);
    let factory = battle
        .building(Team::Player, BuildingKind::Factory)
        .unwrap();
    battle.world().get_mut::<Health>(factory).unwrap().current =
        BuildingKind::Factory.spec().max_health * health_fraction;
    factory
}

fn health(battle: &mut Battle, entity: Entity) -> f32 {
    battle
        .world()
        .get::<Health>(entity)
        .map_or(0.0, |h| h.current)
}

#[test]
fn repairing_restores_health_for_credits_and_then_stops() {
    let mut battle = Battle::sandbox();
    let factory = damaged_factory(&mut battle, 0.5);
    battle.set_credits(Team::Player, 5000);
    battle.order_structure(Team::Player, factory, StructureOrderKind::ToggleRepair);
    battle.run_for(30.0);

    let max = BuildingKind::Factory.spec().max_health;
    assert_eq!(health(&mut battle, factory), max);
    assert!(
        battle.world().get::<Repairing>(factory).is_none(),
        "repair kept running"
    );
    let spent = 5000 - battle.credits(Team::Player);
    let quarter_price = BuildingKind::Factory.spec().cost / 4;
    assert!(
        (spent - quarter_price).abs() <= 2,
        "half a repair cost {spent}, expected {quarter_price}"
    );
}

#[test]
fn repairs_pause_without_credits() {
    let mut battle = Battle::sandbox();
    let factory = damaged_factory(&mut battle, 0.5);
    battle.set_credits(Team::Player, 0);
    battle.order_structure(Team::Player, factory, StructureOrderKind::ToggleRepair);
    battle.run_for(5.0);
    let before = health(&mut battle, factory);
    assert_eq!(before, BuildingKind::Factory.spec().max_health * 0.5);

    battle.set_credits(Team::Player, 5000);
    battle.run_for(5.0);
    assert!(
        health(&mut battle, factory) > before,
        "the repair did not resume"
    );
}

#[test]
fn a_second_repair_order_stops_the_repair() {
    let mut battle = Battle::sandbox();
    let factory = damaged_factory(&mut battle, 0.5);
    battle.set_credits(Team::Player, 5000);
    battle.order_structure(Team::Player, factory, StructureOrderKind::ToggleRepair);
    battle.run_for(1.0);
    battle.order_structure(Team::Player, factory, StructureOrderKind::ToggleRepair);
    battle.run_for(1.0);
    let paused = health(&mut battle, factory);
    battle.run_for(5.0);
    assert_eq!(health(&mut battle, factory), paused);
}

#[test]
fn selling_refunds_half_the_price_and_frees_the_ground() {
    let mut battle = Battle::sandbox();
    let footprint = BuildingKind::Factory.spec().footprint;
    let ground_before = passable_cells(&mut battle, footprint);
    let factory = damaged_factory(&mut battle, 1.0);
    battle.set_credits(Team::Player, 0);
    battle.order_structure(Team::Player, factory, StructureOrderKind::Sell);
    battle.run_for(2.0);

    assert!(!battle.alive(factory));
    assert_eq!(
        battle.credits(Team::Player),
        BuildingKind::Factory.spec().cost / 2
    );
    assert_eq!(passable_cells(&mut battle, footprint), ground_before);
}

fn passable_cells(battle: &mut Battle, footprint: IVec2) -> Vec<bool> {
    let map = battle.world().resource::<GameMap>();
    footprint_cells(FACTORY_SITE, footprint)
        .map(|cell| map.is_passable(cell))
        .collect()
}

#[test]
fn the_command_center_and_enemy_structures_cannot_be_sold() {
    let mut battle = Battle::on_level(LevelDefinition::default());
    let own = battle
        .building(Team::Player, BuildingKind::CommandCenter)
        .unwrap();
    let enemy = battle
        .building(Team::Crimson, BuildingKind::CommandCenter)
        .unwrap();
    let credits = battle.credits(Team::Player);
    battle.order_structure(Team::Player, own, StructureOrderKind::Sell);
    battle.order_structure(Team::Player, enemy, StructureOrderKind::Sell);
    battle.run_for(2.0);
    assert!(battle.alive(own) && battle.alive(enemy));
    assert_eq!(battle.credits(Team::Player), credits);
}

#[test]
fn the_computer_repairs_its_damaged_base() {
    let mut battle = Battle::standard();
    let command_center = battle
        .building(Team::Crimson, BuildingKind::CommandCenter)
        .unwrap();
    let max = BuildingKind::CommandCenter.spec().max_health;
    battle
        .world()
        .get_mut::<Health>(command_center)
        .unwrap()
        .current = max * 0.3;
    battle.run_for(20.0);
    assert!(health(&mut battle, command_center) > max * 0.3);
}

#[test]
fn saves_remember_running_repairs() {
    let mut battle = Battle::on_level(LevelDefinition::default());
    let command_center = battle
        .building(Team::Player, BuildingKind::CommandCenter)
        .unwrap();
    let max = BuildingKind::CommandCenter.spec().max_health;
    battle
        .world()
        .get_mut::<Health>(command_center)
        .unwrap()
        .current = max * 0.5;
    battle.order_structure(
        Team::Player,
        command_center,
        StructureOrderKind::ToggleRepair,
    );
    battle.run_for(0.2);
    battle.world().resource_mut::<SaveRequests>().save = Some(1);
    battle.run_for(0.1);
    battle.world().resource_mut::<SaveRequests>().load = Some(1);
    battle.run_for(0.3);

    let restored = battle
        .building(Team::Player, BuildingKind::CommandCenter)
        .unwrap();
    assert!(battle.world().get::<Repairing>(restored).is_some());
}

#[test]
fn a_repair_bay_mends_vehicles_parked_beside_it_but_not_infantry() {
    let mut battle = Battle::sandbox();
    let bay_origin = IVec2::new(14, 50);
    battle.construct(BuildingKind::RepairBay, Team::Player, bay_origin);
    let beside = bay_origin + IVec2::new(2, 0);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, beside);
    let rifleman = battle.spawn_unit(UnitKind::Rifleman, Team::Player, beside + IVec2::new(0, 1));
    for unit in [tank, rifleman] {
        let mut health = battle.world().get_mut::<Health>(unit).unwrap();
        health.current = health.max * 0.5;
    }
    battle.run_for(5.0);
    let share = |battle: &mut Battle, unit| {
        let health = battle.world().get::<Health>(unit).unwrap();
        health.current / health.max
    };
    assert!(share(&mut battle, tank) > 0.65, "the tank was not repaired");
    assert_eq!(share(&mut battle, rifleman), 0.5);
}
