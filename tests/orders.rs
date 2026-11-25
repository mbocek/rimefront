mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::app::savegame::SaveRequests;
use rimefront::missions::levels::LevelDefinition;
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::components::{Building, Health, Order, Orders, RallyPoint};
use rimefront::sim::production::BuildItem;
use rimefront::world::map::cell_center;

const START: IVec2 = IVec2::new(20, 45);
const DESTINATION: IVec2 = IVec2::new(40, 45);

fn give(battle: &mut Battle, unit: Entity, orders: &[Order]) {
    let mut queue = battle
        .world()
        .get_mut::<Orders>(unit)
        .expect("units take orders");
    queue.clear();
    for &order in orders {
        queue.queue(order);
    }
}

fn near(battle: &mut Battle, unit: Entity, cell: IVec2) -> bool {
    battle.position(unit).xz().distance(cell_center(cell).xz()) < 3.5
}

fn health(battle: &mut Battle, entity: Entity) -> f32 {
    battle
        .world()
        .get::<Health>(entity)
        .map_or(0.0, |health| health.current)
}

/// An unarmed enemy harvester parked beside the route: it never shoots back, so only the order decides the fight.
fn bystander(battle: &mut Battle) -> Entity {
    battle.spawn_unit(UnitKind::Harvester, Team::Crimson, IVec2::new(30, 49))
}

#[test]
fn a_plain_move_walks_past_enemies() {
    let mut battle = Battle::sandbox();
    let rifleman = battle.spawn_unit(UnitKind::Rifleman, Team::Player, START);
    let harvester = bystander(&mut battle);
    give(&mut battle, rifleman, &[Order::Move(DESTINATION)]);
    let took = battle.run_until(40.0, |battle| near(battle, rifleman, DESTINATION));
    assert!(took.is_some(), "the rifleman stopped on the way");
    assert_eq!(
        health(&mut battle, harvester),
        UnitKind::Harvester.spec().max_health
    );
}

#[test]
fn an_attack_move_fights_on_the_way_and_then_arrives() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, START);
    let harvester = bystander(&mut battle);
    give(&mut battle, tank, &[Order::AttackMove(DESTINATION)]);
    let took = battle.run_until(120.0, |battle| near(battle, tank, DESTINATION));
    assert!(took.is_some(), "the tank never resumed its attack-move");
    assert!(
        health(&mut battle, harvester) < UnitKind::Harvester.spec().max_health,
        "the tank ignored the enemy"
    );
}

#[test]
fn queued_waypoints_are_visited_in_order() {
    let mut battle = Battle::sandbox();
    let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, START);
    let first = IVec2::new(20, 55);
    give(
        &mut battle,
        scout,
        &[Order::Move(first), Order::Move(DESTINATION)],
    );
    let mut visited_first = false;
    let took = battle.run_until(60.0, |battle| {
        visited_first |= near(battle, scout, first);
        near(battle, scout, DESTINATION)
    });
    assert!(took.is_some(), "the scout never reached its last waypoint");
    assert!(visited_first, "the scout skipped the first waypoint");
}

#[test]
fn a_patrol_keeps_going_back_and_forth() {
    let mut battle = Battle::sandbox();
    let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, START);
    give(
        &mut battle,
        scout,
        &[Order::Patrol {
            from: START,
            to: DESTINATION,
        }],
    );
    let mut arrivals = Vec::new();
    battle.run_until(60.0, |battle| {
        let end = if near(battle, scout, DESTINATION) {
            Some(DESTINATION)
        } else if near(battle, scout, START) {
            Some(START)
        } else {
            None
        };
        if let Some(end) = end
            && arrivals.last() != Some(&end)
        {
            arrivals.push(end);
        }
        false
    });
    assert!(arrivals.len() >= 4, "patrol only touched {arrivals:?}");
}

#[test]
fn trained_units_head_for_the_rally_point() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::PowerPlant, Team::Player, IVec2::new(6, 47));
    battle.construct(BuildingKind::Barracks, Team::Player, IVec2::new(10, 52));
    let rally = IVec2::new(24, 40);
    let world = battle.world();
    let barracks = world
        .query::<(Entity, &Building)>()
        .iter(world)
        .find(|(_, building)| building.kind == BuildingKind::Barracks)
        .map(|(entity, _)| entity)
        .unwrap();
    world.entity_mut(barracks).insert(RallyPoint(rally));
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Rifleman)));
    battle.run_for(UnitKind::Rifleman.spec().build_secs + 20.0);
    let recruit = battle.units(Team::Player, Some(UnitKind::Rifleman))[0];
    assert!(
        near(&mut battle, recruit, rally),
        "the recruit did not reach the rally point"
    );
}

#[test]
fn saves_keep_orders_and_rally_points() {
    let mut battle = Battle::on_level(LevelDefinition::default());
    let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, START);
    give(
        &mut battle,
        scout,
        &[
            Order::Move(IVec2::new(20, 55)),
            Order::AttackMove(DESTINATION),
        ],
    );
    let world = battle.world();
    let command_center = world
        .query_filtered::<Entity, With<Building>>()
        .iter(world)
        .next()
        .unwrap();
    world
        .entity_mut(command_center)
        .insert(RallyPoint(IVec2::new(12, 40)));
    battle.world().resource_mut::<SaveRequests>().save = Some(1);
    battle.run_for(0.1);
    battle.world().resource_mut::<SaveRequests>().load = Some(1);
    battle.run_for(0.3);

    let world = battle.world();
    let planned: Vec<Vec<Order>> = world
        .query::<&Orders>()
        .iter(world)
        .map(|orders| {
            orders
                .current
                .iter()
                .chain(orders.queued.iter())
                .copied()
                .collect()
        })
        .filter(|orders: &Vec<Order>| !orders.is_empty())
        .collect();
    assert!(
        planned
            .iter()
            .any(|orders| orders.contains(&Order::AttackMove(DESTINATION))),
        "{planned:?}"
    );
    let rallies = world.query::<&RallyPoint>().iter(world).count();
    assert_eq!(rallies, 1);
}
