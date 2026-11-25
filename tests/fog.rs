mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::app::savegame::SaveRequests;
use rimefront::missions::levels::{LevelDefinition, Weather};
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::combat::Targets;
use rimefront::sim::components::{Building, Health};
use rimefront::sim::fog::CellSight;
use rimefront::sim::intel::Intel;
use rimefront::world::map::base_sites;

const OPEN_FIELD: IVec2 = IVec2::new(30, 45);

fn seen_locally(battle: &mut Battle, entity: Entity) -> bool {
    battle
        .world()
        .resource::<Targets>()
        .get(entity)
        .is_some_and(|target| target.seen_by(Team::Player))
}

fn sight_at(battle: &mut Battle, cell: IVec2) -> CellSight {
    battle
        .world()
        .resource::<Intel>()
        .vision(Team::Player)
        .sight_at(cell)
}

#[test]
fn the_enemy_base_starts_hidden_and_the_own_base_visible() {
    let mut battle = Battle::standard();
    battle.run_for(0.5);
    assert_eq!(sight_at(&mut battle, base_sites()[0]), CellSight::Visible);
    assert_eq!(
        sight_at(&mut battle, base_sites()[1]),
        CellSight::Unexplored
    );
    for enemy in battle.units(Team::Crimson, None) {
        assert!(
            !seen_locally(&mut battle, enemy),
            "enemy units are visible through the fog"
        );
    }
}

#[test]
fn units_reveal_the_ground_they_cross_and_remember_it() {
    let mut battle = Battle::sandbox();
    let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, IVec2::new(20, 50));
    let far_point = IVec2::new(45, 50);
    battle.run_for(0.5);
    assert_eq!(sight_at(&mut battle, far_point), CellSight::Unexplored);

    battle.order_move(scout, far_point);
    battle.run_for(10.0);
    assert_eq!(sight_at(&mut battle, far_point), CellSight::Visible);

    battle.order_move(scout, IVec2::new(20, 50));
    battle.run_for(10.0);
    assert_eq!(
        sight_at(&mut battle, far_point),
        CellSight::Explored,
        "explored ground is remembered"
    );
}

#[test]
fn artillery_needs_a_spotter_to_fire_beyond_its_own_sight() {
    let mut battle = Battle::sandbox();
    battle.spawn_unit(UnitKind::Artillery, Team::Player, OPEN_FIELD);
    battle.construct(
        BuildingKind::PowerPlant,
        Team::Crimson,
        OPEN_FIELD + IVec2::new(8, -1),
    );
    let world = battle.world();
    let target = world
        .query_filtered::<Entity, With<Building>>()
        .single(world)
        .unwrap();
    let full_health = BuildingKind::PowerPlant.spec().max_health;

    battle.run_for(10.0);
    assert!(!seen_locally(&mut battle, target));
    assert_eq!(
        battle.world().get::<Health>(target).unwrap().current,
        full_health,
        "fired at an unseen target"
    );

    battle.spawn_unit(
        UnitKind::Rifleman,
        Team::Player,
        OPEN_FIELD + IVec2::new(4, 0),
    );
    battle.run_for(10.0);
    let health = battle
        .world()
        .get::<Health>(target)
        .map_or(0.0, |h| h.current);
    assert!(
        health < full_health,
        "the spotter's view did not let the artillery fire"
    );
}

fn health_of_only_building(battle: &mut Battle) -> f32 {
    let world = battle.world();
    world
        .query_filtered::<&Health, With<Building>>()
        .single(world)
        .map_or(0.0, |health| health.current)
}

#[test]
fn enemy_artillery_needs_a_spotter_too() {
    let mut battle = Battle::sandbox();
    battle.spawn_unit(UnitKind::Artillery, Team::Crimson, OPEN_FIELD);
    battle.construct(
        BuildingKind::PowerPlant,
        Team::Player,
        OPEN_FIELD + IVec2::new(8, -1),
    );
    let full_health = BuildingKind::PowerPlant.spec().max_health;

    battle.run_for(10.0);
    assert_eq!(
        health_of_only_building(&mut battle),
        full_health,
        "the computer fired at a target it cannot see"
    );

    battle.spawn_unit(
        UnitKind::Rifleman,
        Team::Crimson,
        OPEN_FIELD + IVec2::new(4, 0),
    );
    battle.run_for(10.0);
    assert!(health_of_only_building(&mut battle) < full_health);
}

#[test]
fn the_computer_has_to_scout_to_find_the_player() {
    let mut battle = Battle::standard();
    battle.run_for(1.0);
    let known_structures = |battle: &mut Battle| {
        battle
            .world()
            .resource::<Intel>()
            .known_enemy_structures(Team::Crimson)
            .count()
    };
    assert_eq!(
        known_structures(&mut battle),
        0,
        "the computer starts out knowing the player's base"
    );

    let took = battle.run_until(6.0 * 60.0, |battle| known_structures(battle) > 0);
    assert!(took.is_some(), "the computer never found the player's base");
}

#[test]
fn levels_can_switch_the_fog_off() {
    let mut battle = Battle::on_level(LevelDefinition {
        fog_of_war: false,
        ..Default::default()
    });
    battle.run_for(0.5);
    assert_eq!(sight_at(&mut battle, base_sites()[1]), CellSight::Visible);
    for enemy in battle.units(Team::Crimson, None) {
        assert!(seen_locally(&mut battle, enemy));
    }
}

#[test]
fn exploration_is_kept_in_save_games() {
    let mut battle = Battle::sandbox();
    let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, IVec2::new(20, 50));
    let far_point = IVec2::new(45, 50);
    battle.order_move(scout, far_point);
    battle.run_for(10.0);
    battle.world().resource_mut::<SaveRequests>().save = Some(3);
    battle.run_for(0.1);

    battle.world().resource_mut::<SaveRequests>().load = Some(3);
    battle.run_for(0.5);
    assert_ne!(sight_at(&mut battle, far_point), CellSight::Unexplored);
}

#[test]
fn rain_shortens_how_far_units_see() {
    let sees_at = |weather: Weather, share: f32| {
        let mut battle = Battle::on_level(LevelDefinition {
            weather,
            ..default()
        });
        let scout = battle.spawn_unit(UnitKind::Scout, Team::Player, OPEN_FIELD);
        battle.run_for(1.0);
        let reach = UnitKind::Scout.spec().sight * share;
        let spot = battle.position(scout) + Vec3::new(reach, 0.0, 0.0);
        battle
            .world()
            .resource::<Intel>()
            .vision(Team::Player)
            .is_visible(spot)
    };
    assert!(sees_at(Weather::Clear, 0.85));
    assert!(
        !sees_at(Weather::Rain, 0.85),
        "rain should cut sight by a quarter"
    );
    assert!(sees_at(Weather::Rain, 0.6));
}
