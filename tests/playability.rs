mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use rimefront::app::savegame::SaveRequests;
use rimefront::missions::levels::{LevelDefinition, campaign};
use rimefront::sim::catalog::{BuildingKind, Diplomacy, STARTING_CREDITS, Team, UnitKind};
use rimefront::sim::components::*;
use rimefront::sim::movement::order_move;
use rimefront::sim::outcome::GameOutcome;
use rimefront::sim::production::BuildItem;
use rimefront::world::map::{
    GameMap, cell_center, footprint_cells, map_bounds, opposite_cell, world_to_cell,
};

const PLAYER_REFINERY_SITE: IVec2 = IVec2::new(11, 51);
const OPEN_FIELD: IVec2 = IVec2::new(30, 45);

fn arrived(battle: &mut Battle, unit: Entity, goal: IVec2) -> bool {
    battle.position(unit).xz().distance(cell_center(goal).xz()) < 1.5
}

/// Sends every idle combat unit of the team at the nearest enemy, letting it fight whatever it meets.
fn hunt_enemies(battle: &mut Battle, team: Team) {
    let world = battle.world();
    let diplomacy = world.resource::<Diplomacy>().clone();
    let enemies: Vec<Vec3> = world
        .query::<(&Team, &Transform, &Health)>()
        .iter(world)
        .filter(|(owner, ..)| diplomacy.hostile(**owner, team))
        .map(|(_, transform, _)| transform.translation)
        .collect();
    if enemies.is_empty() {
        return;
    }
    world.resource_scope(|world, map: Mut<GameMap>| {
        let mut hunters = world.query::<(&Team, &Transform, &mut Mobile, &AttackTarget)>();
        for (owner, transform, mut mobile, attack) in hunters.iter_mut(world) {
            if *owner != team || mobile.is_moving() || attack.target.is_some() {
                continue;
            }
            let here = transform.translation;
            let nearest = enemies
                .iter()
                .min_by(|a, b| {
                    a.distance_squared(here)
                        .total_cmp(&b.distance_squared(here))
                })
                .expect("checked non-empty");
            order_move(&mut mobile, &map, here, world_to_cell(*nearest), false);
        }
    });
}

#[test]
fn both_sides_start_with_the_same_forces() {
    let mut battle = Battle::standard();
    for team in [Team::Azure, Team::Crimson] {
        assert_eq!(battle.buildings(team), vec![BuildingKind::CommandCenter]);
        assert_eq!(battle.units(team, None).len(), 5);
        assert_eq!(battle.credits(team), STARTING_CREDITS);
    }
}

#[test]
fn infantry_crosses_the_river_through_a_ford() {
    let mut battle = Battle::sandbox();
    let start = IVec2::new(10, 50);
    let goal = IVec2::new(12, 12);
    let rifleman = battle.spawn_unit(UnitKind::Rifleman, Team::Azure, start);
    battle.order_move(rifleman, goal);
    let took = battle.run_until(120.0, |battle| arrived(battle, rifleman, goal));
    assert!(took.is_some(), "rifleman never reached the far bank");
}

#[test]
fn vehicles_drive_to_their_destination() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, IVec2::new(20, 50));
    let goal = IVec2::new(45, 50);
    battle.order_move(tank, goal);
    let took = battle
        .run_until(60.0, |battle| arrived(battle, tank, goal))
        .expect("tank got stuck");
    let distance = (45.0 - 20.0) * 2.0;
    assert!(
        took < distance / UnitKind::Tank.spec().speed * 2.0,
        "tank took {took:.0} s for {distance} m"
    );
}

#[test]
fn a_refinery_brings_a_harvester_that_earns_credits() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::Refinery, Team::Azure, PLAYER_REFINERY_SITE);
    assert_eq!(
        battle.units(Team::Azure, Some(UnitKind::Harvester)).len(),
        1,
        "refinery comes with a harvester"
    );
    let before = battle.credits(Team::Azure);
    let took = battle.run_until(180.0, |battle| battle.credits(Team::Azure) >= before + 500);
    assert!(took.is_some(), "no full load delivered in 3 minutes");
}

#[test]
fn barracks_train_queued_infantry() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::CommandCenter, Team::Azure, IVec2::new(6, 52));
    battle.construct(BuildingKind::PowerPlant, Team::Azure, IVec2::new(6, 47));
    battle.construct(BuildingKind::Barracks, Team::Azure, IVec2::new(10, 52));
    let before = battle.credits(Team::Azure);
    assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Rifleman)));
    assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Rifleman)));
    assert_eq!(
        battle.credits(Team::Azure),
        before - 2 * UnitKind::Rifleman.spec().cost
    );

    battle.run_for(UnitKind::Rifleman.spec().build_secs * 2.0 + 1.0);
    assert_eq!(battle.units(Team::Azure, Some(UnitKind::Rifleman)).len(), 2);
}

#[test]
fn low_power_halves_production_speed() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::Barracks, Team::Azure, IVec2::new(10, 52));
    assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Rifleman)));
    let build_secs = UnitKind::Rifleman.spec().build_secs;
    battle.run_for(build_secs + 1.0);
    assert!(
        battle
            .units(Team::Azure, Some(UnitKind::Rifleman))
            .is_empty(),
        "built at full speed without power"
    );
    battle.run_for(build_secs);
    assert_eq!(battle.units(Team::Azure, Some(UnitKind::Rifleman)).len(), 1);
}

#[test]
fn a_tank_defeats_a_rifle_squad() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, OPEN_FIELD);
    for offset in [IVec2::new(4, 0), IVec2::new(4, 1), IVec2::new(5, -1)] {
        battle.spawn_unit(UnitKind::Rifleman, Team::Crimson, OPEN_FIELD + offset);
    }
    let took = battle.run_until(90.0, |battle| battle.units(Team::Crimson, None).is_empty());
    assert!(took.is_some(), "riflemen survived");
    assert!(battle.alive(tank));
    assert!(
        battle.damage_taken(Team::Azure) > 0.0,
        "riflemen should at least scratch the tank"
    );
}

#[test]
fn a_gun_turret_stops_a_raiding_scout() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::GunTurret, Team::Azure, OPEN_FIELD);
    let scout = battle.spawn_unit(
        UnitKind::Scout,
        Team::Crimson,
        OPEN_FIELD + IVec2::new(9, 0),
    );
    battle.order_move(scout, OPEN_FIELD + IVec2::new(-9, 0));
    let took = battle.run_until(40.0, |battle| !battle.alive(scout));
    assert!(took.is_some(), "scout drove past the turret");
}

#[test]
fn artillery_splash_hurts_a_whole_group() {
    let mut battle = Battle::sandbox();
    let howitzer = battle.spawn_unit(UnitKind::Artillery, Team::Azure, OPEN_FIELD);
    let squad: Vec<Entity> = [IVec2::new(8, 0), IVec2::new(8, 1), IVec2::new(9, 0)]
        .into_iter()
        .map(|offset| battle.spawn_unit(UnitKind::Tank, Team::Crimson, OPEN_FIELD + offset))
        .collect();
    battle
        .world()
        .get_mut::<AttackTarget>(howitzer)
        .unwrap()
        .order(squad[0]);
    battle.run_for(UnitKind::Artillery.spec().weapon.unwrap().reload_secs + 3.0);
    let world = battle.world();
    let hurt = squad
        .iter()
        .filter(|&&tank| {
            world
                .get::<Health>(tank)
                .is_none_or(|health| health.current < health.max)
        })
        .count();
    assert!(hurt >= 2, "only {hurt} tank(s) caught in the blast");
}

#[test]
fn destroyed_buildings_free_their_ground() {
    let mut battle = Battle::sandbox();
    let origin = IVec2::new(30, 20);
    battle.construct(BuildingKind::PowerPlant, Team::Crimson, origin);
    let footprint = BuildingKind::PowerPlant.spec().footprint;
    let world = battle.world();
    assert!(
        footprint_cells(origin, footprint)
            .all(|cell| !world.resource::<GameMap>().is_passable(cell))
    );
    let plant = world
        .query_filtered::<Entity, With<Building>>()
        .single(world)
        .unwrap();
    world.get_mut::<Health>(plant).unwrap().current = 0.0;
    battle.run_for(0.5);
    let world = battle.world();
    assert!(
        footprint_cells(origin, footprint)
            .all(|cell| world.resource::<GameMap>().is_passable(cell))
    );
}

#[test]
fn the_enemy_builds_a_working_economy() {
    let mut battle = Battle::standard();
    battle.run_for(240.0);
    let enemy_base = battle.buildings(Team::Crimson);
    for needed in [
        BuildingKind::PowerPlant,
        BuildingKind::Barracks,
        BuildingKind::Refinery,
    ] {
        assert!(
            enemy_base.contains(&needed),
            "enemy has no {needed:?}: {enemy_base:?}"
        );
    }
    assert!(
        !battle
            .units(Team::Crimson, Some(UnitKind::Harvester))
            .is_empty(),
        "enemy has no harvester"
    );
}

#[test]
fn the_enemy_goes_on_the_attack() {
    let mut battle = Battle::standard();
    let took = battle.run_until(240.0, |battle| battle.damage_taken(Team::Azure) > 0.0);
    assert!(took.is_some(), "the enemy never fired at the player");
}

#[test]
fn a_defenceless_player_is_defeated() {
    let mut battle = Battle::standard();
    battle.despawn_units(Team::Azure);
    let took = battle.run_until(20.0 * 60.0, |battle| {
        battle.outcome() != GameOutcome::Ongoing
    });
    assert!(took.is_some(), "match never ended");
    assert_eq!(battle.outcome(), GameOutcome::Defeat);
}

#[test]
fn a_strong_player_army_wins_the_match() {
    let mut battle = Battle::standard();
    for row in 0..3 {
        for column in 0..4 {
            battle.spawn_unit(
                UnitKind::Tank,
                Team::Azure,
                IVec2::new(12 + column * 2, 46 + row * 2),
            );
        }
    }
    for column in 0..3 {
        battle.spawn_unit(
            UnitKind::Artillery,
            Team::Azure,
            IVec2::new(12 + column * 2, 54),
        );
    }
    let took = battle.run_until(15.0 * 60.0, |battle| {
        hunt_enemies(battle, Team::Azure);
        battle.outcome() != GameOutcome::Ongoing
    });
    assert!(took.is_some(), "match never ended");
    assert_eq!(battle.outcome(), GameOutcome::Victory);
}

#[test]
fn saving_and_loading_restores_the_battle() {
    let mut battle = Battle::standard();
    battle.run_for(60.0);
    battle.world().resource_mut::<SaveRequests>().save = Some(1);
    battle.run_for(0.1);

    let snapshot = |battle: &mut Battle| {
        let teams = [Team::Azure, Team::Crimson];
        let credits = teams.map(|team| battle.credits(team));
        let units = teams.map(|team| battle.units(team, None).len());
        let mut buildings = teams.map(|team| battle.buildings(team));
        buildings
            .iter_mut()
            .for_each(|kinds| kinds.sort_by_key(|kind| format!("{kind:?}")));
        (credits, units, buildings)
    };
    let saved = snapshot(&mut battle);

    battle.set_credits(Team::Azure, 1);
    battle.run_for(30.0);
    battle.world().resource_mut::<SaveRequests>().load = Some(1);
    battle.run_for(0.3);

    assert_eq!(snapshot(&mut battle), saved);
}

#[test]
fn opposite_corners_get_the_same_ground() {
    let levels = campaign().into_iter().map(|entry| entry.level);
    for level in levels.chain([LevelDefinition::default()]) {
        let name = level.name.clone();
        let mut battle = Battle::new(BattleSetup {
            enemy_ai: false,
            starting_forces: false,
            level: Some(level),
        });
        let map = battle.world().resource::<GameMap>();
        for cell in map_bounds().cells() {
            let mirror = opposite_cell(cell);
            if !GameMap::in_bounds(mirror) {
                continue;
            }
            assert_eq!(
                map.is_passable(cell),
                map.is_passable(mirror),
                "{name}: {cell} vs {mirror}"
            );
            assert_eq!(
                map.crystal_at(cell),
                map.crystal_at(mirror),
                "{name}: {cell} vs {mirror}"
            );
        }
    }
}

fn distance_driven_in(season: rimefront::world::season::Season, kind: UnitKind) -> f32 {
    let level = LevelDefinition {
        season,
        weather: rimefront::missions::levels::Weather::Clear,
        ..default()
    };
    let mut battle = Battle::on_level(level);
    let start = OPEN_FIELD;
    let unit = battle.spawn_unit(kind, Team::Azure, start);
    battle.order_move(unit, start + IVec2::new(0, -12));
    battle.run_for(3.0);
    battle.position(unit).xz().distance(cell_center(start).xz())
}

#[test]
fn snow_slows_wheels_more_than_tracks() {
    use rimefront::world::season::Season;
    let buggy_in_summer = distance_driven_in(Season::Summer, UnitKind::Scout);
    let buggy_in_snow = distance_driven_in(Season::Winter, UnitKind::Scout);
    let tank_in_summer = distance_driven_in(Season::Summer, UnitKind::Tank);
    let tank_in_snow = distance_driven_in(Season::Winter, UnitKind::Tank);
    assert!(
        buggy_in_snow < buggy_in_summer * 0.8,
        "{buggy_in_snow} vs {buggy_in_summer}"
    );
    assert!(
        tank_in_snow > tank_in_summer * 0.85,
        "{tank_in_snow} vs {tank_in_summer}"
    );
    assert!(tank_in_snow < tank_in_summer);
}

#[test]
fn winter_freezes_the_river_into_ice_that_carries_units_but_no_structures() {
    use rimefront::world::map::is_water;
    use rimefront::world::season::Season;
    use rimefront::world::terrain::Landscape;
    let river_cells = || -> Vec<IVec2> {
        map_bounds()
            .cells()
            .filter(|&cell| is_water(Landscape::River, cell))
            .collect()
    };
    let mut summer = Battle::on_level(LevelDefinition::default());
    let water = river_cells();
    assert!(!water.is_empty());
    let map = summer.world().resource::<GameMap>();
    assert!(water.iter().all(|&cell| !map.is_passable(cell)));

    let mut winter = Battle::on_level(LevelDefinition {
        season: Season::Winter,
        ..default()
    });
    let map = winter.world().resource::<GameMap>();
    let last = map_bounds().max();
    let inside = |cell: &&IVec2| cell.x != last.x && cell.y != last.y;
    assert!(
        water
            .iter()
            .filter(inside)
            .all(|&cell| map.is_passable(cell))
    );
    assert!(water.iter().all(|&cell| !map.is_buildable(cell)));
}
