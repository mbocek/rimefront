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
    fn the_modern_army_is_every_unit_of_today() {
        assert!(Era::playable().any(|era| era == Era::Modern));
        assert_eq!(UnitKind::Rifleman.era(), Era::Modern);
        assert!(UnitKind::Harvester.is_harvester());
        assert!(UnitKind::Engineer.is_engineer());
        assert!(UnitKind::TroopCarrier.carries_troops());
    }
}
