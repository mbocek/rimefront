mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::missions::levels::{EnemySetup, LevelDefinition, Personality};
use iron_tide::missions::random_battle::{RandomBattleOptions, generate};
use iron_tide::sim::catalog::{BuildingKind, Team};
use iron_tide::sim::components::Building;

fn against(personality: Personality) -> Battle {
    Battle::on_level(LevelDefinition {
        enemies: vec![EnemySetup {
            personality,
            ..EnemySetup::default()
        }],
        ..LevelDefinition::default()
    })
}

fn structures_of(battle: &mut Battle, kind: BuildingKind) -> usize {
    let world = battle.world();
    world
        .query::<(&Building, &Team)>()
        .iter(world)
        .filter(|(building, team)| building.kind == kind && **team == Team::Crimson)
        .count()
}

#[test]
fn a_fortress_guards_its_base_before_a_rusher_does() {
    let mut fortress = against(Personality::Fortress);
    let mut rusher = against(Personality::Rusher);
    fortress.run_for(4.0 * 60.0);
    rusher.run_for(4.0 * 60.0);
    let turrets = |battle: &mut Battle| {
        structures_of(battle, BuildingKind::GunTurret)
            + structures_of(battle, BuildingKind::RocketBunker)
    };
    assert!(
        turrets(&mut fortress) > turrets(&mut rusher),
        "fortress {} against rusher {} guns after four minutes",
        turrets(&mut fortress),
        turrets(&mut rusher)
    );
}

#[test]
fn an_air_power_builds_its_helipad_sooner_than_a_balanced_commander() {
    let first_helipad = |personality| {
        let mut battle = against(personality);
        battle.run_until(15.0 * 60.0, |battle| {
            structures_of(battle, BuildingKind::Helipad) >= 1
        })
    };
    let air = first_helipad(Personality::AirPower).expect("air power never built a Helipad");
    let balanced = first_helipad(Personality::Balanced).unwrap_or(f32::MAX);
    assert!(
        air < balanced,
        "air power at {air} s, balanced at {balanced} s"
    );
}

#[test]
fn older_levels_get_the_balanced_commander_and_random_battles_vary() {
    let older: EnemySetup =
        serde_json::from_str(r#"{"site": "NorthEast", "difficulty": "Hard"}"#).unwrap();
    assert_eq!(older.personality, Personality::Balanced);
    let rolled: Vec<Personality> = (1..=12)
        .flat_map(|seed| generate(&RandomBattleOptions::default(), seed).enemies)
        .map(|enemy| enemy.personality)
        .collect();
    let distinct = Personality::ALL
        .iter()
        .filter(|style| rolled.contains(style))
        .count();
    assert!(
        distinct >= 3,
        "twelve random battles rolled only {distinct} personalities"
    );
}
