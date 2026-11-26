mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::app::savegame::SaveRequests;
use rimefront::missions::levels::LevelDefinition;
use rimefront::sim::catalog::{Team, UnitKind};
use rimefront::sim::components::{HarvestState, Harvester, Health};
use rimefront::sim::veterancy::{Rank, Veterancy};

const FIELD: IVec2 = IVec2::new(20, 45);

fn rank(battle: &mut Battle, unit: Entity) -> Rank {
    battle
        .world()
        .get::<Veterancy>(unit)
        .map_or(Rank::Recruit, |veterancy| veterancy.rank)
}

/// Current and maximum health.
fn health(battle: &mut Battle, entity: Entity) -> (f32, f32) {
    let health = battle.world().get::<Health>(entity).unwrap();
    (health.current, health.max)
}

/// Unarmed enemy harvesters parked in a row beside the shooter: prey that never fights back.
fn park_harvesters(battle: &mut Battle, count: i32) -> Vec<Entity> {
    (0..count)
        .map(|i| {
            let harvester = battle.spawn_unit(
                UnitKind::Harvester,
                Team::Crimson,
                FIELD + IVec2::new(4, i * 2 - 2),
            );
            battle
                .world()
                .get_mut::<Harvester>(harvester)
                .unwrap()
                .state = HarvestState::Holding;
            harvester
        })
        .collect()
}

#[test]
fn destroying_enemies_promotes_and_toughens_a_unit() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD);
    let prey = park_harvesters(&mut battle, 1);
    battle.run_until(120.0, |battle| !battle.alive(prey[0]));
    battle.run_for(0.5);

    assert_eq!(rank(&mut battle, tank), Rank::Veteran);
    let (_, max) = health(&mut battle, tank);
    assert_eq!(max, UnitKind::Tank.spec().max_health * 1.2);
}

#[test]
fn three_times_its_price_in_kills_makes_an_elite() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD);
    let prey = park_harvesters(&mut battle, 3);
    battle.run_until(300.0, |battle| {
        prey.iter().all(|&harvester| !battle.alive(harvester))
    });
    battle.run_for(0.5);
    assert_eq!(rank(&mut battle, tank), Rank::Elite);
}

#[test]
fn veterans_hit_harder() {
    let mut battle = Battle::sandbox();
    battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD);
    let veteran = battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD + IVec2::new(0, 12));
    battle.world().entity_mut(veteran).insert(Veterancy {
        experience: UnitKind::Tank.spec().cost as f32,
        rank: Rank::Veteran,
    });
    let recruit_target =
        battle.spawn_unit(UnitKind::Harvester, Team::Crimson, FIELD + IVec2::new(4, 0));
    let veteran_target = battle.spawn_unit(
        UnitKind::Harvester,
        Team::Crimson,
        FIELD + IVec2::new(4, 12),
    );

    let full = UnitKind::Harvester.spec().max_health;
    let (mut recruit_hit, mut veteran_hit) = (None, None);
    battle.run_until(20.0, |battle| {
        let first_hit = |health: f32| (health < full).then_some(full - health);
        recruit_hit = recruit_hit.or(first_hit(health(battle, recruit_target).0));
        veteran_hit = veteran_hit.or(first_hit(health(battle, veteran_target).0));
        recruit_hit.is_some() && veteran_hit.is_some()
    });
    let (recruit_hit, veteran_hit) = (recruit_hit.unwrap(), veteran_hit.unwrap());
    assert!(
        (veteran_hit / recruit_hit - 1.2).abs() < 0.01,
        "veteran hit {veteran_hit}, recruit hit {recruit_hit}"
    );
}

#[test]
fn saves_keep_experience_and_rank() {
    let mut battle = Battle::on_level(LevelDefinition::default());
    let rifleman = battle.spawn_unit(UnitKind::Rifleman, Team::Azure, FIELD);
    battle.world().entity_mut(rifleman).insert(Veterancy {
        experience: UnitKind::Rifleman.spec().cost as f32 * 3.0,
        rank: Rank::Recruit,
    });
    battle.run_for(0.2);
    assert_eq!(rank(&mut battle, rifleman), Rank::Elite);

    battle.world().resource_mut::<SaveRequests>().save = Some(1);
    battle.run_for(0.1);
    battle.world().resource_mut::<SaveRequests>().load = Some(1);
    battle.run_for(0.3);

    let world = battle.world();
    let elites = world
        .query::<(&Veterancy, &Health)>()
        .iter(world)
        .filter(|(veterancy, health)| {
            veterancy.rank == Rank::Elite
                && health.max == UnitKind::Rifleman.spec().max_health * 1.4
        })
        .count();
    assert_eq!(elites, 1);
}
