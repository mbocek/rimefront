use serde::{Deserialize, Serialize};

use crate::sim::catalog::{ArmorClass, UnitKind};

/// Each think, older sightings count this much less, so the mix answers what the enemy fields now.
const SIGHTING_MEMORY: f32 = 0.9;
/// Below this many remembered sightings the commander keeps its usual mix.
const SIGHTINGS_TO_ADAPT: f32 = 3.0;
/// Even a poor counter keeps some chance of being trained, so armies stay mixed.
const COUNTER_FLOOR: f32 = 0.2;
const MOBILE_CLASSES: [ArmorClass; 3] = [
    ArmorClass::Infantry,
    ArmorClass::Vehicle,
    ArmorClass::Aircraft,
];

/// The enemy units a commander has seen lately, counted by armour class.
#[derive(Clone, Copy, Default, PartialEq, Debug, Serialize, Deserialize)]
pub struct Sightings {
    infantry: f32,
    vehicles: f32,
    aircraft: f32,
}

impl Sightings {
    pub fn observe(&mut self, seen: impl IntoIterator<Item = ArmorClass>) {
        self.infantry *= SIGHTING_MEMORY;
        self.vehicles *= SIGHTING_MEMORY;
        self.aircraft *= SIGHTING_MEMORY;
        for armor in seen {
            match armor {
                ArmorClass::Infantry => self.infantry += 1.0,
                ArmorClass::Vehicle => self.vehicles += 1.0,
                ArmorClass::Aircraft => self.aircraft += 1.0,
                ArmorClass::Structure => {}
            }
        }
    }

    fn total(&self) -> f32 {
        self.infantry + self.vehicles + self.aircraft
    }

    pub fn share(&self, armor: ArmorClass) -> f32 {
        let total = self.total();
        if total < SIGHTINGS_TO_ADAPT {
            return 0.0;
        }
        let count = match armor {
            ArmorClass::Infantry => self.infantry,
            ArmorClass::Vehicle => self.vehicles,
            ArmorClass::Aircraft => self.aircraft,
            ArmorClass::Structure => 0.0,
        };
        count / total
    }
}

/// How well a unit answers the enemies seen lately; 1.0 while too little has been seen or it is unarmed.
pub fn counter_weight(kind: UnitKind, sightings: &Sightings) -> f32 {
    let Some(weapon) = kind.spec().weapon else {
        return 1.0;
    };
    if sightings.total() < SIGHTINGS_TO_ADAPT {
        return 1.0;
    }
    let effectiveness: f32 = MOBILE_CLASSES
        .iter()
        .map(|&armor| sightings.share(armor) * weapon.multiplier_against(armor))
        .sum();
    COUNTER_FLOOR + effectiveness
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(armor: ArmorClass, count: usize) -> Sightings {
        let mut sightings = Sightings::default();
        sightings.observe(std::iter::repeat_n(armor, count));
        sightings
    }

    #[test]
    fn nothing_seen_keeps_the_usual_mix() {
        let sightings = Sightings::default();
        assert_eq!(counter_weight(UnitKind::Tank, &sightings), 1.0);
        assert_eq!(counter_weight(UnitKind::FlakTrack, &sightings), 1.0);
    }

    #[test]
    fn enemy_aircraft_call_for_flak_rather_than_tanks() {
        let air = seen(ArmorClass::Aircraft, 6);
        assert!(
            counter_weight(UnitKind::FlakTrack, &air) > 4.0 * counter_weight(UnitKind::Tank, &air)
        );
        assert!(
            counter_weight(UnitKind::Rocketeer, &air) > counter_weight(UnitKind::Rifleman, &air)
        );
    }

    #[test]
    fn enemy_infantry_call_for_snipers_and_armour_for_rockets() {
        let infantry = seen(ArmorClass::Infantry, 8);
        assert!(
            counter_weight(UnitKind::Sniper, &infantry)
                > counter_weight(UnitKind::Rocketeer, &infantry)
        );
        let armour = seen(ArmorClass::Vehicle, 8);
        assert!(
            counter_weight(UnitKind::Rocketeer, &armour)
                > counter_weight(UnitKind::Sniper, &armour)
        );
    }

    #[test]
    fn old_sightings_fade() {
        let mut sightings = seen(ArmorClass::Aircraft, 5);
        for _ in 0..10 {
            sightings.observe([ArmorClass::Infantry]);
        }
        assert!(sightings.share(ArmorClass::Infantry) > sightings.share(ArmorClass::Aircraft));
    }
}
