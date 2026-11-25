mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::sim::abilities::{Camouflage, MINE_COST, Mine};
use rimefront::sim::catalog::{Team, UnitKind};
use rimefront::sim::components::Mobile;
use rimefront::sim::components::{Health, Weapon};
use rimefront::sim::movement::order_move;
use rimefront::sim::player_commands::PlayerCommand;
use rimefront::world::map::GameMap;

const FIELD: IVec2 = IVec2::new(24, 45);

fn health(battle: &mut Battle, unit: Entity) -> f32 {
    battle
        .world()
        .get::<Health>(unit)
        .map_or(0.0, |health| health.current)
}

fn mines(battle: &mut Battle) -> usize {
    let world = battle.world();
    world.query::<&Mine>().iter(world).count()
}

fn drive(battle: &mut Battle, unit: Entity, goal: IVec2) {
    let world = battle.world();
    let from = world.get::<Transform>(unit).unwrap().translation;
    world.resource_scope(|world, map: Mut<GameMap>| {
        let mut mobile = world.get_mut::<Mobile>(unit).unwrap();
        order_move(&mut mobile, &map, from, goal, true);
    });
}

fn bury_a_mine_at_the_field(battle: &mut Battle) {
    battle.set_credits(Team::Player, 1000);
    let engineer = battle.spawn_unit(UnitKind::Engineer, Team::Player, FIELD);
    let id = battle.sim_id(engineer);
    battle.issue(Team::Player, PlayerCommand::LayMines { units: vec![id] });
    battle.run_for(2.0);
    battle.world().entity_mut(engineer).despawn();
}

#[test]
fn an_engineer_buries_a_mine_that_blows_up_under_an_enemy_tank() {
    let mut battle = Battle::sandbox();
    bury_a_mine_at_the_field(&mut battle);
    assert_eq!(mines(&mut battle), 1);
    assert_eq!(battle.credits(Team::Player), 1000 - MINE_COST);

    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD + IVec2::new(6, 0));
    battle.world().entity_mut(tank).remove::<Weapon>();
    drive(&mut battle, tank, FIELD - IVec2::new(4, 0));
    battle.run_for(8.0);
    assert_eq!(mines(&mut battle), 0, "the mine never went off");
    assert!(health(&mut battle, tank) < UnitKind::Tank.spec().max_health * 0.9);
}

#[test]
fn own_vehicles_drive_over_their_mines() {
    let mut battle = Battle::sandbox();
    bury_a_mine_at_the_field(&mut battle);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD + IVec2::new(6, 0));
    drive(&mut battle, tank, FIELD - IVec2::new(4, 0));
    battle.run_for(8.0);
    assert_eq!(mines(&mut battle), 1);
    assert_eq!(health(&mut battle, tank), UnitKind::Tank.spec().max_health);
}

#[test]
fn a_still_sniper_hides_from_a_distant_tank_until_an_enemy_comes_close() {
    let mut battle = Battle::sandbox();
    let sniper = battle.spawn_unit(UnitKind::Sniper, Team::Player, FIELD);
    battle.world().entity_mut(sniper).remove::<Weapon>();
    battle.run_for(4.0);
    battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD + IVec2::new(5, 0));
    battle.run_for(6.0);
    let full = UnitKind::Sniper.spec().max_health;
    assert_eq!(
        health(&mut battle, sniper),
        full,
        "the tank saw through the camouflage"
    );

    battle.spawn_unit(UnitKind::Rifleman, Team::Crimson, FIELD + IVec2::new(0, 2));
    battle.run_for(6.0);
    assert!(
        health(&mut battle, sniper) < full,
        "a rifleman next to it did not spot it"
    );
}

#[test]
fn without_camouflage_the_same_tank_finds_the_sniper() {
    let mut battle = Battle::sandbox();
    let sniper = battle.spawn_unit(UnitKind::Sniper, Team::Player, FIELD);
    battle
        .world()
        .entity_mut(sniper)
        .remove::<(Weapon, Camouflage)>();
    battle.run_for(4.0);
    battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD + IVec2::new(5, 0));
    battle.run_for(6.0);
    assert!(health(&mut battle, sniper) < UnitKind::Sniper.spec().max_health);
}

#[test]
fn the_computer_mines_the_approach_to_its_base_once_it_has_money_to_spare() {
    let mut battle = Battle::standard();
    battle.set_credits(Team::Crimson, 20_000);
    battle.run_until(600.0, |battle| {
        let world = battle.world();
        world
            .query::<&Mine>()
            .iter(world)
            .any(|mine| mine.team == Team::Crimson)
    });
    let world = battle.world();
    let laid = world
        .query::<&Mine>()
        .iter(world)
        .filter(|mine| mine.team == Team::Crimson)
        .count();
    assert!(laid > 0, "no mines after ten minutes");
}

#[test]
fn buried_mines_survive_saving_and_loading() {
    use rimefront::app::savegame::SaveRequests;
    let mut battle = Battle::standard();
    bury_a_mine_at_the_field(&mut battle);
    assert_eq!(mines(&mut battle), 1);
    battle.world().resource_mut::<SaveRequests>().save = Some(1);
    battle.run_for(0.1);
    battle.world().resource_mut::<SaveRequests>().load = Some(1);
    battle.run_for(0.3);
    let world = battle.world();
    let restored: Vec<Mine> = world.query::<&Mine>().iter(world).copied().collect();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].team, Team::Player);
    assert_eq!(restored[0].cell, FIELD);
}
