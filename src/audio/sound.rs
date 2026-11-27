use std::collections::HashMap;

use bevy::audio::{AudioSink, AudioSinkPlayback, PlaybackMode, Volume};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use rand::RngExt;

use crate::app::match_flow::GameState;
use crate::app::settings::GameSettings;
use crate::audio::hearing::{EAR_GAP, Heard, ear_compensation, hear};
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::outcome::GameOutcome;
use crate::view::camera::RtsCamera;
use crate::view::local_view::LocalView;

/// File variants; one is picked at random each time so repeats do not sound mechanical.
fn files(sound: Sound) -> &'static [&'static str] {
    match sound {
        Sound::RifleShot => &[
            "rifle_shot_1",
            "rifle_shot_2",
            "rifle_shot_3",
            "rifle_shot_4",
        ],
        Sound::MachineGunShot => &["mg_shot_1", "mg_shot_2"],
        Sound::CannonShot => &["cannon_shot_1", "cannon_shot_2"],
        Sound::HeavyCannonShot => &["heavy_cannon_shot"],
        Sound::HowitzerShot => &["howitzer_shot"],
        Sound::MissileLaunch => &["missile_launch"],
        Sound::BulletImpact => &["impact_small_1", "impact_small_2"],
        Sound::ShellExplosion => &["explosion_1", "explosion_2", "explosion_3"],
        Sound::VehicleExplosion => &["explosion_large"],
        Sound::BuildingCollapse => &["building_collapse"],
        Sound::UiClick => &["ui_click"],
        Sound::UiError => &["ui_error"],
        Sound::ConstructionComplete | Sound::ResearchComplete => &["construction_complete"],
        Sound::UnitReady => &["unit_ready"],
        Sound::BuildingPlaced => &["building_placed"],
        Sound::BaseAlert | Sound::HarvesterAlert | Sound::StructureLost => &["base_alert"],
        Sound::Credits => &["credits"],
        Sound::UnitAcknowledge => &["unit_acknowledge"],
        Sound::RepairStarted => &["repair_started"],
        Sound::StructureSold => &["structure_sold"],
        Sound::StructureCaptured => &["structure_captured"],
        Sound::UnitPromoted => &["unit_promoted"],
        Sound::FlakShot => &["flak_shot"],
        Sound::SniperShot => &["sniper_shot"],
        Sound::FlameBurst => &["flame_burst"],
        Sound::GrenadeLaunch => &["grenade_launch_1", "grenade_launch_2"],
        Sound::BowShot => &["bow_shot_1", "bow_shot_2"],
        Sound::CatapultThrow => &["catapult_throw"],
        Sound::BladeStrike => &["blade_strike_1", "blade_strike_2"],
        Sound::MusketShot => &["musket_shot_1", "musket_shot_2"],
        Sound::PulseShot => &["pulse_shot_1", "pulse_shot_2"],
    }
}

/// The same sound heard from across the battlefield: muffled, mostly echo. Empty when it does not carry.
fn distant_files(sound: Sound) -> &'static [&'static str] {
    match sound {
        Sound::RifleShot => &["rifle_shot_1_far", "rifle_shot_2_far"],
        Sound::MachineGunShot => &["mg_shot_1_far"],
        Sound::CannonShot => &["cannon_shot_1_far", "cannon_shot_2_far"],
        Sound::HeavyCannonShot => &["heavy_cannon_shot_far"],
        Sound::HowitzerShot => &["howitzer_shot_far"],
        Sound::MissileLaunch => &["missile_launch_far"],
        Sound::ShellExplosion => &["explosion_1_far", "explosion_2_far", "explosion_3_far"],
        Sound::VehicleExplosion => &["explosion_large_far"],
        Sound::BuildingCollapse => &["building_collapse_far"],
        Sound::FlakShot => &["flak_shot_far"],
        Sound::SniperShot => &["sniper_shot_far"],
        Sound::FlameBurst => &["flame_burst_far"],
        Sound::GrenadeLaunch => &["grenade_launch_1_far"],
        Sound::CatapultThrow => &["catapult_throw_far"],
        Sound::MusketShot => &["musket_shot_1_far"],
        Sound::PulseShot => &["pulse_shot_1_far"],
        _ => &[],
    }
}

/// How many screen widths beyond the edge of the view the sound is still heard.
fn carry(sound: Sound) -> f32 {
    match sound {
        Sound::BulletImpact | Sound::BladeStrike => 0.15,
        Sound::BowShot => 0.4,
        Sound::CatapultThrow => 1.2,
        Sound::FlameBurst | Sound::GrenadeLaunch => 0.4,
        Sound::RifleShot | Sound::MachineGunShot | Sound::MusketShot | Sound::PulseShot => 0.6,
        Sound::FlakShot | Sound::MissileLaunch | Sound::SniperShot => 1.0,
        Sound::CannonShot => 1.2,
        Sound::HeavyCannonShot | Sound::ShellExplosion => 1.6,
        Sound::HowitzerShot | Sound::VehicleExplosion => 2.2,
        Sound::BuildingCollapse => 2.8,
        _ => 0.5,
    }
}

/// How much one such sound on screen stirs the battle music; zero for anything but fighting.
fn fury_raised_by(sound: Sound) -> f32 {
    match sound {
        Sound::RifleShot
        | Sound::MachineGunShot
        | Sound::FlameBurst
        | Sound::GrenadeLaunch
        | Sound::BowShot
        | Sound::BladeStrike
        | Sound::MusketShot
        | Sound::PulseShot => 0.04,
        Sound::CatapultThrow => 0.1,
        Sound::SniperShot | Sound::FlakShot | Sound::MissileLaunch => 0.07,
        Sound::CannonShot | Sound::HeavyCannonShot | Sound::HowitzerShot => 0.1,
        Sound::ShellExplosion => 0.08,
        Sound::VehicleExplosion | Sound::BuildingCollapse => 0.25,
        _ => 0.0,
    }
}

/// How many copies may start within one throttle window, so a squad firing together stays readable.
fn burst_limit(sound: Sound) -> usize {
    match sound {
        Sound::RifleShot
        | Sound::MachineGunShot
        | Sound::BulletImpact
        | Sound::BowShot
        | Sound::BladeStrike
        | Sound::MusketShot
        | Sound::PulseShot => 3,
        Sound::CannonShot
        | Sound::HeavyCannonShot
        | Sound::ShellExplosion
        | Sound::MissileLaunch
        | Sound::FlakShot
        | Sound::FlameBurst
        | Sound::GrenadeLaunch => 2,
        _ => 1,
    }
}

fn base_volume(sound: Sound) -> f32 {
    match sound {
        Sound::RifleShot
        | Sound::MachineGunShot
        | Sound::BowShot
        | Sound::BladeStrike
        | Sound::MusketShot
        | Sound::PulseShot => 0.7,
        Sound::CannonShot | Sound::HeavyCannonShot | Sound::HowitzerShot => 0.5,
        Sound::BulletImpact => 0.35,
        Sound::UiClick | Sound::UnitAcknowledge => 0.5,
        _ => 0.8,
    }
}

#[derive(Resource)]
pub struct SoundLibrary {
    effects: HashMap<Sound, Vec<Handle<AudioSource>>>,
    distant_effects: HashMap<Sound, Vec<Handle<AudioSource>>>,
    menu_theme: Handle<AudioSource>,
    /// Each battle piece as its full mix and its quiet layer, the same length so they loop together.
    battle_themes: Vec<(Handle<AudioSource>, Handle<AudioSource>)>,
    victory: Handle<AudioSource>,
    defeat: Handle<AudioSource>,
}

pub fn load_sounds(mut commands: Commands, asset_server: Res<AssetServer>) {
    let music = |name: &str| asset_server.load(format!("audio/music/{name}.ogg"));
    let load_all = |files: fn(Sound) -> &'static [&'static str]| {
        Sound::ALL
            .into_iter()
            .map(|sound| {
                let variants = files(sound)
                    .iter()
                    .map(|file| asset_server.load(format!("audio/sfx/{file}.ogg")))
                    .collect();
                (sound, variants)
            })
            .collect()
    };
    commands.insert_resource(SoundLibrary {
        effects: load_all(files),
        distant_effects: load_all(distant_files),
        menu_theme: music("menu_theme"),
        battle_themes: vec![
            (music("battle_1"), music("battle_1_calm")),
            (music("battle_2"), music("battle_2_calm")),
        ],
        victory: music("victory"),
        defeat: music("defeat"),
    });
}

const THROTTLE_WINDOW_SECS: f32 = 0.12;
/// The same recording started again this soon blends with itself into a hollow, metallic flanger.
const SELF_BLEND_SECS: f32 = 0.05;
const PITCH_VARIATION: f32 = 0.06;
/// Where the two ears that turn a pan into stereo sit, far below the battlefield.
pub const EARS_AT: Vec3 = Vec3::new(0.0, -1000.0, 0.0);
/// Seconds for the battle music's fury to fall halfway back once the fighting on screen stops.
const FURY_HALF_LIFE_SECS: f32 = 6.0;

#[derive(Default)]
pub struct Throttle {
    recent: HashMap<Sound, (f32, usize)>,
    /// Which recording of each sound started last, and when.
    last_variant: HashMap<Sound, (usize, f32)>,
}

impl Throttle {
    /// A recording other than the one just started, or `None` when the only one would blend with itself.
    fn pick_variant(&mut self, sound: Sound, count: usize, now: f32, roll: usize) -> Option<usize> {
        let mut variant = roll % count;
        if let Some(&(last, started)) = self.last_variant.get(&sound)
            && last % count == variant
            && now - started < SELF_BLEND_SECS
        {
            if count == 1 {
                return None;
            }
            variant = (variant + 1) % count;
        }
        self.last_variant.insert(sound, (variant, now));
        Some(variant)
    }
}

/// Sounds from afar, waiting until they would have travelled to the player.
#[derive(Default)]
pub struct InFlight(Vec<(f32, Sound, Heard)>);

/// How fierce the fighting on screen is, from 0 (quiet) to 1; it chooses between the calm and full music.
#[derive(Resource, Default)]
pub struct BattleFury(pub f32);

/// The listener for every battle sound; sounds are placed around it by their pan alone.
pub fn place_ears(mut commands: Commands) {
    commands.spawn((
        SpatialListener::new(EAR_GAP),
        Transform::from_translation(EARS_AT),
        crate::app::match_flow::Persistent,
    ));
}

pub fn play_sound_cues(
    mut commands: Commands,
    mut cues: MessageReader<SoundCue>,
    mut throttle: Local<Throttle>,
    mut in_flight: Local<InFlight>,
    mut fury: ResMut<BattleFury>,
    time: Res<Time<Real>>,
    library: Option<Res<SoundLibrary>>,
    settings: Res<GameSettings>,
    view: Option<LocalView>,
    cameras: Query<(&Camera, &GlobalTransform, &RtsCamera)>,
) {
    let Some(library) = library else {
        cues.clear();
        return;
    };
    let now = time.elapsed_secs();
    fury.0 *= 0.5f32.powf(time.delta_secs() / FURY_HALF_LIFE_SECS);
    let camera = cameras.single().ok();
    let mut due: Vec<(Sound, Heard)> = Vec::new();
    for cue in cues.read() {
        let heard = match cue.position {
            None => Some(Heard {
                gain: 1.0,
                pan: 0.0,
                distant: false,
                delay_secs: 0.0,
            }),
            Some(position) => {
                let hidden = view
                    .as_ref()
                    .is_some_and(|view| !view.sight().is_visible(position));
                camera
                    .filter(|_| !hidden)
                    .and_then(|(camera, transform, rts)| {
                        let screen = camera
                            .world_to_ndc(transform, position)
                            .filter(|ndc| ndc.z > 0.0 && ndc.z < 1.0)
                            .map(|ndc| ndc.truncate());
                        let metres = position.xz().distance(rts.focus.xz());
                        hear(screen, rts.zoom_fraction(), carry(cue.sound), metres)
                    })
            }
        };
        let Some(heard) = heard else { continue };
        if !heard.distant {
            fury.0 = (fury.0 + fury_raised_by(cue.sound) * heard.gain).min(1.0);
        }
        if heard.delay_secs > 0.0 {
            in_flight.0.push((now + heard.delay_secs, cue.sound, heard));
        } else {
            due.push((cue.sound, heard));
        }
    }
    in_flight.0.retain(|&(arrives, sound, heard)| {
        let arrived = arrives <= now;
        if arrived {
            due.push((sound, heard));
        }
        !arrived
    });

    let mut rng = rand::rng();
    for (sound, heard) in due {
        let (window_start, played) = throttle.recent.entry(sound).or_insert((now, 0));
        if now - *window_start > THROTTLE_WINDOW_SECS {
            *window_start = now;
            *played = 0;
        }
        if *played >= burst_limit(sound) {
            continue;
        }
        let distant = library
            .distant_effects
            .get(&sound)
            .filter(|variants| heard.distant && !variants.is_empty());
        let Some(variants) =
            distant.or_else(|| library.effects.get(&sound).filter(|v| !v.is_empty()))
        else {
            continue;
        };
        let roll = rng.random_range(0..variants.len());
        let Some(variant) = throttle.pick_variant(sound, variants.len(), now, roll) else {
            continue;
        };
        if let Some((_, played)) = throttle.recent.get_mut(&sound) {
            *played += 1;
        }
        let source = variants[variant].clone();
        let volume =
            base_volume(sound) * heard.gain * settings.effects_volume * ear_compensation(heard.pan);
        let place = EARS_AT + Vec3::new(heard.pan, 0.0, 1.0);
        commands.spawn((
            AudioPlayer(source),
            PlaybackSettings::DESPAWN
                .with_spatial(true)
                .with_volume(Volume::Linear(volume))
                .with_speed(1.0 + rng.random_range(-PITCH_VARIATION..PITCH_VARIATION)),
            Transform::from_translation(place),
            GlobalTransform::from_translation(place),
        ));
    }
}

/// Unattended capture runs stay silent so they never play sound on the developer's desk.
pub fn apply_master_volume(settings: Res<GameSettings>, mut global: ResMut<GlobalVolume>) {
    if settings.is_changed() {
        global.volume = master_volume(&settings);
    }
}

/// Capture runs stay silent.
pub fn master_volume(settings: &GameSettings) -> Volume {
    if crate::app::capture::is_capture_run() {
        Volume::SILENT
    } else {
        Volume::Linear(settings.master_volume)
    }
}

/// Clicking any button of the interface gives a short tick.
pub fn click_feedback(
    click: On<PointerClick>,
    buttons: Query<(), With<Hovered>>,
    mut cues: MessageWriter<SoundCue>,
) {
    if buttons.contains(click.entity) && click.button == PointerButton::Primary {
        cues.write(SoundCue::interface(Sound::UiClick));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MusicPiece {
    Menu,
    Battle(usize),
    Victory,
    Defeat,
}

/// One playing piece of music; a battle plays its full mix and its quiet layer together.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum MusicTrack {
    Whole,
    BattleFull,
    BattleCalm,
}

#[derive(Resource, Default)]
pub struct MusicDirector {
    matches_started: usize,
    /// The piece last started; a sting keeps this value after it finishes so it is not replayed.
    current: Option<MusicPiece>,
    /// How far the battle music has crossed from the calm layer to the full mix, 0 to 1.
    blend: f32,
}

const MUSIC_LEVEL: f32 = 0.6;
/// The full mix never fades out entirely, so the battle keeps its pulse when things go quiet.
const QUIET_FULL_MIX: f32 = 0.15;
/// How fast the music follows the fighting, in blend per second; slower down than up.
const BLEND_RISE_PER_SEC: f32 = 0.5;
const BLEND_FALL_PER_SEC: f32 = 0.12;

fn spawn_track(
    commands: &mut Commands,
    source: Handle<AudioSource>,
    mode: PlaybackMode,
    track: MusicTrack,
    volume: f32,
) {
    commands.spawn((
        track,
        AudioPlayer(source),
        PlaybackSettings {
            mode,
            volume: Volume::Linear(volume),
            ..default()
        },
    ));
}

fn spawn_music(commands: &mut Commands, library: &SoundLibrary, piece: MusicPiece, volume: f32) {
    let level = volume * MUSIC_LEVEL;
    match piece {
        MusicPiece::Menu => spawn_track(
            commands,
            library.menu_theme.clone(),
            PlaybackMode::Loop,
            MusicTrack::Whole,
            level,
        ),
        MusicPiece::Battle(index) => {
            let (full, calm) = library.battle_themes[index % library.battle_themes.len()].clone();
            spawn_track(
                commands,
                full,
                PlaybackMode::Loop,
                MusicTrack::BattleFull,
                level * QUIET_FULL_MIX,
            );
            spawn_track(
                commands,
                calm,
                PlaybackMode::Loop,
                MusicTrack::BattleCalm,
                level,
            );
        }
        MusicPiece::Victory => spawn_track(
            commands,
            library.victory.clone(),
            PlaybackMode::Despawn,
            MusicTrack::Whole,
            level,
        ),
        MusicPiece::Defeat => spawn_track(
            commands,
            library.defeat.clone(),
            PlaybackMode::Despawn,
            MusicTrack::Whole,
            level,
        ),
    }
}

/// The two layers of a battle piece, mixed by `blend`: calm alone at 0, the full mix at 1.
fn layer_volume(track: MusicTrack, blend: f32) -> f32 {
    match track {
        MusicTrack::Whole => 1.0,
        MusicTrack::BattleFull => QUIET_FULL_MIX + (1.0 - QUIET_FULL_MIX) * blend,
        MusicTrack::BattleCalm => 1.0 - blend,
    }
}

/// Picks the music for the current screen: the menu theme, alternating battle themes, or the end-of-match sting.
pub fn direct_music(
    mut commands: Commands,
    state: Res<State<GameState>>,
    outcome: Option<Res<GameOutcome>>,
    library: Option<Res<SoundLibrary>>,
    settings: Res<GameSettings>,
    mut director: ResMut<MusicDirector>,
    fury: Res<BattleFury>,
    time: Res<Time<Real>>,
    tracks: Query<Entity, With<MusicTrack>>,
    mut sinks: Query<(&MusicTrack, &mut AudioSink)>,
) {
    let Some(library) = library else { return };
    let outcome = outcome.map_or(GameOutcome::Ongoing, |outcome| *outcome);
    let wanted = match (state.get(), outcome) {
        (GameState::MainMenu | GameState::MapEditor, _) => Some(MusicPiece::Menu),
        (GameState::Playing, GameOutcome::Victory) => Some(MusicPiece::Victory),
        (GameState::Playing, GameOutcome::Defeat) => Some(MusicPiece::Defeat),
        (GameState::Playing, GameOutcome::Ongoing) | (GameState::Replay, _) => {
            Some(MusicPiece::Battle(director.matches_started))
        }
        (GameState::Booting | GameState::Restarting, _) => None,
    };
    if wanted != director.current {
        for track in &tracks {
            commands.entity(track).despawn();
        }
        if let Some(piece) = wanted {
            spawn_music(&mut commands, &library, piece, settings.music_volume);
        }
        director.current = wanted;
        director.blend = 0.0;
    }
    let target = fury.0;
    let step = time.delta_secs()
        * if target > director.blend {
            BLEND_RISE_PER_SEC
        } else {
            BLEND_FALL_PER_SEC
        };
    director.blend += (target - director.blend).clamp(-step, step);
    for (&track, mut sink) in &mut sinks {
        let volume = settings.music_volume * MUSIC_LEVEL * layer_volume(track, director.blend);
        sink.set_volume(Volume::Linear(volume));
    }
}

pub fn count_match_start(mut director: ResMut<MusicDirector>) {
    director.matches_started += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sound_started_twice_at_once_never_blends_with_its_own_recording() {
        let mut throttle = Throttle::default();
        let first = throttle.pick_variant(Sound::ShellExplosion, 3, 0.0, 1);
        let second = throttle.pick_variant(Sound::ShellExplosion, 3, 0.01, 1);
        assert_ne!(first, second);
        assert_eq!(throttle.pick_variant(Sound::FlakShot, 1, 0.0, 0), Some(0));
        assert_eq!(throttle.pick_variant(Sound::FlakShot, 1, 0.01, 0), None);
        assert_eq!(throttle.pick_variant(Sound::FlakShot, 1, 0.2, 0), Some(0));
    }

    #[test]
    fn every_sound_has_its_files() {
        let sfx = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/audio/sfx");
        for sound in Sound::ALL {
            for file in files(sound).iter().chain(distant_files(sound)) {
                let path = sfx.join(format!("{file}.ogg"));
                assert!(path.is_file(), "{sound:?} is missing {}", path.display());
            }
        }
        let music = sfx.with_file_name("music");
        for piece in ["battle_1", "battle_1_calm", "battle_2", "battle_2_calm"] {
            assert!(
                music.join(format!("{piece}.ogg")).is_file(),
                "missing {piece}"
            );
        }
    }

    #[test]
    fn the_battle_music_crosses_from_the_calm_layer_to_the_full_mix() {
        assert_eq!(layer_volume(MusicTrack::BattleCalm, 0.0), 1.0);
        assert_eq!(layer_volume(MusicTrack::BattleFull, 0.0), QUIET_FULL_MIX);
        assert_eq!(layer_volume(MusicTrack::BattleCalm, 1.0), 0.0);
        assert_eq!(layer_volume(MusicTrack::BattleFull, 1.0), 1.0);
    }
}
