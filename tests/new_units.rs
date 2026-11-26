mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::sim::catalog::{Team, UnitKind};
use rimefront::sim::components::Health;

const FIELD: IVec2 = IVec2::new(20, 45);

fn squad(battle: &mut Battle, kind: UnitKind, team: Team, at: IVec2, count: i32) -> Vec<Entity> {
    (0..count)
        .map(|k| battle.spawn_unit(kind, team, at + IVec2::new(0, k - count / 2)))
        .collect()
}

fn survivors(battle: &mut Battle, units: &[Entity]) -> usize {
    units.iter().filter(|&&unit| battle.alive(unit)).count()
}

#[test]
fn grenadiers_break_a_bunched_rifle_squad_of_the_same_price() {
    let mut battle = Battle::sandbox();
    let grenadiers = squad(&mut battle, UnitKind::Grenadier, Team::Crimson, FIELD, 2);
    let riflemen = squad(
        &mut battle,
        UnitKind::Rifleman,
        Team::Azure,
        FIELD + IVec2::new(4, 0),
        5,
    );
    battle.run_until(40.0, |battle| {
        survivors(battle, &grenadiers) == 0 || survivors(battle, &riflemen) == 0
    });
    assert_eq!(survivors(&mut battle, &riflemen), 0, "the riflemen held");
    assert!(survivors(&mut battle, &grenadiers) > 0);
}

#[test]
fn a_tank_rolls_over_grenadiers() {
    let mut battle = Battle::sandbox();
    let grenadiers = squad(&mut battle, UnitKind::Grenadier, Team::Crimson, FIELD, 3);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD + IVec2::new(5, 0));
    battle.run_until(60.0, |battle| {
        survivors(battle, &grenadiers) == 0 || !battle.alive(tank)
    });
    assert!(battle.alive(tank), "the grenadiers destroyed the tank");
    assert_eq!(survivors(&mut battle, &grenadiers), 0);
}

fn wound(battle: &mut Battle, unit: Entity, share: f32) -> f32 {
    let mut health = battle.world().get_mut::<Health>(unit).unwrap();
    health.current = health.max * share;
    health.current
}

fn health(battle: &mut Battle, unit: Entity) -> f32 {
    battle
        .world()
        .get::<Health>(unit)
        .map_or(0.0, |health| health.current)
}

#[test]
fn a_repair_truck_mends_vehicles_in_the_field_but_not_soldiers() {
    let mut battle = Battle::sandbox();
    battle.spawn_unit(UnitKind::RepairTruck, Team::Azure, FIELD);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD + IVec2::new(4, 0));
    let rifleman = battle.spawn_unit(UnitKind::Rifleman, Team::Azure, FIELD + IVec2::new(-1, 1));
    let tank_before = wound(&mut battle, tank, 0.4);
    let rifleman_before = wound(&mut battle, rifleman, 0.4);
    battle.run_for(12.0);
    assert!(
        health(&mut battle, tank) > tank_before + 50.0,
        "the tank was not repaired"
    );
    assert_eq!(
        health(&mut battle, rifleman),
        rifleman_before,
        "the truck treated a soldier"
    );
}

#[test]
fn a_stealth_tank_keeping_still_blends_into_the_ground() {
    let mut battle = Battle::sandbox();
    let stealth_tank = battle.spawn_unit(UnitKind::StealthTank, Team::Crimson, FIELD);
    battle.run_for(4.0);
    let concealed = battle
        .world()
        .get::<rimefront::sim::abilities::Camouflage>(stealth_tank)
        .is_some_and(|camouflage| camouflage.concealed);
    assert!(concealed, "the stealth tank stayed in plain sight");
}

#[test]
fn a_stealth_tank_caught_in_the_open_falls_to_a_pair_of_battle_tanks() {
    let mut battle = Battle::sandbox();
    let stealth_tank = battle.spawn_unit(UnitKind::StealthTank, Team::Crimson, FIELD);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD + IVec2::new(4, 0));
    let second_tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD + IVec2::new(4, 2));
    battle.run_until(60.0, |battle| {
        !battle.alive(stealth_tank) || !battle.alive(tank) && !battle.alive(second_tank)
    });
    assert!(!battle.alive(stealth_tank));
}

#[test]
fn raider_bikes_run_down_a_howitzer() {
    let mut battle = Battle::sandbox();
    let howitzer = battle.spawn_unit(UnitKind::Artillery, Team::Azure, FIELD);
    let bikes = squad(
        &mut battle,
        UnitKind::RaiderBike,
        Team::Crimson,
        FIELD + IVec2::new(4, 0),
        2,
    );
    battle.run_until(40.0, |battle| {
        !battle.alive(howitzer) || survivors(battle, &bikes) == 0
    });
    assert!(!battle.alive(howitzer), "the howitzer held off the bikes");
}

#[test]
fn riflemen_shoot_raider_bikes_off_the_road() {
    let mut battle = Battle::sandbox();
    let bikes = squad(&mut battle, UnitKind::RaiderBike, Team::Crimson, FIELD, 2);
    let riflemen = squad(
        &mut battle,
        UnitKind::Rifleman,
        Team::Azure,
        FIELD + IVec2::new(4, 0),
        7,
    );
    battle.run_until(40.0, |battle| {
        survivors(battle, &bikes) == 0 || survivors(battle, &riflemen) == 0
    });
    assert_eq!(
        survivors(&mut battle, &bikes),
        0,
        "the bikes beat the riflemen"
    );
}

#[test]
fn a_bomber_strikes_once_then_lands_on_its_helipad_to_rearm() {
    use rimefront::sim::catalog::BuildingKind;
    use rimefront::sim::player_commands::PlayerCommand;
    let mut battle = Battle::sandbox();
    let pad_origin = IVec2::new(12, 50);
    battle.construct(BuildingKind::Helipad, Team::Azure, pad_origin);
    let pad_cell = pad_origin + IVec2::ONE;
    let bomber = battle.spawn_unit(UnitKind::Bomber, Team::Azure, pad_cell);
    let pad = battle.position(bomber);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, pad_cell + IVec2::new(14, -6));
    let full = health(&mut battle, tank);
    let units = vec![battle.sim_id(bomber)];
    let target = battle.sim_id(tank);
    battle.issue(Team::Azure, PlayerCommand::Attack { units, target });

    battle.run_until(30.0, |battle| health(battle, tank) < full);
    assert!(health(&mut battle, tank) < full, "the bombs never fell");
    battle.run_for(25.0);
    let back = battle.position(bomber);
    assert!(
        back.xz().distance(pad.xz()) < 3.0,
        "the bomber did not return to its pad: {back}"
    );
    assert!(
        back.y < pad.y + 1.0,
        "the bomber hangs over the pad instead of landing: {back}"
    );
}

#[test]
fn a_fighter_shoots_down_a_gunship_but_leaves_ground_units_alone() {
    let mut battle = Battle::sandbox();
    let fighter = battle.spawn_unit(UnitKind::Fighter, Team::Azure, FIELD);
    let gunship = battle.spawn_unit(UnitKind::Gunship, Team::Crimson, FIELD + IVec2::new(5, 0));
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD + IVec2::new(3, 2));
    let tank_full = health(&mut battle, tank);
    battle.run_until(30.0, |battle| {
        !battle.alive(gunship) || !battle.alive(fighter)
    });
    assert!(!battle.alive(gunship), "the gunship survived the fighter");
    assert!(battle.alive(fighter));
    assert_eq!(
        health(&mut battle, tank),
        tank_full,
        "the fighter fired at the ground"
    );
}

#[test]
fn a_waiting_fighter_circles_where_it_stopped() {
    let mut battle = Battle::sandbox();
    let fighter = battle.spawn_unit(UnitKind::Fighter, Team::Azure, FIELD);
    battle.run_for(0.5);
    let start = battle.position(fighter);
    let mut farthest: f32 = 0.0;
    for _ in 0..12 {
        battle.run_for(0.5);
        farthest = farthest.max(battle.position(fighter).xz().distance(start.xz()));
    }
    assert!(farthest > 3.0, "the fighter hung in the air");
    assert!(
        farthest < 11.0,
        "the fighter wandered off instead of circling"
    );
}

#[test]
fn a_bomber_lifts_straight_off_its_pad_before_flying_away() {
    use rimefront::sim::catalog::BuildingKind;
    let mut battle = Battle::sandbox();
    let pad_origin = IVec2::new(12, 50);
    battle.construct(BuildingKind::Helipad, Team::Azure, pad_origin);
    let bomber = battle.spawn_unit(UnitKind::Bomber, Team::Azure, pad_origin + IVec2::ONE);
    battle.run_for(1.0);
    let on_pad = battle.position(bomber);
    battle.order_move(bomber, pad_origin + IVec2::new(20, 0));
    battle.run_for(1.0);
    let lifting = battle.position(bomber);
    assert!(
        lifting.y > on_pad.y + 1.0,
        "it did not rise: {on_pad} -> {lifting}"
    );
    assert!(
        lifting.xz().distance(on_pad.xz()) < 0.5,
        "it flew off before climbing: {on_pad} -> {lifting}"
    );
    battle.run_for(4.0);
    assert!(
        battle.position(bomber).xz().distance(on_pad.xz()) > 5.0,
        "it never flew off"
    );
}
