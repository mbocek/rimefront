mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use iron_tide::missions::levels::LevelDefinition;
use iron_tide::sim::catalog::{BuildingKind, Team, UnitKind};
use iron_tide::sim::combat::{DamageSource, Targets};
use iron_tide::sim::components::{Building, Health, Stance};
use iron_tide::sim::player_commands::PlayerCommand;
use iron_tide::sim::transport::{CARRIER_CAPACITY, Embarked, Passengers};
use iron_tide::world::map::{GameMap, world_to_cell};

const DEPOT: IVec2 = IVec2::new(20, 45);

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

fn squad(battle: &mut Battle, count: i32) -> Vec<Entity> {
    (0..count)
        .map(|k| {
            battle.spawn_unit(
                UnitKind::Rifleman,
                Team::Player,
                DEPOT + IVec2::new(k % 3, 3 + k / 3),
            )
        })
        .collect()
}

fn board(battle: &mut Battle, soldiers: &[Entity], carrier: Entity) {
    let units = soldiers
        .iter()
        .map(|&soldier| battle.sim_id(soldier))
        .collect();
    let carrier = battle.sim_id(carrier);
    battle.issue(Team::Player, PlayerCommand::Board { units, carrier });
    battle.run_for(10.0);
}

fn riding(battle: &mut Battle, carrier: Entity) -> usize {
    battle
        .world()
        .get::<Passengers>(carrier)
        .map_or(0, |riding| riding.0.len())
}

fn embarked(battle: &mut Battle, soldier: Entity) -> bool {
    battle.world().get::<Embarked>(soldier).is_some()
}

#[test]
fn infantry_climbs_in_and_cannot_be_hit_inside() {
    let mut battle = open_field();
    let carrier = battle.spawn_unit(UnitKind::TroopCarrier, Team::Player, DEPOT);
    let soldiers = squad(&mut battle, 3);
    board(&mut battle, &soldiers, carrier);
    assert_eq!(riding(&mut battle, carrier), 3);
    let targets = battle.world().resource::<Targets>();
    for soldier in &soldiers {
        assert!(
            targets.get(*soldier).is_none(),
            "a passenger can be targeted"
        );
    }
}

#[test]
fn a_carrier_takes_at_most_its_capacity() {
    let mut battle = open_field();
    let carrier = battle.spawn_unit(UnitKind::TroopCarrier, Team::Player, DEPOT);
    let soldiers = squad(&mut battle, 7);
    board(&mut battle, &soldiers, carrier);
    assert_eq!(riding(&mut battle, carrier), CARRIER_CAPACITY);
    let left_outside = soldiers
        .iter()
        .filter(|&&soldier| !embarked(&mut battle, soldier))
        .count();
    assert_eq!(left_outside, soldiers.len() - CARRIER_CAPACITY);
}

#[test]
fn passengers_ride_along_and_get_out_on_free_ground() {
    let mut battle = open_field();
    let carrier = battle.spawn_unit(UnitKind::TroopCarrier, Team::Player, DEPOT);
    let soldiers = squad(&mut battle, 4);
    board(&mut battle, &soldiers, carrier);
    battle.order_move(carrier, DEPOT + IVec2::new(12, 0));
    battle.run_for(10.0);
    let carrier_position = battle.position(carrier);
    for &soldier in &soldiers {
        assert!(
            battle
                .position(soldier)
                .xz()
                .distance(carrier_position.xz())
                < 0.01
        );
    }
    let carriers = vec![battle.sim_id(carrier)];
    battle.issue(Team::Player, PlayerCommand::Unload { carriers });
    battle.run_for(0.5);
    assert_eq!(riding(&mut battle, carrier), 0);
    let mut cells = Vec::new();
    for &soldier in &soldiers {
        assert!(!embarked(&mut battle, soldier));
        let cell = world_to_cell(battle.position(soldier));
        assert!(battle.world().resource::<GameMap>().is_passable(cell));
        cells.push(cell);
    }
    cells.sort_by_key(|cell| (cell.x, cell.y));
    cells.dedup();
    assert_eq!(
        cells.len(),
        soldiers.len(),
        "soldiers got out on top of each other"
    );
}

#[test]
fn passengers_fire_from_the_hatches() {
    let mut battle = open_field();
    let carrier = battle.spawn_unit(UnitKind::TroopCarrier, Team::Player, DEPOT);
    let soldiers = squad(&mut battle, 3);
    board(&mut battle, &soldiers, carrier);
    let units = vec![battle.sim_id(carrier)];
    battle.issue(
        Team::Player,
        PlayerCommand::SetStance {
            units,
            stance: Stance::HoldFire,
        },
    );
    battle.construct(
        BuildingKind::PowerPlant,
        Team::Crimson,
        DEPOT + IVec2::new(0, -4),
    );
    battle.run_for(6.0);
    let by_riflemen = battle
        .ledger()
        .dealt
        .get(&(Team::Player, DamageSource::Unit(UnitKind::Rifleman)))
        .copied()
        .unwrap_or(0.0);
    assert!(by_riflemen > 0.0, "the passengers did not fire");
    let by_carrier = battle
        .ledger()
        .dealt
        .get(&(Team::Player, DamageSource::Unit(UnitKind::TroopCarrier)))
        .copied()
        .unwrap_or(0.0);
    assert_eq!(by_carrier, 0.0, "the carrier fired while holding fire");
}

#[test]
fn passengers_escape_a_destroyed_carrier_hurt() {
    let mut battle = open_field();
    let carrier = battle.spawn_unit(UnitKind::TroopCarrier, Team::Player, DEPOT);
    let soldiers = squad(&mut battle, 2);
    board(&mut battle, &soldiers, carrier);
    battle.world().get_mut::<Health>(carrier).unwrap().current = 0.0;
    battle.run_for(0.5);
    assert!(!battle.alive(carrier));
    let full = UnitKind::Rifleman.spec().max_health;
    for &soldier in &soldiers {
        assert!(battle.alive(soldier) && !embarked(&mut battle, soldier));
        let health = battle.world().get::<Health>(soldier).unwrap().current;
        assert!(health <= full * 0.5 + 0.01, "escaped unhurt with {health}");
    }
    let world = battle.world();
    assert_eq!(world.query::<&Building>().iter(world).count(), 0);
}
