mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::components::Health;
use rimefront::sim::production::BuildItem;

const FIELD: IVec2 = IVec2::new(20, 45);
const BASE: IVec2 = IVec2::new(14, 50);

fn battle_with_full_base(team: Team) -> Battle {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::Barracks, team, BASE);
    battle.construct(BuildingKind::Factory, team, BASE + IVec2::new(0, 3));
    battle.construct(BuildingKind::TechCenter, team, BASE + IVec2::new(4, 0));
    battle.set_credits(team, 20_000);
    battle
}

#[test]
fn the_coalition_cannot_order_syndicate_arms() {
    let mut battle = battle_with_full_base(Team::Player);
    for item in [
        BuildItem::Unit(UnitKind::FlameTrooper),
        BuildItem::Unit(UnitKind::MissileTruck),
        BuildItem::Structure(BuildingKind::RocketBunker),
    ] {
        assert!(!battle.enqueue(Team::Player, item), "{item:?} was accepted");
    }
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Sniper)));
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Bulwark)));
}

#[test]
fn the_syndicate_cannot_order_coalition_arms() {
    let mut battle = battle_with_full_base(Team::Crimson);
    for item in [
        BuildItem::Unit(UnitKind::Sniper),
        BuildItem::Unit(UnitKind::Bulwark),
    ] {
        assert!(
            !battle.enqueue(Team::Crimson, item),
            "{item:?} was accepted"
        );
    }
    assert!(battle.enqueue(Team::Crimson, BuildItem::Unit(UnitKind::FlameTrooper)));
    assert!(battle.enqueue(Team::Crimson, BuildItem::Unit(UnitKind::MissileTruck)));
    assert!(battle.enqueue(
        Team::Crimson,
        BuildItem::Structure(BuildingKind::RocketBunker)
    ));
}

fn health(battle: &mut Battle, entity: Entity) -> f32 {
    battle
        .world()
        .get::<Health>(entity)
        .map_or(0.0, |h| h.current)
}

#[test]
fn a_flame_trooper_burns_a_rifle_squad_at_close_range() {
    let mut battle = Battle::sandbox();
    battle.spawn_unit(UnitKind::FlameTrooper, Team::Crimson, FIELD);
    let riflemen: Vec<Entity> = (0..3)
        .map(|k| {
            battle.spawn_unit(
                UnitKind::Rifleman,
                Team::Player,
                FIELD + IVec2::new(2, k - 1),
            )
        })
        .collect();
    battle.run_until(15.0, |battle| riflemen.iter().all(|&r| !battle.alive(r)));
    assert!(
        riflemen.iter().all(|&r| !battle.alive(r)),
        "the riflemen survived the flames"
    );
}

#[test]
fn a_rocket_bunker_shoots_down_aircraft() {
    let mut battle = Battle::sandbox();
    battle.construct(BuildingKind::RocketBunker, Team::Crimson, FIELD);
    let gunship = battle.spawn_unit(UnitKind::Gunship, Team::Player, FIELD + IVec2::new(4, 0));
    let full = health(&mut battle, gunship);
    battle.run_for(6.0);
    assert!(
        health(&mut battle, gunship) < full,
        "the bunker could not hit the gunship"
    );
}

#[test]
fn a_missile_truck_outranges_a_gun_turret() {
    let turret_range = BuildingKind::GunTurret.spec().weapon.unwrap().range;
    let truck_range = UnitKind::MissileTruck.spec().weapon.unwrap().range;
    assert!(truck_range > turret_range + 4.0);
}

#[test]
fn a_syndicate_campaign_level_swaps_the_armouries() {
    let level = rimefront::missions::levels::campaign()
        .into_iter()
        .map(|entry| entry.level)
        .find(|level| level.player_faction == rimefront::sim::catalog::Faction::Syndicate)
        .expect("the campaign has Syndicate missions");
    let mut battle = Battle::on_level(level);
    battle.set_credits(Team::Player, 20_000);
    battle.set_credits(Team::Crimson, 20_000);
    assert!(battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::FlameTrooper)));
    assert!(!battle.enqueue(Team::Player, BuildItem::Unit(UnitKind::Sniper)));
    assert!(battle.enqueue(Team::Crimson, BuildItem::Unit(UnitKind::Sniper)));
    assert!(!battle.enqueue(Team::Crimson, BuildItem::Unit(UnitKind::FlameTrooper)));
}
