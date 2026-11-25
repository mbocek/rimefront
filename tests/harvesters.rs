mod common;

use std::collections::HashMap;

use bevy::prelude::*;
use common::Battle;
use rimefront::missions::levels::{BaseSite, Difficulty, LevelDefinition};
use rimefront::sim::catalog::{BuildingKind, Team};
use rimefront::sim::components::*;

const MAP_SEEDS: std::ops::Range<u64> = 0..40;
const MATCH_SECS: f32 = 900.0;
const STALL_LIMIT_SECS: f32 = 45.0;
/// Further than two harvesters nudging each other back and forth on one spot.
const REAL_PROGRESS_METERS: f32 = 4.0;

/// Load and place a harvester last showed progress at, and when.
struct LastProgress {
    cargo: u32,
    position: Vec3,
    secs: f32,
}

fn computer_duel(map_seed: u64) -> Battle {
    let mut level = LevelDefinition {
        fog_of_war: false,
        map_seed,
        ..default()
    };
    level.player.site = BaseSite::SouthWest;
    level.enemies[0].force.site = BaseSite::NorthEast;
    let mut battle = Battle::on_level(level);
    battle.add_ai(
        Team::Player,
        BaseSite::SouthWest.origin(),
        Difficulty::Normal,
    );
    battle
}

fn teams_with_refinery(battle: &mut Battle) -> Vec<Team> {
    let world = battle.world();
    world
        .query::<(&Building, &Team)>()
        .iter(world)
        .filter(|(building, _)| building.kind == BuildingKind::Refinery)
        .map(|(_, team)| *team)
        .collect()
}

/// Harvesters on shared lanes and fields used to shove each other off their waypoints for ever.
#[test]
fn harvesters_with_a_refinery_keep_hauling_in_computer_duels() {
    for map_seed in MAP_SEEDS {
        let mut battle = computer_duel(map_seed);
        let mut last_progress: HashMap<Entity, LastProgress> = HashMap::new();
        let mut now = 0.0;
        while now < MATCH_SECS {
            battle.run_for(1.0);
            now += 1.0;
            let can_unload = teams_with_refinery(&mut battle);
            let world = battle.world();
            for (entity, team, harvester, transform) in world
                .query::<(Entity, &Team, &Harvester, &Transform)>()
                .iter(world)
            {
                let position = transform.translation;
                let progress = last_progress.entry(entity).or_insert(LastProgress {
                    cargo: harvester.cargo,
                    position,
                    secs: now,
                });
                if progress.cargo != harvester.cargo
                    || progress.position.distance(position) > REAL_PROGRESS_METERS
                    || harvester.state == HarvestState::Holding
                    || !can_unload.contains(team)
                {
                    *progress = LastProgress {
                        cargo: harvester.cargo,
                        position,
                        secs: now,
                    };
                    continue;
                }
                assert!(
                    now - progress.secs < STALL_LIMIT_SECS,
                    "map seed {map_seed}: {team:?} harvester {entity} stuck in {:?} since {}s",
                    harvester.state,
                    progress.secs
                );
            }
        }
    }
}
