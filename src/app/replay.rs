use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::i18n::{tr, tr_fmt, tr_owned};
use crate::app::match_flow::{GameState, MatchClock};
use crate::app::settings::game_data_directory;
use crate::missions::levels::{ActiveLevel, LevelDefinition};
use crate::sim::catalog::{ArmorClass, BuildingKind, ProjectileStyle, Team, UnitKind};
use crate::sim::combat::{Impact, Projectile};
use crate::sim::components::WallSegment;
use crate::sim::components::{Aircraft, Building, Health, Mobile, Unit};
use crate::sim::cues::BlastSize;
use crate::sim::eras::Era;
use crate::sim::movement::FLIGHT_ALTITUDE;
use crate::sim::neutral_sites::UnclaimedSite;
use crate::sim::outcome::GameOutcome;
use crate::ui::sidebar::{ACCENT, TEXT_MAIN, UiFonts};
use crate::view::animation::{ReplayMotion, animated_unit_model};
use crate::view::assets::ModelLibrary;
use crate::view::effects::{EffectAssets, ShotFlight, spawn_blast};
use crate::world::map::{footprint_center, world_to_cell};

const REPLAY_VERSION: u32 = 1;
const REPLAY_DIRECTORY: &str = "replays";
const FRAME_INTERVAL_SECS: f32 = 0.5;
const KEPT_REPLAYS: usize = 10;
/// A match shorter than this is not worth keeping when it is abandoned.
const MIN_ABANDONED_FRAMES: usize = 20;
const SPEEDS: [f32; 4] = [1.0, 2.0, 4.0, 8.0];
const DECIMETRES_PER_METER: f32 = 10.0;
/// Moving less than this between two snapshots counts as standing still.
const ACTOR_MOVING_DISTANCE: f32 = 0.15;

/// Snapshots of every unit and structure twice a second, enough to watch a match again.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Replay {
    pub version: u32,
    pub level: LevelDefinition,
    pub outcome: String,
    pub recorded_secs: f32,
    pub frames: Vec<Frame>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Frame {
    pub time: f32,
    pub actors: Vec<Actor>,
    /// Shells, missiles and explosions since the previous frame; older replays have none.
    #[serde(default)]
    pub events: Vec<Event>,
}

/// Positions in decimetres, like actors. Rifle bullets are left out: they are many and barely visible.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Style, from, to, arc height and flight time in milliseconds.
    Shot(u8, [i16; 3], [i16; 3], i16, u16),
    /// Blast size and position.
    Blast(u8, [i16; 3]),
}

fn pack(position: Vec3) -> [i16; 3] {
    (position * DECIMETRES_PER_METER)
        .round()
        .to_array()
        .map(|value| value as i16)
}

fn unpack(packed: [i16; 3]) -> Vec3 {
    Vec3::from_array(packed.map(f32::from)) / DECIMETRES_PER_METER
}

const SHOT_STYLES: [ProjectileStyle; 8] = [
    ProjectileStyle::Bullet,
    ProjectileStyle::Shell,
    ProjectileStyle::ArtilleryShell,
    ProjectileStyle::Missile,
    ProjectileStyle::Flame,
    ProjectileStyle::Arrow,
    ProjectileStyle::Boulder,
    ProjectileStyle::Strike,
];
const BLAST_SIZES: [BlastSize; 3] = [
    BlastSize::ShellImpact,
    BlastSize::VehicleWreck,
    BlastSize::BuildingCollapse,
];
/// Scorch radii above these were left by a destroyed vehicle and a destroyed structure.
const VEHICLE_BLAST_RADIUS: f32 = 1.2;
const STRUCTURE_BLAST_RADIUS: f32 = 2.5;
/// A jump of the playback clock longer than this is a seek: events in between are not replayed.
const SEEK_THRESHOLD_SECS: f32 = 1.5;
const SEEK_STEP_SECS: f32 = 10.0;

/// Packed so a half-hour match stays a few megabytes: replay id, look, team, x and z in decimetres,
/// facing in 1/256 turns and health in percent.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Actor(u32, u8, u8, i16, i16, u8, u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Unit(UnitKind),
    Building(BuildingKind),
}

const BUILDING_LOOKS_START: u8 = 100;
/// Stands in an actor's team byte for a neutral site nobody holds.
const UNCLAIMED_CODE: u8 = u8::MAX;

/// Append only: a replay stores a structure's look as its place here.
const BUILDING_LOOKS: [BuildingKind; 17] = [
    BuildingKind::CommandCenter,
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
    BuildingKind::SupplyDepot,
    BuildingKind::ObservationPost,
    BuildingKind::FieldHospital,
    BuildingKind::Watchtower,
    BuildingKind::Palisade,
];

fn all_buildings() -> impl Iterator<Item = BuildingKind> {
    BUILDING_LOOKS.into_iter()
}

impl Look {
    fn code(self) -> u8 {
        match self {
            Look::Unit(kind) => UnitKind::ALL.iter().position(|&k| k == kind).unwrap_or(0) as u8,
            Look::Building(kind) => {
                BUILDING_LOOKS_START + all_buildings().position(|k| k == kind).unwrap_or(0) as u8
            }
        }
    }

    fn from_code(code: u8) -> Option<Look> {
        if code < BUILDING_LOOKS_START {
            UnitKind::ALL.get(code as usize).copied().map(Look::Unit)
        } else {
            all_buildings()
                .nth((code - BUILDING_LOOKS_START) as usize)
                .map(Look::Building)
        }
    }
}

impl Actor {
    pub fn new(id: u32, look: Look, team: Team, position: Vec3, facing: f32, health: f32) -> Self {
        let decimetres = |meters: f32| (meters * DECIMETRES_PER_METER).round() as i16;
        let turns =
            (facing.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 256.0) as u8;
        Actor(
            id,
            look.code(),
            team.index() as u8,
            decimetres(position.x),
            decimetres(position.z),
            turns,
            (health.clamp(0.0, 1.0) * 100.0).round() as u8,
        )
    }

    pub fn unclaimed_site(id: u32, kind: BuildingKind, position: Vec3, facing: f32) -> Self {
        let mut actor = Self::new(id, Look::Building(kind), Team::Azure, position, facing, 1.0);
        actor.2 = UNCLAIMED_CODE;
        actor
    }

    pub fn id(self) -> u32 {
        self.0
    }

    /// `None` for a neutral site nobody holds.
    pub fn holder(self) -> Option<Team> {
        (self.2 != UNCLAIMED_CODE).then(|| self.team())
    }

    pub fn look(self) -> Option<Look> {
        Look::from_code(self.1)
    }

    pub fn team(self) -> Team {
        Team::ALL
            .get(self.2 as usize)
            .copied()
            .unwrap_or(Team::Azure)
    }

    /// Ground position; the height comes from the landscape when it is shown.
    pub fn position(self) -> Vec2 {
        Vec2::new(self.3 as f32, self.4 as f32) / DECIMETRES_PER_METER
    }

    pub fn facing(self) -> f32 {
        self.5 as f32 / 256.0 * std::f32::consts::TAU
    }
}

/// The frame shown at `time` and how far it is towards the next one, in [0, 1].
pub fn frame_at(frames: &[Frame], time: f32) -> (usize, f32) {
    let next = frames.partition_point(|frame| frame.time <= time);
    let current = next.saturating_sub(1);
    let Some(following) = frames.get(next).filter(|_| next > 0) else {
        return (current, 0.0);
    };
    let span = (following.time - frames[current].time).max(f32::EPSILON);
    (
        current,
        ((time - frames[current].time) / span).clamp(0.0, 1.0),
    )
}

fn blend_facing(from: f32, to: f32, t: f32) -> f32 {
    let turn =
        (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    from + turn * t
}

pub fn replay_directory() -> PathBuf {
    game_data_directory().join(REPLAY_DIRECTORY)
}

/// Writes the replay and keeps only the newest few on disk.
pub fn store_replay(directory: &Path, replay: &Replay, stamp: u64) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let path = directory.join(format!("replay-{stamp:012}.json"));
    let json = serde_json::to_string(replay).map_err(|error| error.to_string())?;
    std::fs::write(&path, json).map_err(|error| error.to_string())?;
    let mut stored = replay_files(directory);
    while stored.len() > KEPT_REPLAYS {
        let oldest = stored.remove(stored.len() - 1);
        let _ = std::fs::remove_file(oldest);
    }
    Ok(path)
}

/// Replay files, newest first.
fn replay_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("replay-") && name.ends_with(".json"))
        })
        .collect();
    files.sort();
    files.reverse();
    files
}

#[derive(Clone, Debug)]
pub struct ReplaySummary {
    pub path: PathBuf,
    pub title: String,
    pub outcome: String,
    pub recorded_secs: f32,
}

#[derive(Deserialize)]
struct ReplayHeader {
    level: LevelHeader,
    outcome: String,
    recorded_secs: f32,
}

#[derive(Deserialize)]
struct LevelHeader {
    name: String,
}

pub fn list_replays(directory: &Path) -> Vec<ReplaySummary> {
    replay_files(directory)
        .into_iter()
        .filter_map(|path| {
            let header: ReplayHeader =
                serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
            Some(ReplaySummary {
                path,
                title: header.level.name,
                outcome: header.outcome,
                recorded_secs: header.recorded_secs,
            })
        })
        .collect()
}

pub fn load_replay(path: &Path) -> Result<Replay, String> {
    let json = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let replay: Replay = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    if replay.version != REPLAY_VERSION {
        return Err(format!(
            "replay version {} is not supported",
            replay.version
        ));
    }
    Ok(replay)
}

#[derive(Resource, Default)]
pub struct ReplayRecorder {
    frames: Vec<Frame>,
    pending_events: Vec<Event>,
    timer: f32,
    ids: HashMap<Entity, u32>,
    saved: bool,
}

impl ReplayRecorder {
    fn replay_id(&mut self, entity: Entity) -> u32 {
        let next = self.ids.len() as u32;
        *self.ids.entry(entity).or_insert(next)
    }
}

/// Replays show everything; the fog of war belongs to the match.
pub fn clear_the_fog(mut commands: Commands, level: Res<ActiveLevel>) {
    level.0.shape_the_world();
    commands.insert_resource(crate::sim::intel::Intel::new(false));
}

pub fn stop_watching(mut commands: Commands) {
    commands.remove_resource::<ReplayPlayback>();
}

pub fn start_recording(mut commands: Commands) {
    commands.insert_resource(ReplayRecorder::default());
}

pub fn record_replay_frame(
    time: Res<Time>,
    clock: Res<MatchClock>,
    mut recorder: ResMut<ReplayRecorder>,
    units: Query<
        (Entity, &Unit, &Team, &Transform, &Mobile, &Health),
        Without<crate::sim::transport::Embarked>,
    >,
    buildings: Query<(Entity, &Building, &Team, &Transform, &Health)>,
    unclaimed_sites: Query<(Entity, &Building, &Transform), With<UnclaimedSite>>,
) {
    recorder.timer -= time.delta_secs();
    if recorder.timer > 0.0 || recorder.saved {
        return;
    }
    recorder.timer = FRAME_INTERVAL_SECS;
    let mut actors = Vec::new();
    for (entity, unit, &team, transform, mobile, health) in &units {
        let id = recorder.replay_id(entity);
        let look = Look::Unit(unit.0);
        actors.push(Actor::new(
            id,
            look,
            team,
            transform.translation,
            mobile.facing,
            health.fraction(),
        ));
    }
    for (entity, building, &team, transform, health) in &buildings {
        let id = recorder.replay_id(entity);
        let center = footprint_center(building.origin, building.kind.spec().footprint);
        let facing = transform.rotation.to_euler(EulerRot::YXZ).0;
        let look = Look::Building(building.kind);
        actors.push(Actor::new(
            id,
            look,
            team,
            center,
            facing,
            health.fraction(),
        ));
    }
    for (entity, site, transform) in &unclaimed_sites {
        let id = recorder.replay_id(entity);
        let center = footprint_center(site.origin, site.kind.spec().footprint);
        let facing = transform.rotation.to_euler(EulerRot::YXZ).0;
        actors.push(Actor::unclaimed_site(id, site.kind, center, facing));
    }
    let events = std::mem::take(&mut recorder.pending_events);
    recorder.frames.push(Frame {
        time: clock.elapsed_secs,
        actors,
        events,
    });
}

pub fn record_replay_events(
    mut recorder: ResMut<ReplayRecorder>,
    mut impacts: MessageReader<Impact>,
    shots: Query<&Projectile, Added<Projectile>>,
) {
    for projectile in &shots {
        let (start, end, arc, style, duration) = projectile.flight();
        if style == ProjectileStyle::Bullet {
            continue;
        }
        let style_code = SHOT_STYLES.iter().position(|&s| s == style).unwrap_or(1) as u8;
        let millis = (duration * 1000.0).min(u16::MAX as f32) as u16;
        let arc_decimetres = (arc * DECIMETRES_PER_METER) as i16;
        recorder.pending_events.push(Event::Shot(
            style_code,
            pack(start),
            pack(end),
            arc_decimetres,
            millis,
        ));
    }
    for impact in impacts.read() {
        let size = if impact.scorch_radius > STRUCTURE_BLAST_RADIUS {
            2
        } else if impact.scorch_radius > VEHICLE_BLAST_RADIUS {
            1
        } else {
            0
        };
        recorder
            .pending_events
            .push(Event::Blast(size, pack(impact.position)));
    }
}

fn save_recording(recorder: &mut ReplayRecorder, level: &LevelDefinition, outcome: &str) {
    recorder.saved = true;
    let replay = Replay {
        version: REPLAY_VERSION,
        level: level.clone(),
        outcome: outcome.to_string(),
        recorded_secs: recorder.frames.last().map_or(0.0, |frame| frame.time),
        frames: std::mem::take(&mut recorder.frames),
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    if let Err(error) = store_replay(&replay_directory(), &replay, stamp) {
        warn!("could not store the replay: {error}");
    }
}

pub fn save_replay_when_decided(
    outcome: Res<GameOutcome>,
    level: Res<ActiveLevel>,
    mut recorder: ResMut<ReplayRecorder>,
) {
    if recorder.saved || *outcome == GameOutcome::Ongoing {
        return;
    }
    let result = if *outcome == GameOutcome::Victory {
        "Victory"
    } else {
        "Defeat"
    };
    save_recording(&mut recorder, &level.0, result);
}

/// Closing the game in the middle of a match keeps what was recorded so far.
pub fn save_replay_on_exit(
    mut exits: MessageReader<AppExit>,
    state: Res<State<GameState>>,
    level: Res<ActiveLevel>,
    recorder: Option<ResMut<ReplayRecorder>>,
) {
    let exiting = exits.read().next().is_some();
    if exiting && *state.get() == GameState::Playing {
        save_abandoned_replay(level, recorder);
    }
}

pub fn save_abandoned_replay(level: Res<ActiveLevel>, recorder: Option<ResMut<ReplayRecorder>>) {
    let Some(mut recorder) = recorder else { return };
    if !recorder.saved && recorder.frames.len() >= MIN_ABANDONED_FRAMES {
        save_recording(&mut recorder, &level.0, "Unfinished");
    }
}

/// The replay being watched and where in it the viewer is.
#[derive(Resource)]
pub struct ReplayPlayback {
    replay: Replay,
    clock: f32,
    /// Where the clock stood when events were last shown.
    shown_until: f32,
    speed: usize,
    paused: bool,
    actors: HashMap<u32, Entity>,
}

impl ReplayPlayback {
    pub fn new(replay: Replay) -> Self {
        Self {
            replay,
            clock: 0.0,
            shown_until: 0.0,
            speed: 0,
            paused: false,
            actors: HashMap::new(),
        }
    }
}

#[derive(Component)]
pub struct ReplayActor;

#[derive(Component)]
pub struct ReplayHud;

/// The bar along the bottom of a replay: clicking or dragging on it jumps there.
#[derive(Component)]
pub struct ReplayTimeline;

#[derive(Component)]
pub struct ReplayProgress;

const TIMELINE_BUCKETS: usize = 120;
const TIMELINE_HEIGHT: f32 = 34.0;
const TIMELINE_ACTIVITY: Color = Color::srgba(0.95, 0.55, 0.2, 0.75);
const TIMELINE_PLAYED: Color = Color::srgba(0.35, 0.6, 0.95, 0.35);
const TIMELINE_BACKGROUND: Color = Color::srgba(0.04, 0.05, 0.06, 0.8);

/// Shots fired in each slice of the match, scaled so the fiercest slice is one; shows where the fighting was.
fn fighting_by_time(replay: &Replay) -> Vec<f32> {
    let buckets = TIMELINE_BUCKETS.min(replay.frames.len()).max(1);
    let mut shots = vec![0usize; buckets];
    let length = replay.recorded_secs.max(1.0);
    for frame in &replay.frames {
        let bucket = ((frame.time / length) * buckets as f32) as usize;
        let fired = frame
            .events
            .iter()
            .filter(|event| matches!(event, Event::Shot(..)))
            .count();
        shots[bucket.min(buckets - 1)] += fired;
    }
    let fiercest = shots.iter().copied().max().unwrap_or(0).max(1) as f32;
    shots
        .into_iter()
        .map(|count| count as f32 / fiercest)
        .collect()
}

fn seek_to(playback: &mut ReplayPlayback, hit: &bevy::picking::backend::HitData) {
    let Some(position) = hit.position else { return };
    let share = (position.x + 0.5).clamp(0.0, 1.0);
    playback.clock = share * playback.replay.recorded_secs;
}

fn seek_on_press(press: On<PointerPress>, mut playback: ResMut<ReplayPlayback>) {
    if press.button == PointerButton::Primary {
        seek_to(&mut playback, &press.hit);
    }
}

fn seek_on_drag(
    movement: On<PointerMove>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut playback: ResMut<ReplayPlayback>,
) {
    if mouse.pressed(MouseButton::Left) {
        seek_to(&mut playback, &movement.hit);
    }
}

fn spawn_replay_timeline(commands: &mut Commands, playback: &ReplayPlayback) {
    let fighting = fighting_by_time(&playback.replay);
    commands
        .spawn((
            ReplayTimeline,
            DespawnOnExit(GameState::Replay),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(15.0),
                right: Val::Percent(15.0),
                bottom: Val::Px(22.0),
                height: Val::Px(TIMELINE_HEIGHT),
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            BackgroundColor(TIMELINE_BACKGROUND),
        ))
        .with_children(|bar| {
            bar.spawn((
                ReplayProgress,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    ..default()
                },
                BackgroundColor(TIMELINE_PLAYED),
                Pickable::IGNORE,
            ));
            for share in fighting {
                bar.spawn((
                    Node {
                        flex_grow: 1.0,
                        height: Val::Percent(share * 100.0),
                        ..default()
                    },
                    BackgroundColor(TIMELINE_ACTIVITY),
                    Pickable::IGNORE,
                ));
            }
        })
        .observe(seek_on_press)
        .observe(seek_on_drag);
}

pub fn spawn_replay_hud(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    playback: Res<ReplayPlayback>,
) {
    spawn_replay_timeline(&mut commands, &playback);
    commands
        .spawn((
            DespawnOnExit(GameState::Replay),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(14.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|hud| {
            hud.spawn((
                ReplayHud,
                fonts.heading("", 20.0, ACCENT),
                TextShadow::default(),
            ));
            hud.spawn((
                fonts.text(
                    tr("Space pause  ·  + / - speed  ·  Left / Right 10 s  ·  click the bar below to jump  ·  Esc menu"),
                    15.0,
                    TEXT_MAIN,
                ),
                TextShadow::default(),
            ));
        });
}

pub fn control_replay(
    keys: Res<ButtonInput<KeyCode>>,
    mut playback: ResMut<ReplayPlayback>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        playback.paused = !playback.paused;
    }
    if keys.any_just_pressed([KeyCode::Equal, KeyCode::NumpadAdd]) {
        playback.speed = (playback.speed + 1).min(SPEEDS.len() - 1);
    }
    if keys.any_just_pressed([KeyCode::Minus, KeyCode::NumpadSubtract]) {
        playback.speed = playback.speed.saturating_sub(1);
    }
    let end = playback.replay.recorded_secs;
    if keys.just_pressed(KeyCode::ArrowLeft) {
        playback.clock = (playback.clock - SEEK_STEP_SECS).max(0.0);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        playback.clock = (playback.clock + SEEK_STEP_SECS).min(end);
    }
    if keys.just_pressed(KeyCode::Home) {
        playback.clock = 0.0;
    }
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::MainMenu);
    }
}

/// A replayed shell or missile in flight, flown at the replay's speed.
#[derive(Component)]
pub struct ReplayShot(ShotFlight);

fn show_events(commands: &mut Commands, effects: &EffectAssets, events: &[Event]) {
    for event in events {
        match *event {
            Event::Shot(style, from, to, arc, millis) => {
                let style = SHOT_STYLES
                    .get(style as usize)
                    .copied()
                    .unwrap_or(ProjectileStyle::Shell);
                let start = unpack(from);
                commands.spawn((
                    ReplayShot(ShotFlight {
                        style,
                        start,
                        end: unpack(to),
                        arc_height: f32::from(arc) / DECIMETRES_PER_METER,
                        duration: f32::from(millis) / 1000.0,
                        elapsed: 0.0,
                    }),
                    DespawnOnExit(GameState::Replay),
                    effects.projectile_visual(style),
                    Transform::from_translation(start),
                ));
            }
            Event::Blast(size, at) => {
                let size = BLAST_SIZES
                    .get(size as usize)
                    .copied()
                    .unwrap_or(BlastSize::ShellImpact);
                spawn_blast(commands, effects, unpack(at) + Vec3::Y * 0.5, size);
            }
        }
    }
}

pub fn fly_replay_shots(
    mut commands: Commands,
    time: Res<Time<Real>>,
    playback: Res<ReplayPlayback>,
    mut shots: Query<(Entity, &mut ReplayShot, &mut Transform)>,
) {
    let dt = time.delta_secs()
        * if playback.paused {
            0.0
        } else {
            SPEEDS[playback.speed]
        };
    for (entity, mut shot, mut transform) in &mut shots {
        if shot.0.advance(dt, &mut transform) {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_actor(
    commands: &mut Commands,
    models: &ModelLibrary,
    actor: Actor,
    era: Era,
) -> Option<Entity> {
    let look = actor.look()?;
    let model = match look {
        Look::Unit(kind) => models.unit(kind, actor.team()),
        Look::Building(kind) => match actor.holder() {
            Some(team) => models.building(kind, team, era),
            None => models.unclaimed_site(kind),
        },
    };
    let mut spawned = commands.spawn((
        ReplayActor,
        DespawnOnExit(GameState::Replay),
        model,
        Transform::default(),
    ));
    if let Look::Building(kind) = look
        && kind.is_wall()
    {
        let cell = world_to_cell(Vec3::new(actor.position().x, 0.0, actor.position().y));
        spawned.insert(WallSegment {
            cell,
            team: actor.team(),
        });
    }
    if let Look::Unit(kind) = look {
        spawned.insert((
            animated_unit_model(kind),
            ReplayMotion {
                moving: false,
                infantry: kind.spec().armor == ArmorClass::Infantry,
            },
        ));
        if kind.flies() {
            spawned.insert(Aircraft);
        }
    }
    Some(spawned.id())
}

pub fn play_replay(
    mut commands: Commands,
    time: Res<Time<Real>>,
    level: Res<ActiveLevel>,
    models: Res<ModelLibrary>,
    effects: Res<EffectAssets>,
    mut playback: ResMut<ReplayPlayback>,
    mut actors: Query<(&mut Transform, Option<&mut ReplayMotion>), With<ReplayActor>>,
) {
    let playback = &mut *playback;
    let end = playback.replay.recorded_secs;
    if !playback.paused {
        playback.clock = (playback.clock + time.delta_secs() * SPEEDS[playback.speed]).min(end);
    }
    let frames = &playback.replay.frames;
    if frames.is_empty() {
        return;
    }
    let advanced = playback.clock - playback.shown_until;
    if advanced > 0.0 && advanced < SEEK_THRESHOLD_SECS {
        let (from, until) = (playback.shown_until, playback.clock);
        for frame in frames
            .iter()
            .filter(|frame| frame.time > from && frame.time <= until)
        {
            show_events(&mut commands, &effects, &frame.events);
        }
    }
    playback.shown_until = playback.clock;
    let (index, blend) = frame_at(frames, playback.clock);
    let current = &frames[index];
    let following: HashMap<u32, Actor> = frames
        .get(index + 1)
        .map(|frame| {
            frame
                .actors
                .iter()
                .map(|actor| (actor.id(), *actor))
                .collect()
        })
        .unwrap_or_default();

    let present: Vec<u32> = current.actors.iter().map(|actor| actor.id()).collect();
    playback.actors.retain(|id, entity| {
        let still_here = present.contains(id);
        if !still_here {
            commands.entity(*entity).despawn();
        }
        still_here
    });
    for actor in &current.actors {
        let next = following.get(&actor.id()).copied().unwrap_or(*actor);
        let ground = actor.position().lerp(next.position(), blend);
        let facing = blend_facing(actor.facing(), next.facing(), blend);
        let altitude = match actor.look() {
            Some(Look::Unit(kind)) if kind.flies() => FLIGHT_ALTITUDE,
            _ => 0.0,
        };
        let translation = level
            .0
            .landscape
            .on_ground(Vec3::new(ground.x, 0.0, ground.y))
            + Vec3::Y * altitude;
        let placed =
            Transform::from_translation(translation).with_rotation(Quat::from_rotation_y(facing));
        let moving = actor.position().distance(next.position()) > ACTOR_MOVING_DISTANCE;
        match playback.actors.get(&actor.id()) {
            Some(&entity) => {
                if let Ok((mut transform, motion)) = actors.get_mut(entity) {
                    *transform = placed;
                    if let Some(mut motion) = motion {
                        motion.moving = moving;
                    }
                }
            }
            None => {
                if let Some(entity) = spawn_actor(&mut commands, &models, *actor, level.0.era) {
                    commands.entity(entity).insert(placed);
                    playback.actors.insert(actor.id(), entity);
                }
            }
        }
    }
}

pub fn refresh_replay_hud(
    level: Res<ActiveLevel>,
    playback: Res<ReplayPlayback>,
    mut huds: Query<&mut Text, With<ReplayHud>>,
    mut progress: Query<&mut Node, With<ReplayProgress>>,
) {
    let played = playback.clock / playback.replay.recorded_secs.max(1.0) * 100.0;
    for mut node in &mut progress {
        if node.width != Val::Percent(played) {
            node.width = Val::Percent(played);
        }
    }
    let clock = |secs: f32| format!("{:02}:{:02}", secs as u32 / 60, secs as u32 % 60);
    let state = if playback.paused {
        tr("paused").to_string()
    } else {
        format!("{}x", SPEEDS[playback.speed])
    };
    let line = tr_fmt(
        "REPLAY  {}  ·  {} / {}  ·  {}  ·  {}",
        &[
            &tr_owned(&level.0.name),
            &clock(playback.clock),
            &clock(playback.replay.recorded_secs),
            &state,
            &tr_owned(&playback.replay.outcome),
        ],
    );
    for mut text in &mut huds {
        if text.0 != line {
            text.0 = line.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_looks_keep_the_codes_of_older_replays() {
        assert_eq!(Look::Building(BuildingKind::Wall).code(), 111);
        assert_eq!(Look::Building(BuildingKind::SupplyDepot).code(), 112);
        assert_eq!(Look::Building(BuildingKind::FieldHospital).code(), 114);
        let every_structure = std::iter::once(BuildingKind::CommandCenter)
            .chain(BuildingKind::CONSTRUCTIBLE)
            .chain(BuildingKind::NEUTRAL_SITES);
        for kind in every_structure {
            let code = Look::Building(kind).code();
            assert_eq!(
                Look::from_code(code),
                Some(Look::Building(kind)),
                "{kind:?}"
            );
        }
    }

    fn frame(time: f32, x: f32) -> Frame {
        let actor = Actor::new(
            7,
            Look::Unit(UnitKind::Tank),
            Team::Amber,
            Vec3::new(x, 0.0, 12.0),
            1.0,
            0.5,
        );
        Frame {
            time,
            actors: vec![actor],
            events: Vec::new(),
        }
    }

    #[test]
    fn the_timeline_peaks_where_the_shooting_was() {
        let shot = Event::Shot(0, [0; 3], [10, 0, 0], 0, 100);
        let frames = (0..40)
            .map(|index| Frame {
                time: index as f32 * FRAME_INTERVAL_SECS,
                actors: Vec::new(),
                events: if index >= 30 {
                    vec![shot; 3]
                } else {
                    Vec::new()
                },
            })
            .collect();
        let replay = Replay {
            version: REPLAY_VERSION,
            level: LevelDefinition::default(),
            outcome: String::new(),
            recorded_secs: 20.0,
            frames,
        };
        let fighting = fighting_by_time(&replay);
        assert_eq!(fighting.len(), 40);
        assert_eq!(fighting[0], 0.0);
        assert_eq!(fighting[35], 1.0);
    }

    #[test]
    fn actors_pack_and_unpack_what_matters() {
        let actor = Actor::new(
            42,
            Look::Building(BuildingKind::Wall),
            Team::Violet,
            Vec3::new(31.25, 5.0, 77.0),
            -1.0,
            0.333,
        );
        assert_eq!(actor.id(), 42);
        assert_eq!(actor.look(), Some(Look::Building(BuildingKind::Wall)));
        assert_eq!(actor.team(), Team::Violet);
        assert!((actor.position() - Vec2::new(31.3, 77.0)).length() < 0.05);
        assert!((actor.facing() - (std::f32::consts::TAU - 1.0)).abs() < 0.03);
        assert_eq!(actor.6, 33);
    }

    #[test]
    fn playback_finds_the_frame_and_the_blend_towards_the_next() {
        let frames = vec![frame(0.0, 10.0), frame(0.5, 20.0), frame(1.0, 30.0)];
        assert_eq!(frame_at(&frames, 0.0), (0, 0.0));
        let (index, blend) = frame_at(&frames, 0.75);
        assert_eq!(index, 1);
        assert!((blend - 0.5).abs() < 1e-5);
        assert_eq!(frame_at(&frames, 9.0), (2, 0.0));
    }

    #[test]
    fn replays_recorded_before_events_still_load() {
        let old_frame = r#"{"time":1.5,"actors":[[3,4,1,100,200,0,100]]}"#;
        let frame: Frame = serde_json::from_str(old_frame).unwrap();
        assert!(frame.events.is_empty());
        let shot = Event::Shot(1, pack(Vec3::new(1.0, 2.0, 3.0)), pack(Vec3::ZERO), 5, 400);
        let round_trip: Event =
            serde_json::from_str(&serde_json::to_string(&shot).unwrap()).unwrap();
        assert_eq!(round_trip, shot);
        assert_eq!(
            unpack(pack(Vec3::new(1.25, -2.0, 30.0))),
            Vec3::new(1.3, -2.0, 30.0)
        );
    }

    #[test]
    fn only_the_newest_replays_are_kept() {
        let directory =
            std::env::temp_dir().join(format!("rimefront_replays_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let replay = Replay {
            version: REPLAY_VERSION,
            level: LevelDefinition::default(),
            outcome: "Victory".into(),
            recorded_secs: 1.0,
            frames: vec![frame(0.0, 1.0), frame(1.0, 2.0)],
        };
        for stamp in 0..(KEPT_REPLAYS as u64 + 3) {
            store_replay(&directory, &replay, stamp).unwrap();
        }
        let listed = list_replays(&directory);
        assert_eq!(listed.len(), KEPT_REPLAYS);
        assert!(
            listed[0]
                .path
                .ends_with(format!("replay-{:012}.json", KEPT_REPLAYS + 2))
        );
        assert_eq!(load_replay(&listed[0].path).unwrap().frames, replay.frames);
        let _ = std::fs::remove_dir_all(&directory);
    }
}
