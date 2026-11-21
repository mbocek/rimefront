mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use iron_tide::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::components::{Building, Health, Stance};
use iron_tide::levels::LevelDefinition;
use iron_tide::player_commands::PlayerCommand;

const POST: IVec2 = IVec2::new(20, 45);

/// Without fog, so a target out of reach is still seen and only the stance keeps the tank from it.
fn open_field() -> Battle {
    Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(LevelDefinition {
            fog_of_war: false,
            ..LevelDefinition::default()
        }),
    })
}

fn health(battle: &mut Battle, entity: Entity) -> f32 {
    battle
        .world()
        .get::<Health>(entity)
        .map_or(0.0, |health| health.current)
}

/// An enemy Power Plant neither moves nor shoots back, so only the stance decides the fight.
fn target_at(battle: &mut Battle, offset: IVec2) -> Entity {
    battle.construct(BuildingKind::PowerPlant, Team::Crimson, POST + offset);
    battle.run_for(0.1);
    let world = battle.world();
    world
        .query::<(Entity, &Building)>()
        .iter(world)
        .find(|(_, building)| building.origin == POST + offset)
        .map(|(entity, _)| entity)
        .expect("the Power Plant stands")
}

fn tank_with(battle: &mut Battle, stance: Stance) -> Entity {
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, POST);
    let units = vec![battle.sim_id(tank)];
    battle.issue(Team::Player, PlayerCommand::SetStance { units, stance });
    battle.run_for(0.2);
    assert_eq!(battle.world().get::<Stance>(tank), Some(&stance));
    tank
}

fn untouched(battle: &mut Battle, target: Entity) -> bool {
    health(battle, target) == BuildingKind::PowerPlant.spec().max_health
}

/// Within sight but beyond the tank's reach, so only a unit that chases reaches it.
const BEYOND_REACH: IVec2 = IVec2::new(0, 8);
const WITHIN_REACH: IVec2 = IVec2::new(0, 3);

#[test]
fn an_aggressive_unit_chases_what_it_sees() {
    let mut battle = open_field();
    let tank = tank_with(&mut battle, Stance::Aggressive);
    let target = target_at(&mut battle, BEYOND_REACH);
    let gap = |battle: &mut Battle| battle.position(tank).distance(battle.position(target));
    let before = gap(&mut battle);
    battle.run_for(8.0);
    assert!(!untouched(&mut battle, target));
    assert!(gap(&mut battle) < before - 2.0, "it did not close in");
}

#[test]
fn a_unit_holding_its_position_fires_within_reach_and_stays_put() {
    let mut battle = open_field();
    let tank = tank_with(&mut battle, Stance::HoldPosition);
    let start = battle.position(tank);
    let far = target_at(&mut battle, BEYOND_REACH);
    battle.run_for(8.0);
    assert!(
        untouched(&mut battle, far),
        "it chased an enemy out of reach"
    );
    assert!(
        battle.position(tank).xz().distance(start.xz()) < 0.5,
        "it left its post"
    );
    let near = target_at(&mut battle, WITHIN_REACH);
    battle.run_for(8.0);
    assert!(
        !untouched(&mut battle, near),
        "it did not fire within reach"
    );
}

#[test]
fn a_unit_holding_fire_shoots_only_when_ordered() {
    let mut battle = open_field();
    let tank = tank_with(&mut battle, Stance::HoldFire);
    let target = target_at(&mut battle, WITHIN_REACH);
    battle.run_for(8.0);
    assert!(untouched(&mut battle, target), "it fired without an order");
    let units = vec![battle.sim_id(tank)];
    let ordered = battle.sim_id(target);
    battle.issue(
        Team::Player,
        PlayerCommand::Attack {
            units,
            target: ordered,
        },
    );
    battle.run_for(8.0);
    assert!(
        !untouched(&mut battle, target),
        "it ignored the order to attack"
    );
}
