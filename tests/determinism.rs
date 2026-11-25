mod common;

use common::Battle;
use rimefront::missions::levels::{BaseSite, Difficulty};
use rimefront::sim::catalog::Team;
use rimefront::sim::determinism::simulation_checksum;

const CHECK_EVERY_SECS: f32 = 15.0;
const MATCH_SECS: f32 = 8.0 * 60.0;

fn computer_against_computer() -> Battle {
    let mut battle = Battle::standard();
    battle.add_ai(Team::Player, BaseSite::SouthWest.origin(), Difficulty::Hard);
    battle
}

#[test]
fn the_same_battle_plays_out_identically_twice() {
    let mut first = computer_against_computer();
    let mut second = computer_against_computer();
    let mut elapsed = 0.0;
    while elapsed < MATCH_SECS {
        first.run_for(CHECK_EVERY_SECS);
        second.run_for(CHECK_EVERY_SECS);
        elapsed += CHECK_EVERY_SECS;
        assert_eq!(
            simulation_checksum(first.world()),
            simulation_checksum(second.world()),
            "the battles drifted apart by {elapsed} s"
        );
    }
    assert!(
        first.damage_taken(Team::Player) + first.damage_taken(Team::Crimson) > 0.0,
        "nothing happened, so the comparison proves little"
    );
}

/// Stands in for presentation, which adds its own components at moments that differ between machines.
#[derive(bevy::prelude::Component)]
struct DrawnBySomeone;

fn shuffle_query_order(battle: &mut Battle, round: u32) {
    use bevy::prelude::*;
    use rimefront::sim::combat::Projectile;
    use rimefront::sim::components::{Building, Unit};
    let world = battle.world();
    let units: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Unit>, With<Building>, With<Projectile>)>>()
        .iter(world)
        .collect();
    for (index, unit) in units.into_iter().enumerate() {
        if (index as u32 + round).is_multiple_of(3) {
            if world.entity(unit).contains::<DrawnBySomeone>() {
                world.entity_mut(unit).remove::<DrawnBySomeone>();
            } else {
                world.entity_mut(unit).insert(DrawnBySomeone);
            }
        }
    }
}

#[test]
fn presentation_reordering_units_does_not_change_the_battle() {
    let mut plain = computer_against_computer();
    let mut drawn = computer_against_computer();
    let mut elapsed = 0.0;
    let mut round = 0;
    while elapsed < MATCH_SECS {
        for _ in 0..(CHECK_EVERY_SECS as u32) {
            plain.run_for(1.0);
            shuffle_query_order(&mut drawn, round);
            drawn.run_for(1.0);
            round += 1;
        }
        elapsed += CHECK_EVERY_SECS;
        assert_eq!(
            simulation_checksum(plain.world()),
            simulation_checksum(drawn.world()),
            "unit order changed the battle by {elapsed} s"
        );
    }
}

const COMMANDER_JOINS_AT_TICK: u64 = 6;
const SECOND_MATCH_SAMPLE_TICKS: u64 = 600;

fn step_to_tick(battle: &mut Battle, tick: u64) {
    while battle.tick() < tick {
        battle.app.update();
    }
}

fn restart_match(battle: &mut Battle) {
    use bevy::prelude::*;
    use rimefront::app::match_flow::GameState;
    battle
        .world()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Restarting);
    battle.app.update();
    while battle.state() != GameState::Playing {
        battle.app.update();
    }
}

fn computer_battle_checksums(battle: &mut Battle) -> Vec<u64> {
    step_to_tick(battle, COMMANDER_JOINS_AT_TICK);
    battle.add_ai(Team::Player, BaseSite::SouthWest.origin(), Difficulty::Hard);
    (1..=4)
        .map(|sample| {
            step_to_tick(
                battle,
                COMMANDER_JOINS_AT_TICK + sample * SECOND_MATCH_SAMPLE_TICKS,
            );
            simulation_checksum(battle.world())
        })
        .collect()
}

#[test]
fn a_second_match_in_the_same_process_plays_out_like_a_fresh_one() {
    let fresh = computer_battle_checksums(&mut Battle::standard());
    let mut rematch = Battle::standard();
    step_to_tick(&mut rematch, 3);
    restart_match(&mut rematch);
    assert_eq!(
        fresh,
        computer_battle_checksums(&mut rematch),
        "a match played after an earlier one drifted from a fresh one"
    );
}
