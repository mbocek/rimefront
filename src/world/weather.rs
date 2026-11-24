use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::app::i18n::tr;
use crate::app::match_flow::MatchClock;
use crate::missions::levels::{ActiveLevel, LevelDefinition, Weather};
use crate::sim::notifications::Notifications;
use crate::view::terrain_material::{TerrainMaterial, TerrainMaterialHandle};
use crate::world::season::Season;

const CLEAR_SUNLIGHT: f32 = 7_000.0;
const RAIN_SUNLIGHT: f32 = 3_200.0;
const CLEAR_AMBIENT: f32 = 280.0;
const RAIN_AMBIENT: f32 = 190.0;
/// How long one kind of sky takes to turn into the next.
const TRANSITION_SECS: f32 = 30.0;
const CHANCE_CLOUDS_BRING_RAIN: f64 = 0.6;
/// Far longer than any match, so the forecast never runs out.
const FORECAST_SECS: f32 = 6.0 * 3600.0;
/// The ground soaks within half a minute of rain and takes a couple of minutes to dry.
const SOAKING_PER_SEC: f32 = 1.0 / 25.0;
const DRYING_PER_SEC: f32 = 1.0 / 120.0;
/// Keeps the weather's random sequence apart from the map's, which also starts from the map seed.
const FORECAST_SALT: u64 = 0x0057_4541_5448_4552;
/// Smaller changes are not worth uploading the ground material again.
const TERRAIN_UPDATE_STEP: f32 = 0.01;
/// Share of their sight that units lose in a downpour or a snowfall.
const SIGHT_LOST_IN_RAIN: f32 = 0.25;
/// Rainfall above this counts as raining for the player's notice.
pub const RAINING: f32 = 0.5;

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

/// The sky as shown now; the ground's wetness lags behind the rain.
#[derive(Resource)]
pub struct WeatherCycle {
    pub now: SkyNow,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct SkyNow {
    pub cloud_cover: f32,
    pub rainfall: f32,
    pub wet_ground: f32,
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
    fn sky_at(&self, secs: f32) -> (f32, f32) {
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

pub fn begin_weather(
    mut commands: Commands,
    level: Res<ActiveLevel>,
    clock: Option<Res<MatchClock>>,
) {
    let secs = clock.map_or(0.0, |clock| clock.elapsed_secs);
    let (cloud_cover, rainfall) = WeatherForecast::for_level(&level.0).sky_at(secs);
    commands.insert_resource(WeatherCycle {
        now: SkyNow {
            cloud_cover,
            rainfall,
            wet_ground: rainfall,
        },
    });
}

/// Tells the player when rain or snow starts shortening sight, and when it stops.
pub fn announce_weather(
    level: Res<ActiveLevel>,
    cycle: Option<Res<WeatherCycle>>,
    mut raining: Local<Option<bool>>,
    mut notifications: ResMut<Notifications>,
) {
    let Some(cycle) = cycle else { return };
    let now_raining = cycle.now.rainfall > RAINING;
    let was_raining = raining.replace(now_raining);
    if was_raining.is_none_or(|was| was == now_raining) {
        return;
    }
    let winter = level.0.season == Season::Winter;
    notifications.push(match (now_raining, winter) {
        (true, false) => tr("Rain closes in: units see less far"),
        (true, true) => tr("Snow is falling: units see less far"),
        (false, _) => tr("The weather clears"),
    });
}

/// Moves the sky along its forecast: clouds dim the sun and the sky light, rain soaks the ground.
pub fn follow_the_weather(
    time: Res<Time<Virtual>>,
    clock: Res<MatchClock>,
    forecast: Option<Res<WeatherForecast>>,
    level: Option<Res<ActiveLevel>>,
    cycle: Option<ResMut<WeatherCycle>>,
    terrain: Option<Res<TerrainMaterialHandle>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    mut suns: Query<&mut DirectionalLight>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    let (Some(level), Some(mut cycle), Some(forecast)) = (level, cycle, forecast) else {
        return;
    };
    let (cloud_cover, rainfall) = forecast.sky_at(clock.elapsed_secs);
    let rate = if rainfall > cycle.now.wet_ground {
        SOAKING_PER_SEC
    } else {
        DRYING_PER_SEC
    };
    let wet_ground = cycle.now.wet_ground
        + (rainfall - cycle.now.wet_ground)
            .clamp(-rate * time.delta_secs(), rate * time.delta_secs());
    cycle.now = SkyNow {
        cloud_cover,
        rainfall,
        wet_ground,
    };

    let season = level.0.season;
    let illuminance = CLEAR_SUNLIGHT.lerp(RAIN_SUNLIGHT, cloud_cover);
    for mut sun in &mut suns {
        if sun.illuminance != illuminance || sun.color != season.sunlight_color() {
            sun.illuminance = illuminance;
            sun.color = season.sunlight_color();
        }
    }
    let brightness = CLEAR_AMBIENT.lerp(RAIN_AMBIENT, cloud_cover) * season.ambient_factor();
    if ambient.brightness != brightness {
        ambient.brightness = brightness;
    }

    let soaked = if season == Season::Winter {
        0.0
    } else {
        wet_ground
    };
    let Some(material) = terrain
        .as_ref()
        .and_then(|terrain| terrain_materials.get(&terrain.0))
    else {
        return;
    };
    let shown = material.extension.settings;
    let stale = (shown.wet_ground - soaked).abs() > TERRAIN_UPDATE_STEP
        || (shown.cloud_cover - cloud_cover).abs() > TERRAIN_UPDATE_STEP;
    if !stale {
        return;
    }
    if let Some(mut material) = terrain
        .as_ref()
        .and_then(|terrain| terrain_materials.get_mut(&terrain.0))
    {
        material.extension.settings.wet_ground = soaked;
        material.extension.settings.cloud_cover = cloud_cover;
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
