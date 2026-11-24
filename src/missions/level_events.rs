use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::match_flow::MatchClock;
use crate::missions::levels::{ActiveLevel, BaseSite};
use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::components::{Building, Order, Orders, Unit};
use crate::sim::economy::Economies;
use crate::sim::spawn::spawn_unit;
use crate::view::assets::ModelLibrary;
use crate::world::map::{GameMap, cell_center, map_bounds};

/// Arrivals line up this many abreast, the way a starting force does.
const ARRIVAL_COLUMNS: i32 = 3;
/// How far in from the map's corner arrivals appear, so they can spread out on the map.
const ARRIVAL_INSET_CELLS: i32 = 3;

/// Something that happens during a mission once its trigger is met; each event happens once.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LevelEvent {
    pub when: Trigger,
    pub then: Vec<EventAction>,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Trigger {
    AfterMinutes {
        minutes: f32,
    },
    /// A side is down to this many structures, walls not counted.
    StructuresLeft {
        team: Team,
        at_most: u32,
    },
    UnitsLeft {
        team: Team,
        at_most: u32,
    },
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum EventAction {
    /// Shown to the player; written in English and translated like a briefing.
    Message {
        text: String,
    },
    /// Units that drive in from the map corner of `from`: the player's head for its base, an opponent's
    /// attack-move on the player's base.
    Reinforcements {
        team: Team,
        units: Vec<UnitKind>,
        from: BaseSite,
    },
    Credits {
        team: Team,
        amount: i32,
    },
}

impl LevelEvent {
    /// Every side the event names, so a level can be checked for sides it does not have.
    pub fn teams(&self) -> impl Iterator<Item = Team> + '_ {
        let trigger_team = match self.when {
            Trigger::StructuresLeft { team, .. } | Trigger::UnitsLeft { team, .. } => Some(team),
            Trigger::AfterMinutes { .. } => None,
        };
        let action_teams = self.then.iter().filter_map(|action| match action {
            EventAction::Reinforcements { team, .. } | EventAction::Credits { team, .. } => {
                Some(*team)
            }
            EventAction::Message { .. } => None,
        });
        trigger_team.into_iter().chain(action_teams)
    }
}

/// Which of the active level's events have happened, by index.
#[derive(Resource, Default, Clone, Debug)]
pub struct EventsFired(pub Vec<bool>);

/// A level event's message, for the interface to show.
#[derive(Message, Clone, Debug)]
pub struct MissionMessage(pub String);

#[allow(clippy::too_many_arguments)]
pub fn run_level_events(
    mut commands: Commands,
    level: Res<ActiveLevel>,
    clock: Res<MatchClock>,
    models: Res<ModelLibrary>,
    map: Res<GameMap>,
    mut economies: ResMut<Economies>,
    mut fired: ResMut<EventsFired>,
    mut messages: MessageWriter<MissionMessage>,
    buildings: Query<(&Building, &Team)>,
    units: Query<&Team, With<Unit>>,
) {
    let level = &level.0;
    fired.0.resize(level.events.len(), false);
    for (index, event) in level.events.iter().enumerate() {
        if fired.0[index] || !triggered(event.when, clock.elapsed_secs, &buildings, &units) {
            continue;
        }
        fired.0[index] = true;
        for action in event.then.iter().cloned() {
            match action {
                EventAction::Message { text } => {
                    messages.write(MissionMessage(text));
                }
                EventAction::Reinforcements { team, units, from } => {
                    let toward = level.player.site.origin() + level.player.site.rally_offset();
                    bring_reinforcements(&mut commands, &models, &map, team, &units, from, toward);
                }
                EventAction::Credits { team, amount } => {
                    economies.of_mut(team).credits += amount;
                }
            }
        }
    }
}

fn triggered(
    when: Trigger,
    elapsed_secs: f32,
    buildings: &Query<(&Building, &Team)>,
    units: &Query<&Team, With<Unit>>,
) -> bool {
    match when {
        Trigger::AfterMinutes { minutes } => elapsed_secs >= minutes * 60.0,
        Trigger::StructuresLeft { team, at_most } => {
            let standing = buildings
                .iter()
                .filter(|(building, owner)| **owner == team && building.kind != BuildingKind::Wall)
                .count();
            standing as u32 <= at_most
        }
        Trigger::UnitsLeft { team, at_most } => {
            units.iter().filter(|&&owner| owner == team).count() as u32 <= at_most
        }
    }
}

fn map_middle_cell() -> IVec2 {
    let bounds = map_bounds();
    bounds.min + bounds.size / 2
}

/// The cell a few steps in from the map corner nearest to a base site.
fn corner_of(site: BaseSite) -> IVec2 {
    let bounds = map_bounds();
    let first = bounds.min + IVec2::splat(ARRIVAL_INSET_CELLS);
    let last = bounds.max() - IVec2::splat(ARRIVAL_INSET_CELLS);
    let middle = map_middle_cell();
    let origin = site.origin();
    let pick = |coordinate: i32, middle: i32, first: i32, last: i32| {
        if coordinate < middle { first } else { last }
    };
    IVec2::new(
        pick(origin.x, middle.x, first.x, last.x),
        pick(origin.y, middle.y, first.y, last.y),
    )
}

fn bring_reinforcements(
    commands: &mut Commands,
    models: &ModelLibrary,
    map: &GameMap,
    team: Team,
    units: &[UnitKind],
    from: BaseSite,
    player_base: IVec2,
) {
    let corner = corner_of(from);
    let inward = (map_middle_cell() - corner).signum();
    let facing = crate::sim::components::yaw_towards(cell_center(corner), cell_center(player_base));
    let order = if team == Team::Player {
        Order::Move(player_base)
    } else {
        Order::AttackMove(player_base)
    };
    for (index, &kind) in units.iter().enumerate() {
        let index = index as i32;
        let slot = IVec2::new(index % ARRIVAL_COLUMNS, index / ARRIVAL_COLUMNS) * inward;
        let wanted = corner + slot;
        let cell = map.nearest_passable(wanted).unwrap_or(wanted);
        let unit = spawn_unit(commands, models, kind, team, cell_center(cell), facing);
        commands.entity(unit).insert(Orders {
            current: Some(order),
            queued: default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_event_reads_from_json() {
        let json = r#"{
            "when": { "AfterMinutes": { "minutes": 4.0 } },
            "then": [
                { "Message": { "text": "Bombers inbound." } },
                { "Reinforcements": { "team": "Player", "units": ["Bomber", "Bomber"], "from": "NorthWest" } },
                { "Credits": { "team": "Player", "amount": 1500 } }
            ]
        }"#;
        let event: LevelEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.when, Trigger::AfterMinutes { minutes: 4.0 });
        assert_eq!(event.then.len(), 3);
    }

    #[test]
    fn a_level_refuses_events_for_sides_it_does_not_have() {
        let mut level = crate::missions::levels::LevelDefinition::default();
        level.events.push(LevelEvent {
            when: Trigger::AfterMinutes { minutes: 1.0 },
            then: vec![EventAction::Credits {
                team: Team::Violet,
                amount: 500,
            }],
        });
        assert_eq!(level.problems().len(), 1);
    }
}
