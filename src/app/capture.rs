use crate::app::replay::{ReplayPlayback, list_replays, load_replay, replay_directory};
use crate::net::lobby::{NetLobby, network_level};
use crate::ui::main_menu::MainMenu;
use bevy::camera::RenderTarget;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::app::settings::custom_levels_directory;
use bevy::render::render_resource::TextureFormat;
use bevy::window::{CursorOptions, PrimaryWindow};

use crate::app::match_flow::GameState;
use crate::app::savegame::SaveRequests;
use crate::missions::levels::{
    ActiveLevel, LevelDefinition, SkirmishOptions, campaign, custom_levels,
};
use crate::net::world_client::WorldClient;
use crate::sim::catalog::Team;
use crate::sim::components::{Health, Selected, Unit};
use crate::ui::map_editor::open_map_editor;
use crate::ui::menu::MenuState;
use crate::view::camera::RtsCamera;
use crate::world::map::cell_center;
use crate::world::map_edits::Brush;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

const CAPTURE_PATH_VAR: &str = "RIMEFRONT_CAPTURE";
const CAPTURE_DELAY_VAR: &str = "RIMEFRONT_CAPTURE_AFTER_SECS";
const CAPTURE_FOCUS_VAR: &str = "RIMEFRONT_CAPTURE_FOCUS_CELL";
const CAPTURE_SAVE_SLOT_VAR: &str = "RIMEFRONT_CAPTURE_SAVE_SLOT";
const CAPTURE_LOAD_SLOT_VAR: &str = "RIMEFRONT_CAPTURE_LOAD_SLOT";
const CAPTURE_MISSION_VAR: &str = "RIMEFRONT_CAPTURE_MISSION";
const CAPTURE_REPLAY_VAR: &str = "RIMEFRONT_CAPTURE_REPLAY";
const CAPTURE_SELECT_ALL_VAR: &str = "RIMEFRONT_CAPTURE_SELECT_ALL";
const CAPTURE_ZOOM_VAR: &str = "RIMEFRONT_CAPTURE_ZOOM";
const CAPTURE_MENU_VAR: &str = "RIMEFRONT_CAPTURE_MENU";
/// `RIMEFRONT_CAPTURE_EDITOR=1` opens the map editor on the chosen mission instead of playing it;
/// `RIMEFRONT_CAPTURE_EDITOR=paint` also drags each brush across the ground beside the player's base, and
/// `RIMEFRONT_CAPTURE_EDITOR=level` shows the level settings page.
const CAPTURE_EDITOR_VAR: &str = "RIMEFRONT_CAPTURE_EDITOR";
/// `RIMEFRONT_CAPTURE_OUTCOME=victory` (or `defeat`) ends the battle that way after two seconds.
const CAPTURE_OUTCOME_VAR: &str = "RIMEFRONT_CAPTURE_OUTCOME";
/// `RIMEFRONT_CAPTURE_LANGUAGE=Slovak` (or `Czech`, `English`) shows the interface in that language.
const CAPTURE_LANGUAGE_VAR: &str = "RIMEFRONT_CAPTURE_LANGUAGE";
/// `RIMEFRONT_CAPTURE_RANDOM=42` plays the random battle rolled from that seed with default options, without
/// fog so the whole map shows.
const CAPTURE_RANDOM_VAR: &str = "RIMEFRONT_CAPTURE_RANDOM";
/// `RIMEFRONT_CAPTURE_HOST=2` hosts a network battle for that many players and `RIMEFRONT_CAPTURE_JOIN=127.0.0.1`
/// joins one.
const CAPTURE_HOST_VAR: &str = "RIMEFRONT_CAPTURE_HOST";
/// `RIMEFRONT_CAPTURE_MAIN_MENU=1` stays on the main menu's start screen; a page name opens that page
/// (`campaign`, `random`, `custom`, `editor`, `network`, `replays`).
const CAPTURE_MAIN_MENU_VAR: &str = "RIMEFRONT_CAPTURE_MAIN_MENU";
const CAPTURE_JOIN_VAR: &str = "RIMEFRONT_CAPTURE_JOIN";
/// `RIMEFRONT_CAPTURE_OPEN_ROOM=ws://host:port/room/NAME` opens a two-player room on a dedicated server.
const CAPTURE_OPEN_ROOM_VAR: &str = "RIMEFRONT_CAPTURE_OPEN_ROOM";
/// `RIMEFRONT_CAPTURE_CLAIM_SITES=6` gives out that many starting sites of a wilds level two seconds in,
/// widening the world as it would for newcomers.
const CAPTURE_CLAIM_SITES_VAR: &str = "RIMEFRONT_CAPTURE_CLAIM_SITES";
/// `RIMEFRONT_CAPTURE_WORLD=29` plays the world grown from that seed, run by the game itself;
/// `RIMEFRONT_CAPTURE_WORLD=ws://host:port/world` enters a server's world.
const CAPTURE_WORLD_VAR: &str = "RIMEFRONT_CAPTURE_WORLD";
/// `RIMEFRONT_CAPTURE_DESTROY=1` destroys the player's vehicles two seconds in, to show their wrecks.
const CAPTURE_DESTROY_VAR: &str = "RIMEFRONT_CAPTURE_DESTROY";
const LOAD_AFTER_SECS: f32 = 1.5;
/// A hosting capture run starts the battle only after this, so spectators can join too.
const HOST_WAITS_FOR_GUESTS_SECS: f32 = 10.0;
const DEFAULT_CAPTURE_DELAY_SECS: f32 = 5.0;
const FRAMES_TO_FLUSH_SCREENSHOT: u32 = 10;
const CAPTURE_SIZE: UVec2 = UVec2::new(1600, 900);

pub fn is_capture_run() -> bool {
    std::env::var_os(CAPTURE_PATH_VAR).is_some()
}

/// Development aid: with `RIMEFRONT_CAPTURE=shot.png` the game saves one screenshot and quits.
/// `RIMEFRONT_CAPTURE_SAVE_SLOT` / `_LOAD_SLOT` exercise saving and loading in the same unattended run.
/// Input is ignored meanwhile so a stray mouse on the desktop cannot steer the run.
pub struct CapturePlugin;

#[derive(Resource)]
struct PendingCapture {
    path: String,
    save_slot: Option<usize>,
    load_slot: Option<usize>,
    remaining_secs: f32,
    frames_after_shot: Option<u32>,
}

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        let Ok(path) = std::env::var(CAPTURE_PATH_VAR) else {
            return;
        };
        let delay = std::env::var(CAPTURE_DELAY_VAR)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_CAPTURE_DELAY_SECS);
        let slot_from = |var: &str| std::env::var(var).ok().and_then(|value| value.parse().ok());
        crate::app::capture_pointer::add_pointer_script(app, &path);
        app.insert_resource(PendingCapture {
            path,
            save_slot: slot_from(CAPTURE_SAVE_SLOT_VAR),
            load_slot: slot_from(CAPTURE_LOAD_SLOT_VAR),
            remaining_secs: delay,
            frames_after_shot: None,
        })
        .add_systems(Startup, (let_mouse_pass_through, use_requested_language))
        .add_systems(
            PostStartup,
            (
                spawn_capture_camera,
                start_requested_mission.after(crate::app::match_flow::finish_booting),
            ),
        )
        .add_systems(PreUpdate, discard_keyboard.after(InputSystems))
        .add_systems(Update, (follow_main_camera, capture_then_exit).chain())
        .add_systems(Update, accept_network_offer)
        .add_systems(Update, open_requested_main_menu_section)
        .add_systems(
            Update,
            drag_test_strokes
                .after(crate::ui::map_editor::choose_brush_by_key)
                .before(crate::ui::map_editor::paint_under_cursor)
                .run_if(in_state(GameState::MapEditor))
                .run_if(|| std::env::var(CAPTURE_EDITOR_VAR).is_ok_and(|value| value == "paint")),
        )
        .add_systems(
            Update,
            (
                select_all_own_units,
                zoom_camera,
                open_requested_menu_page,
                decide_requested_outcome,
                claim_requested_sites,
                destroy_own_vehicles.run_if(|| std::env::var_os(CAPTURE_DESTROY_VAR).is_some()),
            )
                .run_if(in_state(GameState::Playing)),
        );
        if let Some(cell) = std::env::var(CAPTURE_FOCUS_VAR)
            .ok()
            .and_then(|value| parse_cell(&value))
        {
            app.insert_resource(CaptureFocus(cell)).add_systems(
                OnEnter(GameState::Playing),
                focus_camera.after(crate::view::camera::focus_on_player_base),
            );
        }
    }
}

/// Screenshots of the window surface come out black on Bevy 0.20 here, so the capture renders
/// through a copy of the game camera into an offscreen image instead, interface included.
#[derive(Resource)]
pub struct CaptureTarget(pub Handle<Image>);

#[derive(Component)]
struct CaptureCamera;

fn spawn_capture_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut game_cameras: Query<(Entity, &mut Camera), With<RtsCamera>>,
) {
    let Ok((game_camera, mut window_view)) = game_cameras.single_mut() else {
        return;
    };
    let target = images.add(Image::new_target_texture(
        CAPTURE_SIZE.x,
        CAPTURE_SIZE.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands
        .entity(game_camera)
        .clone_and_spawn()
        .remove::<RtsCamera>()
        .insert((
            CaptureCamera,
            Camera {
                is_active: true,
                ..window_view.clone()
            },
            RenderTarget::from(target.clone()),
            IsDefaultUiCamera,
        ));
    commands.insert_resource(CaptureTarget(target));
    // Rendering the window too would double the frame cost and skew the fps a capture run reports.
    window_view.is_active = false;
}

fn follow_main_camera(
    game_cameras: Query<&Transform, (With<RtsCamera>, Without<CaptureCamera>)>,
    mut capture_cameras: Query<&mut Transform, With<CaptureCamera>>,
) {
    let Ok(game_view) = game_cameras.single() else {
        return;
    };
    for mut view in &mut capture_cameras {
        *view = *game_view;
    }
}

fn capture_then_exit(
    mut commands: Commands,
    time: Res<Time<Real>>,
    target: Option<Res<CaptureTarget>>,
    diagnostics: Res<DiagnosticsStore>,
    mut requests: ResMut<SaveRequests>,
    mut capture: ResMut<PendingCapture>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(frames) = capture.frames_after_shot.as_mut() {
        *frames += 1;
        if *frames > FRAMES_TO_FLUSH_SCREENSHOT {
            exit.write(AppExit::Success);
        }
        return;
    }
    capture.remaining_secs -= time.delta_secs();
    if time.elapsed_secs() > LOAD_AFTER_SECS
        && let Some(slot) = capture.load_slot.take()
    {
        requests.load = Some(slot);
    }
    let Some(target) = target else { return };
    if capture.remaining_secs <= 0.0 {
        let fps = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(|fps| fps.smoothed())
            .unwrap_or_default();
        let entities = diagnostics
            .get(&bevy::diagnostic::EntityCountDiagnosticsPlugin::ENTITY_COUNT)
            .and_then(|count| count.value())
            .unwrap_or_default();
        info!("capture: average {fps:.1} fps, {entities:.0} entities");
        requests.save = capture.save_slot.take();
        commands
            .spawn(Screenshot::image(target.0.clone()))
            .observe(save_to_disk(capture.path.clone()));
        capture.frames_after_shot = Some(0);
    }
}

#[derive(Resource)]
struct CaptureFocus(IVec2);

fn parse_cell(value: &str) -> Option<IVec2> {
    let (x, y) = value.split_once(',')?;
    Some(IVec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
}

fn focus_camera(focus: Res<CaptureFocus>, mut cameras: Query<&mut RtsCamera>) {
    for mut camera in &mut cameras {
        camera.focus = cell_center(focus.0);
    }
}

/// `RIMEFRONT_CAPTURE_SELECT_ALL=1` selects every own unit, to show the selection marks.
fn select_all_own_units(
    mut commands: Commands,
    units: Query<(Entity, &Team), (With<Unit>, Without<Selected>)>,
) {
    if std::env::var_os(CAPTURE_SELECT_ALL_VAR).is_none() {
        return;
    }
    for (entity, team) in &units {
        if *team == Team::Player {
            commands.entity(entity).insert(Selected);
        }
    }
}

/// `RIMEFRONT_CAPTURE_ZOOM=150` sets the camera distance in meters.
fn zoom_camera(mut cameras: Query<&mut RtsCamera>) {
    let Some(distance) = std::env::var(CAPTURE_ZOOM_VAR)
        .ok()
        .and_then(|value| value.parse().ok())
    else {
        return;
    };
    for mut camera in &mut cameras {
        camera.distance = distance;
        camera.target_distance = distance;
    }
}

/// `RIMEFRONT_CAPTURE_MENU=keys` opens the pause menu on that page (game, graphics, keys, ...).
fn open_requested_menu_page(mut menu: ResMut<MenuState>) {
    if let Ok(page) = std::env::var(CAPTURE_MENU_VAR) {
        menu.show_page_named(&page);
    }
}

/// A capture run that hosts or joins shows the network page, and a joining one accepts the offer at once.
fn accept_network_offer(
    time: Res<Time<Real>>,
    mut lobby: ResMut<NetLobby>,
    menu: Option<ResMut<MainMenu>>,
) {
    let joining = std::env::var_os(CAPTURE_JOIN_VAR).is_some();
    let leading = std::env::var_os(CAPTURE_HOST_VAR).is_some()
        || std::env::var_os(CAPTURE_OPEN_ROOM_VAR).is_some();
    if !joining && !leading {
        return;
    }
    if let Some(mut menu) = menu {
        menu.show_network_page();
    }
    if joining && lobby.can_accept_offer() {
        lobby.accept_offer();
    }
    let others_had_time_to_join = time.elapsed_secs() > HOST_WAITS_FOR_GUESTS_SECS;
    if others_had_time_to_join && lobby.can_start() {
        lobby.start_battle();
    }
}

fn open_requested_main_menu_section(menu: Option<ResMut<MainMenu>>) {
    if let (Some(mut menu), Ok(section)) = (menu, std::env::var(CAPTURE_MAIN_MENU_VAR)) {
        menu.open_section_named(&section);
    }
}

fn use_requested_language(mut settings: ResMut<crate::app::settings::GameSettings>) {
    let requested = std::env::var(CAPTURE_LANGUAGE_VAR).ok().and_then(|name| {
        crate::app::i18n::Language::ALL
            .into_iter()
            .find(|language| format!("{language:?}") == name)
    });
    if let Some(language) = requested {
        settings.language = language;
    }
}

fn decide_requested_outcome(
    clock: Res<crate::app::match_flow::MatchClock>,
    mut outcome: ResMut<crate::sim::outcome::GameOutcome>,
) {
    use crate::sim::outcome::GameOutcome;
    let wanted = match std::env::var(CAPTURE_OUTCOME_VAR).as_deref() {
        Ok("victory") => GameOutcome::Victory,
        Ok("defeat") => GameOutcome::Defeat,
        _ => return,
    };
    if clock.elapsed_secs > 2.0 && *outcome == GameOutcome::Ongoing {
        *outcome = wanted;
    }
}

fn claim_requested_sites(world: &mut World) {
    let Some(sites) = std::env::var(CAPTURE_CLAIM_SITES_VAR)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return;
    };
    let clock = world
        .resource::<crate::app::match_flow::MatchClock>()
        .elapsed_secs;
    let claimed = world
        .get_resource::<crate::world::sites::StartingSites>()
        .map_or(0, |given| given.claimed.len());
    if clock < 2.0 || claimed >= sites {
        return;
    }
    let home = crate::world::map::base_sites()[0];
    for _ in claimed..sites {
        crate::world::sites::claim_starting_site(world, home);
    }
}

fn destroy_own_vehicles(
    mut done: Local<bool>,
    clock: Res<crate::app::match_flow::MatchClock>,
    player: Res<crate::sim::player_commands::LocalTeam>,
    mut units: Query<(&Unit, &Team, &mut Health, &Transform)>,
) {
    if *done || clock.elapsed_secs < 2.0 {
        return;
    }
    *done = true;
    for (unit, &team, mut health, transform) in &mut units {
        if team == player.0 && unit.0.spec().armor == crate::sim::catalog::ArmorClass::Vehicle {
            health.current = 0.0;
            let cell = crate::world::map::world_to_cell(transform.translation);
            info!(
                "capture: destroyed a {:?} at cell {},{}",
                unit.0, cell.x, cell.y
            );
        }
    }
}

fn let_mouse_pass_through(mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    for mut cursor in &mut cursors {
        cursor.hit_test = false;
    }
}

/// One row of cells per brush, painted a cell per frame, then the last row cleared halfway.
fn drag_test_strokes(
    mut frame: Local<i32>,
    level: Res<ActiveLevel>,
    mut editor: ResMut<crate::ui::map_editor::MapEditor>,
    mut cursor: ResMut<crate::view::camera::Cursor>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut cameras: Query<&mut RtsCamera>,
) {
    const STROKE_CELLS: i32 = 10;
    let strokes = [
        (Brush::Crystal, MouseButton::Left),
        (Brush::Forest, MouseButton::Left),
        (Brush::Rock, MouseButton::Left),
        (Brush::Rock, MouseButton::Right),
    ];
    mouse.release_all();
    let stroke = (*frame / STROKE_CELLS) as usize;
    let Some(&(brush, button)) = strokes.get(stroke) else {
        return;
    };
    let step = *frame % STROKE_CELLS;
    let row = stroke.min(2) as i32 * 4;
    let length = if button == MouseButton::Right {
        STROKE_CELLS / 2
    } else {
        STROKE_CELLS
    };
    *frame += 1;
    if step >= length {
        return;
    }
    let base = level.0.player.site.origin();
    for mut camera in &mut cameras {
        camera.focus = cell_center(base + IVec2::new(27, 0));
    }
    editor.brush = brush;
    cursor.over_interface = false;
    cursor.ground = Some(cell_center(base + IVec2::new(22 + step, -4 + row)));
    mouse.press(button);
}

fn discard_keyboard(mut keys: ResMut<ButtonInput<KeyCode>>) {
    keys.reset_all();
}

/// `RIMEFRONT_CAPTURE_MISSION=2` starts the second campaign mission instead of waiting in the main menu;
/// numbers past the campaign continue with the custom levels.
/// `RIMEFRONT_CAPTURE_REPLAY=1` watches the newest replay instead of playing.
fn start_requested_mission(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut lobby: ResMut<NetLobby>,
) {
    if std::env::var_os(CAPTURE_MAIN_MENU_VAR).is_some() {
        return;
    }
    if std::env::var_os(CAPTURE_HOST_VAR).is_some() {
        let players = std::env::var(CAPTURE_HOST_VAR)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(2);
        lobby.host(network_level(&SkirmishOptions::default(), players), players);
        return;
    }
    if let Ok(address) = std::env::var(CAPTURE_JOIN_VAR) {
        lobby.join(&address);
        return;
    }
    if let Ok(world) = std::env::var(CAPTURE_WORLD_VAR) {
        let client = match world.parse() {
            Ok(map_seed) => WorldClient::alone(map_seed),
            Err(_) => {
                WorldClient::on_server(&world, None).expect("the server's world can be reached")
            }
        };
        commands.insert_resource(client);
        return;
    }
    if let Ok(address) = std::env::var(CAPTURE_OPEN_ROOM_VAR) {
        lobby.open_room(&address, network_level(&SkirmishOptions::default(), 2), 2);
        return;
    }
    let newest_replay = std::env::var_os(CAPTURE_REPLAY_VAR)
        .and_then(|_| list_replays(&replay_directory()).into_iter().next())
        .and_then(|summary| load_replay(&summary.path).ok());
    if let Some(replay) = newest_replay {
        commands.insert_resource(ActiveLevel(replay.level.clone()));
        commands.insert_resource(ReplayPlayback::new(replay));
        next_state.set(GameState::Replay);
        return;
    }
    let requested_mission = std::env::var(CAPTURE_MISSION_VAR)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|number| {
            campaign()
                .into_iter()
                .chain(custom_levels(&custom_levels_directory()))
                .nth(number.saturating_sub(1))
        });
    let rolled = std::env::var(CAPTURE_RANDOM_VAR)
        .ok()
        .and_then(|value| value.parse().ok())
        .map(|seed| LevelDefinition {
            fog_of_war: false,
            ..crate::missions::random_battle::generate(
                &crate::missions::random_battle::RandomBattleOptions::default(),
                seed,
            )
        });
    let level = rolled
        .or(requested_mission.map(|mission| mission.level))
        .unwrap_or_default();
    if std::env::var_os(CAPTURE_EDITOR_VAR).is_some() {
        open_map_editor(&mut commands, &mut next_state, level, None);
        if std::env::var(CAPTURE_EDITOR_VAR).is_ok_and(|value| value == "level") {
            let mut editor = crate::ui::map_editor::MapEditor::new(None);
            editor.page = crate::ui::map_editor::EditorPage::Level;
            commands.insert_resource(editor);
        }
        return;
    }
    commands.insert_resource(ActiveLevel(level));
    next_state.set(GameState::Playing);
}
