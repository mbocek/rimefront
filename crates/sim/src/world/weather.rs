use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::levels::{LevelDefinition, Weather};

/// How long one kind of sky takes to turn into the next.
const TRANSITION_SECS: f32 = 30.0;
const CHANCE_CLOUDS_BRING_RAIN: f64 = 0.6;
/// Far longer than any match, so the forecast never runs out.
const FORECAST_SECS: f32 = 6.0 * 3600.0;
/// Keeps the weather's random sequence apart from the map's, which also starts from the map seed.
const FORECAST_SALT: u64 = 0x0057_4541_5448_4552;
/// Share of their sight that units lose in a downpour or a snowfall.
const SIGHT_LOST_IN_RAIN: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sky {
    Clear,
    Overcast,
    Rain,
}

impl Sky {
    fn cloud_cover(self) -> f32 {
        match self {
            Sky::Clear => 0.0,
            Sky::Overcast => 0.7,
            Sky::Rain => 1.0,
        }
    }

    fn rainfall(self) -> f32 {
        if self == Sky::Rain { 1.0 } else { 0.0 }
    }

    fn lasts_secs(self, rng: &mut StdRng) -> f32 {
        match self {
            Sky::Clear => rng.random_range(150.0..330.0),
            Sky::Overcast => rng.random_range(60.0..150.0),
            Sky::Rain => rng.random_range(70.0..200.0),
        }
    }

    fn turns_into(self, rng: &mut StdRng) -> Sky {
        match self {
            Sky::Clear | Sky::Rain => Sky::Overcast,
            Sky::Overcast if rng.random_bool(CHANCE_CLOUDS_BRING_RAIN) => Sky::Rain,
            Sky::Overcast => Sky::Clear,
        }
    }
}

/// The weather of a match, from the map seed so every player of a network battle gets the same: it shortens
/// sight in the simulation and drives the sky.
#[derive(Resource, Clone, Debug)]
pub struct WeatherForecast {
    /// When each spell of weather starts, in seconds of match time.
    spells: Vec<(f32, Sky)>,
}

impl WeatherForecast {
    pub fn for_level(level: &LevelDefinition) -> Self {
        let first = match level.weather {
            Weather::Rain => Sky::Rain,
            Weather::Clear => Sky::Clear,
        };
        Self::forecast(level.map_seed, first)
    }

    fn forecast(map_seed: u64, first: Sky) -> Self {
        let mut rng = StdRng::seed_from_u64(map_seed ^ FORECAST_SALT);
        let mut spells = vec![(0.0, first)];
        let mut sky = first;
        let mut starts = sky.lasts_secs(&mut rng);
        while starts < FORECAST_SECS {
            sky = sky.turns_into(&mut rng);
            spells.push((starts, sky));
            starts += sky.lasts_secs(&mut rng);
        }
        Self { spells }
    }

    /// How far units see at `secs` into the match, as a share of their usual sight.
    pub fn sight_factor(&self, secs: f32) -> f32 {
        1.0 - SIGHT_LOST_IN_RAIN * self.sky_at(secs).1
    }

    /// Cloud cover and rainfall at `secs` into the match, blending into the next spell as it approaches.
    pub fn sky_at(&self, secs: f32) -> (f32, f32) {
        let current = self
            .spells
            .partition_point(|&(start, _)| start <= secs)
            .max(1)
            - 1;
        let (_, sky) = self.spells[current];
        let Some(&(next_starts, next)) = self.spells.get(current + 1) else {
            return (sky.cloud_cover(), sky.rainfall());
        };
        let turning = ((secs - (next_starts - TRANSITION_SECS)) / TRANSITION_SECS).clamp(0.0, 1.0);
        let blend = turning * turning * (3.0 - 2.0 * turning);
        (
            sky.cloud_cover().lerp(next.cloud_cover(), blend),
            sky.rainfall().lerp(next.rainfall(), blend),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rain_shortens_sight_by_a_quarter() {
        let rainy = WeatherForecast::forecast(3, Sky::Rain);
        assert_eq!(rainy.sight_factor(0.0), 0.75);
        let clear = WeatherForecast::forecast(3, Sky::Clear);
        assert_eq!(clear.sight_factor(0.0), 1.0);
    }

    #[test]
    fn every_player_of_a_map_gets_the_same_forecast() {
        let one = WeatherForecast::forecast(7, Sky::Clear);
        let other = WeatherForecast::forecast(7, Sky::Clear);
        assert_eq!(one.spells, other.spells);
    }

    #[test]
    fn the_sky_changes_during_a_match_and_starts_as_the_level_says() {
        let cycle = WeatherForecast::forecast(3, Sky::Rain);
        assert_eq!(cycle.sky_at(0.0), (1.0, 1.0));
        let skies: Vec<Sky> = cycle.spells.iter().take(6).map(|&(_, sky)| sky).collect();
        assert!(skies.contains(&Sky::Overcast));
        assert!(cycle.spells[1].0 < 600.0);
    }

    #[test]
    fn rain_fades_in_instead_of_switching_on() {
        let cycle = WeatherForecast::forecast(11, Sky::Overcast);
        let (turn, next) = cycle.spells[1];
        let (_, halfway) = cycle.sky_at(turn - TRANSITION_SECS * 0.5);
        if next == Sky::Rain {
            assert!(halfway > 0.0 && halfway < 1.0);
        } else {
            assert_eq!(halfway, 0.0);
        }
    }
}
