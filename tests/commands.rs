mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::determinism::simulation_checksum;
use iron_tide::sim::player_commands::{GroundOrder, PlayerCommand};
use iron_tide::sim::production::BuildItem;
use iron_tide::world::map::{cell_center, player_base_cell};

const FIELD: IVec2 = IVec2::new(20, 45);

fn move_order(battle: &mut Battle, unit: Entity, goal: IVec2) -> PlayerCommand {
    PlayerCommand::Ground {
        units: vec![battle.sim_id(unit)],
        goal,
        order: GroundOrder::Move,
        queued: false,
        refinery: None,
    }
}

#[test]
fn a_command_moves_only_the_issuing_teams_units() {
    let mut battle = Battle::sandbox();
    let own = battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD);
    let enemy = battle.spawn_unit(UnitKind::Tank, Team::Crimson, FIELD + IVec2::new(-12, 12));
    let goal = FIELD + IVec2::new(6, 0);
    let order = move_order(&mut battle, own, goal);
    battle.issue(Team::Player, order);
    let stolen = move_order(&mut battle, enemy, goal);
    battle.issue(Team::Player, stolen);
    let enemy_start = battle.position(enemy);
    battle.run_for(8.0);
    assert!(battle.position(own).distance(cell_center(goal)) < 2.5);
    assert!(
        battle.position(enemy).distance(enemy_start) < 0.5,
        "the player moved an enemy tank"
    );
}

#[test]
fn production_and_placement_go_through_commands() {
    let mut battle = Battle::sandbox();
    let base = player_base_cell();
    battle.construct(BuildingKind::CommandCenter, Team::Player, base);
    battle.set_credits(Team::Player, 5000);
    battle.issue(
        Team::Player,
        PlayerCommand::Enqueue {
            item: BuildItem::Structure(BuildingKind::PowerPlant),
            copies: 1,
        },
    );
    battle.run_for(BuildingKind::PowerPlant.spec().build_secs + 1.0);
    battle.issue(
        Team::Player,
        PlayerCommand::PlaceStructure {
            kind: BuildingKind::PowerPlant,
            origin: base + IVec2::new(4, 0),
        },
    );
    battle.run_for(1.0);
    assert!(
        battle
            .building(Team::Player, BuildingKind::PowerPlant)
            .is_some()
    );
}

#[test]
fn the_same_commands_on_the_same_ticks_replay_the_same_battle() {
    let play = |with_orders: bool| {
        let mut battle = Battle::standard();
        battle.run_for(5.0);
        if with_orders {
            let tanks = battle.units(Team::Player, Some(UnitKind::Tank));
            let units = tanks.iter().map(|&tank| battle.sim_id(tank)).collect();
            battle.issue(
                Team::Player,
                PlayerCommand::Ground {
                    units,
                    goal: IVec2::new(32, 32),
                    order: GroundOrder::AttackMove,
                    queued: false,
                    refinery: None,
                },
            );
        }
        battle.run_for(90.0);
        simulation_checksum(battle.world())
    };
    let first = play(true);
    assert_eq!(first, play(true));
    assert_ne!(first, play(false), "the orders changed nothing");
}

fn group_order(
    battle: &mut Battle,
    units: &[Entity],
    goal: IVec2,
    order: GroundOrder,
) -> PlayerCommand {
    PlayerCommand::Ground {
        units: units.iter().map(|&unit| battle.sim_id(unit)).collect(),
        goal,
        order,
        queued: false,
        refinery: None,
    }
}

#[test]
fn a_group_move_keeps_to_the_pace_of_its_slowest_unit() {
    const SECS: f32 = 4.0;
    let tank_speed = UnitKind::Tank.spec().speed;
    let scout_speed = UnitKind::Scout.spec().speed;
    assert!(scout_speed > tank_speed * 1.5);
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD);
    let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, FIELD + IVec2::new(0, 2));
    let goal = FIELD + IVec2::new(20, 0);
    let start = battle.position(scout);
    let order = group_order(&mut battle, &[tank, scout], goal, GroundOrder::Move);
    battle.issue(Team::Player, order);
    battle.run_for(SECS);
    let in_the_group = battle.position(scout).distance(start);
    assert!(
        in_the_group <= tank_speed * SECS + 0.5,
        "the scout ran ahead of the tank's pace: {in_the_group} m in {SECS} s"
    );

    let start = battle.position(scout);
    let alone = group_order(
        &mut battle,
        &[scout],
        goal + IVec2::new(0, 12),
        GroundOrder::Move,
    );
    battle.issue(Team::Player, alone);
    battle.run_for(SECS);
    let on_its_own = battle.position(scout).distance(start);
    assert!(
        on_its_own > tank_speed * SECS * 1.2,
        "a scout sent alone keeps its own speed, moved {on_its_own} m"
    );
}

#[test]
fn an_attack_move_group_keeps_its_pace_until_it_meets_the_enemy() {
    const SECS: f32 = 4.0;
    let tank_speed = UnitKind::Tank.spec().speed;
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD);
    let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, FIELD + IVec2::new(0, 2));
    let start = battle.position(scout);
    let order = group_order(
        &mut battle,
        &[tank, scout],
        FIELD + IVec2::new(20, 0),
        GroundOrder::AttackMove,
    );
    battle.issue(Team::Player, order);
    battle.run_for(SECS);
    let travelled = battle.position(scout).distance(start);
    assert!(
        travelled <= tank_speed * SECS + 0.5,
        "the scout left the tank behind on an attack-move: {travelled} m"
    );
}
