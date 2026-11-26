use crate::missions::level_events::{EventAction, LevelEvent, Trigger};
use crate::missions::levels::{BaseSite, LevelDefinition};
use crate::sim::catalog::{Faction, Team, UnitKind};

pub const MOST_EVENTS: usize = 8;
pub const LONGEST_MESSAGE: usize = 160;
const LONGEST_WAIT_MINUTES: f32 = 60.0;
const MOST_LEFT: u32 = 40;
const CREDIT_STEP: i32 = 500;
const MOST_CREDITS: i32 = 20_000;

/// Ready-made reinforcements, so a player making a level picks a force instead of listing units.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ForcePreset {
    Infantry,
    Armour,
    AirWing,
}

impl ForcePreset {
    pub const ALL: [ForcePreset; 3] = [
        ForcePreset::Infantry,
        ForcePreset::Armour,
        ForcePreset::AirWing,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ForcePreset::Infantry => "Infantry squad",
            ForcePreset::Armour => "Armour platoon",
            ForcePreset::AirWing => "Air wing",
        }
    }

    /// Each faction sends its own kind of force: the Coalition its Bulwarks and Bombers, the Syndicate its
    /// Stealth Tanks and Fighters.
    pub fn units(self, faction: Faction) -> Vec<UnitKind> {
        use UnitKind::*;
        match (self, faction) {
            (ForcePreset::Infantry, Faction::Coalition) => {
                vec![Rifleman, Rifleman, Rifleman, Rocketeer, Rocketeer, Medic]
            }
            (ForcePreset::Infantry, Faction::Syndicate) => {
                vec![
                    Rifleman,
                    Rifleman,
                    Rifleman,
                    Rocketeer,
                    Rocketeer,
                    FlameTrooper,
                ]
            }
            (ForcePreset::Armour, Faction::Coalition) => vec![Tank, Tank, Bulwark, RepairTruck],
            (ForcePreset::Armour, Faction::Syndicate) => {
                vec![Tank, Tank, StealthTank, MissileTruck]
            }
            (ForcePreset::AirWing, Faction::Coalition) => vec![Gunship, Gunship, Bomber],
            (ForcePreset::AirWing, Faction::Syndicate) => vec![Gunship, Gunship, Fighter],
        }
    }

    pub fn of(units: &[UnitKind], faction: Faction) -> Option<ForcePreset> {
        ForcePreset::ALL
            .into_iter()
            .find(|preset| preset.units(faction) == units)
    }
}

/// The sides a level has, the player first: the ones an event can name.
pub fn sides(level: &LevelDefinition) -> Vec<Team> {
    std::iter::once(Team::Azure)
        .chain(level.opponents().map(|(team, _)| team))
        .collect()
}

fn next_side(level: &LevelDefinition, team: Team) -> Team {
    let sides = sides(level);
    let at = sides.iter().position(|&side| side == team).unwrap_or(0);
    sides[(at + 1) % sides.len()]
}

pub fn add_event(level: &mut LevelDefinition) -> Option<usize> {
    if level.events.len() >= MOST_EVENTS {
        return None;
    }
    level.events.push(LevelEvent {
        when: Trigger::AfterMinutes { minutes: 5.0 },
        then: Vec::new(),
    });
    Some(level.events.len() - 1)
}

pub fn remove_event(level: &mut LevelDefinition, index: usize) {
    if index < level.events.len() {
        level.events.remove(index);
    }
}

/// After some minutes, then a side down to so many structures, then to so many units.
pub fn cycle_trigger(level: &mut LevelDefinition, index: usize) {
    let enemy = sides(level).get(1).copied().unwrap_or(Team::Crimson);
    let Some(event) = level.events.get_mut(index) else {
        return;
    };
    event.when = match event.when {
        Trigger::AfterMinutes { .. } => Trigger::StructuresLeft {
            team: enemy,
            at_most: 3,
        },
        Trigger::StructuresLeft { team, .. } => Trigger::UnitsLeft { team, at_most: 5 },
        Trigger::UnitsLeft { .. } => Trigger::AfterMinutes { minutes: 5.0 },
    };
}

/// Moves the trigger's number: minutes for a timed event, what is left for the others.
pub fn adjust_trigger(level: &mut LevelDefinition, index: usize, step: i32) {
    let Some(event) = level.events.get_mut(index) else {
        return;
    };
    match &mut event.when {
        Trigger::AfterMinutes { minutes } => {
            *minutes = (*minutes + step as f32).clamp(1.0, LONGEST_WAIT_MINUTES);
        }
        Trigger::StructuresLeft { at_most, .. } | Trigger::UnitsLeft { at_most, .. } => {
            *at_most = at_most.saturating_add_signed(step).min(MOST_LEFT);
        }
    }
}

pub fn cycle_trigger_side(level: &mut LevelDefinition, index: usize) {
    let Some(team) = level.events.get(index).and_then(|event| match event.when {
        Trigger::StructuresLeft { team, .. } | Trigger::UnitsLeft { team, .. } => Some(team),
        Trigger::AfterMinutes { .. } => None,
    }) else {
        return;
    };
    let next = next_side(level, team);
    if let Some(event) = level.events.get_mut(index)
        && let Trigger::StructuresLeft { team, .. } | Trigger::UnitsLeft { team, .. } =
            &mut event.when
    {
        *team = next;
    }
}

/// The event's message, if it has one.
pub fn message(event: &LevelEvent) -> Option<&str> {
    event.then.iter().find_map(|action| match action {
        EventAction::Message { text } => Some(text.as_str()),
        _ => None,
    })
}

/// The message to type into, created empty when the event has none; an empty message is dropped on save.
pub fn message_mut(level: &mut LevelDefinition, index: usize) -> Option<&mut String> {
    let event = level.events.get_mut(index)?;
    let at = match event
        .then
        .iter()
        .position(|action| matches!(action, EventAction::Message { .. }))
    {
        Some(at) => at,
        None => {
            event.then.insert(
                0,
                EventAction::Message {
                    text: String::new(),
                },
            );
            0
        }
    };
    match &mut event.then[at] {
        EventAction::Message { text } => Some(text),
        _ => None,
    }
}

/// What the event does besides speaking: its first action that is not a message.
pub fn main_action(event: &LevelEvent) -> Option<&EventAction> {
    event
        .then
        .iter()
        .find(|action| !matches!(action, EventAction::Message { .. }))
}

fn main_action_mut(event: &mut LevelEvent) -> Option<&mut EventAction> {
    event
        .then
        .iter_mut()
        .find(|action| !matches!(action, EventAction::Message { .. }))
}

/// Nothing, then reinforcements for the player, then credits for the player, then nothing again.
pub fn cycle_action(level: &mut LevelDefinition, index: usize) {
    let faction = level.player_faction;
    let corner = level.player.site;
    let Some(event) = level.events.get_mut(index) else {
        return;
    };
    let next = match main_action(event) {
        None => Some(EventAction::Reinforcements {
            team: Team::Azure,
            units: ForcePreset::Infantry.units(faction),
            from: corner,
        }),
        Some(EventAction::Reinforcements { .. }) => Some(EventAction::Credits {
            team: Team::Azure,
            amount: 1000,
        }),
        Some(_) => None,
    };
    event
        .then
        .retain(|action| matches!(action, EventAction::Message { .. }));
    event.then.extend(next);
}

pub fn cycle_action_side(level: &mut LevelDefinition, index: usize) {
    let Some(team) =
        level
            .events
            .get(index)
            .and_then(main_action)
            .and_then(|action| match action {
                EventAction::Reinforcements { team, .. } | EventAction::Credits { team, .. } => {
                    Some(*team)
                }
                EventAction::Message { .. } => None,
            })
    else {
        return;
    };
    let next = next_side(level, team);
    let (old_faction, new_faction) = (level.faction_of(team), level.faction_of(next));
    let Some(action) = level.events.get_mut(index).and_then(main_action_mut) else {
        return;
    };
    match action {
        EventAction::Reinforcements { team, units, .. } => {
            let preset = ForcePreset::of(units, old_faction).unwrap_or(ForcePreset::Infantry);
            *team = next;
            *units = preset.units(new_faction);
        }
        EventAction::Credits { team, .. } => *team = next,
        EventAction::Message { .. } => {}
    }
}

pub fn cycle_preset(level: &mut LevelDefinition, index: usize) {
    let factions = |team: Team| level.faction_of(team);
    let Some((team, current)) = level
        .events
        .get(index)
        .and_then(main_action)
        .and_then(|action| match action {
            EventAction::Reinforcements { team, units, .. } => {
                Some((*team, ForcePreset::of(units, factions(*team))))
            }
            _ => None,
        })
    else {
        return;
    };
    let faction = level.faction_of(team);
    let next = match current {
        Some(preset) => {
            let at = ForcePreset::ALL
                .iter()
                .position(|&p| p == preset)
                .unwrap_or(0);
            ForcePreset::ALL[(at + 1) % ForcePreset::ALL.len()]
        }
        None => ForcePreset::Infantry,
    };
    if let Some(EventAction::Reinforcements { units, .. }) =
        level.events.get_mut(index).and_then(main_action_mut)
    {
        *units = next.units(faction);
    }
}

pub fn cycle_corner(level: &mut LevelDefinition, index: usize) {
    if let Some(EventAction::Reinforcements { from, .. }) =
        level.events.get_mut(index).and_then(main_action_mut)
    {
        let at = BaseSite::ALL
            .iter()
            .position(|site| site == from)
            .unwrap_or(0);
        *from = BaseSite::ALL[(at + 1) % BaseSite::ALL.len()];
    }
}

pub fn adjust_credits(level: &mut LevelDefinition, index: usize, step: i32) {
    if let Some(EventAction::Credits { amount, .. }) =
        level.events.get_mut(index).and_then(main_action_mut)
    {
        *amount = (*amount + step * CREDIT_STEP).clamp(CREDIT_STEP, MOST_CREDITS);
    }
}

/// Drops messages left empty, so a saved level carries only what the player wrote.
pub fn tidy_events(level: &mut LevelDefinition) {
    for event in &mut level.events {
        event.then.retain(
            |action| !matches!(action, EventAction::Message { text } if text.trim().is_empty()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_added_event_cycles_through_its_triggers_and_actions() {
        let mut level = LevelDefinition::default();
        let index = add_event(&mut level).unwrap();
        adjust_trigger(&mut level, index, 3);
        assert_eq!(
            level.events[index].when,
            Trigger::AfterMinutes { minutes: 8.0 }
        );
        cycle_trigger(&mut level, index);
        assert_eq!(
            level.events[index].when,
            Trigger::StructuresLeft {
                team: Team::Crimson,
                at_most: 3
            }
        );
        cycle_trigger(&mut level, index);
        cycle_trigger(&mut level, index);
        assert_eq!(
            level.events[index].when,
            Trigger::AfterMinutes { minutes: 5.0 }
        );

        cycle_action(&mut level, index);
        let Some(EventAction::Reinforcements { units, .. }) = main_action(&level.events[index])
        else {
            panic!("expected reinforcements");
        };
        assert_eq!(*units, ForcePreset::Infantry.units(Faction::Coalition));
        cycle_preset(&mut level, index);
        let Some(EventAction::Reinforcements { units, .. }) = main_action(&level.events[index])
        else {
            panic!("expected reinforcements");
        };
        assert_eq!(*units, ForcePreset::Armour.units(Faction::Coalition));
        cycle_action(&mut level, index);
        adjust_credits(&mut level, index, 2);
        assert_eq!(
            main_action(&level.events[index]),
            Some(&EventAction::Credits {
                team: Team::Azure,
                amount: 2000
            })
        );
        cycle_action(&mut level, index);
        assert!(main_action(&level.events[index]).is_none());
        assert!(level.problems().is_empty());
    }

    #[test]
    fn reinforcements_sent_to_the_other_side_come_from_its_faction() {
        let mut level = LevelDefinition::default();
        let index = add_event(&mut level).unwrap();
        cycle_action(&mut level, index);
        cycle_preset(&mut level, index);
        cycle_action_side(&mut level, index);
        assert_eq!(
            main_action(&level.events[index]),
            Some(&EventAction::Reinforcements {
                team: Team::Crimson,
                units: ForcePreset::Armour.units(Faction::Syndicate),
                from: BaseSite::SouthWest,
            })
        );
    }

    #[test]
    fn an_empty_message_is_dropped_and_a_written_one_kept() {
        let mut level = LevelDefinition::default();
        let first = add_event(&mut level).unwrap();
        let second = add_event(&mut level).unwrap();
        message_mut(&mut level, first).unwrap().push_str("Hold on!");
        message_mut(&mut level, second);
        tidy_events(&mut level);
        assert_eq!(message(&level.events[first]), Some("Hold on!"));
        assert_eq!(message(&level.events[second]), None);
    }

    #[test]
    fn a_level_holds_a_limited_number_of_events() {
        let mut level = LevelDefinition::default();
        for _ in 0..MOST_EVENTS {
            assert!(add_event(&mut level).is_some());
        }
        assert!(add_event(&mut level).is_none());
        remove_event(&mut level, 0);
        assert_eq!(level.events.len(), MOST_EVENTS - 1);
    }
}
