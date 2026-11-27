use bevy::prelude::*;

use crate::levels::ActiveLevel;
use crate::match_flow::MatchClock;
use crate::sim::catalog::UnitKind;
use crate::world::season::Season;
use crate::world::weather::WeatherForecast;

/// What carries a unit over the ground, which decides how much snow and mud slow it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunningGear {
    Feet,
    Wheels,
    Tracks,
    /// Aircraft, which the ground never slows.
    Wings,
}

impl RunningGear {
    pub fn of(kind: UnitKind) -> RunningGear {
        match kind {
            UnitKind::Rifleman
            | UnitKind::Rocketeer
            | UnitKind::Medic
            | UnitKind::Engineer
            | UnitKind::Sniper
            | UnitKind::FlameTrooper
            | UnitKind::Grenadier
            | UnitKind::Spearman
            | UnitKind::Archer
            | UnitKind::Healer
            | UnitKind::Builder
            | UnitKind::Horseman
            | UnitKind::Musketeer
            | UnitKind::Bombardier
            | UnitKind::Surgeon
            | UnitKind::Sapper
            | UnitKind::Dragoon
            | UnitKind::Cuirassier
            | UnitKind::Fusilier
            | UnitKind::MortarTeam
            | UnitKind::Orderly
            | UnitKind::Pioneer => RunningGear::Feet,
            UnitKind::Scout
            | UnitKind::MissileTruck
            | UnitKind::RepairTruck
            | UnitKind::RaiderBike
            | UnitKind::TroopCarrier
            | UnitKind::Chariot
            | UnitKind::Catapult
            | UnitKind::Wagon
            | UnitKind::OreCart
            | UnitKind::FieldGun
            | UnitKind::OreWagon
            | UnitKind::ArmouredCar
            | UnitKind::SiegeGun
            | UnitKind::Lorry
            | UnitKind::SteamHarvester => RunningGear::Wheels,
            UnitKind::Landship => RunningGear::Tracks,
            UnitKind::Tank
            | UnitKind::Bulwark
            | UnitKind::Artillery
            | UnitKind::Harvester
            | UnitKind::FlakTrack
            | UnitKind::StealthTank => RunningGear::Tracks,
            UnitKind::Gunship
            | UnitKind::Bomber
            | UnitKind::Fighter
            | UnitKind::Biplane
            | UnitKind::BiplaneBomber => RunningGear::Wings,
        }
    }

    fn in_snow(self) -> f32 {
        match self {
            RunningGear::Feet => 0.85,
            RunningGear::Wheels => 0.7,
            RunningGear::Tracks => 0.9,
            RunningGear::Wings => 1.0,
        }
    }

    fn in_mud(self) -> f32 {
        match self {
            RunningGear::Feet => 0.9,
            RunningGear::Wheels => 0.75,
            RunningGear::Tracks => 0.95,
            RunningGear::Wings => 1.0,
        }
    }
}

/// How the ground goes underfoot right now: snow all winter, mud while it rains in autumn or spring.
#[derive(Resource, Clone, Copy, PartialEq, Debug, Default)]
pub struct Ground {
    snow: bool,
    /// From 0, dry, to 1, a downpour's worth of mud.
    mud: f32,
}

impl Ground {
    pub fn of(season: Season, rainfall: f32) -> Ground {
        match season {
            Season::Winter => Ground {
                snow: true,
                mud: 0.0,
            },
            Season::Autumn | Season::Spring => Ground {
                snow: false,
                mud: rainfall,
            },
            Season::Summer => Ground::default(),
        }
    }

    /// The share of its usual speed a unit on `gear` keeps.
    pub fn speed_factor(self, gear: RunningGear) -> f32 {
        if self.snow {
            return gear.in_snow();
        }
        1.0 + (gear.in_mud() - 1.0) * self.mud
    }
}

pub fn update_ground(
    mut ground: ResMut<Ground>,
    level: Res<ActiveLevel>,
    clock: Option<Res<MatchClock>>,
    forecast: Option<Res<WeatherForecast>>,
) {
    let rainfall = match (clock, forecast) {
        (Some(clock), Some(forecast)) => forecast.sky_at(clock.elapsed_secs).1,
        _ => 0.0,
    };
    let now = Ground::of(level.0.season, rainfall);
    if *ground != now {
        *ground = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snow_slows_wheels_most_and_never_aircraft() {
        let snow = Ground::of(Season::Winter, 0.0);
        let wheels = snow.speed_factor(RunningGear::Wheels);
        assert!(wheels < snow.speed_factor(RunningGear::Feet));
        assert!(snow.speed_factor(RunningGear::Feet) < snow.speed_factor(RunningGear::Tracks));
        assert_eq!(snow.speed_factor(RunningGear::Wings), 1.0);
    }

    #[test]
    fn only_autumn_rain_makes_mud() {
        assert_eq!(
            Ground::of(Season::Summer, 1.0).speed_factor(RunningGear::Wheels),
            1.0
        );
        assert_eq!(
            Ground::of(Season::Autumn, 0.0).speed_factor(RunningGear::Wheels),
            1.0
        );
        let half = Ground::of(Season::Autumn, 0.5).speed_factor(RunningGear::Wheels);
        let full = Ground::of(Season::Autumn, 1.0).speed_factor(RunningGear::Wheels);
        assert!(full < half && half < 1.0);
    }
}
