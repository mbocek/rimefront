mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::match_stats::MatchStats;
use rimefront::sim::production::BuildItem;

const FIELD: IVec2 = IVec2::new(20, 45);

#[test]
fn the_tally_counts_kills_losses_damage_and_new_units() {
    let mut battle = Battle::sandbox();
    battle.run_for(1.0);
    battle.spawn_unit(UnitKind::Tank, Team::Azure, FIELD);
    let prey = battle.spawn_unit(UnitKind::Harvester, Team::Crimson, FIELD + IVec2::new(4, 0));
    battle.run_until(120.0, |battle| !battle.alive(prey));
    battle.run_for(0.5);

    let stats = battle.world().resource::<MatchStats>();
    let player = *stats.of(Team::Azure);
    let crimson = *stats.of(Team::Crimson);
    assert_eq!(player.enemies_destroyed, 1);
    assert_eq!(crimson.units_lost, 1);
    assert!(player.damage_dealt >= UnitKind::Harvester.spec().max_health);
    assert_eq!(crimson.damage_taken, player.damage_dealt);
    assert_eq!(
        player.units_trained, 1,
        "the tank spawned after the start counts as trained"
    );

    battle.construct(BuildingKind::PowerPlant, Team::Azure, IVec2::new(6, 47));
    battle.construct(BuildingKind::Barracks, Team::Azure, IVec2::new(10, 52));
    battle.set_credits(Team::Azure, 1000);
    assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Rifleman)));
    battle.run_for(UnitKind::Rifleman.spec().build_secs + 1.0);
    let player = *battle.world().resource::<MatchStats>().of(Team::Azure);
    assert_eq!(player.units_trained, 2);
    assert_eq!(player.structures_built, 2);
}

#[test]
fn an_attack_on_a_harvester_is_announced_with_its_place() {
    use rimefront::sim::notifications::{LastAlert, Notifications};
    let mut battle = Battle::sandbox();
    let field = IVec2::new(24, 45);
    battle.spawn_unit(UnitKind::Harvester, Team::Azure, field);
    battle.spawn_unit(UnitKind::Rifleman, Team::Crimson, field + IVec2::new(3, 0));
    battle.run_for(4.0);
    let world = battle.world();
    let spot = world
        .resource::<LastAlert>()
        .0
        .expect("no alert was recorded");
    let first_shots_land_near_the_spawn =
        rimefront::world::map::cell_center(field).distance(spot) < 8.0;
    assert!(first_shots_land_near_the_spawn, "alert at {spot}");
    assert!(
        world
            .resource::<Notifications>()
            .visible()
            .any(|(line, _)| line == "Our harvester is under attack!")
    );
}

#[test]
fn only_the_local_side_is_told_its_news() {
    use rimefront::sim::cues::Sound;
    use rimefront::sim::notifications::Notifications;
    use rimefront::sim::player_commands::PlayerCommand;
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::CommandCenter, Team::Azure, IVec2::new(6, 47));
    battle.set_credits(Team::Azure, 1000);
    battle.set_credits(Team::Crimson, 0);
    let power_plant = BuildItem::Structure(BuildingKind::PowerPlant);
    for team in [Team::Azure, Team::Crimson] {
        battle.issue(
            team,
            PlayerCommand::Enqueue {
                item: power_plant,
                copies: 1,
            },
        );
    }
    battle.run_for(power_plant.build_secs() + 0.5);

    let lines: Vec<String> = battle
        .world()
        .resource::<Notifications>()
        .visible()
        .map(|(line, _)| line.to_string())
        .collect();
    assert_eq!(lines, ["Construction complete"]);
    assert_eq!(battle.heard(Sound::ConstructionComplete), 1);
}
