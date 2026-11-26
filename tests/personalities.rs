mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::missions::levels::{
    Difficulty, EnemySetup, LevelDefinition, Personality, SkirmishOptions,
};
use rimefront::missions::random_battle::{DifficultyChoice, RandomBattleOptions, generate};
use rimefront::sim::catalog::{BuildingKind, Team};
use rimefront::sim::components::Building;

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
    let hard = RandomBattleOptions {
        difficulty: DifficultyChoice::Fixed(Difficulty::Hard),
        ..RandomBattleOptions::default()
    };
    let rolled: Vec<Personality> = (1..=12)
        .flat_map(|seed| generate(&hard, seed).enemies)
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

#[test]
fn easy_and_normal_commanders_play_the_balanced_way_unless_told_otherwise() {
    for seed in 1..=12 {
        for enemy in generate(&RandomBattleOptions::default(), seed).enemies {
            assert_eq!(
                enemy.personality,
                Personality::Balanced,
                "random battle {seed}"
            );
        }
    }
    for map_variant in 1..=5 {
        let mixed = |difficulty| {
            SkirmishOptions {
                difficulty,
                map_variant,
                ..default()
            }
            .to_level()
            .enemies[0]
                .personality
        };
        assert_eq!(mixed(Difficulty::Easy), Personality::Balanced);
        assert_eq!(mixed(Difficulty::Normal), Personality::Balanced);
    }
    let rusher_asked_for = SkirmishOptions {
        personality: Some(Personality::Rusher),
        ..default()
    };
    assert_eq!(
        rusher_asked_for.to_level().enemies[0].personality,
        Personality::Rusher
    );
}

fn structure_health(battle: &mut Battle, team: Team) -> f32 {
    use rimefront::sim::components::Health;
    let world = battle.world();
    world
        .query::<(&Building, &Team, &Health)>()
        .iter(world)
        .filter(|(_, owner, _)| **owner == team)
        .map(|(_, _, health)| health.current)
        .sum()
}

/// When a Normal commander of each personality first hurts the base of a player who only sits in it, and
/// when that player is beaten, on a few map variants:
/// `cargo test --release --test personalities -- --ignored --nocapture`.
#[test]
#[ignore]
fn pressure_on_a_passive_player_by_personality() {
    use rimefront::sim::outcome::GameOutcome;
    const LIMIT_SECS: f32 = 25.0 * 60.0;
    const VARIANTS: [u64; 6] = [1, 2, 3, 4, 5, 6];
    for personality in Personality::ALL {
        let mut first_hits = Vec::new();
        let mut defeats = Vec::new();
        for map_variant in VARIANTS {
            let level = SkirmishOptions {
                difficulty: Difficulty::Normal,
                personality: Some(personality),
                map_variant,
                ..default()
            }
            .to_level();
            let mut battle = Battle::on_level(level);
            battle.run_for(1.0);
            let untouched = structure_health(&mut battle, Team::Azure);
            let first_hit = battle.run_until(LIMIT_SECS, |battle| {
                structure_health(battle, Team::Azure) < untouched - 1.0
            });
            first_hits.push(first_hit.map_or(f32::NAN, |_| battle.elapsed_secs / 60.0));
            battle.run_until(LIMIT_SECS - battle.elapsed_secs, |battle| {
                battle.outcome() == GameOutcome::Defeat
            });
            let beaten = battle.outcome() == GameOutcome::Defeat;
            defeats.push(if beaten {
                battle.elapsed_secs / 60.0
            } else {
                f32::NAN
            });
        }
        let tenths = |values: &[f32]| {
            values
                .iter()
                .map(|m| (m * 10.0).round() / 10.0)
                .collect::<Vec<_>>()
        };
        println!(
            "{personality:?}: base first hit at {:?} min; beaten at {:?} min",
            tenths(&first_hits),
            tenths(&defeats)
        );
    }
}
