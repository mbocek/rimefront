mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::sim::catalog::{Team, UnitKind};
use rimefront::sim::combat::Targets;
use rimefront::sim::sides::admit_side;

#[test]
fn a_side_that_joins_sees_for_itself_and_fights_everyone() {
    let mut battle = Battle::sandbox();
    let newcomer = admit_side(battle.world());
    assert_eq!(newcomer, Team::from_index(4));
    let tank = battle.spawn_unit(UnitKind::Tank, newcomer, IVec2::new(30, 45));
    let target = battle.spawn_unit(UnitKind::Rifleman, Team::Player, IVec2::new(33, 45));
    battle.run_for(0.5);
    let seen = battle
        .world()
        .resource::<Targets>()
        .get(target)
        .is_some_and(|target| target.seen_by(newcomer));
    assert!(seen, "the newcomer's tank sees nothing");

    battle.run_for(10.0);
    assert!(battle.alive(tank));
    assert!(!battle.alive(target), "the newcomer never fought");
    assert!(battle.damage_taken(Team::Player) > 0.0);
}

#[test]
fn a_sheltered_side_neither_suffers_nor_deals_harm() {
    use rimefront::sim::catalog::BuildingKind;
    use rimefront::sim::player_commands::PlayerCommand;
    use rimefront::sim::sides::Shelter;
    let mut battle = Battle::sandbox();
    let away = admit_side(battle.world());
    battle.world().resource_mut::<Shelter>().0.insert(away);
    battle.construct(BuildingKind::GunTurret, away, IVec2::new(30, 45));
    battle.spawn_unit(UnitKind::Tank, away, IVec2::new(30, 49));
    let tank = battle.spawn_unit(UnitKind::Tank, Team::Player, IVec2::new(34, 47));
    let turret = battle.building(away, BuildingKind::GunTurret).unwrap();
    let turret_id = battle.sim_id(turret);
    let tank_id = battle.sim_id(tank);
    battle.issue(
        Team::Player,
        PlayerCommand::Attack {
            units: vec![tank_id],
            target: turret_id,
        },
    );
    battle.run_for(10.0);
    assert_eq!(
        battle.damage_taken(away),
        0.0,
        "the sheltered side was hurt"
    );
    assert_eq!(
        battle.damage_taken(Team::Player),
        0.0,
        "the sheltered side fired"
    );

    battle.world().resource_mut::<Shelter>().0.remove(away);
    battle.run_for(10.0);
    assert!(battle.damage_taken(away) > 0.0, "the shelter never lifted");
}

#[test]
fn a_side_that_joins_charges_its_support_powers() {
    use rimefront::sim::support_powers::{SupportPower, SupportPowers};
    let mut battle = Battle::sandbox();
    let newcomer = admit_side(battle.world());
    battle.run_for(SupportPower::ReconSweep.recharge_secs() + 1.0);
    let powers = battle.world().resource::<SupportPowers>();
    assert!(powers.is_charged(newcomer, SupportPower::ReconSweep));
}

#[test]
fn a_retired_side_leaves_nothing_and_its_number_is_given_out_again() {
    use rimefront::sim::catalog::BuildingKind;
    use rimefront::sim::economy::Economies;
    use rimefront::sim::sides::retire_side;
    let mut battle = Battle::sandbox();
    let gone = admit_side(battle.world());
    battle.construct(BuildingKind::Wall, gone, IVec2::new(30, 45));
    battle.spawn_unit(UnitKind::Tank, gone, IVec2::new(34, 45));
    battle.set_credits(gone, 77);
    retire_side(battle.world(), gone);
    assert!(battle.buildings(gone).is_empty() && battle.units(gone, None).is_empty());

    let next = admit_side(battle.world());
    assert_eq!(next, gone);
    let starting = Economies::default().of(next).credits;
    assert_eq!(battle.credits(next), starting);

    retire_side(battle.world(), Team::Crimson);
    let after_a_level_side = admit_side(battle.world());
    assert_ne!(
        after_a_level_side,
        Team::Crimson,
        "a level's side was given out again"
    );
}

#[test]
fn a_commander_searches_the_sites_a_world_gave_out_too() {
    use rimefront::sim::intel::Intel;
    use rimefront::world::map::cell_center;
    let mut battle = Battle::sandbox();
    let claimed = IVec2::new(40, 40);
    let beside_it = cell_center(claimed + IVec2::new(-4, -4));
    let intel = battle.world().resource::<Intel>();
    let searched = intel
        .base_site_to_search(Team::Crimson, beside_it, &[claimed])
        .expect("somewhere to search");
    assert_eq!(searched, cell_center(claimed + IVec2::ONE));
    let without = intel.base_site_to_search(Team::Crimson, beside_it, &[]);
    assert_ne!(without, Some(searched));
}
