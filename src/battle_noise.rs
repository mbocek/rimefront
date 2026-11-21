use bevy::audio::{PlaybackMode, SpatialAudioSink, Volume};
use bevy::prelude::*;

use crate::camera::RtsCamera;
use crate::catalog::UnitKind;
use crate::components::{Building, HarvestState, Harvester, Mobile, Unit};
use crate::hearing::{ear_compensation, hear};
use crate::local_view::LocalView;
use crate::match_flow::Persistent;
use crate::settings::GameSettings;
use crate::sound::EARS_AT;
use crate::structures::Repairing;

/// The steady sounds of a battle. Each plays as one loop for every unit making it, louder the more of them are
/// on screen and placed where most of them are.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Noise {
    TrackedEngines,
    WheeledEngines,
    Rotors,
    JetEngines,
    Footsteps,
    Harvesting,
    Construction,
}

impl Noise {
    const ALL: [Noise; 7] = [
        Noise::TrackedEngines,
        Noise::WheeledEngines,
        Noise::Rotors,
        Noise::JetEngines,
        Noise::Footsteps,
        Noise::Harvesting,
        Noise::Construction,
    ];

    fn file(self) -> &'static str {
        match self {
            Noise::TrackedEngines => "loop_tracked_engines",
            Noise::WheeledEngines => "loop_wheeled_engines",
            Noise::Rotors => "loop_rotors",
            Noise::JetEngines => "loop_jet_engines",
            Noise::Footsteps => "loop_footsteps",
            Noise::Harvesting => "loop_harvesting",
            Noise::Construction => "loop_construction",
        }
    }

    /// The loops differ a lot in density; this evens them out against the gunfire.
    fn base_volume(self) -> f32 {
        match self {
            Noise::TrackedEngines => 0.4,
            Noise::WheeledEngines => 0.25,
            Noise::Rotors => 0.3,
            Noise::JetEngines => 0.3,
            Noise::Footsteps => 0.4,
            Noise::Harvesting => 0.2,
            Noise::Construction => 0.45,
        }
    }

    /// How many screen widths beyond the edge of the view it is still heard.
    fn carry(self) -> f32 {
        match self {
            Noise::Rotors | Noise::JetEngines => 0.5,
            Noise::TrackedEngines => 0.3,
            _ => 0.15,
        }
    }
}

/// What a unit adds to the battle's noise, and how loud: an engine idles quietly and roars when it drives.
pub fn noise_of_unit(kind: UnitKind, moving: bool, harvesting: bool) -> Option<(Noise, f32)> {
    let engine = |noise| Some((noise, if moving { 1.0 } else { IDLING_ENGINE }));
    match kind {
        _ if harvesting => Some((Noise::Harvesting, 1.0)),
        UnitKind::Gunship => Some((Noise::Rotors, if moving { 1.0 } else { HOVERING_ROTORS })),
        UnitKind::Bomber | UnitKind::Fighter => {
            Some((Noise::JetEngines, if moving { 1.0 } else { IDLING_ENGINE }))
        }
        UnitKind::Scout | UnitKind::MissileTruck | UnitKind::TroopCarrier => {
            engine(Noise::WheeledEngines)
        }
        _ if UnitKind::INFANTRY.contains(&kind) => moving.then_some((Noise::Footsteps, 1.0)),
        _ => engine(Noise::TrackedEngines),
    }
}

const IDLING_ENGINE: f32 = 0.3;
/// How long engineers are heard finishing a structure after it is placed.
const FINISHING_SECS: f32 = 3.0;
const HOVERING_ROTORS: f32 = 0.75;
/// A crowd sounds louder than one unit, but ten tanks are not ten times as loud as one.
const CROWD_CEILING: f32 = 1.6;
const RISE_PER_SEC: f32 = 4.0;
const FALL_PER_SEC: f32 = 1.5;
const PAN_FOLLOW_PER_SEC: f32 = 3.0;

/// How loud a crowd of sources sounds together and where it seems to be. Each source is `(gain, pan)`.
pub fn crowd(sources: &[(f32, f32)]) -> (f32, f32) {
    let total: f32 = sources.iter().map(|(gain, _)| gain).sum();
    if total <= 0.0 {
        return (0.0, 0.0);
    }
    let power: f32 = sources.iter().map(|(gain, _)| gain * gain).sum();
    let pan = sources.iter().map(|(gain, pan)| gain * pan).sum::<f32>() / total;
    (power.sqrt().min(CROWD_CEILING), pan)
}

#[derive(Component, Default)]
pub struct NoiseVoice {
    loudness: f32,
    pan: f32,
}

pub fn start_battle_noise(mut commands: Commands, asset_server: Res<AssetServer>) {
    for noise in Noise::ALL {
        commands.spawn((
            noise,
            NoiseVoice::default(),
            AudioPlayer::new(asset_server.load(format!("audio/sfx/{}.ogg", noise.file()))),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::SILENT,
                spatial: true,
                ..default()
            },
            Transform::from_translation(EARS_AT + Vec3::Z),
            Persistent,
        ));
    }
}

/// Structures placed moments ago, still being finished, with the time their work ends.
#[derive(Default)]
pub struct FreshStructures(Vec<(Entity, f32)>);

/// Listens to every unit and working structure on screen and turns each loop up or down to match.
pub fn mix_battle_noise(
    mut fresh: Local<FreshStructures>,
    time: Res<Time<Real>>,
    game_time: Res<Time<Virtual>>,
    settings: Res<GameSettings>,
    global: Res<GlobalVolume>,
    view: Option<LocalView>,
    cameras: Query<(&Camera, &GlobalTransform, &RtsCamera)>,
    units: Query<(&Unit, &Mobile, &Transform, Option<&Harvester>)>,
    structures: Query<(Entity, &Transform, Has<Repairing>), With<Building>>,
    placed: Query<Entity, Added<Building>>,
    mut voices: Query<
        (
            &Noise,
            &mut NoiseVoice,
            &mut Transform,
            &mut SpatialAudioSink,
        ),
        (Without<Unit>, Without<Building>),
    >,
) {
    let now = time.elapsed_secs();
    fresh.0.retain(|&(_, ends)| ends > now);
    let whole_base_appeared = placed.iter().count() == structures.iter().count();
    if !whole_base_appeared {
        fresh
            .0
            .extend(placed.iter().map(|entity| (entity, now + FINISHING_SECS)));
    }
    let mut sources: Vec<(Noise, f32, f32)> = Vec::new();
    let camera = cameras.single().ok();
    if let (Some((camera, camera_transform, rts)), false) = (camera, game_time.is_paused()) {
        let mut listen = |noise: Noise, loudness: f32, position: Vec3| {
            if view
                .as_ref()
                .is_some_and(|view| !view.sight().is_visible(position))
            {
                return;
            }
            let screen = camera
                .world_to_ndc(camera_transform, position)
                .filter(|ndc| ndc.z > 0.0 && ndc.z < 1.0)
                .map(|ndc| ndc.truncate());
            let metres = position.xz().distance(rts.focus.xz());
            if let Some(heard) = hear(screen, rts.zoom_fraction(), noise.carry(), metres) {
                sources.push((noise, heard.gain * loudness, heard.pan));
            }
        };
        for (unit, mobile, transform, harvester) in &units {
            let harvesting =
                harvester.is_some_and(|h| matches!(h.state, HarvestState::Mining { .. }));
            if let Some((noise, loudness)) = noise_of_unit(unit.0, mobile.is_moving(), harvesting) {
                listen(noise, loudness, transform.translation);
            }
        }
        for (structure, transform, repairing) in &structures {
            let finishing = fresh.0.iter().any(|&(entity, _)| entity == structure);
            if repairing || finishing {
                listen(Noise::Construction, 1.0, transform.translation);
            }
        }
    }

    let dt = time.delta_secs();
    let master = global.volume.to_linear() * settings.effects_volume;
    for (noise, mut voice, mut transform, mut sink) in &mut voices {
        let heard: Vec<(f32, f32)> = sources
            .iter()
            .filter(|(source, ..)| source == noise)
            .map(|&(_, gain, pan)| (gain, pan))
            .collect();
        let (wanted, pan) = crowd(&heard);
        let rate = if wanted > voice.loudness {
            RISE_PER_SEC
        } else {
            FALL_PER_SEC
        };
        voice.loudness += (wanted - voice.loudness) * (rate * dt).min(1.0);
        if wanted > 0.0 {
            voice.pan += (pan - voice.pan) * (PAN_FOLLOW_PER_SEC * dt).min(1.0);
        }
        transform.translation = EARS_AT + Vec3::new(voice.pan, 0.0, 1.0);
        let volume = voice.loudness * noise.base_volume() * master * ear_compensation(voice.pan);
        sink.set_volume(Volume::Linear(volume));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crowd_is_louder_than_one_but_not_by_its_number() {
        let (one, _) = crowd(&[(1.0, 0.0)]);
        let (four, _) = crowd(&[(1.0, 0.0); 4]);
        let (twenty, _) = crowd(&[(1.0, 0.0); 20]);
        assert!(four > one);
        assert!(twenty <= CROWD_CEILING);
        assert_eq!(crowd(&[]), (0.0, 0.0));
    }

    #[test]
    fn a_crowd_is_heard_where_most_of_it_is() {
        let (_, pan) = crowd(&[(1.0, -0.8), (0.2, 0.8)]);
        assert!(pan < -0.4);
    }

    #[test]
    fn engines_roar_on_the_move_and_boots_fall_silent_at_rest() {
        assert_eq!(
            noise_of_unit(UnitKind::Tank, true, false),
            Some((Noise::TrackedEngines, 1.0))
        );
        assert_eq!(
            noise_of_unit(UnitKind::Tank, false, false),
            Some((Noise::TrackedEngines, IDLING_ENGINE))
        );
        assert_eq!(
            noise_of_unit(UnitKind::Scout, true, false).unwrap().0,
            Noise::WheeledEngines
        );
        assert_eq!(
            noise_of_unit(UnitKind::Gunship, false, false).unwrap().0,
            Noise::Rotors
        );
        assert_eq!(
            noise_of_unit(UnitKind::Rifleman, true, false),
            Some((Noise::Footsteps, 1.0))
        );
        assert_eq!(noise_of_unit(UnitKind::Rifleman, false, false), None);
        assert_eq!(
            noise_of_unit(UnitKind::Harvester, false, true).unwrap().0,
            Noise::Harvesting
        );
    }
}
