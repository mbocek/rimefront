use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::sim::cues::Sound;

/// A side in the battle. The first four are the sides a level names; a world that players join has more.
#[derive(Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Team(u8);

#[allow(non_upper_case_globals)]
impl Team {
    pub const Player: Team = Team(0);
    pub const Crimson: Team = Team(1);
    pub const Amber: Team = Team(2);
    pub const Violet: Team = Team(3);

    /// The sides a level names, the player's first.
    pub const ALL: [Team; 4] = [Team::Player, Team::Crimson, Team::Amber, Team::Violet];
    pub const OPPONENTS: [Team; 3] = [Team::Crimson, Team::Amber, Team::Violet];
    pub const MOST: usize = u8::MAX as usize + 1;

    pub fn from_index(index: usize) -> Team {
        Team(u8::try_from(index).expect("at most Team::MOST sides"))
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    fn look(self) -> &'static SideLook {
        &SIDE_LOOKS[self.index() % SIDE_LOOKS.len()]
    }

    /// The name in the usual line-up, where the player leads the Coalition.
    pub fn name(self) -> String {
        self.name_in(Faction::default_of(self))
    }

    pub fn name_in(self, faction: Faction) -> String {
        if self == Team::Player {
            return faction.name().to_string();
        }
        format!("{} {}", self.look().name, faction.name())
    }

    pub fn model_variant(self) -> &'static str {
        self.look().model_variant
    }

    pub fn ui_color(self) -> Color {
        self.look().color
    }

    /// How saved games, levels and the network name the side.
    fn key(self) -> String {
        match NAMED_SIDES.get(self.index()) {
            Some(name) => name.to_string(),
            None => format!("Side{}", self.0),
        }
    }

    fn from_key(key: &str) -> Option<Team> {
        if let Some(index) = NAMED_SIDES.iter().position(|name| *name == key) {
            return Some(Team::from_index(index));
        }
        key.strip_prefix("Side")?.parse().ok().map(Team)
    }
}

const NAMED_SIDES: [&str; 4] = ["Player", "Crimson", "Amber", "Violet"];

struct SideLook {
    name: &'static str,
    color: Color,
    model_variant: &'static str,
}

/// Sides past these take the same looks again; only four tints of the models exist.
const SIDE_LOOKS: [SideLook; 8] = [
    SideLook {
        name: "Azure",
        color: Color::srgb(0.25, 0.55, 1.0),
        model_variant: "blue",
    },
    SideLook {
        name: "Crimson",
        color: Color::srgb(1.0, 0.3, 0.25),
        model_variant: "red",
    },
    SideLook {
        name: "Amber",
        color: Color::srgb(1.0, 0.62, 0.15),
        model_variant: "orange",
    },
    SideLook {
        name: "Violet",
        color: Color::srgb(0.72, 0.4, 1.0),
        model_variant: "purple",
    },
    SideLook {
        name: "Jade",
        color: Color::srgb(0.25, 0.85, 0.45),
        model_variant: "blue",
    },
    SideLook {
        name: "Rust",
        color: Color::srgb(0.75, 0.4, 0.2),
        model_variant: "red",
    },
    SideLook {
        name: "Gold",
        color: Color::srgb(0.95, 0.85, 0.3),
        model_variant: "orange",
    },
    SideLook {
        name: "Rose",
        color: Color::srgb(1.0, 0.5, 0.75),
        model_variant: "purple",
    },
];

impl Serialize for Team {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.key())
    }
}

impl<'de> Deserialize<'de> for Team {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = String::deserialize(deserializer)?;
        Team::from_key(&key)
            .ok_or_else(|| serde::de::Error::custom(format!("no side is called {key}")))
    }
}

/// Whose armoury a team builds from; each side has units and structures the other cannot order.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Faction {
    #[default]
    Coalition,
    Syndicate,
}

impl Faction {
    pub fn opposite(self) -> Faction {
        match self {
            Faction::Coalition => Faction::Syndicate,
            Faction::Syndicate => Faction::Coalition,
        }
    }

    /// The player leads the level's chosen faction and every computer side the other one.
    pub fn of(team: Team, player_faction: Faction) -> Faction {
        if team == Team::Player {
            player_faction
        } else {
            player_faction.opposite()
        }
    }

    fn default_of(team: Team) -> Faction {
        Faction::of(team, Faction::default())
    }

    fn name(self) -> &'static str {
        match self {
            Faction::Coalition => "Coalition",
            Faction::Syndicate => "Syndicate",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Faction::Coalition => "COALITION",
            Faction::Syndicate => "SYNDICATE",
        }
    }
}

/// Which teams fight on the same side.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Alliance {
    /// The Syndicate commanders are allies against the Coalition.
    #[default]
    CoalitionAgainstSyndicate,
    FreeForAll,
    /// The Coalition and Amber against Crimson and Violet: with the default sites, the two western bases
    /// against the two eastern ones, each pair the mirror image of the other.
    TwoAgainstTwo,
}

impl Alliance {
    pub const ALL: [Alliance; 3] = [
        Alliance::CoalitionAgainstSyndicate,
        Alliance::FreeForAll,
        Alliance::TwoAgainstTwo,
    ];

    fn camp(self, team: Team) -> usize {
        match self {
            Alliance::CoalitionAgainstSyndicate => usize::from(team != Team::Player),
            Alliance::FreeForAll => team.index(),
            Alliance::TwoAgainstTwo => usize::from(matches!(team, Team::Crimson | Team::Violet)),
        }
    }
}

/// Who fights whom in the current match: sides of one camp are allies, every other side an enemy.
#[derive(Resource, Clone, PartialEq, Eq, Debug)]
pub struct Diplomacy {
    /// Indexed by `Team::index`; a side past the end is a camp of its own.
    camps: Vec<usize>,
}

impl Diplomacy {
    pub fn of(alliance: Alliance) -> Self {
        Self {
            camps: Team::ALL.map(|team| alliance.camp(team)).to_vec(),
        }
    }

    fn camp(&self, team: Team) -> usize {
        let own_camp = Team::MOST + team.index();
        self.camps.get(team.index()).copied().unwrap_or(own_camp)
    }

    pub fn hostile(&self, team: Team, other: Team) -> bool {
        self.camp(team) != self.camp(other)
    }

    /// `team` leaves its camp for the camp of `ally`.
    pub fn join_camp_of(&mut self, team: Team, ally: Team) {
        let camp = self.camp(ally);
        let own_camps = self.camps.len()..=team.index();
        self.camps.extend(own_camps.map(|index| Team::MOST + index));
        self.camps[team.index()] = camp;
    }
}

impl Default for Diplomacy {
    fn default() -> Self {
        Self::of(Alliance::default())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmorClass {
    Infantry,
    Vehicle,
    Structure,
    Aircraft,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectileStyle {
    Bullet,
    Shell,
    ArtilleryShell,
    Missile,
    Flame,
}

#[derive(Clone, Copy, Debug)]
pub struct WeaponSpec {
    pub range: f32,
    pub damage: f32,
    pub reload_secs: f32,
    pub projectile_speed: f32,
    pub splash_radius: f32,
    pub style: ProjectileStyle,
    pub versus_infantry: f32,
    pub versus_vehicle: f32,
    pub versus_structure: f32,
    /// Zero for weapons that cannot shoot upwards at aircraft at all.
    pub versus_aircraft: f32,
    /// What firing it sounds like.
    pub report: Sound,
}

impl WeaponSpec {
    pub fn multiplier_against(&self, armor: ArmorClass) -> f32 {
        match armor {
            ArmorClass::Infantry => self.versus_infantry,
            ArmorClass::Vehicle => self.versus_vehicle,
            ArmorClass::Structure => self.versus_structure,
            ArmorClass::Aircraft => self.versus_aircraft,
        }
    }

    pub fn can_hit(&self, armor: ArmorClass) -> bool {
        self.multiplier_against(armor) > 0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum UnitKind {
    Rifleman,
    Rocketeer,
    Medic,
    Engineer,
    Scout,
    Tank,
    Bulwark,
    Artillery,
    Harvester,
    Gunship,
    FlakTrack,
    Sniper,
    FlameTrooper,
    MissileTruck,
    Grenadier,
    RepairTruck,
    StealthTank,
    RaiderBike,
    Bomber,
    Fighter,
    TroopCarrier,
}

pub struct UnitSpec {
    pub name: &'static str,
    pub role: &'static str,
    pub model_id: &'static str,
    pub cost: i32,
    pub build_secs: f32,
    pub max_health: f32,
    pub speed: f32,
    pub turn_rate: f32,
    pub radius: f32,
    pub armor: ArmorClass,
    pub weapon: Option<WeaponSpec>,
    pub has_turret: bool,
    pub producer: BuildingKind,
    /// A structure needed besides the producer before the unit can be ordered.
    pub requires: Option<BuildingKind>,
    /// None when both factions can order it.
    pub faction: Option<Faction>,
    /// Health restored per second to wounded friendly units of its own armour class nearby (medics treat
    /// infantry, repair trucks vehicles); zero for everyone else.
    pub heal_per_sec: f32,
    /// Radius in meters this unit reveals through the fog of war.
    pub sight: f32,
}

impl UnitKind {
    pub const INFANTRY: [UnitKind; 7] = [
        UnitKind::Rifleman,
        UnitKind::Rocketeer,
        UnitKind::Grenadier,
        UnitKind::Sniper,
        UnitKind::FlameTrooper,
        UnitKind::Medic,
        UnitKind::Engineer,
    ];
    pub const VEHICLES: [UnitKind; 14] = [
        UnitKind::Scout,
        UnitKind::TroopCarrier,
        UnitKind::RaiderBike,
        UnitKind::Tank,
        UnitKind::StealthTank,
        UnitKind::Bulwark,
        UnitKind::Artillery,
        UnitKind::MissileTruck,
        UnitKind::FlakTrack,
        UnitKind::Gunship,
        UnitKind::Bomber,
        UnitKind::Fighter,
        UnitKind::RepairTruck,
        UnitKind::Harvester,
    ];
    /// New kinds go at the end: replays store a unit's look as its place in this list.
    pub const ALL: [UnitKind; 21] = [
        UnitKind::Rifleman,
        UnitKind::Rocketeer,
        UnitKind::Medic,
        UnitKind::Engineer,
        UnitKind::Scout,
        UnitKind::Tank,
        UnitKind::Bulwark,
        UnitKind::Artillery,
        UnitKind::Harvester,
        UnitKind::Gunship,
        UnitKind::FlakTrack,
        UnitKind::Sniper,
        UnitKind::FlameTrooper,
        UnitKind::MissileTruck,
        UnitKind::Grenadier,
        UnitKind::RepairTruck,
        UnitKind::StealthTank,
        UnitKind::RaiderBike,
        UnitKind::Bomber,
        UnitKind::Fighter,
        UnitKind::TroopCarrier,
    ];

    pub fn flies(self) -> bool {
        self.spec().armor == ArmorClass::Aircraft
    }

    pub fn spec(self) -> &'static UnitSpec {
        match self {
            UnitKind::Rifleman => &RIFLEMAN,
            UnitKind::Scout => &SCOUT,
            UnitKind::Tank => &TANK,
            UnitKind::Artillery => &ARTILLERY,
            UnitKind::Harvester => &HARVESTER,
            UnitKind::Gunship => &GUNSHIP,
            UnitKind::Rocketeer => &ROCKETEER,
            UnitKind::Medic => &MEDIC,
            UnitKind::Engineer => &ENGINEER,
            UnitKind::Bulwark => &BULWARK,
            UnitKind::FlakTrack => &FLAK_TRACK,
            UnitKind::Sniper => &SNIPER,
            UnitKind::FlameTrooper => &FLAME_TROOPER,
            UnitKind::MissileTruck => &MISSILE_TRUCK,
            UnitKind::Grenadier => &GRENADIER,
            UnitKind::RepairTruck => &REPAIR_TRUCK,
            UnitKind::StealthTank => &STEALTH_TANK,
            UnitKind::RaiderBike => &RAIDER_BIKE,
            UnitKind::Bomber => &BOMBER,
            UnitKind::Fighter => &FIGHTER,
            UnitKind::TroopCarrier => &TROOP_CARRIER,
        }
    }

    /// Aircraft that strike once and fly back to a Helipad to rearm before the next run.
    pub fn rearms_at_pad(self) -> bool {
        self == UnitKind::Bomber
    }

    /// Aircraft that land on their Helipad when idle there instead of hovering, as only a helicopter can.
    pub fn lands_on_pad(self) -> bool {
        matches!(self, UnitKind::Bomber | UnitKind::Fighter)
    }

    /// Units that blend into the ground after keeping still for a while.
    pub fn hides_when_still(self) -> bool {
        matches!(self, UnitKind::Sniper | UnitKind::StealthTank)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum BuildingKind {
    CommandCenter,
    PowerPlant,
    Refinery,
    Barracks,
    Factory,
    GunTurret,
    Wall,
    Helipad,
    FlakCannon,
    TechCenter,
    RepairBay,
    RocketBunker,
    SupplyDepot,
    ObservationPost,
    FieldHospital,
}

pub struct BuildingSpec {
    pub name: &'static str,
    pub role: &'static str,
    pub model_id: &'static str,
    pub footprint: IVec2,
    pub cost: i32,
    pub build_secs: f32,
    pub max_health: f32,
    pub power: i32,
    pub prerequisites: &'static [BuildingKind],
    pub weapon: Option<WeaponSpec>,
    /// Radius in meters this structure reveals through the fog of war.
    pub sight: f32,
    /// None when both factions can build it.
    pub faction: Option<Faction>,
}

impl BuildingKind {
    pub const CONSTRUCTIBLE: [BuildingKind; 11] = [
        BuildingKind::PowerPlant,
        BuildingKind::Refinery,
        BuildingKind::Barracks,
        BuildingKind::Factory,
        BuildingKind::TechCenter,
        BuildingKind::RepairBay,
        BuildingKind::Helipad,
        BuildingKind::GunTurret,
        BuildingKind::FlakCannon,
        BuildingKind::RocketBunker,
        BuildingKind::Wall,
    ];

    /// Stand unclaimed on the map until an engineer takes one; nobody builds them.
    pub const NEUTRAL_SITES: [BuildingKind; 3] = [
        BuildingKind::SupplyDepot,
        BuildingKind::ObservationPost,
        BuildingKind::FieldHospital,
    ];

    pub fn is_neutral_site(self) -> bool {
        Self::NEUTRAL_SITES.contains(&self)
    }

    /// Walls are bought segment by segment while being placed, and do not keep a defeated side alive.
    pub fn is_wall(self) -> bool {
        self == BuildingKind::Wall
    }

    /// Walls and captured neutral sites do not keep a side in the war.
    pub fn keeps_a_side_alive(self) -> bool {
        !self.is_wall() && !self.is_neutral_site()
    }

    pub fn spec(self) -> &'static BuildingSpec {
        match self {
            BuildingKind::CommandCenter => &COMMAND_CENTER,
            BuildingKind::PowerPlant => &POWER_PLANT,
            BuildingKind::Refinery => &REFINERY,
            BuildingKind::Barracks => &BARRACKS,
            BuildingKind::Factory => &FACTORY,
            BuildingKind::GunTurret => &GUN_TURRET,
            BuildingKind::Wall => &WALL,
            BuildingKind::Helipad => &HELIPAD,
            BuildingKind::FlakCannon => &FLAK_CANNON,
            BuildingKind::TechCenter => &TECH_CENTER,
            BuildingKind::RepairBay => &REPAIR_BAY,
            BuildingKind::RocketBunker => &ROCKET_BUNKER,
            BuildingKind::SupplyDepot => &SUPPLY_DEPOT,
            BuildingKind::ObservationPost => &OBSERVATION_POST,
            BuildingKind::FieldHospital => &FIELD_HOSPITAL,
        }
    }
}

const RIFLE: WeaponSpec = WeaponSpec {
    range: 8.0,
    damage: 9.0,
    reload_secs: 0.8,
    projectile_speed: 60.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.2,
    versus_structure: 0.25,
    versus_aircraft: 0.25,
    report: Sound::RifleShot,
};

const SCOUT_MACHINE_GUN: WeaponSpec = WeaponSpec {
    range: 10.0,
    damage: 7.0,
    reload_secs: 0.35,
    projectile_speed: 70.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.45,
    versus_structure: 0.3,
    versus_aircraft: 0.55,
    report: Sound::MachineGunShot,
};

const TANK_CANNON: WeaponSpec = WeaponSpec {
    range: 11.0,
    damage: 45.0,
    reload_secs: 1.6,
    projectile_speed: 40.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.6,
    versus_vehicle: 1.0,
    versus_structure: 0.8,
    versus_aircraft: 0.0,
    report: Sound::CannonShot,
};

/// Lobbed, so it lands a little off and blasts whoever stands near; cannot reach aircraft.
const GRENADE_LAUNCHER: WeaponSpec = WeaponSpec {
    range: 9.0,
    damage: 45.0,
    reload_secs: 2.2,
    projectile_speed: 14.0,
    splash_radius: 2.2,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 1.0,
    versus_vehicle: 0.15,
    versus_structure: 0.9,
    versus_aircraft: 0.0,
    report: Sound::GrenadeLaunch,
};

/// Fired from hiding: a hard first blow against armour, little use against soldiers.
const STEALTH_MISSILES: WeaponSpec = WeaponSpec {
    range: 10.0,
    damage: 52.0,
    reload_secs: 2.0,
    projectile_speed: 30.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.6,
    versus_vehicle: 1.0,
    versus_structure: 0.7,
    versus_aircraft: 0.4,
    report: Sound::MissileLaunch,
};

const BIKE_ROCKETS: WeaponSpec = WeaponSpec {
    range: 8.0,
    damage: 38.0,
    reload_secs: 2.4,
    projectile_speed: 28.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.3,
    versus_vehicle: 1.0,
    versus_structure: 0.4,
    versus_aircraft: 0.3,
    report: Sound::MissileLaunch,
};

/// One heavy stick of bombs per sortie; the long reload is the flight home and rearming.
const BOMB_LOAD: WeaponSpec = WeaponSpec {
    range: 3.0,
    damage: 240.0,
    reload_secs: 18.0,
    projectile_speed: 16.0,
    splash_radius: 3.2,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.8,
    versus_vehicle: 1.0,
    versus_structure: 1.4,
    versus_aircraft: 0.0,
    report: Sound::MissileLaunch,
};

/// Air-to-air missiles: nothing on the ground is a target.
const INTERCEPTOR_MISSILES: WeaponSpec = WeaponSpec {
    range: 12.0,
    damage: 48.0,
    reload_secs: 1.8,
    projectile_speed: 40.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.0,
    versus_vehicle: 0.0,
    versus_structure: 0.0,
    versus_aircraft: 1.5,
    report: Sound::MissileLaunch,
};

const HOWITZER: WeaponSpec = WeaponSpec {
    range: 20.0,
    damage: 80.0,
    reload_secs: 4.0,
    projectile_speed: 18.0,
    splash_radius: 2.2,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 0.5,
    versus_vehicle: 0.6,
    versus_structure: 1.2,
    versus_aircraft: 0.0,
    report: Sound::HowitzerShot,
};

const TURRET_CANNON: WeaponSpec = WeaponSpec {
    range: 12.0,
    damage: 32.0,
    reload_secs: 1.0,
    projectile_speed: 45.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.8,
    versus_vehicle: 1.0,
    versus_structure: 0.5,
    versus_aircraft: 0.6,
    report: Sound::CannonShot,
};

/// Bursting shells that only reach aircraft: the pit cannot depress its guns at the ground.
const FLAK_GUNS: WeaponSpec = WeaponSpec {
    range: 15.0,
    damage: 24.0,
    reload_secs: 0.5,
    projectile_speed: 70.0,
    splash_radius: 1.2,
    style: ProjectileStyle::Bullet,
    versus_infantry: 0.0,
    versus_vehicle: 0.0,
    versus_structure: 0.0,
    versus_aircraft: 2.0,
    report: Sound::FlakShot,
};

const FLAK_AUTOCANNON: WeaponSpec = WeaponSpec {
    range: 13.0,
    damage: 16.0,
    reload_secs: 0.45,
    projectile_speed: 70.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 0.6,
    versus_vehicle: 0.25,
    versus_structure: 0.15,
    versus_aircraft: 1.8,
    report: Sound::FlakShot,
};

const SNIPER_RIFLE: WeaponSpec = WeaponSpec {
    range: 16.0,
    damage: 60.0,
    reload_secs: 2.5,
    projectile_speed: 120.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.5,
    versus_vehicle: 0.05,
    versus_structure: 0.05,
    versus_aircraft: 0.0,
    report: Sound::SniperShot,
};

const FLAMETHROWER: WeaponSpec = WeaponSpec {
    range: 6.0,
    damage: 12.0,
    reload_secs: 0.25,
    projectile_speed: 22.0,
    splash_radius: 1.2,
    style: ProjectileStyle::Flame,
    versus_infantry: 1.5,
    versus_vehicle: 0.12,
    versus_structure: 0.9,
    versus_aircraft: 0.0,
    report: Sound::FlameBurst,
};

const ROCKET_SALVO: WeaponSpec = WeaponSpec {
    range: 18.0,
    damage: 55.0,
    reload_secs: 3.5,
    projectile_speed: 24.0,
    splash_radius: 2.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.8,
    versus_vehicle: 0.9,
    versus_structure: 1.1,
    versus_aircraft: 0.0,
    report: Sound::MissileLaunch,
};

const BUNKER_ROCKETS: WeaponSpec = WeaponSpec {
    range: 13.0,
    damage: 40.0,
    reload_secs: 1.6,
    projectile_speed: 30.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.5,
    versus_vehicle: 1.1,
    versus_structure: 0.5,
    versus_aircraft: 0.8,
    report: Sound::MissileLaunch,
};

const RIFLEMAN: UnitSpec = UnitSpec {
    name: "Rifleman",
    role: "Cheap infantry. Strong against soldiers, weak against armor.",
    model_id: "rifleman",
    cost: 100,
    build_secs: 5.0,
    max_health: 50.0,
    speed: 2.4,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(RIFLE),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const SCOUT: UnitSpec = UnitSpec {
    name: "Scout Buggy",
    role: "Fast wheeled raider with a roof machine gun. Hunts infantry.",
    model_id: "scout",
    cost: 300,
    build_secs: 8.0,
    max_health: 150.0,
    speed: 7.0,
    turn_rate: 5.0,
    radius: 1.0,
    armor: ArmorClass::Vehicle,
    weapon: Some(SCOUT_MACHINE_GUN),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 22.0,
};

const TROOP_CARRIER: UnitSpec = UnitSpec {
    name: "Troop Carrier",
    role: "Armoured wheels for five infantry, who fire from its hatches. A light machine gun of its own.",
    model_id: "troop_carrier",
    cost: 700,
    build_secs: 12.0,
    max_health: 420.0,
    speed: 4.2,
    turn_rate: 3.5,
    radius: 1.15,
    armor: ArmorClass::Vehicle,
    weapon: Some(SCOUT_MACHINE_GUN),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 16.0,
};

const TANK: UnitSpec = UnitSpec {
    name: "Battle Tank",
    role: "Main battle tank. Heavy cannon, thick armor.",
    model_id: "tank",
    cost: 700,
    build_secs: 14.0,
    max_health: 420.0,
    speed: 3.6,
    turn_rate: 2.5,
    radius: 1.3,
    armor: ArmorClass::Vehicle,
    weapon: Some(TANK_CANNON),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 16.0,
};

const ARTILLERY: UnitSpec = UnitSpec {
    name: "Howitzer",
    role: "Long-range artillery. Splash damage, devastating against bases.",
    model_id: "artillery",
    cost: 900,
    build_secs: 18.0,
    max_health: 200.0,
    speed: 2.8,
    turn_rate: 2.0,
    radius: 1.3,
    armor: ArmorClass::Vehicle,
    weapon: Some(HOWITZER),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const HARVESTER: UnitSpec = UnitSpec {
    name: "Harvester",
    role: "Mines crystals and delivers them to a refinery. Unarmed.",
    model_id: "harvester",
    cost: 1000,
    build_secs: 16.0,
    max_health: 600.0,
    speed: 3.0,
    turn_rate: 2.0,
    radius: 1.5,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 10.0,
};

const MISSILE_LAUNCHER: WeaponSpec = WeaponSpec {
    range: 9.5,
    damage: 42.0,
    reload_secs: 2.2,
    projectile_speed: 26.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.3,
    versus_vehicle: 1.25,
    versus_structure: 0.7,
    versus_aircraft: 1.1,
    report: Sound::MissileLaunch,
};

const TWIN_CANNON: WeaponSpec = WeaponSpec {
    range: 11.5,
    damage: 62.0,
    reload_secs: 1.7,
    projectile_speed: 38.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.6,
    versus_vehicle: 1.0,
    versus_structure: 1.0,
    versus_aircraft: 0.0,
    report: Sound::HeavyCannonShot,
};

const ROCKETEER: UnitSpec = UnitSpec {
    name: "Rocketeer",
    role: "Anti-armour infantry with a guided missile launcher. Weak against soldiers.",
    model_id: "rocketeer",
    cost: 250,
    build_secs: 7.0,
    max_health: 60.0,
    speed: 2.2,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(MISSILE_LAUNCHER),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const MEDIC: UnitSpec = UnitSpec {
    name: "Field Medic",
    role: "Unarmed. Treats wounded infantry nearby and follows the squad.",
    model_id: "medic",
    cost: 200,
    build_secs: 6.0,
    max_health: 60.0,
    speed: 2.4,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 8.0,
    sight: 12.0,
};

const ENGINEER: UnitSpec = UnitSpec {
    name: "Engineer",
    role: "Unarmed. Right-click an enemy structure to capture it, or a damaged own one to repair it fully.",
    model_id: "engineer",
    cost: 500,
    build_secs: 10.0,
    max_health: 50.0,
    speed: 2.6,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const ROCKET_PODS: WeaponSpec = WeaponSpec {
    range: 9.0,
    damage: 30.0,
    reload_secs: 1.3,
    projectile_speed: 32.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.9,
    versus_vehicle: 1.1,
    versus_structure: 0.7,
    versus_aircraft: 0.4,
    report: Sound::MissileLaunch,
};

const GUNSHIP: UnitSpec = UnitSpec {
    name: "Gunship",
    role: "Attack helicopter with rocket pods. Flies over anything; tanks and howitzers cannot hit it.",
    model_id: "gunship",
    cost: 1100,
    build_secs: 18.0,
    max_health: 320.0,
    speed: 6.5,
    turn_rate: 3.0,
    radius: 1.6,
    armor: ArmorClass::Aircraft,
    weapon: Some(ROCKET_PODS),
    has_turret: false,
    producer: BuildingKind::Helipad,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 20.0,
};

const BULWARK: UnitSpec = UnitSpec {
    name: "Bulwark",
    role: "Heavy assault tank with twin cannons. Slow but very hard to stop.",
    model_id: "bulwark",
    cost: 1200,
    build_secs: 24.0,
    max_health: 1100.0,
    speed: 2.2,
    turn_rate: 1.6,
    radius: 1.6,
    armor: ArmorClass::Vehicle,
    weapon: Some(TWIN_CANNON),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: Some(Faction::Coalition),
    heal_per_sec: 0.0,
    sight: 16.0,
};

const FLAK_TRACK: UnitSpec = UnitSpec {
    name: "Flak Track",
    role: "Mobile anti-aircraft gun. Shreds gunships, weak against armour.",
    model_id: "flak_track",
    cost: 700,
    build_secs: 11.0,
    max_health: 260.0,
    speed: 4.2,
    turn_rate: 3.5,
    radius: 1.5,
    armor: ArmorClass::Vehicle,
    weapon: Some(FLAK_AUTOCANNON),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 16.0,
};

const SNIPER: UnitSpec = UnitSpec {
    name: "Sniper",
    role: "Marksman who drops infantry from far away. Useless against armour.",
    model_id: "sniper",
    cost: 500,
    build_secs: 9.0,
    max_health: 60.0,
    speed: 2.3,
    turn_rate: 6.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(SNIPER_RIFLE),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: Some(Faction::Coalition),
    heal_per_sec: 0.0,
    sight: 20.0,
};

const FLAME_TROOPER: UnitSpec = UnitSpec {
    name: "Flame Trooper",
    role: "Burns out infantry and buildings at close range. Weak against armour.",
    model_id: "flame_trooper",
    cost: 300,
    build_secs: 7.0,
    max_health: 100.0,
    speed: 2.6,
    turn_rate: 8.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(FLAMETHROWER),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: Some(Faction::Syndicate),
    heal_per_sec: 0.0,
    sight: 9.0,
};

const GRENADIER: UnitSpec = UnitSpec {
    name: "Grenadier",
    role: "Lobs grenades into groups of soldiers and at buildings. Slow to reload, weak against armour.",
    model_id: "grenadier",
    cost: 250,
    build_secs: 7.0,
    max_health: 80.0,
    speed: 2.3,
    turn_rate: 9.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(GRENADE_LAUNCHER),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const BOMBER: UnitSpec = UnitSpec {
    name: "Bomber",
    role: "Flies one bombing run against ground targets, then returns to its Helipad to rearm. Flak shoots it down.",
    model_id: "bomber",
    cost: 1500,
    build_secs: 20.0,
    max_health: 280.0,
    speed: 9.0,
    turn_rate: 2.5,
    radius: 1.6,
    armor: ArmorClass::Aircraft,
    weapon: Some(BOMB_LOAD),
    has_turret: false,
    producer: BuildingKind::Helipad,
    requires: Some(BuildingKind::TechCenter),
    faction: Some(Faction::Coalition),
    heal_per_sec: 0.0,
    sight: 18.0,
};

const FIGHTER: UnitSpec = UnitSpec {
    name: "Fighter",
    role: "Fast interceptor with air-to-air missiles. Hunts gunships and bombers, cannot touch the ground.",
    model_id: "fighter",
    cost: 1000,
    build_secs: 14.0,
    max_health: 220.0,
    speed: 11.0,
    turn_rate: 4.0,
    radius: 1.2,
    armor: ArmorClass::Aircraft,
    weapon: Some(INTERCEPTOR_MISSILES),
    has_turret: false,
    producer: BuildingKind::Helipad,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 22.0,
};

const RAIDER_BIKE: UnitSpec = UnitSpec {
    name: "Raider Bike",
    role: "Fast, cheap motorbike with anti-armour rockets. Hunts harvesters and artillery, dies to gunfire.",
    model_id: "raider_bike",
    cost: 350,
    build_secs: 7.0,
    max_health: 140.0,
    speed: 8.0,
    turn_rate: 6.0,
    radius: 0.7,
    armor: ArmorClass::Vehicle,
    weapon: Some(BIKE_ROCKETS),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: Some(Faction::Syndicate),
    heal_per_sec: 0.0,
    sight: 20.0,
};

const STEALTH_TANK: UnitSpec = UnitSpec {
    name: "Stealth Tank",
    role: "Hides when it keeps still and ambushes armour with missiles. Thin armour in an open fight.",
    model_id: "stealth_tank",
    cost: 850,
    build_secs: 15.0,
    max_health: 380.0,
    speed: 4.6,
    turn_rate: 3.0,
    radius: 1.2,
    armor: ArmorClass::Vehicle,
    weapon: Some(STEALTH_MISSILES),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: Some(Faction::Syndicate),
    heal_per_sec: 0.0,
    sight: 18.0,
};

const REPAIR_TRUCK: UnitSpec = UnitSpec {
    name: "Repair Truck",
    role: "Unarmed. Drives to damaged vehicles nearby and repairs them in the field.",
    model_id: "repair_truck",
    cost: 600,
    build_secs: 10.0,
    max_health: 220.0,
    speed: 4.5,
    turn_rate: 4.5,
    radius: 1.0,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::RepairBay),
    faction: Some(Faction::Coalition),
    heal_per_sec: 12.0,
    sight: 14.0,
};

const MISSILE_TRUCK: UnitSpec = UnitSpec {
    name: "Missile Truck",
    role: "Rocket artillery on wheels. Outranges defenses, fragile up close.",
    model_id: "missile_truck",
    cost: 1000,
    build_secs: 18.0,
    max_health: 220.0,
    speed: 4.0,
    turn_rate: 2.5,
    radius: 1.4,
    armor: ArmorClass::Vehicle,
    weapon: Some(ROCKET_SALVO),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: Some(Faction::Syndicate),
    heal_per_sec: 0.0,
    sight: 14.0,
};

const COMMAND_CENTER: BuildingSpec = BuildingSpec {
    name: "Command Center",
    role: "Heart of the base. Lose every structure and the war is lost.",
    model_id: "command_center",
    footprint: IVec2::new(3, 3),
    cost: 0,
    build_secs: 0.0,
    max_health: 1600.0,
    power: 20,
    prerequisites: &[],
    weapon: None,
    sight: 18.0,
    faction: None,
};

const POWER_PLANT: BuildingSpec = BuildingSpec {
    name: "Power Plant",
    role: "Generates 100 power. Low power halves production speed.",
    model_id: "power_plant",
    footprint: IVec2::new(2, 2),
    cost: 300,
    build_secs: 9.0,
    max_health: 500.0,
    power: 100,
    prerequisites: &[BuildingKind::CommandCenter],
    weapon: None,
    sight: 10.0,
    faction: None,
};

const REFINERY: BuildingSpec = BuildingSpec {
    name: "Refinery",
    role: "Turns crystals into credits. Comes with a free harvester.",
    model_id: "refinery",
    footprint: IVec2::new(3, 3),
    cost: 1500,
    build_secs: 18.0,
    max_health: 900.0,
    power: -40,
    prerequisites: &[BuildingKind::PowerPlant],
    weapon: None,
    sight: 12.0,
    faction: None,
};

const BARRACKS: BuildingSpec = BuildingSpec {
    name: "Barracks",
    role: "Trains infantry.",
    model_id: "barracks",
    footprint: IVec2::new(2, 2),
    cost: 400,
    build_secs: 11.0,
    max_health: 600.0,
    power: -20,
    prerequisites: &[BuildingKind::PowerPlant],
    weapon: None,
    sight: 10.0,
    faction: None,
};

const FACTORY: BuildingSpec = BuildingSpec {
    name: "War Factory",
    role: "Builds vehicles, including harvesters.",
    model_id: "factory",
    footprint: IVec2::new(3, 3),
    cost: 1800,
    build_secs: 22.0,
    max_health: 1000.0,
    power: -50,
    prerequisites: &[BuildingKind::Refinery],
    weapon: None,
    sight: 12.0,
    faction: None,
};

const GUN_TURRET: BuildingSpec = BuildingSpec {
    name: "Gun Turret",
    role: "Static defense cannon. Good against vehicles.",
    model_id: "gun_turret",
    footprint: IVec2::new(1, 1),
    cost: 600,
    build_secs: 10.0,
    max_health: 500.0,
    power: -25,
    prerequisites: &[BuildingKind::Barracks],
    weapon: Some(TURRET_CANNON),
    sight: 16.0,
    faction: None,
};

const WALL: BuildingSpec = BuildingSpec {
    name: "Wall",
    role: "Concrete barrier. Drag to lay a line; each segment is paid as it is placed.",
    model_id: "wall",
    footprint: IVec2::new(1, 1),
    cost: 50,
    build_secs: 0.0,
    max_health: 600.0,
    power: 0,
    prerequisites: &[BuildingKind::Barracks],
    weapon: None,
    sight: 4.0,
    faction: None,
};

const HELIPAD: BuildingSpec = BuildingSpec {
    name: "Helipad",
    role: "Builds gunships and repairs aircraft hovering over the pad.",
    model_id: "helipad",
    footprint: IVec2::new(2, 2),
    cost: 1000,
    build_secs: 14.0,
    max_health: 700.0,
    power: -30,
    prerequisites: &[BuildingKind::Factory],
    weapon: None,
    sight: 12.0,
    faction: None,
};

const FLAK_CANNON: BuildingSpec = BuildingSpec {
    name: "Flak Cannon",
    role: "Anti-aircraft gun pit. Cannot shoot at ground targets.",
    model_id: "flak_cannon",
    footprint: IVec2::new(1, 1),
    cost: 700,
    build_secs: 10.0,
    max_health: 450.0,
    power: -25,
    prerequisites: &[BuildingKind::Barracks],
    weapon: Some(FLAK_GUNS),
    sight: 18.0,
    faction: None,
};

const TECH_CENTER: BuildingSpec = BuildingSpec {
    name: "Tech Center",
    role: "Research labs. Unlocks heavy units: Bulwark or Missile Truck, Howitzer and Gunship.",
    model_id: "tech_center",
    footprint: IVec2::new(2, 2),
    cost: 1500,
    build_secs: 20.0,
    max_health: 800.0,
    power: -60,
    prerequisites: &[BuildingKind::Factory],
    weapon: None,
    sight: 12.0,
    faction: None,
};

const REPAIR_BAY: BuildingSpec = BuildingSpec {
    name: "Repair Bay",
    role: "Repairs vehicles that wait next to it.",
    model_id: "repair_bay",
    footprint: IVec2::new(2, 2),
    cost: 800,
    build_secs: 12.0,
    max_health: 700.0,
    power: -20,
    prerequisites: &[BuildingKind::Factory],
    weapon: None,
    sight: 10.0,
    faction: None,
};

const ROCKET_BUNKER: BuildingSpec = BuildingSpec {
    name: "Rocket Bunker",
    role: "Concrete dome with a rocket launcher. Hits vehicles and aircraft alike.",
    model_id: "rocket_bunker",
    footprint: IVec2::new(1, 1),
    cost: 800,
    build_secs: 12.0,
    max_health: 650.0,
    power: -30,
    prerequisites: &[BuildingKind::Barracks],
    weapon: Some(BUNKER_ROCKETS),
    sight: 15.0,
    faction: Some(Faction::Syndicate),
};

/// `cost` of a neutral site is what destroying it is worth to a veteran; nobody pays it.
const SUPPLY_DEPOT: BuildingSpec = BuildingSpec {
    name: "Supply Depot",
    role: "Neutral. Once an engineer captures it, it sends its owner credits every few seconds.",
    model_id: "supply_depot",
    footprint: IVec2::new(2, 2),
    cost: 1000,
    build_secs: 0.0,
    max_health: 900.0,
    power: 0,
    prerequisites: &[],
    weapon: None,
    sight: 8.0,
    faction: None,
};

const OBSERVATION_POST: BuildingSpec = BuildingSpec {
    name: "Observation Post",
    role: "Neutral. Once an engineer captures it, its tower watches far across the battlefield.",
    model_id: "observation_post",
    footprint: IVec2::new(3, 3),
    cost: 1000,
    build_secs: 0.0,
    max_health: 800.0,
    power: 0,
    prerequisites: &[],
    weapon: None,
    sight: 32.0,
    faction: None,
};

const FIELD_HOSPITAL: BuildingSpec = BuildingSpec {
    name: "Field Hospital",
    role: "Neutral. Once an engineer captures it, it heals its owner's infantry nearby.",
    model_id: "field_hospital",
    footprint: IVec2::new(2, 2),
    cost: 1000,
    build_secs: 0.0,
    max_health: 800.0,
    power: 0,
    prerequisites: &[],
    weapon: None,
    sight: 10.0,
    faction: None,
};

pub const STARTING_CREDITS: i32 = 5000;
pub const HARVESTER_CAPACITY: u32 = 500;
pub const CRYSTAL_PER_CELL: u32 = 600;

#[cfg(test)]
mod tests {
    use super::*;

    fn reachable_from_command_center() -> Vec<BuildingKind> {
        let mut owned = vec![BuildingKind::CommandCenter];
        loop {
            let next = BuildingKind::CONSTRUCTIBLE.into_iter().find(|kind| {
                !owned.contains(kind) && kind.spec().prerequisites.iter().all(|p| owned.contains(p))
            });
            match next {
                Some(kind) => owned.push(kind),
                None => return owned,
            }
        }
    }

    #[test]
    fn walls_and_captured_sites_do_not_keep_a_side_in_the_war() {
        assert!(BuildingKind::PowerPlant.keeps_a_side_alive());
        assert!(!BuildingKind::Wall.keeps_a_side_alive());
        for site in BuildingKind::NEUTRAL_SITES {
            assert!(!site.keeps_a_side_alive());
            assert!(!BuildingKind::CONSTRUCTIBLE.contains(&site));
        }
    }

    #[test]
    fn the_whole_tech_tree_is_reachable() {
        let owned = reachable_from_command_center();
        for kind in BuildingKind::CONSTRUCTIBLE {
            assert!(owned.contains(&kind), "{kind:?} can never be built");
        }
        for kind in UnitKind::INFANTRY.iter().chain(UnitKind::VEHICLES.iter()) {
            assert!(
                owned.contains(&kind.spec().producer),
                "{kind:?} has no buildable producer"
            );
            if let Some(required) = kind.spec().requires {
                assert!(
                    owned.contains(&required),
                    "{kind:?} needs an unbuildable {required:?}"
                );
            }
        }
    }

    #[test]
    fn weapons_have_a_clear_role() {
        let rifle = UnitKind::Rifleman.spec().weapon.unwrap();
        let cannon = UnitKind::Tank.spec().weapon.unwrap();
        let howitzer = UnitKind::Artillery.spec().weapon.unwrap();
        assert!(
            rifle.multiplier_against(ArmorClass::Infantry)
                > rifle.multiplier_against(ArmorClass::Vehicle)
        );
        assert!(
            cannon.multiplier_against(ArmorClass::Vehicle)
                > rifle.multiplier_against(ArmorClass::Vehicle)
        );
        assert!(howitzer.range > cannon.range && howitzer.splash_radius > 0.0);
    }

    #[test]
    fn a_base_can_power_its_own_production() {
        let supply: i32 = [BuildingKind::CommandCenter, BuildingKind::PowerPlant]
            .iter()
            .map(|k| k.spec().power)
            .sum();
        let demand: i32 = [BuildingKind::Refinery, BuildingKind::Barracks]
            .iter()
            .map(|k| -k.spec().power)
            .sum();
        assert!(supply >= demand);
        assert!(
            STARTING_CREDITS
                >= BuildingKind::PowerPlant.spec().cost + BuildingKind::Refinery.spec().cost
        );
    }

    #[test]
    fn sides_keep_their_names_in_saved_text() {
        assert_eq!(
            serde_json::to_string(&Team::Crimson).unwrap(),
            r#""Crimson""#
        );
        for team in [
            Team::Player,
            Team::Violet,
            Team::from_index(4),
            Team::from_index(200),
        ] {
            let text = serde_json::to_string(&team).unwrap();
            assert_eq!(serde_json::from_str::<Team>(&text).unwrap(), team, "{text}");
        }
        assert!(serde_json::from_str::<Team>(r#""Nobody""#).is_err());
    }
}
