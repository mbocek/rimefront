mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use rimefront::missions::levels::{ActiveLevel, BaseSite, Difficulty, SkirmishOptions};
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::components::{Building, Health, Order, Orders, Unit};
use rimefront::sim::determinism::SimId;
use rimefront::sim::eras::Era;
use rimefront::sim::outcome::GameOutcome;
use rimefront::sim::player_commands::PlayerCommand;
use rimefront::sim::production::{BuildItem, Lane, Production, ProductionLane};
use rimefront::sim::promotion::{CALM_AFTER_THE_AGE_TURNS_SECS, near_a_producer};
use rimefront::sim::structures::{StructureOrderKind, sale_refund};
use rimefront::sim::veterancy::{Rank, Veterancy};

const BARRACKS_SITE: IVec2 = IVec2::new(14, 50);
const BESIDE_THE_BARRACKS: IVec2 = IVec2::new(18, 50);
const FAR_AWAY: IVec2 = IVec2::new(40, 30);
const FIELD: IVec2 = IVec2::new(20, 45);

fn ancient_battle(enemy_ai: bool, starting_forces: bool) -> Battle {
    let level = SkirmishOptions {
        era: Era::Antiquity,
        ..default()
    }
    .to_level();
    Battle::new(BattleSetup {
        enemy_ai,
        starting_forces,
        level: Some(level),
    })
}

fn turn_the_age(battle: &mut Battle, era: Era) {
    battle.world().resource_mut::<ActiveLevel>().0.era = era;
    battle.run_for(0.5);
}

fn kind_of(battle: &mut Battle, unit: Entity) -> UnitKind {
    battle.world().get::<Unit>(unit).expect("unit exists").0
}

fn eras_of(battle: &mut Battle, team: Team) -> Vec<Era> {
    let world = battle.world();
    world
        .query::<(&Unit, &Team)>()
        .iter(world)
        .filter(|(_, owner)| **owner == team)
        .map(|(unit, _)| unit.0.era())
        .collect()
}

fn squad(battle: &mut Battle, kind: UnitKind, team: Team, at: IVec2, count: i32) -> Vec<Entity> {
    (0..count)
        .map(|k| battle.spawn_unit(kind, team, at + IVec2::new(0, k - count / 2)))
        .collect()
}

fn survivors(battle: &mut Battle, units: &[Entity]) -> usize {
    units.iter().filter(|&&unit| battle.alive(unit)).count()
}

#[test]
fn an_ancient_battle_starts_with_an_ancient_army() {
    let mut battle = ancient_battle(false, true);
    for team in [Team::Azure, Team::Crimson] {
        let eras = eras_of(&mut battle, team);
        assert!(!eras.is_empty(), "{team:?} has no army");
        assert!(eras.iter().all(|&era| era == Era::Antiquity), "{eras:?}");
    }
}

#[test]
fn an_ancient_base_builds_only_its_own_age() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    battle.set_credits(Team::Azure, 5000);
    assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Spearman)));
    assert!(!battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Rifleman)));
    assert!(!battle.enqueue(Team::Azure, BuildItem::Structure(BuildingKind::GunTurret)));
    assert!(!battle.enqueue(Team::Azure, BuildItem::Structure(BuildingKind::Helipad)));
    assert!(battle.enqueue(Team::Azure, BuildItem::Structure(BuildingKind::Watchtower)));
}

#[test]
fn spears_stop_horsemen_of_the_same_price() {
    let mut battle = ancient_battle(false, false);
    let spearmen = squad(&mut battle, UnitKind::Spearman, Team::Azure, FIELD, 5);
    let horsemen = squad(
        &mut battle,
        UnitKind::Horseman,
        Team::Crimson,
        FIELD + IVec2::new(4, 0),
        2,
    );
    battle.run_until(60.0, |battle| {
        survivors(battle, &spearmen) == 0 || survivors(battle, &horsemen) == 0
    });
    assert_eq!(
        survivors(&mut battle, &horsemen),
        0,
        "the horsemen broke through"
    );
    assert!(survivors(&mut battle, &spearmen) > 0);
}

#[test]
fn horsemen_ride_down_archers_of_the_same_price() {
    let mut battle = ancient_battle(false, false);
    let archers = squad(&mut battle, UnitKind::Archer, Team::Azure, FIELD, 3);
    let horsemen = squad(
        &mut battle,
        UnitKind::Horseman,
        Team::Crimson,
        FIELD + IVec2::new(6, 0),
        2,
    );
    battle.run_until(60.0, |battle| {
        survivors(battle, &archers) == 0 || survivors(battle, &horsemen) == 0
    });
    assert_eq!(survivors(&mut battle, &archers), 0, "the archers held");
    assert!(survivors(&mut battle, &horsemen) > 0);
}

#[test]
fn a_new_age_refunds_the_old_queue_and_spares_structures_for_a_while() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    battle.set_credits(Team::Azure, 1000);
    for _ in 0..3 {
        assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Spearman)));
    }
    turn_the_age(&mut battle, Era::Modern);
    assert_eq!(battle.credits(Team::Azure), 1000, "the queue was refunded");
    let production = battle.world().resource::<Production>();
    assert_eq!(production.era(), Era::Modern);
    assert!(production.structures_spared());
    assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Rifleman)));
    assert!(!battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Spearman)));

    battle.run_for(CALM_AFTER_THE_AGE_TURNS_SECS);
    assert!(!battle.world().resource::<Production>().structures_spared());
}

#[test]
fn a_spared_structure_takes_no_damage() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    let barracks = battle
        .building(Team::Azure, BuildingKind::Barracks)
        .unwrap();
    turn_the_age(&mut battle, Era::Modern);
    squad(
        &mut battle,
        UnitKind::Rifleman,
        Team::Crimson,
        BARRACKS_SITE + IVec2::new(5, 0),
        4,
    );
    battle.run_for(20.0);
    let health = battle.world().get::<Health>(barracks).unwrap();
    assert_eq!(health.current, health.max);
}

#[test]
fn a_unit_is_promoted_at_its_producer_and_keeps_its_share_of_health() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    let spearman = battle.spawn_unit(UnitKind::Spearman, Team::Azure, BESIDE_THE_BARRACKS);
    {
        let mut health = battle.world().get_mut::<Health>(spearman).unwrap();
        health.current = health.max * 0.5;
    }
    turn_the_age(&mut battle, Era::Modern);
    battle.set_credits(Team::Azure, 1000);
    let id = battle.sim_id(spearman);
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id] });
    let promoted = battle.run_until(30.0, |battle| {
        kind_of(battle, spearman) == UnitKind::Rifleman
    });
    assert!(promoted.is_some(), "the spearman was never promoted");
    let price = BuildItem::Promotion {
        unit: id,
        into: UnitKind::Rifleman,
    }
    .cost();
    assert_eq!(battle.credits(Team::Azure), 1000 - price);
    assert!(price < UnitKind::Rifleman.spec().cost);
    let health = battle.world().get::<Health>(spearman).unwrap();
    assert_eq!(health.max, UnitKind::Rifleman.spec().max_health);
    assert!(
        (health.fraction() - 0.5).abs() < 0.01,
        "{}",
        health.fraction()
    );
}

#[test]
fn a_unit_far_from_its_producer_is_not_promoted() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    let spearman = battle.spawn_unit(UnitKind::Spearman, Team::Azure, FAR_AWAY);
    turn_the_age(&mut battle, Era::Modern);
    battle.set_credits(Team::Azure, 1000);
    let id = battle.sim_id(spearman);
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id] });
    battle.run_for(10.0);
    assert_eq!(kind_of(&mut battle, spearman), UnitKind::Spearman);
    assert_eq!(battle.credits(Team::Azure), 1000);
}

#[test]
fn a_watchtower_is_rebuilt_as_a_gun_turret() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    battle.construct(BuildingKind::Watchtower, Team::Azure, BESIDE_THE_BARRACKS);
    let tower = battle
        .building(Team::Azure, BuildingKind::Watchtower)
        .unwrap();
    turn_the_age(&mut battle, Era::Modern);
    battle.set_credits(Team::Azure, 1000);
    let id = battle.sim_id(tower);
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id] });
    let rebuilt = battle.run_until(30.0, |battle| {
        battle.world().get::<Building>(tower).map(|b| b.kind) == Some(BuildingKind::GunTurret)
    });
    assert!(rebuilt.is_some(), "the watchtower was never rebuilt");
}

#[test]
fn the_computer_fields_an_ancient_army_and_promotes_it_when_the_age_turns() {
    let mut battle = ancient_battle(true, true);
    battle.run_for(240.0);
    let before = eras_of(&mut battle, Team::Crimson);
    assert!(before.len() > 5, "the computer built no army: {before:?}");
    assert!(
        before.iter().all(|&era| era == Era::Antiquity),
        "{before:?}"
    );

    turn_the_age(&mut battle, Era::Modern);
    battle.set_credits(Team::Crimson, 20_000);
    let at_home = units_near_their_producers(&mut battle, Team::Crimson);
    assert!(!at_home.is_empty(), "no ancient unit waits at home");
    battle.run_for(120.0);
    let promoted = at_home
        .iter()
        .filter(|&&unit| battle.alive(unit) && kind_of(&mut battle, unit).era() == Era::Modern)
        .count();
    assert!(
        promoted * 2 >= at_home.len(),
        "only {promoted} of {} promoted",
        at_home.len()
    );
}

fn units_near_their_producers(battle: &mut Battle, team: Team) -> Vec<Entity> {
    let world = battle.world();
    let buildings: Vec<(Building, Team)> = world
        .query::<(&Building, &Team)>()
        .iter(world)
        .map(|(building, owner)| {
            let copy = Building {
                kind: building.kind,
                origin: building.origin,
            };
            (copy, *owner)
        })
        .collect();
    world
        .query::<(Entity, &Unit, &Team, &Transform)>()
        .iter(world)
        .filter(|(_, unit, owner, transform)| {
            **owner == team
                && unit.0.promoted_into(Era::Modern).is_some_and(|into| {
                    near_a_producer(
                        transform.translation,
                        into,
                        team,
                        buildings.iter().map(|(building, owner)| (building, owner)),
                    )
                })
        })
        .map(|(entity, ..)| entity)
        .collect()
}

#[test]
fn a_defence_sold_while_promoted_is_not_rebuilt_and_its_promotion_is_refunded() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    battle.construct(BuildingKind::Watchtower, Team::Azure, BESIDE_THE_BARRACKS);
    let tower = battle
        .building(Team::Azure, BuildingKind::Watchtower)
        .unwrap();
    turn_the_age(&mut battle, Era::Modern);
    battle.set_credits(Team::Azure, 1000);
    let id = battle.sim_id(tower);
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id] });
    battle.run_for(0.5);
    battle.order_structure(Team::Azure, tower, StructureOrderKind::Sell);
    let rebuilt = battle.run_until(30.0, |battle| {
        battle.world().get::<Building>(tower).map(|b| b.kind) == Some(BuildingKind::GunTurret)
    });
    assert!(rebuilt.is_none(), "a sold watchtower became a gun turret");
    let sold_for = sale_refund(BuildingKind::Watchtower);
    assert_eq!(battle.credits(Team::Azure), 1000 + sold_for);
}

#[test]
fn a_promotion_whose_unit_falls_is_refunded() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    let spearman = battle.spawn_unit(UnitKind::Spearman, Team::Azure, BESIDE_THE_BARRACKS);
    turn_the_age(&mut battle, Era::Modern);
    battle.set_credits(Team::Azure, 1000);
    let id = battle.sim_id(spearman);
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id, id] });
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id] });
    battle.run_for(0.5);
    let price = BuildItem::Promotion {
        unit: id,
        into: UnitKind::Rifleman,
    }
    .cost();
    assert_eq!(battle.credits(Team::Azure), 1000 - price, "charged once");
    battle.world().get_mut::<Health>(spearman).unwrap().current = 0.0;
    battle.run_for(1.0);
    assert!(!battle.alive(spearman));
    assert_eq!(battle.credits(Team::Azure), 1000);
    let lane = battle
        .world()
        .resource::<Production>()
        .lane(Team::Azure, Lane::Infantry);
    assert!(lane.queue.is_empty());
}

#[test]
fn a_veteran_keeps_its_rank_and_its_bonus_health_when_promoted() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    let spearman = battle.spawn_unit(UnitKind::Spearman, Team::Azure, BESIDE_THE_BARRACKS);
    let experience = UnitKind::Spearman.spec().cost as f32 * 3.0;
    let (veterancy, health) = Veterancy::restored(UnitKind::Spearman, experience, 1000.0);
    let rank = veterancy.rank;
    assert_ne!(rank, Rank::Recruit);
    battle
        .world()
        .entity_mut(spearman)
        .insert((veterancy, health));
    turn_the_age(&mut battle, Era::Modern);
    battle.set_credits(Team::Azure, 1000);
    let id = battle.sim_id(spearman);
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id] });
    battle.run_until(30.0, |battle| {
        kind_of(battle, spearman) == UnitKind::Rifleman
    });
    let promoted = battle.world().get::<Veterancy>(spearman).copied().unwrap();
    assert_eq!(promoted.rank, rank);
    assert_eq!(Rank::earned(UnitKind::Rifleman, promoted.experience), rank);
    let max = battle.world().get::<Health>(spearman).unwrap().max;
    assert_eq!(
        max,
        UnitKind::Rifleman.spec().max_health * rank.health_factor()
    );
}

#[test]
fn a_restored_queue_gives_back_its_promotions() {
    let mut lane = ProductionLane::default();
    let promotion = BuildItem::Promotion {
        unit: SimId(9),
        into: UnitKind::Rifleman,
    };
    let rifleman = BuildItem::Unit(UnitKind::Rifleman);
    let mut credits = 0;
    lane.restore([promotion, rifleman], 1.5, None, &mut credits);
    assert_eq!(lane.queue, [rifleman]);
    assert_eq!(lane.progress_secs, 0.0, "the progress was the promotion's");
    assert_eq!(credits, promotion.cost());
}

#[test]
fn an_ancient_battle_between_computers_is_fought_to_the_end() {
    let mut battle = ancient_battle(true, true);
    battle.add_ai(Team::Azure, BaseSite::SouthWest.origin(), Difficulty::Hard);
    let over = battle.run_until(60.0 * 60.0, |battle| {
        battle.outcome() != GameOutcome::Ongoing || both_sides_exhausted(battle)
    });
    assert!(over.is_some(), "an hour passed and both sides still fight");
    for team in [Team::Azure, Team::Crimson] {
        let eras = eras_of(&mut battle, team);
        assert!(eras.iter().all(|&era| era == Era::Antiquity), "{eras:?}");
    }
}

/// Both sides out of units and too poor for a harvester of their age: the mutual exhaustion docs/balance.md
/// describes.
fn both_sides_exhausted(battle: &mut Battle) -> bool {
    let era = battle.world().resource::<ActiveLevel>().0.era;
    let harvester = UnitKind::Harvester.in_era(era).expect("every age harvests");
    [Team::Azure, Team::Crimson].into_iter().all(|team| {
        battle.units(team, None).is_empty() && battle.credits(team) < harvester.spec().cost
    })
}

fn gunpowder_battle(enemy_ai: bool, starting_forces: bool) -> Battle {
    let level = SkirmishOptions {
        era: Era::Gunpowder,
        ..default()
    }
    .to_level();
    Battle::new(BattleSetup {
        enemy_ai,
        starting_forces,
        level: Some(level),
    })
}

#[test]
fn a_gunpowder_base_fields_and_builds_its_own_age() {
    let mut battle = gunpowder_battle(false, true);
    let eras = eras_of(&mut battle, Team::Azure);
    assert!(eras.iter().all(|&era| era == Era::Gunpowder), "{eras:?}");
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    battle.set_credits(Team::Azure, 5000);
    assert!(battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Musketeer)));
    assert!(!battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Spearman)));
    assert!(!battle.enqueue(Team::Azure, BuildItem::Unit(UnitKind::Rifleman)));
    assert!(!battle.enqueue(Team::Azure, BuildItem::Structure(BuildingKind::Watchtower)));
    assert!(battle.enqueue(Team::Azure, BuildItem::Structure(BuildingKind::CannonTower)));
    let palisade = BuildItem::Structure(BuildingKind::Palisade);
    assert!(
        battle
            .world()
            .resource::<Production>()
            .allows(Team::Azure, palisade)
    );
}

#[test]
fn the_palisade_lasts_through_the_age_of_gunpowder_and_becomes_a_wall_after_it() {
    assert_eq!(BuildingKind::Palisade.promoted_into(Era::Gunpowder), None);
    assert_eq!(
        BuildingKind::Palisade.promoted_into(Era::Modern),
        Some(BuildingKind::Wall)
    );
    assert_eq!(
        BuildingKind::Watchtower.promoted_into(Era::Gunpowder),
        Some(BuildingKind::CannonTower)
    );
    assert_eq!(
        UnitKind::Spearman.promoted_into(Era::Gunpowder),
        Some(UnitKind::Musketeer)
    );
    assert_eq!(
        UnitKind::Musketeer.promoted_into(Era::Modern),
        Some(UnitKind::Rifleman)
    );
    assert_eq!(
        Era::Antiquity.next_playable(),
        Some(Era::Gunpowder),
        "a world goes through the age of gunpowder"
    );
}

#[test]
fn spearmen_become_musketeers_when_gunpowder_comes() {
    let mut battle = ancient_battle(false, false);
    battle.construct(BuildingKind::Barracks, Team::Azure, BARRACKS_SITE);
    let spearman = battle.spawn_unit(UnitKind::Spearman, Team::Azure, BESIDE_THE_BARRACKS);
    turn_the_age(&mut battle, Era::Gunpowder);
    battle.set_credits(Team::Azure, 1000);
    let id = battle.sim_id(spearman);
    battle.issue(Team::Azure, PlayerCommand::Promote { ids: vec![id] });
    let promoted = battle.run_until(30.0, |battle| {
        battle.world().get::<Unit>(spearman).map(|unit| unit.0) == Some(UnitKind::Musketeer)
    });
    assert!(promoted.is_some(), "the spearman was never promoted");
}

/// A block of `count` units, four abreast, attack-moving towards `towards`, as the balance tool's duels deploy.
fn block_attacking(
    battle: &mut Battle,
    kind: UnitKind,
    team: Team,
    at: IVec2,
    count: i32,
    towards: IVec2,
) -> Vec<Entity> {
    (0..count)
        .map(|k| {
            let unit = battle.spawn_unit(kind, team, at + IVec2::new(k % 4 - 2, k / 4 - 1));
            battle
                .world()
                .get_mut::<Orders>(unit)
                .unwrap()
                .replace_with(Order::AttackMove(towards));
            unit
        })
        .collect()
}

#[test]
fn a_line_of_muskets_stops_cuirassiers_of_the_same_price() {
    let mut battle = gunpowder_battle(false, false);
    let (west, east) = (IVec2::new(22, 46), IVec2::new(38, 46));
    let line = block_attacking(
        &mut battle,
        UnitKind::Musketeer,
        Team::Azure,
        west,
        22,
        east,
    );
    let riders = block_attacking(
        &mut battle,
        UnitKind::Cuirassier,
        Team::Crimson,
        east,
        4,
        west,
    );
    battle.run_until(150.0, |battle| {
        survivors(battle, &line) == 0 || survivors(battle, &riders) == 0
    });
    assert_eq!(
        survivors(&mut battle, &riders),
        0,
        "the cuirassiers broke the line"
    );
    assert!(survivors(&mut battle, &line) > 0);
}

#[test]
fn a_gunpowder_battle_between_computers_is_fought_to_the_end() {
    let mut battle = gunpowder_battle(true, true);
    battle.add_ai(Team::Azure, BaseSite::SouthWest.origin(), Difficulty::Hard);
    let over = battle.run_until(60.0 * 60.0, |battle| {
        battle.outcome() != GameOutcome::Ongoing || both_sides_exhausted(battle)
    });
    assert!(over.is_some(), "an hour passed and both sides still fight");
    for team in [Team::Azure, Team::Crimson] {
        let eras = eras_of(&mut battle, team);
        assert!(eras.iter().all(|&era| era == Era::Gunpowder), "{eras:?}");
    }
}

fn industrial_battle(enemy_ai: bool, starting_forces: bool) -> Battle {
    let level = SkirmishOptions {
        era: Era::Industrial,
        ..default()
    }
    .to_level();
    Battle::new(BattleSetup {
        enemy_ai,
        starting_forces,
        level: Some(level),
    })
}

#[test]
fn the_industrial_age_takes_to_the_air_and_builds_flak_and_walls() {
    let mut battle = industrial_battle(false, true);
    let eras = eras_of(&mut battle, Team::Azure);
    assert!(eras.iter().all(|&era| era == Era::Industrial), "{eras:?}");
    let production = battle.world().resource::<Production>();
    for allowed in [
        BuildItem::Structure(BuildingKind::Helipad),
        BuildItem::Structure(BuildingKind::FlakCannon),
        BuildItem::Structure(BuildingKind::Wall),
        BuildItem::Structure(BuildingKind::Pillbox),
        BuildItem::Unit(UnitKind::Biplane),
        BuildItem::Unit(UnitKind::BiplaneBomber),
    ] {
        assert!(production.allows(Team::Azure, allowed), "{allowed:?}");
    }
    for forbidden in [
        BuildItem::Structure(BuildingKind::Palisade),
        BuildItem::Structure(BuildingKind::GunTurret),
        BuildItem::Unit(UnitKind::Fighter),
    ] {
        assert!(!production.allows(Team::Azure, forbidden), "{forbidden:?}");
    }
    assert!(UnitKind::Biplane.lands_on_pad());
    assert!(UnitKind::BiplaneBomber.rearms_at_pad());
}

#[test]
fn every_age_on_the_way_promotes_into_the_next() {
    let ages = [Era::Antiquity, Era::Gunpowder, Era::Industrial, Era::Modern];
    assert_eq!(Era::playable().collect::<Vec<_>>(), ages);
    let mut infantry = UnitKind::Spearman;
    for &era in &ages[1..] {
        infantry = infantry.promoted_into(era).expect("infantry in every age");
        assert_eq!(infantry.era(), era);
    }
    assert_eq!(infantry, UnitKind::Rifleman);
    assert_eq!(
        BuildingKind::CannonTower.promoted_into(Era::Industrial),
        Some(BuildingKind::Pillbox)
    );
    assert_eq!(
        BuildingKind::Palisade.promoted_into(Era::Industrial),
        Some(BuildingKind::Wall)
    );
    assert_eq!(BuildingKind::Wall.promoted_into(Era::Modern), None);
}

#[test]
fn an_industrial_battle_between_computers_is_fought_to_the_end() {
    let mut battle = industrial_battle(true, true);
    battle.add_ai(Team::Azure, BaseSite::SouthWest.origin(), Difficulty::Hard);
    let over = battle.run_until(60.0 * 60.0, |battle| {
        battle.outcome() != GameOutcome::Ongoing || both_sides_exhausted(battle)
    });
    assert!(over.is_some(), "an hour passed and both sides still fight");
}
