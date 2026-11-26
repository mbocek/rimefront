mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use rimefront::missions::levels::{BaseSite, Difficulty, LevelDefinition};
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::components::{Building, Health};
use rimefront::sim::neutral_sites::UnclaimedSite;
use rimefront::sim::player_commands::PlayerCommand;
use rimefront::world::map::opposite_origin;

fn level_with_sites() -> LevelDefinition {
    LevelDefinition {
        neutral_sites: true,
        fog_of_war: false,
        ..LevelDefinition::default()
    }
}

fn field_with_sites() -> Battle {
    Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(level_with_sites()),
    })
}

fn unclaimed_sites(battle: &mut Battle) -> Vec<(Entity, BuildingKind, IVec2)> {
    let world = battle.world();
    world
        .query_filtered::<(Entity, &Building), With<UnclaimedSite>>()
        .iter(world)
        .map(|(entity, building)| (entity, building.kind, building.origin))
        .collect()
}

fn held_by(battle: &mut Battle, kind: BuildingKind) -> Vec<(Entity, Team)> {
    let world = battle.world();
    world
        .query::<(Entity, &Building, &Team)>()
        .iter(world)
        .filter(|(_, building, _)| building.kind == kind)
        .map(|(entity, _, &team)| (entity, team))
        .collect()
}

fn site_of(battle: &mut Battle, kind: BuildingKind) -> (Entity, IVec2) {
    let (entity, _, origin) = unclaimed_sites(battle)
        .into_iter()
        .find(|(_, found, _)| *found == kind)
        .expect("the map has this site");
    (entity, origin)
}

/// Sends a fresh engineer from beside the site to take it, and waits until it is in.
fn capture(battle: &mut Battle, team: Team, kind: BuildingKind) -> Entity {
    let (site, origin) = site_of(battle, kind);
    let engineer = battle.spawn_unit(UnitKind::Engineer, team, origin + IVec2::new(-2, 0));
    let units = vec![battle.sim_id(engineer)];
    let structure = battle.sim_id(site);
    battle.issue(team, PlayerCommand::SendEngineers { units, structure });
    battle.run_until(30.0, |battle| !held_by(battle, kind).is_empty());
    let (captured, owner) = held_by(battle, kind)[0];
    assert_eq!(owner, team);
    captured
}

#[test]
fn a_map_with_sites_has_observation_posts_and_mirrored_pairs() {
    let mut battle = field_with_sites();
    let sites = unclaimed_sites(&mut battle);
    let count = |kind| sites.iter().filter(|(_, found, _)| *found == kind).count();
    assert!(
        (1..=2).contains(&count(BuildingKind::ObservationPost)),
        "one post in the middle, or a pair beside a ford there"
    );
    assert_eq!(count(BuildingKind::SupplyDepot), 2);
    assert_eq!(count(BuildingKind::FieldHospital), 2);
    for &(_, kind, origin) in &sites {
        let twin = opposite_origin(origin, kind.spec().footprint);
        assert!(
            sites
                .iter()
                .any(|&(_, other, at)| other == kind && at == twin),
            "{kind:?} at {origin} has no mirror image"
        );
    }
}

#[test]
fn levels_without_sites_stay_as_they_were() {
    let mut battle = Battle::sandbox();
    assert!(unclaimed_sites(&mut battle).is_empty());
}

#[test]
fn a_captured_supply_depot_pays_its_owner() {
    let mut battle = field_with_sites();
    capture(&mut battle, Team::Azure, BuildingKind::SupplyDepot);
    let before = battle.credits(Team::Azure);
    battle.run_for(20.0);
    let earned = battle.credits(Team::Azure) - before;
    assert!((150..=250).contains(&earned), "earned {earned} in 20 s");
}

#[test]
fn a_captured_field_hospital_heals_its_owners_infantry() {
    let mut battle = field_with_sites();
    capture(&mut battle, Team::Azure, BuildingKind::FieldHospital);
    let (_, origin) = held_by(&mut battle, BuildingKind::FieldHospital)
        .first()
        .map(|&(entity, _)| (entity, battle_origin(&mut battle, entity)))
        .unwrap();
    let wounded = battle.spawn_unit(UnitKind::Rifleman, Team::Azure, origin + IVec2::new(-2, 0));
    battle.world().get_mut::<Health>(wounded).unwrap().current = 10.0;
    battle.run_for(5.0);
    let health = battle.world().get::<Health>(wounded).unwrap().current;
    assert!(health > 30.0, "healed only to {health}");
}

fn battle_origin(battle: &mut Battle, building: Entity) -> IVec2 {
    battle.world().get::<Building>(building).unwrap().origin
}

#[test]
fn a_destroyed_site_returns_to_nobody() {
    let mut battle = field_with_sites();
    let depot = capture(&mut battle, Team::Azure, BuildingKind::SupplyDepot);
    let origin = battle_origin(&mut battle, depot);
    battle.world().get_mut::<Health>(depot).unwrap().current = 0.0;
    battle.run_for(1.0);
    assert!(held_by(&mut battle, BuildingKind::SupplyDepot).is_empty());
    let back = unclaimed_sites(&mut battle)
        .into_iter()
        .any(|(_, kind, at)| kind == BuildingKind::SupplyDepot && at == origin);
    assert!(back, "the depot did not return to nobody");
}

#[test]
fn the_computer_sends_an_engineer_to_take_a_site() {
    let mut battle = Battle::on_level(level_with_sites());
    battle.add_ai(Team::Azure, BaseSite::SouthWest.origin(), Difficulty::Hard);
    let taken = battle.run_until(8.0 * 60.0, |battle| {
        BuildingKind::NEUTRAL_SITES
            .into_iter()
            .any(|kind| !held_by(battle, kind).is_empty())
    });
    assert!(
        taken.is_some(),
        "no commander captured a site in eight minutes"
    );
}

#[test]
fn sites_never_cut_the_bases_off_from_each_other() {
    use rimefront::missions::random_battle::{RandomBattleOptions, generate};
    use rimefront::world::map::{GameMap, base_sites};
    for seed in 1..=8 {
        let level = generate(&RandomBattleOptions::default(), seed);
        let mut battle = Battle::new(BattleSetup {
            enemy_ai: false,
            starting_forces: false,
            level: Some(level),
        });
        assert!(
            !unclaimed_sites(&mut battle).is_empty(),
            "seed {seed} has no sites"
        );
        let [south_west, north_east, ..] = base_sites();
        let map = battle.world().resource::<GameMap>();
        assert!(
            map.find_path(south_west, north_east).is_some(),
            "on seed {seed} the sites cut the bases apart"
        );
    }
}
