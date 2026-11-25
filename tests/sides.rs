mod common;

use bevy::prelude::*;
use common::Battle;
use iron_tide::sim::catalog::{Team, UnitKind};
use iron_tide::sim::combat::Targets;
use iron_tide::sim::sides::admit_side;

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
    use iron_tide::sim::catalog::BuildingKind;
    use iron_tide::sim::player_commands::PlayerCommand;
    use iron_tide::sim::sides::Shelter;
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
