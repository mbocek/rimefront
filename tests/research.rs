mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::components::Health;
use rimefront::sim::production::{BuildItem, Production};
use rimefront::sim::research::Upgrade;

const BASE: IVec2 = IVec2::new(14, 50);
const FIELD: IVec2 = IVec2::new(30, 45);

fn researched(battle: &mut Battle, team: Team, upgrade: Upgrade) -> bool {
    battle
        .world()
        .resource::<Production>()
        .has_researched(team, upgrade)
}

fn research(battle: &mut Battle, team: Team, upgrade: Upgrade) {
    battle
        .world()
        .resource_mut::<Production>()
        .complete_research(team, upgrade);
}

#[test]
fn research_needs_a_tech_center_and_happens_once() {
    let mut battle = Battle::sandbox();
    battle.set_credits(Team::Player, 10_000);
    let armour = BuildItem::Research(Upgrade::CompositeArmour);
    assert!(!battle.enqueue(Team::Player, armour));

    battle.construct(BuildingKind::TechCenter, Team::Player, BASE);
    assert!(battle.enqueue(Team::Player, armour));
    assert!(!battle.enqueue(Team::Player, armour), "queued twice");
    let research_secs = Upgrade::CompositeArmour.spec().research_secs;
    battle.run_until(research_secs * 3.0, |battle| {
        researched(battle, Team::Player, Upgrade::CompositeArmour)
    });
    assert!(researched(
        &mut battle,
        Team::Player,
        Upgrade::CompositeArmour
    ));
    assert!(!battle.enqueue(Team::Player, armour), "researched twice");
    assert!(!researched(
        &mut battle,
        Team::Crimson,
        Upgrade::CompositeArmour
    ));
}

fn tank_health_after_a_rocketeer_volley(armoured: bool) -> f32 {
    let mut battle = Battle::sandbox();
    if armoured {
        research(&mut battle, Team::Player, Upgrade::CompositeArmour);
    }
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD);
    battle
        .world()
        .entity_mut(tank)
        .remove::<rimefront::sim::components::Weapon>();
    battle.spawn_unit(UnitKind::Rocketeer, Team::Crimson, FIELD + IVec2::new(3, 0));
    battle.run_for(6.0);
    battle
        .world()
        .get::<Health>(tank)
        .map_or(0.0, |h| h.current)
}

#[test]
fn composite_armour_softens_hits_on_vehicles() {
    let full = UnitKind::Tank.spec().max_health;
    let plain_loss = full - tank_health_after_a_rocketeer_volley(false);
    let armoured_loss = full - tank_health_after_a_rocketeer_volley(true);
    assert!(plain_loss > 0.0, "the rocketeer never fired");
    assert!(
        (armoured_loss / plain_loss - 0.8).abs() < 0.01,
        "armoured {armoured_loss} vs plain {plain_loss}"
    );
}

fn rifleman_loss_to(shooter_upgrade: Option<Upgrade>) -> f32 {
    let mut battle = Battle::sandbox();
    if let Some(upgrade) = shooter_upgrade {
        research(&mut battle, Team::Crimson, upgrade);
    }
    let target = battle.spawn_unit(UnitKind::Tank, Team::Player, FIELD);
    battle
        .world()
        .entity_mut(target)
        .remove::<rimefront::sim::components::Weapon>();
    battle.spawn_unit(UnitKind::Rifleman, Team::Crimson, FIELD + IVec2::new(3, 0));
    battle.run_for(8.0);
    UnitKind::Tank.spec().max_health
        - battle
            .world()
            .get::<Health>(target)
            .map_or(0.0, |h| h.current)
}

#[test]
fn warheads_and_drills_make_the_same_rifleman_deal_more_damage() {
    let plain = rifleman_loss_to(None);
    assert!(plain > 0.0, "the rifleman never fired");
    assert!(rifleman_loss_to(Some(Upgrade::HighExplosiveWarheads)) > plain * 1.1);
    assert!(rifleman_loss_to(Some(Upgrade::CombatDrills)) > plain * 1.2);
}
