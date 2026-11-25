mod common;

use bevy::prelude::*;
use common::Battle;
use rimefront::missions::level_events::{EventAction, LevelEvent, Trigger};
use rimefront::missions::levels::{BaseSite, LevelDefinition};
use rimefront::sim::catalog::{BuildingKind, Team, UnitKind};
use rimefront::sim::components::{Building, Health};
use rimefront::world::map::cell_center;

fn level_with(events: Vec<LevelEvent>) -> LevelDefinition {
    let mut level = LevelDefinition {
        events,
        ..default()
    };
    level.enemies[0].force.units.clear();
    level
}

#[test]
fn reinforcements_and_credits_arrive_on_time_and_head_for_the_base() {
    let mut battle = Battle::on_level(level_with(vec![LevelEvent {
        when: Trigger::AfterMinutes { minutes: 0.1 },
        then: vec![
            EventAction::Message {
                text: "Armour is on its way.".into(),
            },
            EventAction::Reinforcements {
                team: Team::Player,
                units: vec![UnitKind::Bulwark, UnitKind::Bulwark],
                from: BaseSite::NorthWest,
            },
            EventAction::Credits {
                team: Team::Player,
                amount: 1000,
            },
        ],
    }]));
    battle.run_for(5.0);
    assert!(
        battle
            .units(Team::Player, Some(UnitKind::Bulwark))
            .is_empty()
    );
    let credits_before = battle.credits(Team::Player);
    battle.run_for(3.0);
    let bulwarks = battle.units(Team::Player, Some(UnitKind::Bulwark));
    assert_eq!(bulwarks.len(), 2);
    assert!(battle.credits(Team::Player) >= credits_before + 1000);

    let base = cell_center(BaseSite::SouthWest.origin());
    let arrived_at = battle.position(bulwarks[0]).distance(base);
    battle.run_for(20.0);
    assert!(
        battle.position(bulwarks[0]).distance(base) < arrived_at - 10.0,
        "the reinforcements did not head for the base"
    );
    battle.run_for(10.0);
    assert_eq!(
        battle.units(Team::Player, Some(UnitKind::Bulwark)).len(),
        2,
        "an event happens only once"
    );
}

#[test]
fn losing_a_base_can_call_in_the_enemy_reserve() {
    let mut battle = Battle::on_level(level_with(vec![LevelEvent {
        when: Trigger::StructuresLeft {
            team: Team::Crimson,
            at_most: 0,
        },
        then: vec![EventAction::Reinforcements {
            team: Team::Crimson,
            units: vec![UnitKind::RaiderBike, UnitKind::RaiderBike],
            from: BaseSite::NorthEast,
        }],
    }]));
    battle.run_for(2.0);
    assert!(
        battle
            .units(Team::Crimson, Some(UnitKind::RaiderBike))
            .is_empty()
    );
    let world = battle.world();
    let structures: Vec<Entity> = world
        .query::<(Entity, &Building, &Team)>()
        .iter(world)
        .filter(|(_, building, team)| {
            **team == Team::Crimson && building.kind != BuildingKind::Wall
        })
        .map(|(entity, ..)| entity)
        .collect();
    for structure in structures {
        world.get_mut::<Health>(structure).unwrap().current = 0.0;
    }
    battle.run_for(1.0);
    assert_eq!(
        battle
            .units(Team::Crimson, Some(UnitKind::RaiderBike))
            .len(),
        2
    );
}
