mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::app::savegame::{SaveDirectory, SaveRequests, slot_summary};
use rimefront::missions::levels::{
    BaseSite, Difficulty, LevelDefinition, Objective, SkirmishOptions, campaign,
};
use rimefront::sim::catalog::{Alliance, BuildingKind, Diplomacy, Team, UnitKind};
use rimefront::sim::components::*;
use rimefront::sim::engineers::{EngineerJob, EngineerTask};
use rimefront::sim::outcome::GameOutcome;
use rimefront::sim::production::BuildItem;
use rimefront::world::map::GameMap;
use rimefront::world::season::Season;

const OPEN_FIELD: IVec2 = IVec2::new(30, 45);

fn quiet_level(objective: Objective) -> LevelDefinition {
    let mut level = LevelDefinition {
        objective,
        ..default()
    };
    level.enemies[0].force.units.clear();
    level
}

/// Every refinery comes with a harvester.
fn free_harvesters(buildings: &[BuildingKind]) -> usize {
    buildings
        .iter()
        .filter(|&&kind| kind == BuildingKind::Refinery)
        .count()
}

#[test]
fn every_campaign_mission_sets_up_its_battle() {
    for mission in campaign() {
        let level = mission.level;
        let mut battle = Battle::on_level(level.clone());
        assert!(
            battle
                .buildings(Team::Player)
                .contains(&BuildingKind::CommandCenter),
            "{}",
            level.name
        );
        for building in &level.player.buildings {
            assert!(
                battle.buildings(Team::Player).contains(building),
                "{}: missing {building:?}",
                level.name
            );
        }
        for (team, enemy) in level.opponents() {
            assert!(
                battle
                    .buildings(team)
                    .contains(&BuildingKind::CommandCenter),
                "{}: {team:?}",
                level.name
            );
            assert_eq!(
                battle.units(team, None).len(),
                enemy.force.units.len() + free_harvesters(&enemy.force.buildings)
            );
            assert_eq!(battle.credits(team), enemy.force.credits);
        }
        assert_eq!(
            battle.units(Team::Player, None).len(),
            level.player.units.len() + free_harvesters(&level.player.buildings)
        );
        battle.run_for(30.0);
        assert_eq!(
            battle.outcome(),
            GameOutcome::Ongoing,
            "{} ended within 30 s",
            level.name
        );
    }
}

#[test]
fn three_enemy_commanders_each_build_a_base() {
    let level = SkirmishOptions {
        enemies: 3,
        difficulty: Difficulty::Hard,
        ..default()
    }
    .to_level();
    let mut battle = Battle::on_level(level);
    battle.run_for(240.0);
    for team in Team::OPPONENTS {
        let base = battle.buildings(team);
        assert!(base.len() >= 3, "{team:?} only has {base:?}");
    }
}

#[test]
fn alliances_decide_who_fights_whom() {
    let against_syndicate = Diplomacy::default();
    assert!(against_syndicate.hostile(Team::Crimson, Team::Player));
    assert!(against_syndicate.hostile(Team::Player, Team::Violet));
    assert!(!against_syndicate.hostile(Team::Crimson, Team::Amber));
    let pairs = Diplomacy::of(Alliance::TwoAgainstTwo);
    assert!(!pairs.hostile(Team::Player, Team::Amber));
    assert!(!pairs.hostile(Team::Crimson, Team::Violet));
    assert!(pairs.hostile(Team::Amber, Team::Violet));
    let everyone = Diplomacy::of(Alliance::FreeForAll);
    assert!(everyone.hostile(Team::Crimson, Team::Amber));
    assert!(!everyone.hostile(Team::Violet, Team::Violet));
    let mut joined = Diplomacy::default();
    let newcomer = Team::from_index(6);
    assert!(joined.hostile(newcomer, Team::Player) && joined.hostile(newcomer, Team::Crimson));
    joined.join_camp_of(newcomer, Team::Player);
    assert!(!joined.hostile(newcomer, Team::Player));
    assert!(joined.hostile(newcomer, Team::Crimson));
    assert!(joined.hostile(newcomer, Team::from_index(5)));
}

#[test]
fn a_two_against_two_battle_takes_every_corner_and_ends() {
    let options = SkirmishOptions {
        alliance: Alliance::TwoAgainstTwo,
        enemies: 1,
        ..SkirmishOptions::default()
    };
    let level = options.to_level();
    assert_eq!(level.enemies.len(), 3);
    let mut battle = Battle::on_level(level);
    battle.add_ai(Team::Player, BaseSite::SouthWest.origin(), Difficulty::Hard);
    battle.run_for(60.0);
    assert!(battle.outcome() == GameOutcome::Ongoing);
    assert!(battle.hostile_units(Team::Player) > 0);
}

#[test]
fn surviving_the_timer_wins_a_defence_mission() {
    let mut battle = Battle::on_level(quiet_level(Objective::Survive { minutes: 0.5 }));
    battle.run_for(20.0);
    assert_eq!(battle.outcome(), GameOutcome::Ongoing);
    battle.run_for(15.0);
    assert_eq!(battle.outcome(), GameOutcome::Victory);
}

#[test]
fn destroying_the_command_centers_wins_even_with_enemies_left() {
    let mut battle = Battle::on_level(LevelDefinition {
        objective: Objective::DestroyCommandCenters,
        ..default()
    });
    let world = battle.world();
    let enemy_centers: Vec<Entity> = world
        .query::<(Entity, &Building, &Team)>()
        .iter(world)
        .filter(|(_, building, team)| {
            **team == Team::Crimson && building.kind == BuildingKind::CommandCenter
        })
        .map(|(entity, ..)| entity)
        .collect();
    for center in enemy_centers {
        world.get_mut::<Health>(center).unwrap().current = 0.0;
    }
    battle.run_for(0.5);
    assert!(
        !battle.units(Team::Crimson, None).is_empty(),
        "enemy troops are still alive"
    );
    assert_eq!(battle.outcome(), GameOutcome::Victory);
}

#[test]
fn a_full_treasury_wins_an_economy_mission() {
    let mut battle = Battle::on_level(quiet_level(Objective::ReachCredits { credits: 9000 }));
    battle.run_for(1.0);
    assert_eq!(battle.outcome(), GameOutcome::Ongoing);
    battle.set_credits(Team::Player, 9500);
    battle.run_for(0.5);
    assert_eq!(battle.outcome(), GameOutcome::Victory);
}

#[test]
fn missions_can_withhold_units() {
    let mut battle = Battle::on_level(LevelDefinition {
        player: rimefront::missions::levels::ForceSetup {
            site: BaseSite::SouthWest,
            credits: 50_000,
            buildings: vec![
                BuildingKind::PowerPlant,
                BuildingKind::Refinery,
                BuildingKind::Factory,
                BuildingKind::Barracks,
            ],
            units: Vec::new(),
        },
        available_units: vec![UnitKind::Rifleman, UnitKind::Harvester],
        ..quiet_level(Objective::DestroyAllEnemies)
    });
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Rifleman)));
    assert!(
        !battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Tank)),
        "tanks are not part of this mission"
    );
}

#[test]
fn a_medic_patches_up_wounded_infantry() {
    let mut battle = Battle::sandbox();
    let soldier = battle.spawn_unit(UnitKind::Rifleman, Team::Player, OPEN_FIELD);
    battle.spawn_unit(UnitKind::Medic, Team::Player, OPEN_FIELD + IVec2::new(4, 0));
    battle.world().get_mut::<Health>(soldier).unwrap().current = 10.0;
    battle.run_for(12.0);
    let health = battle.world().get::<Health>(soldier).unwrap().current;
    assert!(health > 40.0, "soldier only back to {health:.0} HP");
}

#[test]
fn rocketeers_take_down_a_tank() {
    let mut battle = Battle::sandbox();
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, OPEN_FIELD + IVec2::new(5, 0));
    for row in 0..3 {
        battle.spawn_unit(
            UnitKind::Rocketeer,
            Team::Player,
            OPEN_FIELD + IVec2::new(0, row),
        );
    }
    let took = battle.run_until(60.0, |battle| !battle.alive(tank));
    assert!(took.is_some(), "the tank survived three rocketeers");
}

#[test]
fn a_bulwark_outfights_a_battle_tank() {
    let mut battle = Battle::sandbox();
    let bulwark = battle.spawn_unit(UnitKind::Bulwark, Team::Player, OPEN_FIELD);
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Crimson, OPEN_FIELD + IVec2::new(5, 0));
    let took = battle.run_until(60.0, |battle| !battle.alive(tank) || !battle.alive(bulwark));
    assert!(took.is_some(), "the duel never ended");
    assert!(battle.alive(bulwark) && !battle.alive(tank));
}

#[test]
fn a_save_from_the_pause_menu_is_written_at_once() {
    let mut battle = Battle::on_level(SkirmishOptions::default().to_level());
    battle.run_for(1.0);
    battle.world().resource_mut::<Time<Virtual>>().pause();
    battle.world().resource_mut::<SaveRequests>().save = Some(3);
    battle.run_for(0.1);
    let directory = battle.world().resource::<SaveDirectory>().clone();
    assert!(slot_summary(&directory, 3).is_some());
}

#[test]
fn a_saved_mission_reloads_with_its_level() {
    let level = SkirmishOptions {
        enemies: 2,
        map_variant: 3,
        ..default()
    }
    .to_level();
    let mut battle = Battle::on_level(level.clone());
    battle.run_for(20.0);
    battle.world().resource_mut::<SaveRequests>().save = Some(2);
    battle.run_for(0.1);
    let amber_buildings = battle.buildings(Team::Amber).len();

    battle.world().resource_mut::<SaveRequests>().load = Some(2);
    battle.run_for(0.3);
    assert_eq!(
        battle
            .world()
            .resource::<rimefront::missions::levels::ActiveLevel>()
            .0,
        level
    );
    assert_eq!(battle.buildings(Team::Amber).len(), amber_buildings);
}

#[test]
fn walls_left_standing_do_not_keep_the_enemy_alive() {
    let mut battle = Battle::on_level(LevelDefinition {
        objective: Objective::DestroyAllEnemies,
        ..default()
    });
    battle.construct(BuildingKind::Wall, Team::Crimson, IVec2::new(48, 14));
    let world = battle.world();
    let enemies: Vec<(Entity, bool)> = world
        .query::<(Entity, &Team, Option<&Building>)>()
        .iter(world)
        .filter(|(_, team, _)| **team == Team::Crimson)
        .map(|(entity, _, building)| {
            (
                entity,
                building.is_some_and(|b| b.kind == BuildingKind::Wall),
            )
        })
        .collect();
    for (entity, is_wall) in enemies {
        if !is_wall {
            world.entity_mut(entity).despawn();
        }
    }
    battle.run_for(0.5);
    assert_eq!(battle.outcome(), GameOutcome::Victory);
}

#[test]
fn walls_block_movement() {
    let mut battle = Battle::sandbox();
    let wall = IVec2::new(25, 45);
    battle.construct(BuildingKind::Wall, Team::Player, wall);
    assert!(!battle.world().resource::<GameMap>().is_passable(wall));
}

#[test]
fn capturing_the_enemy_command_center_wins_silent_takeover() {
    let level = campaign()
        .into_iter()
        .map(|entry| entry.level)
        .find(|level| level.objective == Objective::CaptureCommandCenter)
        .expect("a campaign mission asks for a capture");
    let mut battle = Battle::on_level(level);
    battle.despawn_units(Team::Crimson);
    let enemy_center = battle
        .building(Team::Crimson, BuildingKind::CommandCenter)
        .unwrap();
    let origin = battle.world().get::<Building>(enemy_center).unwrap().origin;
    let engineer = battle.spawn_unit(UnitKind::Engineer, Team::Player, origin + IVec2::new(-3, 1));
    battle
        .world()
        .entity_mut(engineer)
        .insert(EngineerTask::new(enemy_center, EngineerJob::Capture));
    let took = battle.run_until(30.0, |battle| battle.outcome() == GameOutcome::Victory);
    assert!(took.is_some(), "the capture did not win the mission");
}

#[test]
fn the_campaign_passes_through_the_seasons() {
    let seasons: Vec<Season> = campaign().iter().map(|entry| entry.level.season).collect();
    assert_eq!(seasons[0], Season::Summer);
    assert!(seasons.contains(&Season::Autumn));
    assert!(seasons.contains(&Season::Winter));
}
