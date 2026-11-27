use serde::{Deserialize, Serialize};

use crate::sim::catalog::{ArmorClass, Team};
use crate::sim::production::Production;

/// A one-time improvement a team researches in its Tech Center; it lasts for the rest of the match.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Upgrade {
    CompositeArmour,
    HighExplosiveWarheads,
    CombatDrills,
}

pub struct UpgradeSpec {
    pub name: &'static str,
    pub role: &'static str,
    pub icon_id: &'static str,
    pub cost: i32,
    pub research_secs: f32,
}

const COMPOSITE_ARMOUR: UpgradeSpec = UpgradeSpec {
    name: "Composite Armour",
    role: "Vehicles and aircraft take 20 % less damage.",
    icon_id: "upgrade_armour",
    cost: 1500,
    research_secs: 40.0,
};

const HIGH_EXPLOSIVE_WARHEADS: UpgradeSpec = UpgradeSpec {
    name: "High-Explosive Warheads",
    role: "Every weapon of yours deals 15 % more damage.",
    icon_id: "upgrade_warheads",
    cost: 2000,
    research_secs: 45.0,
};

const COMBAT_DRILLS: UpgradeSpec = UpgradeSpec {
    name: "Combat Drills",
    role: "Infantry reload 25 % faster.",
    icon_id: "upgrade_drills",
    cost: 1000,
    research_secs: 30.0,
};

const ARMOURED_DAMAGE_TAKEN: f32 = 0.8;
const WARHEAD_DAMAGE_FACTOR: f32 = 1.15;
const DRILLED_RELOAD_FACTOR: f32 = 0.75;

impl Upgrade {
    pub const ALL: [Upgrade; 3] = [
        Upgrade::CompositeArmour,
        Upgrade::HighExplosiveWarheads,
        Upgrade::CombatDrills,
    ];

    pub fn spec(self) -> &'static UpgradeSpec {
        match self {
            Upgrade::CompositeArmour => &COMPOSITE_ARMOUR,
            Upgrade::HighExplosiveWarheads => &HIGH_EXPLOSIVE_WARHEADS,
            Upgrade::CombatDrills => &COMBAT_DRILLS,
        }
    }
}

pub fn outgoing_damage_factor(production: &Production, shooter: Team) -> f32 {
    if production.has_researched(shooter, Upgrade::HighExplosiveWarheads) {
        WARHEAD_DAMAGE_FACTOR
    } else {
        1.0
    }
}

pub fn incoming_damage_factor(production: &Production, victim: Team, armor: ArmorClass) -> f32 {
    if armor == ArmorClass::Structure && production.structures_spared() {
        return 0.0;
    }
    let armoured = matches!(armor, ArmorClass::Vehicle | ArmorClass::Aircraft)
        && production.has_researched(victim, Upgrade::CompositeArmour);
    if armoured { ARMOURED_DAMAGE_TAKEN } else { 1.0 }
}

pub fn reload_factor(production: &Production, shooter: Team, armor: Option<ArmorClass>) -> f32 {
    let drilled = armor == Some(ArmorClass::Infantry)
        && production.has_researched(shooter, Upgrade::CombatDrills);
    if drilled { DRILLED_RELOAD_FACTOR } else { 1.0 }
}
