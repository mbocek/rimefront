use bevy::prelude::*;

use crate::app::i18n::tr;
use crate::app::match_flow::MatchClock;
use crate::missions::levels::ActiveLevel;
use crate::sim::notifications::Notifications;
use crate::view::terrain_material::{TerrainMaterial, TerrainMaterialHandle};
use crate::world::season::Season;
use crate::world::weather::WeatherForecast;

const CLEAR_SUNLIGHT: f32 = 7_000.0;
const RAIN_SUNLIGHT: f32 = 3_200.0;
const CLEAR_AMBIENT: f32 = 280.0;
const RAIN_AMBIENT: f32 = 190.0;
/// The ground soaks within half a minute of rain and takes a couple of minutes to dry.
const SOAKING_PER_SEC: f32 = 1.0 / 25.0;
const DRYING_PER_SEC: f32 = 1.0 / 120.0;
/// Smaller changes are not worth uploading the ground material again.
const TERRAIN_UPDATE_STEP: f32 = 0.01;
/// Rainfall above this counts as raining for the player's notice.
pub const RAINING: f32 = 0.5;
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
