//! Ages a battle is fought in. Every unit belongs to one era and one line; when the era turns, a unit is
//! promoted to the next member of its line. See docs/eras.md.

use serde::{Deserialize, Serialize};

use crate::i18n::tr;
use crate::sim::catalog::{BuildingKind, UnitKind};

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub enum Era {
    Antiquity,
    Gunpowder,
    Industrial,
    #[default]
    Modern,
    Future,
}

impl Era {
    pub const ALL: [Era; 5] = [
        Era::Antiquity,
        Era::Gunpowder,
        Era::Industrial,
        Era::Modern,
        Era::Future,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Era::Antiquity => tr("Antiquity"),
            Era::Gunpowder => tr("Gunpowder"),
            Era::Industrial => tr("Industrial age"),
            Era::Modern => tr("Modern"),
            Era::Future => tr("Future"),
        }
    }

    /// The age called `name` in English, in any case, as command lines give it.
    pub fn named(name: &str) -> Option<Era> {
        Era::ALL
            .into_iter()
            .find(|era| format!("{era:?}").eq_ignore_ascii_case(name))
    }

    /// Eras that have an army of their own yet.
    pub fn playable() -> impl Iterator<Item = Era> {
        Era::ALL
            .into_iter()
            .filter(|&era| UnitKind::ALL.iter().any(|kind| kind.era() == era))
    }

    pub fn next_playable(self) -> Option<Era> {
        Era::playable().find(|&era| era > self)
    }
}

const TICKS_PER_SEC: u64 = 1000 / crate::SIMULATION_TICK.as_millis() as u64;
const TICKS_PER_HOUR: u64 = 3600 * TICKS_PER_SEC;
const TICKS_PER_DAY: u64 = 24 * TICKS_PER_HOUR;

/// When a persistent world's ages turn. An age lasts at least `era_hours` (the final one `finale_hours`) and
/// turns at the next `turn_hour` of a world day after that, so its players know the evening it ends on. The
/// world counts in ticks: its `f32` match clock stops advancing after twelve days.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AgeSchedule {
    pub first: Era,
    pub era_hours: u32,
    pub finale_hours: u32,
    pub turn_hour: u32,
}

/// Ages of a week, turning at eight in the evening: a world of friends sees them turn. A public world wants
/// longer ones (docs/server.md).
impl Default for AgeSchedule {
    fn default() -> Self {
        Self::in_days(Era::Antiquity, 7, 3, 20)
    }
}

impl AgeSchedule {
    pub const FINAL_AGE: Era = Era::Future;

    pub fn in_days(first: Era, era_days: u32, finale_days: u32, turn_hour: u32) -> Self {
        Self {
            first,
            era_hours: era_days * 24,
            finale_hours: finale_days * 24,
            turn_hour: turn_hour % 24,
        }
    }

    pub fn era_at(&self, tick: u64) -> Era {
        self.age_at(tick).0
    }

    /// The tick the age running at `tick` turns into the next one; `None` while there is no next one.
    pub fn next_turn_after(&self, tick: u64) -> Option<u64> {
        let (era, started) = self.age_at(tick);
        era.next_playable().map(|_| self.turn_of(era, started))
    }

    /// The tick the age running at `tick` began.
    pub fn age_started_at(&self, tick: u64) -> u64 {
        self.age_at(tick).1
    }

    fn age_at(&self, tick: u64) -> (Era, u64) {
        let mut era = self.first;
        let mut started = 0;
        while let Some(next) = era.next_playable() {
            let turn = self.turn_of(era, started);
            if tick < turn {
                break;
            }
            (era, started) = (next, turn);
        }
        (era, started)
    }

    fn turn_of(&self, era: Era, started: u64) -> u64 {
        let hours = match era {
            AgeSchedule::FINAL_AGE => self.finale_hours,
            _ => self.era_hours,
        };
        let earliest = started + u64::from(hours) * TICKS_PER_HOUR;
        let turn_in_day = u64::from(self.turn_hour) * TICKS_PER_HOUR;
        let day = earliest.saturating_sub(turn_in_day).div_ceil(TICKS_PER_DAY);
        (day * TICKS_PER_DAY + turn_in_day).max(started + 1)
    }
}

/// The ages a structure stands in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Eras {
    Only(Era),
    Since(Era),
}

impl Eras {
    pub fn contain(self, era: Era) -> bool {
        match self {
            Eras::Only(own) => own == era,
            Eras::Since(first) => era >= first,
        }
    }
}

/// The place a unit holds in an army whatever the age: the archer, the musketeer and the grenadier are one line.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum UnitLine {
    Infantry,
    AntiArmour,
    Ranged,
    Marksman,
    Flame,
    Healer,
    Builder,
    Fast,
    Raider,
    Main,
    Heavy,
    Stealth,
    Siege,
    RocketArtillery,
    AntiAir,
    Transport,
    Harvester,
    Repair,
    Gunship,
    Fighter,
    Bomber,
}

/// The defences that change with the age; every other structure stays the same building in a new look.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum DefenceLine {
    Tower,
    AntiAir,
    Bunker,
    Wall,
}

impl UnitKind {
    pub fn era(self) -> Era {
        self.spec().era
    }

    pub fn line(self) -> UnitLine {
        self.spec().line
    }

    pub fn of_line_in(line: UnitLine, era: Era) -> Option<UnitKind> {
        UnitKind::ALL
            .into_iter()
            .find(|kind| kind.line() == line && kind.era() == era)
    }

    /// What this unit is called in `era`; `None` when its line has no member there.
    pub fn in_era(self, era: Era) -> Option<UnitKind> {
        UnitKind::of_line_in(self.line(), era)
    }

    /// The unit a promotion into `era` makes of this one; only ever forward in time.
    pub fn promoted_into(self, era: Era) -> Option<UnitKind> {
        (era > self.era()).then(|| self.in_era(era)).flatten()
    }

    pub fn is_harvester(self) -> bool {
        self.line() == UnitLine::Harvester
    }

    pub fn is_engineer(self) -> bool {
        self.line() == UnitLine::Builder
    }

    pub fn carries_troops(self) -> bool {
        self.line() == UnitLine::Transport
    }
}

impl BuildingKind {
    pub fn eras(self) -> Eras {
        self.spec().era
    }

    pub fn defence_line(self) -> Option<DefenceLine> {
        self.spec().line
    }

    pub fn exists_in(self, era: Era) -> bool {
        self.eras().contain(era)
    }

    /// What this structure is called in `era`; `None` when its line has no member there.
    pub fn in_era(self, era: Era) -> Option<BuildingKind> {
        let Some(line) = self.defence_line() else {
            return self.exists_in(era).then_some(self);
        };
        BuildingKind::CONSTRUCTIBLE
            .into_iter()
            .find(|kind| kind.defence_line() == Some(line) && kind.exists_in(era))
    }

    /// The defence a promotion into `era` makes of this one; only ever forward in time.
    pub fn promoted_into(self, era: Era) -> Option<BuildingKind> {
        let Eras::Only(own) = self.eras() else {
            return None;
        };
        (era > own).then(|| self.in_era(era)).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_has_one_unit_per_era_at_most() {
        for kind in UnitKind::ALL {
            let same = UnitKind::ALL
                .iter()
                .filter(|other| other.line() == kind.line() && other.era() == kind.era())
                .count();
            assert_eq!(same, 1, "{kind:?} shares its line and era");
            assert_eq!(kind.in_era(kind.era()), Some(kind));
        }
    }

    #[test]
    fn promotion_only_goes_forward() {
        for kind in UnitKind::ALL {
            assert_eq!(kind.promoted_into(kind.era()), None);
            if let Some(earlier) = Era::ALL.into_iter().find(|&era| era < kind.era()) {
                assert_eq!(kind.promoted_into(earlier), None);
            }
        }
    }

    #[test]
    fn every_era_structure_has_one_member_per_line() {
        for kind in BuildingKind::CONSTRUCTIBLE {
            for era in Era::playable() {
                if let Some(other) = kind.in_era(era) {
                    assert!(other.exists_in(era), "{kind:?} in {era:?} is {other:?}");
                }
            }
        }
    }

    #[test]
    fn an_age_turns_at_the_evening_after_its_days_are_over() {
        let schedule = AgeSchedule::in_days(Era::Antiquity, 7, 3, 20);
        let turn = (7 * 24 + 20) * TICKS_PER_HOUR;
        assert_eq!(schedule.era_at(0), Era::Antiquity);
        assert_eq!(schedule.era_at(turn - 1), Era::Antiquity);
        assert_eq!(schedule.era_at(turn), Era::Modern);
        assert_eq!(schedule.next_turn_after(0), Some(turn));
        assert_eq!(schedule.age_started_at(turn + 5), turn);
        assert_eq!(
            schedule.next_turn_after(turn),
            None,
            "no age after the modern one yet"
        );
        assert_eq!(schedule.era_at(u64::MAX / 2), Era::Modern);
    }

    #[test]
    fn an_age_shorter_than_a_day_still_waits_for_the_turning_hour() {
        let schedule = AgeSchedule {
            first: Era::Antiquity,
            era_hours: 1,
            finale_hours: 1,
            turn_hour: 1,
        };
        assert_eq!(schedule.next_turn_after(0), Some(TICKS_PER_HOUR));
        let late = AgeSchedule {
            turn_hour: 0,
            ..schedule
        };
        assert_eq!(late.next_turn_after(0), Some(TICKS_PER_DAY));
    }

    #[test]
    fn the_modern_army_is_every_unit_of_today() {
        assert!(Era::playable().any(|era| era == Era::Modern));
        assert_eq!(UnitKind::Rifleman.era(), Era::Modern);
        assert!(UnitKind::Harvester.is_harvester());
        assert!(UnitKind::Engineer.is_engineer());
        assert!(UnitKind::TroopCarrier.carries_troops());
    }
}
