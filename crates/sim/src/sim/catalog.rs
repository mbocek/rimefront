use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::sim::cues::Sound;
use crate::sim::eras::{DefenceLine, Era, Eras, UnitLine};

/// A side in the battle. The first four are the sides a level names; a world that players join has more.
/// Which side is played from this machine is `LocalTeam`, not necessarily `Team::Azure`.
#[derive(Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Team(u8);

#[allow(non_upper_case_globals)]
impl Team {
    pub const Azure: Team = Team(0);
    pub const Crimson: Team = Team(1);
    pub const Amber: Team = Team(2);
    pub const Violet: Team = Team(3);

    /// The sides a level names, the one its player leads first.
    pub const ALL: [Team; 4] = [Team::Azure, Team::Crimson, Team::Amber, Team::Violet];
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
        if self == Team::Azure {
            return faction.name().to_string();
        }
        format!("{} {}", self.look().name, faction.name())
    }

    /// The colour its models' side-coloured surfaces take unless its player chose another.
    pub fn model_tint(self) -> Color {
        self.look().model_tint
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

/// Saved games, levels and replays name the first side "Player", from before sides were named by colour.
const NAMED_SIDES: [&str; 4] = ["Player", "Crimson", "Amber", "Violet"];

struct SideLook {
    name: &'static str,
    color: Color,
    /// Linear, as the models' palettes are painted; the first four are the tints the models were once
    /// exported in, in tools/blender/rimefront_models/materials.py.
    model_tint: Color,
}

/// Sides past these take the same looks again, unless their players chose colours of their own.
const SIDE_LOOKS: [SideLook; 8] = [
    SideLook {
        name: "Azure",
        color: Color::srgb(0.25, 0.55, 1.0),
        model_tint: Color::linear_rgb(0.10, 0.30, 0.85),
    },
    SideLook {
        name: "Crimson",
        color: Color::srgb(1.0, 0.3, 0.25),
        model_tint: Color::linear_rgb(0.80, 0.12, 0.10),
    },
    SideLook {
        name: "Amber",
        color: Color::srgb(1.0, 0.62, 0.15),
        model_tint: Color::linear_rgb(0.95, 0.48, 0.06),
    },
    SideLook {
        name: "Violet",
        color: Color::srgb(0.72, 0.4, 1.0),
        model_tint: Color::linear_rgb(0.48, 0.16, 0.78),
    },
    SideLook {
        name: "Jade",
        color: Color::srgb(0.25, 0.85, 0.45),
        model_tint: Color::linear_rgb(0.05, 0.55, 0.22),
    },
    SideLook {
        name: "Rust",
        color: Color::srgb(0.75, 0.4, 0.2),
        model_tint: Color::linear_rgb(0.50, 0.20, 0.07),
    },
    SideLook {
        name: "Gold",
        color: Color::srgb(0.95, 0.85, 0.3),
        model_tint: Color::linear_rgb(0.85, 0.68, 0.08),
    },
    SideLook {
        name: "Rose",
        color: Color::srgb(1.0, 0.5, 0.75),
        model_tint: Color::linear_rgb(0.90, 0.32, 0.55),
    },
];

/// By name where people may read it (levels, saved games), by number in compact binary forms.
impl Serialize for Team {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.key())
        } else {
            serializer.serialize_u8(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Team {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if !deserializer.is_human_readable() {
            return u8::deserialize(deserializer).map(Team);
        }
        let key = String::deserialize(deserializer)?;
        Team::from_key(&key)
            .ok_or_else(|| serde::de::Error::custom(format!("no side is called {key}")))
    }
}

/// A colour a player chose for their side's models, in sRGB, as it is saved and travels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct SideColor(pub [u8; 3]);

impl SideColor {
    pub fn color(self) -> Color {
        let [red, green, blue] = self.0;
        Color::srgb_u8(red, green, blue)
    }

    /// How far apart two colours look, roughly: the squared distance of their sRGB values.
    pub fn distance(self, other: SideColor) -> u32 {
        self.0
            .iter()
            .zip(other.0)
            .map(|(&a, b)| (i32::from(a) - i32::from(b)).unsigned_abs().pow(2))
            .sum()
    }
}

/// The colours a player may choose for their side, far enough apart to tell sides by.
pub const SIDE_PALETTE: [(&str, SideColor); 12] = [
    ("Azure", SideColor([64, 140, 255])),
    ("Crimson", SideColor([220, 45, 40])),
    ("Amber", SideColor([245, 140, 20])),
    ("Violet", SideColor([150, 60, 210])),
    ("Jade", SideColor([20, 170, 90])),
    ("Rust", SideColor([150, 70, 30])),
    ("Gold", SideColor([230, 200, 40])),
    ("Rose", SideColor([240, 110, 170])),
    ("Teal", SideColor([20, 170, 180])),
    ("Lime", SideColor([140, 210, 40])),
    ("Slate", SideColor([110, 120, 140])),
    ("Ivory", SideColor([235, 235, 225])),
];

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
        if team == Team::Azure {
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
            Alliance::CoalitionAgainstSyndicate => usize::from(team != Team::Azure),
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
        self.cover(team);
        self.camps[team.index()] = camp;
    }

    /// `team` leaves its allies and stands alone.
    pub fn leave_camp(&mut self, team: Team) {
        self.cover(team);
        self.camps[team.index()] = Team::MOST + team.index();
    }

    /// The other sides in `team`'s camp, lowest first.
    pub fn allies_of(&self, team: Team) -> Vec<Team> {
        (0..self.camps.len())
            .map(Team::from_index)
            .filter(|&other| other != team && !self.hostile(team, other))
            .collect()
    }

    /// Every camp of more than one side, each lowest side first, the camps by their lowest side.
    pub fn alliances(&self) -> Vec<Vec<Team>> {
        let mut camps: Vec<Vec<Team>> = Vec::new();
        for team in (0..self.camps.len()).map(Team::from_index) {
            match camps.iter_mut().find(|camp| !self.hostile(camp[0], team)) {
                Some(camp) => camp.push(team),
                None => camps.push(vec![team]),
            }
        }
        camps.retain(|camp| camp.len() > 1);
        camps
    }

    /// Every side in a camp of its own, as in a free-for-all, then `team` and `allies` in one camp.
    pub fn with_allies(team: Team, allies: &[Team]) -> Self {
        let mut diplomacy = Self::of(Alliance::FreeForAll);
        for &ally in allies {
            diplomacy.join_camp_of(ally, team);
        }
        diplomacy
    }

    fn cover(&mut self, team: Team) {
        let own_camps = self.camps.len()..=team.index();
        self.camps.extend(own_camps.map(|index| Team::MOST + index));
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ProjectileStyle {
    Bullet,
    Shell,
    ArtilleryShell,
    Missile,
    Flame,
    Arrow,
    Boulder,
    /// A blow struck at arm's length: nothing flies, the hit lands at once.
    Strike,
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
    Spearman,
    Archer,
    Healer,
    Builder,
    Horseman,
    Chariot,
    Catapult,
    Wagon,
    OreCart,
    Musketeer,
    Bombardier,
    Surgeon,
    Sapper,
    Dragoon,
    Cuirassier,
    FieldGun,
    OreWagon,
    Fusilier,
    MortarTeam,
    Orderly,
    Pioneer,
    ArmouredCar,
    Landship,
    SiegeGun,
    Lorry,
    SteamHarvester,
    Biplane,
    BiplaneBomber,
    ExoTrooper,
    Lancer,
    Medtech,
    Fabricator,
    HoverBike,
    HoverTank,
    RailCannon,
    HoverCarrier,
    HarvestDrone,
    Interceptor,
}

pub struct UnitSpec {
    pub era: Era,
    pub line: UnitLine,
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
    pub const INFANTRY: [UnitKind; 23] = [
        UnitKind::Spearman,
        UnitKind::Archer,
        UnitKind::Healer,
        UnitKind::Builder,
        UnitKind::Musketeer,
        UnitKind::Bombardier,
        UnitKind::Surgeon,
        UnitKind::Sapper,
        UnitKind::Fusilier,
        UnitKind::MortarTeam,
        UnitKind::Orderly,
        UnitKind::Pioneer,
        UnitKind::ExoTrooper,
        UnitKind::Lancer,
        UnitKind::Medtech,
        UnitKind::Fabricator,
        UnitKind::Rifleman,
        UnitKind::Rocketeer,
        UnitKind::Grenadier,
        UnitKind::Sniper,
        UnitKind::FlameTrooper,
        UnitKind::Medic,
        UnitKind::Engineer,
    ];
    pub const VEHICLES: [UnitKind; 36] = [
        UnitKind::Horseman,
        UnitKind::Chariot,
        UnitKind::Catapult,
        UnitKind::Wagon,
        UnitKind::OreCart,
        UnitKind::Dragoon,
        UnitKind::Cuirassier,
        UnitKind::FieldGun,
        UnitKind::OreWagon,
        UnitKind::ArmouredCar,
        UnitKind::Landship,
        UnitKind::SiegeGun,
        UnitKind::Lorry,
        UnitKind::SteamHarvester,
        UnitKind::Biplane,
        UnitKind::BiplaneBomber,
        UnitKind::HoverBike,
        UnitKind::HoverTank,
        UnitKind::RailCannon,
        UnitKind::HoverCarrier,
        UnitKind::HarvestDrone,
        UnitKind::Interceptor,
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
    pub const ALL: [UnitKind; 59] = [
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
        UnitKind::Spearman,
        UnitKind::Archer,
        UnitKind::Healer,
        UnitKind::Builder,
        UnitKind::Horseman,
        UnitKind::Chariot,
        UnitKind::Catapult,
        UnitKind::Wagon,
        UnitKind::OreCart,
        UnitKind::Musketeer,
        UnitKind::Bombardier,
        UnitKind::Surgeon,
        UnitKind::Sapper,
        UnitKind::Dragoon,
        UnitKind::Cuirassier,
        UnitKind::FieldGun,
        UnitKind::OreWagon,
        UnitKind::Fusilier,
        UnitKind::MortarTeam,
        UnitKind::Orderly,
        UnitKind::Pioneer,
        UnitKind::ArmouredCar,
        UnitKind::Landship,
        UnitKind::SiegeGun,
        UnitKind::Lorry,
        UnitKind::SteamHarvester,
        UnitKind::Biplane,
        UnitKind::BiplaneBomber,
        UnitKind::ExoTrooper,
        UnitKind::Lancer,
        UnitKind::Medtech,
        UnitKind::Fabricator,
        UnitKind::HoverBike,
        UnitKind::HoverTank,
        UnitKind::RailCannon,
        UnitKind::HoverCarrier,
        UnitKind::HarvestDrone,
        UnitKind::Interceptor,
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
            UnitKind::Spearman => &SPEARMAN,
            UnitKind::Archer => &ARCHER,
            UnitKind::Healer => &HEALER,
            UnitKind::Builder => &BUILDER,
            UnitKind::Horseman => &HORSEMAN,
            UnitKind::Chariot => &CHARIOT,
            UnitKind::Catapult => &CATAPULT,
            UnitKind::Wagon => &WAGON,
            UnitKind::OreCart => &ORE_CART,
            UnitKind::Musketeer => &MUSKETEER,
            UnitKind::Bombardier => &BOMBARDIER,
            UnitKind::Surgeon => &SURGEON,
            UnitKind::Sapper => &SAPPER,
            UnitKind::Dragoon => &DRAGOON,
            UnitKind::Cuirassier => &CUIRASSIER,
            UnitKind::FieldGun => &FIELD_GUN,
            UnitKind::OreWagon => &ORE_WAGON,
            UnitKind::Fusilier => &FUSILIER,
            UnitKind::MortarTeam => &MORTAR_TEAM,
            UnitKind::Orderly => &ORDERLY,
            UnitKind::Pioneer => &PIONEER,
            UnitKind::ArmouredCar => &ARMOURED_CAR,
            UnitKind::Landship => &LANDSHIP,
            UnitKind::SiegeGun => &SIEGE_GUN,
            UnitKind::Lorry => &LORRY,
            UnitKind::SteamHarvester => &STEAM_HARVESTER,
            UnitKind::Biplane => &BIPLANE,
            UnitKind::BiplaneBomber => &BIPLANE_BOMBER,
            UnitKind::ExoTrooper => &EXO_TROOPER,
            UnitKind::Lancer => &LANCER,
            UnitKind::Medtech => &MEDTECH,
            UnitKind::Fabricator => &FABRICATOR,
            UnitKind::HoverBike => &HOVER_BIKE,
            UnitKind::HoverTank => &HOVER_TANK,
            UnitKind::RailCannon => &RAIL_CANNON,
            UnitKind::HoverCarrier => &HOVER_CARRIER,
            UnitKind::HarvestDrone => &HARVEST_DRONE,
            UnitKind::Interceptor => &INTERCEPTOR,
        }
    }

    /// Aircraft that strike once and fly back to a Helipad to rearm before the next run.
    pub fn rearms_at_pad(self) -> bool {
        self.line() == UnitLine::Bomber
    }

    /// Aircraft that land on their Helipad when idle there instead of hovering, as only a helicopter can.
    pub fn lands_on_pad(self) -> bool {
        matches!(self.line(), UnitLine::Bomber | UnitLine::Fighter)
    }

    /// Units that blend into the ground after keeping still for a while.
    pub fn hides_when_still(self) -> bool {
        matches!(self.line(), UnitLine::Marksman | UnitLine::Stealth)
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
    Watchtower,
    Palisade,
    CannonTower,
    Pillbox,
    LaserTurret,
    Project,
}

pub struct BuildingSpec {
    pub era: Eras,
    pub line: Option<DefenceLine>,
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
    pub const CONSTRUCTIBLE: [BuildingKind; 17] = [
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
        BuildingKind::Watchtower,
        BuildingKind::Palisade,
        BuildingKind::CannonTower,
        BuildingKind::Pillbox,
        BuildingKind::LaserTurret,
        BuildingKind::Project,
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
        self.spec().line == Some(DefenceLine::Wall)
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
            BuildingKind::Watchtower => &WATCHTOWER,
            BuildingKind::CannonTower => &CANNON_TOWER,
            BuildingKind::Pillbox => &PILLBOX,
            BuildingKind::LaserTurret => &LASER_TURRET,
            BuildingKind::Project => &PROJECT,
            BuildingKind::Palisade => &PALISADE,
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
    era: Era::Modern,
    line: UnitLine::Infantry,
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
    era: Era::Modern,
    line: UnitLine::Fast,
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
    era: Era::Modern,
    line: UnitLine::Transport,
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
    era: Era::Modern,
    line: UnitLine::Main,
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
    era: Era::Modern,
    line: UnitLine::Siege,
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
    era: Era::Modern,
    line: UnitLine::Harvester,
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
    era: Era::Modern,
    line: UnitLine::AntiArmour,
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
    era: Era::Modern,
    line: UnitLine::Healer,
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
    era: Era::Modern,
    line: UnitLine::Builder,
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
    era: Era::Modern,
    line: UnitLine::Gunship,
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
    era: Era::Modern,
    line: UnitLine::Heavy,
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
    era: Era::Modern,
    line: UnitLine::AntiAir,
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
    era: Era::Modern,
    line: UnitLine::Marksman,
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
    era: Era::Modern,
    line: UnitLine::Flame,
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
    era: Era::Modern,
    line: UnitLine::Ranged,
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
    era: Era::Modern,
    line: UnitLine::Bomber,
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
    era: Era::Modern,
    line: UnitLine::Fighter,
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
    era: Era::Modern,
    line: UnitLine::Raider,
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
    era: Era::Modern,
    line: UnitLine::Stealth,
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
    era: Era::Modern,
    line: UnitLine::Repair,
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
    era: Era::Modern,
    line: UnitLine::RocketArtillery,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Only(Era::Modern),
    line: Some(DefenceLine::Tower),
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
    era: Eras::Since(Era::Industrial),
    line: Some(DefenceLine::Wall),
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
    era: Eras::Since(Era::Industrial),
    line: None,
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
    era: Eras::Since(Era::Industrial),
    line: Some(DefenceLine::AntiAir),
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Only(Era::Modern),
    line: Some(DefenceLine::Bunker),
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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
    era: Eras::Since(Era::Antiquity),
    line: None,
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

/// Thrust at arm's length; a wall of spears is what stops a charge.
const SPEAR: WeaponSpec = WeaponSpec {
    range: 1.8,
    damage: 11.0,
    reload_secs: 1.0,
    projectile_speed: 40.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Strike,
    versus_infantry: 0.75,
    versus_vehicle: 1.1,
    versus_structure: 0.2,
    versus_aircraft: 0.0,
    report: Sound::BladeStrike,
};

const SABRE: WeaponSpec = WeaponSpec {
    range: 1.9,
    damage: 15.0,
    reload_secs: 0.9,
    projectile_speed: 40.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Strike,
    versus_infantry: 1.2,
    versus_vehicle: 0.65,
    versus_structure: 0.15,
    versus_aircraft: 0.0,
    report: Sound::BladeStrike,
};

const BOW: WeaponSpec = WeaponSpec {
    range: 9.0,
    damage: 15.0,
    reload_secs: 1.2,
    projectile_speed: 22.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Arrow,
    versus_infantry: 1.0,
    versus_vehicle: 0.35,
    versus_structure: 0.1,
    versus_aircraft: 0.0,
    report: Sound::BowShot,
};

const CHARIOT_BOW: WeaponSpec = WeaponSpec {
    range: 9.5,
    damage: 22.0,
    reload_secs: 1.3,
    projectile_speed: 24.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Arrow,
    versus_infantry: 1.0,
    versus_vehicle: 0.8,
    versus_structure: 0.35,
    versus_aircraft: 0.0,
    report: Sound::BowShot,
};

/// Lobbed stone: slow, inaccurate and crushing against walls.
const CATAPULT_ARM: WeaponSpec = WeaponSpec {
    range: 18.0,
    damage: 70.0,
    reload_secs: 5.0,
    projectile_speed: 14.0,
    splash_radius: 2.0,
    style: ProjectileStyle::Boulder,
    versus_infantry: 0.45,
    versus_vehicle: 0.6,
    versus_structure: 1.5,
    versus_aircraft: 0.0,
    report: Sound::CatapultThrow,
};

const TOWER_BOWS: WeaponSpec = WeaponSpec {
    range: 11.0,
    damage: 17.0,
    reload_secs: 0.9,
    projectile_speed: 24.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Arrow,
    versus_infantry: 1.0,
    versus_vehicle: 0.7,
    versus_structure: 0.3,
    versus_aircraft: 0.0,
    report: Sound::BowShot,
};

const SPEARMAN: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Infantry,
    name: "Spearman",
    role: "Cheap infantry with spear and shield. Holds the line and stops horsemen.",
    model_id: "spearman",
    cost: 100,
    build_secs: 5.0,
    max_health: 65.0,
    speed: 2.3,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(SPEAR),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const ARCHER: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Ranged,
    name: "Archer",
    role: "Shoots arrows from afar. Strong against soldiers, weak in close combat.",
    model_id: "archer",
    cost: 130,
    build_secs: 6.0,
    max_health: 45.0,
    speed: 2.3,
    turn_rate: 9.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(BOW),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const HEALER: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Healer,
    name: "Healer",
    role: "Unarmed. Binds the wounds of infantry nearby and follows the squad.",
    model_id: "healer",
    cost: 150,
    build_secs: 6.0,
    max_health: 50.0,
    speed: 2.4,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 6.0,
    sight: 12.0,
};

const BUILDER: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Builder,
    name: "Builder",
    role: "Unarmed. Right-click an enemy structure to take it over, or a damaged own one to mend it.",
    model_id: "builder",
    cost: 400,
    build_secs: 9.0,
    max_health: 50.0,
    speed: 2.5,
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

const HORSEMAN: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Fast,
    name: "Horseman",
    role: "Fast rider with a sabre. Runs down archers, breaks on spears.",
    model_id: "horseman",
    cost: 250,
    build_secs: 7.0,
    max_health: 150.0,
    speed: 6.2,
    turn_rate: 6.0,
    radius: 0.8,
    armor: ArmorClass::Vehicle,
    weapon: Some(SABRE),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 18.0,
};

const CHARIOT: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Main,
    name: "Chariot",
    role: "Two horses and an archer on a war cart. The backbone of an ancient army.",
    model_id: "chariot",
    cost: 550,
    build_secs: 12.0,
    max_health: 400.0,
    speed: 4.6,
    turn_rate: 3.0,
    radius: 1.1,
    armor: ArmorClass::Vehicle,
    weapon: Some(CHARIOT_BOW),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 15.0,
};

const CATAPULT: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Siege,
    name: "Catapult",
    role: "Hurls stones far beyond bowshot. Breaks walls and towers, helpless up close.",
    model_id: "catapult",
    cost: 700,
    build_secs: 16.0,
    max_health: 180.0,
    speed: 2.0,
    turn_rate: 1.8,
    radius: 1.2,
    armor: ArmorClass::Vehicle,
    weapon: Some(CATAPULT_ARM),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const WAGON: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Transport,
    name: "Supply Wagon",
    role: "A covered wagon for five infantry, who shoot from under its canvas. Unarmed itself.",
    model_id: "wagon",
    cost: 400,
    build_secs: 9.0,
    max_health: 320.0,
    speed: 4.0,
    turn_rate: 3.5,
    radius: 1.1,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const ORE_CART: UnitSpec = UnitSpec {
    era: Era::Antiquity,
    line: UnitLine::Harvester,
    name: "Ore Cart",
    role: "Ox cart that gathers crystals and hauls them to a smelter. Unarmed.",
    model_id: "ore_cart",
    cost: 800,
    build_secs: 14.0,
    max_health: 500.0,
    speed: 2.7,
    turn_rate: 2.2,
    radius: 1.4,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 10.0,
};

const WATCHTOWER: BuildingSpec = BuildingSpec {
    era: Eras::Only(Era::Antiquity),
    line: Some(DefenceLine::Tower),
    name: "Watchtower",
    role: "Timber tower manned by archers. Covers the approaches to the base.",
    model_id: "watchtower",
    footprint: IVec2::new(1, 1),
    cost: 500,
    build_secs: 9.0,
    max_health: 450.0,
    power: -10,
    prerequisites: &[BuildingKind::Barracks],
    weapon: Some(TOWER_BOWS),
    sight: 16.0,
    faction: None,
};

const PALISADE: BuildingSpec = BuildingSpec {
    era: Eras::Span(Era::Antiquity, Era::Gunpowder),
    line: Some(DefenceLine::Wall),
    name: "Palisade",
    role: "Sharpened stakes. Drag to lay a line; each stretch is paid as it is placed.",
    model_id: "palisade",
    footprint: IVec2::new(1, 1),
    cost: 30,
    build_secs: 0.0,
    max_health: 450.0,
    power: 0,
    prerequisites: &[BuildingKind::Barracks],
    weapon: None,
    sight: 4.0,
    faction: None,
};

const MUSKET: WeaponSpec = WeaponSpec {
    range: 8.0,
    damage: 16.0,
    reload_secs: 2.0,
    projectile_speed: 45.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.55,
    versus_structure: 0.2,
    versus_aircraft: 0.0,
    report: Sound::MusketShot,
};

const HAND_GRENADES: WeaponSpec = WeaponSpec {
    range: 7.0,
    damage: 30.0,
    reload_secs: 2.6,
    projectile_speed: 12.0,
    splash_radius: 1.8,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 1.0,
    versus_vehicle: 0.4,
    versus_structure: 0.8,
    versus_aircraft: 0.0,
    report: Sound::GrenadeLaunch,
};

const CARBINE: WeaponSpec = WeaponSpec {
    range: 7.5,
    damage: 18.0,
    reload_secs: 1.4,
    projectile_speed: 45.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.6,
    versus_structure: 0.2,
    versus_aircraft: 0.0,
    report: Sound::MusketShot,
};

const HEAVY_SABRE: WeaponSpec = WeaponSpec {
    range: 1.9,
    damage: 26.0,
    reload_secs: 1.0,
    projectile_speed: 40.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Strike,
    versus_infantry: 0.9,
    versus_vehicle: 0.9,
    versus_structure: 0.2,
    versus_aircraft: 0.0,
    report: Sound::BladeStrike,
};

const FIELD_GUN_SHOT: WeaponSpec = WeaponSpec {
    range: 17.0,
    damage: 75.0,
    reload_secs: 4.5,
    projectile_speed: 20.0,
    splash_radius: 1.8,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 0.6,
    versus_vehicle: 0.9,
    versus_structure: 1.3,
    versus_aircraft: 0.0,
    report: Sound::CannonShot,
};

const TOWER_CANNON: WeaponSpec = WeaponSpec {
    range: 12.0,
    damage: 40.0,
    reload_secs: 2.2,
    projectile_speed: 30.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.7,
    versus_vehicle: 1.0,
    versus_structure: 0.4,
    versus_aircraft: 0.0,
    report: Sound::CannonShot,
};

const MUSKETEER: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Infantry,
    name: "Musketeer",
    role: "Infantry with musket and bayonet. Fires slow, heavy volleys.",
    model_id: "musketeer",
    cost: 120,
    build_secs: 5.5,
    max_health: 70.0,
    speed: 2.3,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(MUSKET),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const BOMBARDIER: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Ranged,
    name: "Bombardier",
    role: "Throws grenades over the line. Scatters massed infantry, weak against riders.",
    model_id: "bombardier",
    cost: 200,
    build_secs: 7.0,
    max_health: 60.0,
    speed: 2.3,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(HAND_GRENADES),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 13.0,
};

const SURGEON: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Healer,
    name: "Field Surgeon",
    role: "Unarmed. Treats wounded infantry nearby and follows the squad.",
    model_id: "surgeon",
    cost: 180,
    build_secs: 6.0,
    max_health: 55.0,
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

const SAPPER: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Builder,
    name: "Sapper",
    role: "Unarmed. Right-click an enemy structure to take it over, or a damaged own one to mend it.",
    model_id: "sapper",
    cost: 450,
    build_secs: 9.0,
    max_health: 55.0,
    speed: 2.5,
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

const DRAGOON: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Fast,
    name: "Dragoon",
    role: "Mounted infantry with a carbine. Fast, hunts gunners and skirmishers.",
    model_id: "dragoon",
    cost: 350,
    build_secs: 8.0,
    max_health: 220.0,
    speed: 5.6,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Vehicle,
    weapon: Some(CARBINE),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 18.0,
};

const CUIRASSIER: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Main,
    name: "Cuirassier",
    role: "Armoured heavy cavalry. Charges home with the sabre; muskets in a line hurt it.",
    model_id: "cuirassier",
    cost: 650,
    build_secs: 13.0,
    max_health: 400.0,
    speed: 4.8,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Vehicle,
    weapon: Some(HEAVY_SABRE),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 15.0,
};

const FIELD_GUN: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Siege,
    name: "Field Gun",
    role: "A bronze cannon on a carriage. Batters structures and massed troops from afar.",
    model_id: "field_gun",
    cost: 800,
    build_secs: 16.0,
    max_health: 220.0,
    speed: 2.4,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Vehicle,
    weapon: Some(FIELD_GUN_SHOT),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const ORE_WAGON: UnitSpec = UnitSpec {
    era: Era::Gunpowder,
    line: UnitLine::Harvester,
    name: "Ore Wagon",
    role: "Horse-drawn wagon that gathers crystals and hauls them to the foundry. Unarmed.",
    model_id: "ore_wagon",
    cost: 900,
    build_secs: 14.0,
    max_health: 560.0,
    speed: 3.0,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 10.0,
};

const CANNON_TOWER: BuildingSpec = BuildingSpec {
    era: Eras::Only(Era::Gunpowder),
    line: Some(DefenceLine::Tower),
    name: "Cannon Tower",
    role: "Stone tower with a gun on its top. Covers the approaches to the base.",
    model_id: "cannon_tower",
    footprint: IVec2::new(1, 1),
    cost: 650,
    build_secs: 10.0,
    max_health: 650.0,
    power: -10,
    prerequisites: &[BuildingKind::Barracks],
    weapon: Some(TOWER_CANNON),
    sight: 17.0,
    faction: None,
};

const BOLT_RIFLE: WeaponSpec = WeaponSpec {
    range: 8.5,
    damage: 12.0,
    reload_secs: 1.2,
    projectile_speed: 60.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.25,
    versus_structure: 0.2,
    versus_aircraft: 0.2,
    report: Sound::RifleShot,
};

const LIGHT_MORTAR: WeaponSpec = WeaponSpec {
    range: 11.0,
    damage: 24.0,
    reload_secs: 2.8,
    projectile_speed: 14.0,
    splash_radius: 1.6,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 1.0,
    versus_vehicle: 0.3,
    versus_structure: 0.8,
    versus_aircraft: 0.0,
    report: Sound::GrenadeLaunch,
};

const CAR_MACHINE_GUN: WeaponSpec = WeaponSpec {
    range: 8.0,
    damage: 7.0,
    reload_secs: 0.35,
    projectile_speed: 60.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.2,
    versus_structure: 0.1,
    versus_aircraft: 0.3,
    report: Sound::MachineGunShot,
};

const LANDSHIP_GUNS: WeaponSpec = WeaponSpec {
    range: 9.0,
    damage: 30.0,
    reload_secs: 1.6,
    projectile_speed: 35.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.8,
    versus_vehicle: 1.0,
    versus_structure: 0.9,
    versus_aircraft: 0.0,
    report: Sound::CannonShot,
};

const SIEGE_SHELL: WeaponSpec = WeaponSpec {
    range: 19.0,
    damage: 85.0,
    reload_secs: 4.5,
    projectile_speed: 18.0,
    splash_radius: 2.0,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 0.5,
    versus_vehicle: 0.7,
    versus_structure: 1.3,
    versus_aircraft: 0.0,
    report: Sound::HowitzerShot,
};

const BIPLANE_GUNS: WeaponSpec = WeaponSpec {
    range: 8.0,
    damage: 12.0,
    reload_secs: 0.4,
    projectile_speed: 60.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 0.0,
    versus_vehicle: 0.0,
    versus_structure: 0.0,
    versus_aircraft: 1.0,
    report: Sound::MachineGunShot,
};

const BIPLANE_BOMBS: WeaponSpec = WeaponSpec {
    range: 3.0,
    damage: 110.0,
    reload_secs: 16.0,
    projectile_speed: 14.0,
    splash_radius: 2.8,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.8,
    versus_vehicle: 1.0,
    versus_structure: 1.4,
    versus_aircraft: 0.0,
    report: Sound::CannonShot,
};

const PILLBOX_GUN: WeaponSpec = WeaponSpec {
    range: 10.0,
    damage: 9.0,
    reload_secs: 0.3,
    projectile_speed: 60.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.25,
    versus_structure: 0.1,
    versus_aircraft: 0.0,
    report: Sound::MachineGunShot,
};

const FUSILIER: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Infantry,
    name: "Fusilier",
    role: "Infantry with a bolt-action rifle. Cheap and steady.",
    model_id: "fusilier",
    cost: 110,
    build_secs: 5.0,
    max_health: 75.0,
    speed: 2.5,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(BOLT_RIFLE),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const MORTAR_TEAM: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Ranged,
    name: "Mortar Team",
    role: "Lobs mortar bombs over cover. Breaks massed infantry, weak against cars.",
    model_id: "mortar_team",
    cost: 220,
    build_secs: 7.0,
    max_health: 60.0,
    speed: 2.3,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(LIGHT_MORTAR),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 13.0,
};

const ORDERLY: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Healer,
    name: "Orderly",
    role: "Unarmed. Treats wounded infantry nearby and follows the squad.",
    model_id: "orderly",
    cost: 180,
    build_secs: 6.0,
    max_health: 55.0,
    speed: 2.5,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 9.0,
    sight: 12.0,
};

const PIONEER: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Builder,
    name: "Pioneer",
    role: "Unarmed. Right-click an enemy structure to take it over, or a damaged own one to mend it.",
    model_id: "pioneer",
    cost: 480,
    build_secs: 9.0,
    max_health: 55.0,
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

const ARMOURED_CAR: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Fast,
    name: "Armoured Car",
    role: "A fast car with a machine gun turret. Hunts infantry and mortars.",
    model_id: "armoured_car",
    cost: 500,
    build_secs: 9.0,
    max_health: 260.0,
    speed: 6.0,
    turn_rate: 5.0,
    radius: 1.0,
    armor: ArmorClass::Vehicle,
    weapon: Some(CAR_MACHINE_GUN),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 18.0,
};

const LANDSHIP: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Main,
    name: "Landship",
    role: "A rhomboid tank with guns in its sponsons. Slow, crosses anything, hard to stop.",
    model_id: "landship",
    cost: 800,
    build_secs: 14.0,
    max_health: 620.0,
    speed: 2.3,
    turn_rate: 2.5,
    radius: 1.4,
    armor: ArmorClass::Vehicle,
    weapon: Some(LANDSHIP_GUNS),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const SIEGE_GUN: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Siege,
    name: "Siege Gun",
    role: "A heavy gun on steel wheels. Shells structures and troops from far behind the line.",
    model_id: "siege_gun",
    cost: 950,
    build_secs: 17.0,
    max_health: 260.0,
    speed: 2.2,
    turn_rate: 3.0,
    radius: 1.1,
    armor: ArmorClass::Vehicle,
    weapon: Some(SIEGE_SHELL),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const LORRY: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Transport,
    name: "Lorry",
    role: "A canvas-backed lorry for five infantry, who shoot from under its tilt. Unarmed itself.",
    model_id: "lorry",
    cost: 500,
    build_secs: 9.0,
    max_health: 340.0,
    speed: 5.0,
    turn_rate: 4.0,
    radius: 1.0,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const STEAM_HARVESTER: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Harvester,
    name: "Steam Harvester",
    role: "A traction engine that rakes up crystals and hauls them to the works. Unarmed.",
    model_id: "steam_harvester",
    cost: 1000,
    build_secs: 15.0,
    max_health: 650.0,
    speed: 2.4,
    turn_rate: 3.0,
    radius: 1.1,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 10.0,
};

const BIPLANE: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Fighter,
    name: "Biplane",
    role: "A fighter of wood and canvas. Only shoots aircraft; returns to its Helipad to rearm.",
    model_id: "biplane",
    cost: 700,
    build_secs: 12.0,
    max_health: 170.0,
    speed: 9.0,
    turn_rate: 4.0,
    radius: 1.2,
    armor: ArmorClass::Aircraft,
    weapon: Some(BIPLANE_GUNS),
    has_turret: false,
    producer: BuildingKind::Helipad,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 20.0,
};

const BIPLANE_BOMBER: UnitSpec = UnitSpec {
    era: Era::Industrial,
    line: UnitLine::Bomber,
    name: "Biplane Bomber",
    role: "Flies one bombing run against ground targets, then returns to its Helipad to rearm.",
    model_id: "biplane_bomber",
    cost: 1100,
    build_secs: 18.0,
    max_health: 230.0,
    speed: 7.5,
    turn_rate: 2.5,
    radius: 1.6,
    armor: ArmorClass::Aircraft,
    weapon: Some(BIPLANE_BOMBS),
    has_turret: false,
    producer: BuildingKind::Helipad,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 16.0,
};

const PILLBOX: BuildingSpec = BuildingSpec {
    era: Eras::Only(Era::Industrial),
    line: Some(DefenceLine::Tower),
    name: "Pillbox",
    role: "A concrete machine gun nest. Mows down infantry on the approaches.",
    model_id: "pillbox",
    footprint: IVec2::new(1, 1),
    cost: 700,
    build_secs: 10.0,
    max_health: 800.0,
    power: -10,
    prerequisites: &[BuildingKind::Barracks],
    weapon: Some(PILLBOX_GUN),
    sight: 16.0,
    faction: None,
};

const PULSE_RIFLE: WeaponSpec = WeaponSpec {
    range: 9.0,
    damage: 12.0,
    reload_secs: 0.9,
    projectile_speed: 70.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.2,
    versus_structure: 0.3,
    versus_aircraft: 0.3,
    report: Sound::PulseShot,
};

const LANCE: WeaponSpec = WeaponSpec {
    range: 10.0,
    damage: 50.0,
    reload_secs: 2.4,
    projectile_speed: 30.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Missile,
    versus_infantry: 0.5,
    versus_vehicle: 1.3,
    versus_structure: 0.6,
    versus_aircraft: 0.6,
    report: Sound::MissileLaunch,
};

const BIKE_BLASTER: WeaponSpec = WeaponSpec {
    range: 8.0,
    damage: 14.0,
    reload_secs: 0.4,
    projectile_speed: 70.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.3,
    versus_structure: 0.1,
    versus_aircraft: 0.2,
    report: Sound::PulseShot,
};

const HOVER_RAIL: WeaponSpec = WeaponSpec {
    range: 12.0,
    damage: 60.0,
    reload_secs: 1.8,
    projectile_speed: 60.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Shell,
    versus_infantry: 0.6,
    versus_vehicle: 1.0,
    versus_structure: 0.8,
    versus_aircraft: 0.0,
    report: Sound::HeavyCannonShot,
};

const RAIL_CANNON_SHOT: WeaponSpec = WeaponSpec {
    range: 22.0,
    damage: 95.0,
    reload_secs: 4.5,
    projectile_speed: 24.0,
    splash_radius: 2.2,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 0.5,
    versus_vehicle: 0.8,
    versus_structure: 1.3,
    versus_aircraft: 0.0,
    report: Sound::HowitzerShot,
};

const INTERCEPTOR_LASERS: WeaponSpec = WeaponSpec {
    range: 9.0,
    damage: 16.0,
    reload_secs: 0.5,
    projectile_speed: 70.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 0.0,
    versus_vehicle: 0.0,
    versus_structure: 0.0,
    versus_aircraft: 1.0,
    report: Sound::PulseShot,
};

const LASER_TURRET_BEAM: WeaponSpec = WeaponSpec {
    range: 13.0,
    damage: 30.0,
    reload_secs: 1.2,
    projectile_speed: 70.0,
    splash_radius: 0.0,
    style: ProjectileStyle::Bullet,
    versus_infantry: 1.0,
    versus_vehicle: 0.9,
    versus_structure: 0.3,
    versus_aircraft: 0.5,
    report: Sound::PulseShot,
};

const EXO_TROOPER: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Infantry,
    name: "Exo Trooper",
    role: "Infantry in a powered suit with a pulse rifle. Tough and quick.",
    model_id: "exo_trooper",
    cost: 150,
    build_secs: 5.0,
    max_health: 110.0,
    speed: 2.8,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(PULSE_RIFLE),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const LANCER: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Ranged,
    name: "Lancer",
    role: "Fires guided lances. Pierces hover armour and hits aircraft; weak against troopers.",
    model_id: "lancer",
    cost: 350,
    build_secs: 8.0,
    max_health: 90.0,
    speed: 2.6,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: Some(LANCE),
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const MEDTECH: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Healer,
    name: "Medtech",
    role: "Unarmed. Treats wounded infantry nearby and follows the squad.",
    model_id: "medtech",
    cost: 250,
    build_secs: 6.0,
    max_health: 70.0,
    speed: 2.8,
    turn_rate: 10.0,
    radius: 0.4,
    armor: ArmorClass::Infantry,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Barracks,
    requires: None,
    faction: None,
    heal_per_sec: 12.0,
    sight: 12.0,
};

const FABRICATOR: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Builder,
    name: "Fabricator",
    role: "Unarmed. Right-click an enemy structure to take it over, or a damaged own one to mend it.",
    model_id: "fabricator",
    cost: 600,
    build_secs: 9.0,
    max_health: 70.0,
    speed: 2.9,
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

const HOVER_BIKE: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Fast,
    name: "Hover Bike",
    role: "A fast skimmer with a blaster. Hunts infantry and lancers.",
    model_id: "hover_bike",
    cost: 450,
    build_secs: 8.0,
    max_health: 220.0,
    speed: 8.0,
    turn_rate: 6.0,
    radius: 0.8,
    armor: ArmorClass::Vehicle,
    weapon: Some(BIKE_BLASTER),
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 18.0,
};

const HOVER_TANK: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Main,
    name: "Hover Tank",
    role: "The backbone of a future army: a hovering hull with a rail gun.",
    model_id: "hover_tank",
    cost: 1000,
    build_secs: 15.0,
    max_health: 700.0,
    speed: 4.5,
    turn_rate: 3.5,
    radius: 1.3,
    armor: ArmorClass::Vehicle,
    weapon: Some(HOVER_RAIL),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 15.0,
};

const RAIL_CANNON: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Siege,
    name: "Rail Cannon",
    role: "Hurls slugs across the field. Breaks structures and massed troops from afar.",
    model_id: "rail_cannon",
    cost: 1300,
    build_secs: 18.0,
    max_health: 320.0,
    speed: 3.0,
    turn_rate: 3.0,
    radius: 1.2,
    armor: ArmorClass::Vehicle,
    weapon: Some(RAIL_CANNON_SHOT),
    has_turret: true,
    producer: BuildingKind::Factory,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 12.0,
};

const HOVER_CARRIER: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Transport,
    name: "Hover Carrier",
    role: "Carries five infantry, who shoot from its hatches. Unarmed itself.",
    model_id: "hover_carrier",
    cost: 700,
    build_secs: 10.0,
    max_health: 450.0,
    speed: 6.0,
    turn_rate: 4.0,
    radius: 1.2,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 14.0,
};

const HARVEST_DRONE: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Harvester,
    name: "Harvest Drone",
    role: "A hovering collector that gathers crystals and hauls them to the refinery. Unarmed.",
    model_id: "harvest_drone",
    cost: 1200,
    build_secs: 15.0,
    max_health: 700.0,
    speed: 3.0,
    turn_rate: 3.0,
    radius: 1.2,
    armor: ArmorClass::Vehicle,
    weapon: None,
    has_turret: false,
    producer: BuildingKind::Factory,
    requires: None,
    faction: None,
    heal_per_sec: 0.0,
    sight: 10.0,
};

const INTERCEPTOR: UnitSpec = UnitSpec {
    era: Era::Future,
    line: UnitLine::Fighter,
    name: "Interceptor",
    role: "A drone fighter. Only shoots aircraft; returns to its Helipad to recharge.",
    model_id: "interceptor",
    cost: 1100,
    build_secs: 14.0,
    max_health: 240.0,
    speed: 12.0,
    turn_rate: 4.5,
    radius: 1.2,
    armor: ArmorClass::Aircraft,
    weapon: Some(INTERCEPTOR_LASERS),
    has_turret: false,
    producer: BuildingKind::Helipad,
    requires: Some(BuildingKind::TechCenter),
    faction: None,
    heal_per_sec: 0.0,
    sight: 22.0,
};

const LASER_TURRET: BuildingSpec = BuildingSpec {
    era: Eras::Only(Era::Future),
    line: Some(DefenceLine::Tower),
    name: "Laser Turret",
    role: "A turret with a laser emitter. Burns infantry, hover craft and low aircraft alike.",
    model_id: "laser_turret",
    footprint: IVec2::new(1, 1),
    cost: 900,
    build_secs: 11.0,
    max_health: 900.0,
    power: -15,
    prerequisites: &[BuildingKind::Barracks],
    weapon: Some(LASER_TURRET_BEAM),
    sight: 17.0,
    faction: None,
};

/// What a world's final age races to finish and hold; see docs/eras.md.
const PROJECT: BuildingSpec = BuildingSpec {
    era: Eras::Only(Era::Future),
    line: None,
    name: "Project",
    role: "The work of the final age. Finish it and hold it to win the world.",
    model_id: "project",
    footprint: IVec2::new(3, 3),
    cost: 12_000,
    build_secs: 600.0,
    max_health: 20_000.0,
    power: -50,
    prerequisites: &[BuildingKind::TechCenter],
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
            Team::Azure,
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
