mod common;

use bevy::prelude::*;
use common::{Battle, BattleSetup};
use rimefront::missions::levels::{BaseSite, Difficulty, LevelDefinition};
use rimefront::sim::catalog::{BuildingKind, Team};
use rimefront::sim::combat::DamageSource;
use rimefront::sim::components::{Building, Health};
use rimefront::sim::intel::Intel;
use rimefront::sim::player_commands::PlayerCommand;
use rimefront::sim::support_powers::SupportPower;
use rimefront::world::map::cell_center;

const HOME: IVec2 = IVec2::new(8, 50);
const TARGET: IVec2 = IVec2::new(40, 20);

fn field(fog_of_war: bool) -> Battle {
    Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(LevelDefinition {
            fog_of_war,
            ..LevelDefinition::default()
        }),
    })
}

fn health_of(battle: &mut Battle, team: Team, kind: BuildingKind) -> f32 {
    let world = battle.world();
    world
        .query::<(&Building, &Team, &Health)>()
        .iter(world)
        .find(|(building, owner, _)| building.kind == kind && **owner == team)
        .map_or(0.0, |(.., health)| health.current)
}

fn call(battle: &mut Battle, power: SupportPower, cell: IVec2) {
    battle.issue(Team::Player, PlayerCommand::CallSupport { power, cell });
}

#[test]
fn a_barrage_needs_a_tech_center_and_its_charge_then_hits_the_spot() {
    let mut battle = field(false);
    battle.construct(BuildingKind::CommandCenter, Team::Player, HOME);
    battle.construct(BuildingKind::PowerPlant, Team::Crimson, TARGET);
    let intact = BuildingKind::PowerPlant.spec().max_health;
    battle.run_for(SupportPower::Barrage.recharge_secs() + 1.0);
    call(&mut battle, SupportPower::Barrage, TARGET);
    battle.run_for(8.0);
    assert_eq!(
        health_of(&mut battle, Team::Crimson, BuildingKind::PowerPlant),
        intact,
        "a barrage came without a Tech Center"
    );
    battle.construct(
        BuildingKind::TechCenter,
        Team::Player,
        HOME + IVec2::new(4, 0),
    );
    battle.run_for(0.5);
    call(&mut battle, SupportPower::Barrage, TARGET);
    battle.run_for(8.0);
    assert!(health_of(&mut battle, Team::Crimson, BuildingKind::PowerPlant) < intact);
}

#[test]
fn a_recon_flight_reveals_the_spot_for_a_while() {
    let mut battle = field(true);
    battle.construct(BuildingKind::CommandCenter, Team::Player, HOME);
    let spot = cell_center(TARGET);
    let seen = |battle: &mut Battle| {
        battle
            .world()
            .resource::<Intel>()
            .vision(Team::Player)
            .is_visible(spot)
    };
    battle.run_for(SupportPower::ReconSweep.recharge_secs() + 1.0);
    assert!(!seen(&mut battle));
    call(&mut battle, SupportPower::ReconSweep, TARGET);
    battle.run_for(1.0);
    assert!(seen(&mut battle), "the recon flight showed nothing");
    battle.run_for(25.0);
    assert!(!seen(&mut battle), "the recon flight never ended");
}

#[test]
fn computer_commanders_call_barrages() {
    let mut battle = Battle::standard();
    battle.add_ai(Team::Player, BaseSite::SouthWest.origin(), Difficulty::Hard);
    battle.run_for(12.0 * 60.0);
    let ledger = battle.ledger();
    let by_support = |team| {
        ledger
            .dealt
            .get(&(team, DamageSource::Support))
            .copied()
            .unwrap_or(0.0)
    };
    assert!(
        by_support(Team::Player) + by_support(Team::Crimson) > 0.0,
        "no commander called a barrage in twelve minutes"
    );
}
