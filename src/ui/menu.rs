use crate::net::network::NetSession;
use crate::ui::notifications::Notifications;
use crate::ui::structure_tools::StructureTool;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;

use crate::app::i18n::{Language, tr, tr_fmt, tr_owned};
use crate::app::match_flow::GameState;
use crate::app::match_flow::MatchClock;
use crate::app::savegame::{
    NAMED_SLOTS, QUICK_SAVE_SLOT, SaveDirectory, SaveRequests, slot_label, slot_summary,
};
use crate::app::settings::{
    FrameLimit, GameSettings, GameSpeed, HealthBars, QualityPreset, ScrollSpeed, ShadowQuality,
    VegetationDetail,
};
use crate::missions::levels::ActiveLevel;
use crate::sim::components::Selected;
use crate::sim::economy::Economies;
use crate::sim::match_stats::MatchStats;
use crate::sim::outcome::objective_status;
use crate::sim::player_commands::LocalTeam;
use crate::ui::key_bindings::{Action, is_bindable};
use crate::ui::placement::Placement;
use crate::ui::sidebar::{ACCENT, FRAME, FRAME_BRIGHT, PANEL, TEXT_DIM, TEXT_MAIN, UiFonts};

const MENU_WIDTH: f32 = 620.0;
const VALUE_BUTTON_WIDTH: f32 = 190.0;
const KEY_BUTTON_WIDTH: f32 = 110.0;
const BUTTON_IDLE: Color = Color::srgb(0.13, 0.15, 0.17);
const BUTTON_HOVERED: Color = Color::srgb(0.19, 0.22, 0.25);
const PRIMARY: Color = Color::srgb(0.62, 0.45, 0.10);
const PRIMARY_HOVERED: Color = Color::srgb(0.76, 0.56, 0.14);
const ACTION_WIDTH: f32 = 220.0;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum MenuPage {
    #[default]
    Game,
    Graphics,
    Gameplay,
    Keys,
    Audio,
    SaveLoad,
}

impl MenuPage {
    const ALL: [MenuPage; 6] = [
        MenuPage::Game,
        MenuPage::Graphics,
        MenuPage::Gameplay,
        MenuPage::Keys,
        MenuPage::Audio,
        MenuPage::SaveLoad,
    ];

    fn title(self) -> &'static str {
        match self {
            MenuPage::Game => tr("GAME"),
            MenuPage::Graphics => tr("GRAPHICS"),
            MenuPage::Gameplay => tr("GAMEPLAY"),
            MenuPage::Keys => tr("KEYS"),
            MenuPage::Audio => tr("AUDIO"),
            MenuPage::SaveLoad => tr("SAVE / LOAD"),
        }
    }
}

#[derive(Resource, Default)]
pub struct MenuState {
    open: bool,
    page: MenuPage,
    needs_rebuild: bool,
    /// The action waiting for the player to press its new key.
    rebinding: Option<Action>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Setting {
    Preset,
    Shadows,
    AmbientOcclusion,
    Bloom,
    AntiAliasing,
    Vegetation,
    Wind,
    Vsync,
    FrameLimit,
    Fullscreen,
    GameSpeed,
    EdgeScroll,
    ScrollSpeed,
    DamageNumbers,
    CheckForUpdates,
    HealthBars,
    PerformanceOverlay,
    MasterVolume,
    MusicVolume,
    EffectsVolume,
    VoiceVolume,
    Language,
}

const GRAPHICS_SETTINGS: [Setting; 10] = [
    Setting::Preset,
    Setting::Shadows,
    Setting::AmbientOcclusion,
    Setting::Bloom,
    Setting::AntiAliasing,
    Setting::Vegetation,
    Setting::Wind,
    Setting::Vsync,
    Setting::FrameLimit,
    Setting::Fullscreen,
];

const AUDIO_SETTINGS: [Setting; 4] = [
    Setting::MasterVolume,
    Setting::MusicVolume,
    Setting::EffectsVolume,
    Setting::VoiceVolume,
];
const VOLUME_STEP: f32 = 0.1;

const GAMEPLAY_SETTINGS: [Setting; 8] = [
    Setting::Language,
    Setting::GameSpeed,
    Setting::EdgeScroll,
    Setting::ScrollSpeed,
    Setting::DamageNumbers,
    Setting::HealthBars,
    Setting::PerformanceOverlay,
    Setting::CheckForUpdates,
];

fn percent(volume: f32) -> String {
    format!("{:.0} %", volume * 100.0)
}

/// Cycles 0 % to 100 % in steps of ten, wrapping at both ends.
fn step_volume(volume: f32, forward: bool) -> f32 {
    let steps = (1.0 / VOLUME_STEP).round() as i32;
    let current = (volume / VOLUME_STEP).round() as i32;
    let next = if forward {
        (current + 1) % (steps + 1)
    } else {
        (current + steps) % (steps + 1)
    };
    next as f32 * VOLUME_STEP
}

fn on_off(value: bool) -> String {
    if value {
        tr("On").into()
    } else {
        tr("Off").into()
    }
}

/// Steps through a fixed list of choices, wrapping at both ends.
fn step<T: Copy + PartialEq>(choices: &[T], current: T, forward: bool) -> T {
    let index = choices
        .iter()
        .position(|&choice| choice == current)
        .unwrap_or(0);
    let next = if forward {
        (index + 1) % choices.len()
    } else {
        (index + choices.len() - 1) % choices.len()
    };
    choices[next]
}

impl Setting {
    fn label(self) -> &'static str {
        match self {
            Setting::Preset => tr("Quality preset"),
            Setting::Shadows => tr("Shadows"),
            Setting::AmbientOcclusion => tr("Ambient occlusion"),
            Setting::Bloom => tr("Glow (bloom)"),
            Setting::AntiAliasing => tr("Anti-aliasing (SMAA)"),
            Setting::Vegetation => tr("Grass and flowers"),
            Setting::Wind => tr("Wind in vegetation"),
            Setting::Vsync => "VSync",
            Setting::FrameLimit => tr("Frame rate limit"),
            Setting::Fullscreen => tr("Fullscreen"),
            Setting::GameSpeed => tr("Game speed"),
            Setting::EdgeScroll => tr("Edge scrolling"),
            Setting::ScrollSpeed => tr("Scroll speed"),
            Setting::DamageNumbers => tr("Damage numbers"),
            Setting::CheckForUpdates => tr("Check for updates"),
            Setting::HealthBars => tr("Health bars"),
            Setting::PerformanceOverlay => tr("Performance overlay (F3)"),
            Setting::MasterVolume => tr("Master volume"),
            Setting::MusicVolume => tr("Music volume"),
            Setting::EffectsVolume => tr("Effects volume"),
            Setting::VoiceVolume => tr("Voice volume"),
            Setting::Language => tr("Language"),
        }
    }

    fn value(self, settings: &GameSettings) -> String {
        match self {
            Setting::Preset => tr_owned(&format!("{:?}", settings.preset)),
            Setting::Shadows => tr_owned(&format!("{:?}", settings.shadows)),
            Setting::AmbientOcclusion => on_off(settings.ambient_occlusion),
            Setting::Bloom => on_off(settings.bloom),
            Setting::AntiAliasing => on_off(settings.anti_aliasing),
            Setting::Vegetation => tr_owned(&format!("{:?}", settings.vegetation)),
            Setting::Wind => on_off(settings.wind),
            Setting::Vsync => on_off(settings.vsync),
            Setting::FrameLimit => settings
                .frame_limit
                .max_fps()
                .map_or("Unlimited".into(), |fps| format!("{fps:.0} fps")),
            Setting::Fullscreen => on_off(settings.fullscreen),
            Setting::GameSpeed => format!("{:.2}x", settings.game_speed_factor()),
            Setting::EdgeScroll => on_off(settings.edge_scroll),
            Setting::ScrollSpeed => tr_owned(&format!("{:?}", settings.scroll_speed)),
            Setting::DamageNumbers => on_off(settings.damage_numbers),
            Setting::CheckForUpdates => on_off(settings.check_for_updates),
            Setting::HealthBars => match settings.health_bars {
                HealthBars::Always => tr("Always").into(),
                HealthBars::DamagedOrSelected => tr("Damaged / selected").into(),
                HealthBars::SelectedOnly => tr("Selected only").into(),
            },
            Setting::PerformanceOverlay => on_off(settings.performance_overlay),
            Setting::MasterVolume => percent(settings.master_volume),
            Setting::MusicVolume => percent(settings.music_volume),
            Setting::EffectsVolume => percent(settings.effects_volume),
            Setting::VoiceVolume => percent(settings.voice_volume),
            Setting::Language => settings.language.native_name().into(),
        }
    }

    fn is_detail_option(self) -> bool {
        matches!(
            self,
            Setting::Shadows
                | Setting::AmbientOcclusion
                | Setting::Bloom
                | Setting::Vegetation
                | Setting::Wind
        )
    }

    fn change(self, settings: &mut GameSettings, forward: bool) {
        match self {
            Setting::Preset => {
                let presets = [
                    QualityPreset::Low,
                    QualityPreset::Medium,
                    QualityPreset::High,
                ];
                let next = step(&presets, settings.preset, forward);
                settings.apply_preset(next);
                return;
            }
            Setting::Shadows => {
                let levels = [
                    ShadowQuality::Off,
                    ShadowQuality::Low,
                    ShadowQuality::Medium,
                    ShadowQuality::High,
                ];
                settings.shadows = step(&levels, settings.shadows, forward);
            }
            Setting::AmbientOcclusion => settings.ambient_occlusion = !settings.ambient_occlusion,
            Setting::Bloom => settings.bloom = !settings.bloom,
            Setting::AntiAliasing => settings.anti_aliasing = !settings.anti_aliasing,
            Setting::Vegetation => {
                let levels = [
                    VegetationDetail::Off,
                    VegetationDetail::Near,
                    VegetationDetail::Far,
                ];
                settings.vegetation = step(&levels, settings.vegetation, forward);
            }
            Setting::Wind => settings.wind = !settings.wind,
            Setting::Vsync => settings.vsync = !settings.vsync,
            Setting::FrameLimit => {
                let limits = [
                    FrameLimit::Fps30,
                    FrameLimit::Fps60,
                    FrameLimit::Fps120,
                    FrameLimit::Unlimited,
                ];
                settings.frame_limit = step(&limits, settings.frame_limit, forward);
            }
            Setting::Fullscreen => settings.fullscreen = !settings.fullscreen,
            Setting::GameSpeed => {
                let speeds = [
                    GameSpeed::Slow,
                    GameSpeed::Normal,
                    GameSpeed::Fast,
                    GameSpeed::Fastest,
                ];
                settings.game_speed = step(&speeds, settings.game_speed, forward);
            }
            Setting::EdgeScroll => settings.edge_scroll = !settings.edge_scroll,
            Setting::ScrollSpeed => {
                let speeds = [ScrollSpeed::Slow, ScrollSpeed::Normal, ScrollSpeed::Fast];
                settings.scroll_speed = step(&speeds, settings.scroll_speed, forward);
            }
            Setting::DamageNumbers => settings.damage_numbers = !settings.damage_numbers,
            Setting::CheckForUpdates => settings.check_for_updates = !settings.check_for_updates,
            Setting::HealthBars => {
                let modes = [
                    HealthBars::Always,
                    HealthBars::DamagedOrSelected,
                    HealthBars::SelectedOnly,
                ];
                settings.health_bars = step(&modes, settings.health_bars, forward);
            }
            Setting::PerformanceOverlay => {
                settings.performance_overlay = !settings.performance_overlay
            }
            Setting::MasterVolume => {
                settings.master_volume = step_volume(settings.master_volume, forward)
            }
            Setting::MusicVolume => {
                settings.music_volume = step_volume(settings.music_volume, forward)
            }
            Setting::EffectsVolume => {
                settings.effects_volume = step_volume(settings.effects_volume, forward)
            }
            Setting::VoiceVolume => {
                settings.voice_volume = step_volume(settings.voice_volume, forward)
            }
            Setting::Language => {
                settings.language = step(&Language::ALL, settings.language, forward);
            }
        }
        if self.is_detail_option() {
            settings.preset = QualityPreset::Custom;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MenuAction {
    Resume,
    Restart,
    MainMenu,
    Quit,
    Save(usize),
    Load(usize),
    ResetKeys,
}

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct MenuContent;

#[derive(Component)]
pub struct MenuTab(MenuPage);

#[derive(Component)]
pub struct SettingButton(Setting);

#[derive(Component)]
pub struct SettingValue(Setting);

#[derive(Component)]
pub struct ActionButton(MenuAction);

#[derive(Component)]
pub struct KeyButton(Action);

#[derive(Component)]
pub struct KeyValue(Action);

/// Where the menu is opened: paused in a match with every page, or over the main menu with the settings only.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuPlace {
    Match,
    MainMenu,
}

impl MenuPage {
    fn shown_in(self, place: MenuPlace) -> bool {
        place == MenuPlace::Match || !matches!(self, MenuPage::Game | MenuPage::SaveLoad)
    }
}

pub fn spawn_menu(commands: &mut Commands, fonts: &UiFonts, place: MenuPlace) {
    let hint = match place {
        MenuPlace::Match => tr("paused  -  Esc / F10 to resume"),
        MenuPlace::MainMenu => tr("Esc to close"),
    };
    let mut root = commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        GlobalZIndex(10),
        Visibility::Hidden,
    ));
    if place == MenuPlace::MainMenu {
        root.insert(DespawnOnExit(GameState::MainMenu));
    }
    root.with_children(|backdrop| {
        backdrop
            .spawn((
                Node {
                    width: Val::Px(MENU_WIDTH),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(18.0)),
                    row_gap: Val::Px(12.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(FRAME_BRIGHT),
            ))
            .with_children(|panel| {
                panel
                    .spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn(fonts.heading("IRON TIDE", 30.0, ACCENT));
                        header.spawn(fonts.text(hint, 15.0, TEXT_DIM));
                    });
                panel
                    .spawn(Node {
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|tabs| {
                        for page in MenuPage::ALL
                            .into_iter()
                            .filter(|page| page.shown_in(place))
                        {
                            tabs.spawn((
                                MenuTab(page),
                                Hovered::default(),
                                Node {
                                    flex_grow: 1.0,
                                    height: Val::Px(32.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(BUTTON_IDLE),
                                BorderColor::all(FRAME),
                            ))
                            .with_children(|tab| {
                                tab.spawn(fonts.heading(page.title(), 15.0, TEXT_MAIN));
                            })
                            .observe(select_page);
                        }
                    });
                panel.spawn((
                    MenuContent,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        min_height: Val::Px(380.0),
                        ..default()
                    },
                ));
            });
    });
}

pub fn toggle_menu(
    keys: Res<ButtonInput<KeyCode>>,
    placement: Res<Placement>,
    tool: Res<StructureTool>,
    selected: Query<(), With<Selected>>,
    mut menu: ResMut<MenuState>,
) {
    if menu.rebinding.is_some() && keys.just_pressed(KeyCode::Escape) {
        menu.rebinding = None;
        return;
    }
    let escape_is_free = placement.active.is_none() && tool.0.is_none() && selected.is_empty();
    let wants_toggle = keys.just_pressed(KeyCode::F10)
        || (keys.just_pressed(KeyCode::Escape) && (menu.open || escape_is_free));
    if wants_toggle {
        let open = !menu.open;
        set_open(&mut menu, open);
    }
}

impl MenuState {
    /// Opens the menu on the page whose English debug name matches, ignoring case.
    pub fn show_page_named(&mut self, name: &str) {
        let Some(page) = MenuPage::ALL
            .into_iter()
            .find(|page| format!("{page:?}").eq_ignore_ascii_case(name))
        else {
            return;
        };
        if self.open && self.page == page {
            return;
        }
        self.page = page;
        set_open(self, true);
    }
}

fn set_open(menu: &mut MenuState, open: bool) {
    menu.open = open;
    menu.needs_rebuild = open;
    menu.rebinding = None;
}

/// A fresh match starts with the pause menu closed and time running.
pub fn close_menu(mut menu: ResMut<MenuState>) {
    *menu = MenuState::default();
}

/// Opens the menu on its first settings page, as the main menu's Settings tile does.
pub fn open_settings(menu: &mut MenuState) {
    menu.page = MenuPage::Graphics;
    set_open(menu, true);
}

pub fn close_settings(menu: &mut MenuState) {
    set_open(menu, false);
}

impl MenuState {
    pub fn is_open(&self) -> bool {
        self.open
    }
}

pub fn open_menu_from_button(mut menu: ResMut<MenuState>) {
    set_open(&mut menu, true);
}

/// The simulation runs on virtual time, so pausing it freezes the battle while the menu is open.
pub fn pause_while_menu_open(
    menu: Res<MenuState>,
    mut time: ResMut<Time<Virtual>>,
    mut roots: Query<&mut Visibility, With<MenuRoot>>,
) {
    if !menu.is_changed() {
        return;
    }
    if menu.open {
        time.pause();
    } else {
        time.unpause();
    }
    for mut visibility in &mut roots {
        *visibility = if menu.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn select_page(click: On<PointerClick>, tabs: Query<&MenuTab>, mut menu: ResMut<MenuState>) {
    if let Ok(tab) = tabs.get(click.entity) {
        menu.page = tab.0;
        menu.needs_rebuild = true;
        menu.rebinding = None;
    }
}

fn change_setting(
    click: On<PointerClick>,
    buttons: Query<&SettingButton>,
    mut settings: ResMut<GameSettings>,
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    let forward = match click.button {
        PointerButton::Primary => true,
        PointerButton::Secondary => false,
        PointerButton::Middle => return,
    };
    button.0.change(&mut settings, forward);
}

fn start_rebinding(
    click: On<PointerClick>,
    buttons: Query<&KeyButton>,
    mut menu: ResMut<MenuState>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Ok(button) = buttons.get(click.entity) {
        menu.rebinding = Some(button.0);
    }
}

pub fn capture_key_binding(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MenuState>,
    mut settings: ResMut<GameSettings>,
) {
    let Some(action) = menu.rebinding else {
        return;
    };
    if let Some(&key) = keys.get_just_pressed().find(|&&key| is_bindable(key)) {
        settings.keys.bind(action, key);
        menu.rebinding = None;
    }
}

fn run_action(
    click: On<PointerClick>,
    buttons: Query<&ActionButton>,
    mut menu: ResMut<MenuState>,
    mut settings: ResMut<GameSettings>,
    mut requests: ResMut<SaveRequests>,
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
    (session, mut notifications): (Option<Res<NetSession>>, ResMut<Notifications>),
) {
    let Ok(button) = buttons.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Primary {
        return;
    }
    let rewinds_the_battle = matches!(
        button.0,
        MenuAction::Restart | MenuAction::Save(_) | MenuAction::Load(_)
    );
    if session.is_some() && rewinds_the_battle {
        notifications.push(tr("Not available in a network battle").to_string());
        return;
    }
    match button.0 {
        MenuAction::Resume => set_open(&mut menu, false),
        MenuAction::Restart => {
            set_open(&mut menu, false);
            next_state.set(GameState::Restarting);
        }
        MenuAction::MainMenu => {
            set_open(&mut menu, false);
            next_state.set(GameState::MainMenu);
        }
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
        MenuAction::Save(slot) => {
            requests.save = Some(slot);
            menu.needs_rebuild = true;
        }
        MenuAction::Load(slot) => {
            requests.load = Some(slot);
            set_open(&mut menu, false);
        }
        MenuAction::ResetKeys => {
            settings.keys.reset();
            menu.rebinding = None;
        }
    }
}

/// What the pause menu tells about the battle in progress.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BattleSummary<'w> {
    local: Res<'w, LocalTeam>,
    level: Res<'w, ActiveLevel>,
    clock: Res<'w, MatchClock>,
    economies: Res<'w, Economies>,
    stats: Res<'w, MatchStats>,
}

fn spawn_battle_card(row: &mut ChildSpawnerCommands, fonts: &UiFonts, battle: &BattleSummary) {
    let tally = battle.stats.of(battle.local.0);
    let elapsed = battle.clock.elapsed_secs as u32;
    let facts = [
        (
            tr("Objective"),
            objective_status(
                &battle.level,
                &battle.clock,
                &battle.economies,
                battle.local.0,
            ),
        ),
        (tr("Time"), format!("{}:{:02}", elapsed / 60, elapsed % 60)),
        (tr("Enemies destroyed"), tally.enemies_destroyed.to_string()),
        (
            tr("Units trained / lost"),
            format!("{} / {}", tally.units_trained, tally.units_lost),
        ),
        (
            tr("Structures built / lost"),
            format!("{} / {}", tally.structures_built, tally.structures_lost),
        ),
    ];
    row.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(6.0),
            border: UiRect::left(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BUTTON_IDLE),
        BorderColor::all(ACCENT),
    ))
    .with_children(|card| {
        card.spawn(fonts.heading(tr_owned(&battle.level.0.name), 20.0, ACCENT));
        for (label, value) in facts {
            card.spawn(Node {
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn((
                    fonts.heading(label, 13.0, TEXT_DIM),
                    Node {
                        width: Val::Px(150.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                line.spawn(fonts.text(value, 15.0, TEXT_MAIN));
            });
        }
    });
}

fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: impl Into<String>,
    width: f32,
) -> Entity {
    parent
        .spawn((
            Hovered::default(),
            Node {
                width: Val::Px(width),
                height: Val::Px(30.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BUTTON_IDLE),
            BorderColor::all(FRAME),
        ))
        .with_children(|button| {
            button.spawn(fonts.heading(label, 15.0, TEXT_MAIN));
        })
        .id()
}

fn spawn_setting_row(
    content: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    settings: &GameSettings,
    setting: Setting,
) {
    content
        .spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(fonts.text(setting.label(), 17.0, TEXT_MAIN));
            row.spawn((
                SettingButton(setting),
                Hovered::default(),
                Node {
                    width: Val::Px(VALUE_BUTTON_WIDTH),
                    height: Val::Px(30.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(BUTTON_IDLE),
                BorderColor::all(FRAME),
            ))
            .with_children(|button| {
                button.spawn((
                    SettingValue(setting),
                    fonts.heading(setting.value(settings), 15.0, ACCENT),
                ));
            })
            .observe(change_setting);
        });
}

fn spawn_key_column(
    content: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    settings: &GameSettings,
    actions: &[Action],
) {
    content
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|column| {
            for &action in actions {
                column
                    .spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(fonts.text(action.name(), 15.0, TEXT_MAIN));
                        row.spawn((
                            KeyButton(action),
                            Hovered::default(),
                            Node {
                                width: Val::Px(KEY_BUTTON_WIDTH),
                                height: Val::Px(30.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(BUTTON_IDLE),
                            BorderColor::all(FRAME),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                KeyValue(action),
                                fonts.heading(settings.keys.label(action), 15.0, ACCENT),
                            ));
                        })
                        .observe(start_rebinding);
                    });
            }
        });
}

fn spawn_slot_row(
    content: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    directory: &SaveDirectory,
    slot: usize,
) {
    let summary = slot_summary(directory, slot);
    content
        .spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|info| {
                info.spawn(fonts.heading(slot_label(slot), 17.0, TEXT_MAIN));
                let (detail, color) = match &summary {
                    Some(summary) => (summary.clone(), TEXT_DIM),
                    None => (tr("empty").to_string(), FRAME_BRIGHT),
                };
                info.spawn(fonts.text(detail, 14.0, color));
            });
            let save = spawn_button(row, fonts, tr("Save"), 90.0);
            row.commands()
                .entity(save)
                .insert(ActionButton(MenuAction::Save(slot)))
                .observe(run_action);
            if summary.is_some() {
                let load = spawn_button(row, fonts, tr("Load"), 90.0);
                row.commands()
                    .entity(load)
                    .insert(ActionButton(MenuAction::Load(slot)))
                    .observe(run_action);
            }
        });
}

pub fn rebuild_menu_page(
    mut commands: Commands,
    mut menu: ResMut<MenuState>,
    fonts: Option<Res<UiFonts>>,
    settings: Res<GameSettings>,
    save_directory: Res<SaveDirectory>,
    battle: BattleSummary,
    contents: Query<Entity, With<MenuContent>>,
) {
    if !menu.needs_rebuild {
        return;
    }
    let (Some(fonts), Ok(content)) = (fonts, contents.single()) else {
        return;
    };
    menu.needs_rebuild = false;
    commands.entity(content).despawn_related::<Children>();
    let page = menu.page;
    commands.entity(content).with_children(|content| match page {
        MenuPage::Game => {
            content.spawn(fonts.text(
                tr_fmt(
                    "Battle is paused. Graphics and gameplay options are saved automatically.\n{} quick save, {} quick load, {} performance overlay.",
                    &[
                        &settings.keys.label(Action::QuickSave),
                        &settings.keys.label(Action::QuickLoad),
                        &settings.keys.label(Action::PerformanceOverlay),
                    ],
                ),
                16.0,
                TEXT_DIM,
            ));
            content
                .spawn(Node {
                    column_gap: Val::Px(20.0),
                    margin: UiRect::top(Val::Px(6.0)),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|column| {
                        let resume = spawn_button(column, &fonts, tr("Resume"), ACTION_WIDTH);
                        column
                            .commands()
                            .entity(resume)
                            .insert((ActionButton(MenuAction::Resume), BackgroundColor(PRIMARY)))
                            .observe(run_action);
                        let actions = [
                            (tr("Restart mission"), MenuAction::Restart),
                            (tr("Back to main menu"), MenuAction::MainMenu),
                            (tr("Quit to desktop"), MenuAction::Quit),
                        ];
                        for (label, action) in actions {
                            let button = spawn_button(column, &fonts, label, ACTION_WIDTH);
                            column.commands().entity(button).insert(ActionButton(action)).observe(run_action);
                        }
                    });
                    spawn_battle_card(row, &fonts, &battle);
                });
        }
        MenuPage::Graphics => {
            for setting in GRAPHICS_SETTINGS {
                spawn_setting_row(content, &fonts, &settings, setting);
            }
            content.spawn(fonts.text(tr("Left click: next value, right click: previous."), 14.0, TEXT_DIM));
        }
        MenuPage::Gameplay => {
            for setting in GAMEPLAY_SETTINGS {
                spawn_setting_row(content, &fonts, &settings, setting);
            }
        }
        MenuPage::Keys => {
            let (left, right) = Action::ALL.split_at(Action::ALL.len() / 2);
            content
                .spawn(Node {
                    column_gap: Val::Px(24.0),
                    ..default()
                })
                .with_children(|columns| {
                    spawn_key_column(columns, &fonts, &settings, left);
                    spawn_key_column(columns, &fonts, &settings, right);
                });
            content.spawn(fonts.text(
                tr("Click an action, then press its new key. A key already in use swaps places. Esc cancels."),
                14.0,
                TEXT_DIM,
            ));
            let reset = spawn_button(content, &fonts, tr("Reset to defaults"), 240.0);
            content.commands().entity(reset).insert(ActionButton(MenuAction::ResetKeys)).observe(run_action);
        }
        MenuPage::Audio => {
            for setting in AUDIO_SETTINGS {
                spawn_setting_row(content, &fonts, &settings, setting);
            }
        }
        MenuPage::SaveLoad => {
            spawn_slot_row(content, &fonts, &save_directory, QUICK_SAVE_SLOT);
            for slot in NAMED_SLOTS {
                spawn_slot_row(content, &fonts, &save_directory, slot);
            }
        }
    });
}

pub fn refresh_menu(
    menu: Res<MenuState>,
    settings: Res<GameSettings>,
    mut values: Query<(&SettingValue, &mut Text)>,
    mut key_values: Query<(&KeyValue, &mut Text), Without<SettingValue>>,
    mut tabs: Query<(&MenuTab, &mut BorderColor, &mut BackgroundColor, &Hovered)>,
    mut buttons: Query<
        (&Hovered, &mut BackgroundColor, Option<&ActionButton>),
        (
            Without<MenuTab>,
            Or<(With<SettingButton>, With<ActionButton>, With<KeyButton>)>,
        ),
    >,
) {
    if !menu.open {
        return;
    }
    for (value, mut text) in &mut values {
        let current = value.0.value(&settings);
        if text.0 != current {
            text.0 = current;
        }
    }
    for (value, mut text) in &mut key_values {
        let current = if menu.rebinding == Some(value.0) {
            tr("press a key")
        } else {
            settings.keys.label(value.0)
        };
        if text.0 != current {
            text.0 = current.to_string();
        }
    }
    for (tab, mut border, mut background, hovered) in &mut tabs {
        let active = tab.0 == menu.page;
        *border = BorderColor::all(if active { ACCENT } else { FRAME });
        background.0 = if hovered.get() || active {
            BUTTON_HOVERED
        } else {
            BUTTON_IDLE
        };
    }
    for (hovered, mut background, action) in &mut buttons {
        let primary = action.is_some_and(|action| action.0 == MenuAction::Resume);
        background.set_if_neq(BackgroundColor(match (primary, hovered.get()) {
            (true, true) => PRIMARY_HOVERED,
            (true, false) => PRIMARY,
            (false, true) => BUTTON_HOVERED,
            (false, false) => BUTTON_IDLE,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_steps_by_ten_percent_and_wraps() {
        assert!((step_volume(0.5, true) - 0.6).abs() < 1e-6);
        assert_eq!(step_volume(1.0, true), 0.0);
        assert!((step_volume(0.0, false) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn stepping_wraps_in_both_directions() {
        let choices = [1, 2, 3];
        assert_eq!(step(&choices, 3, true), 1);
        assert_eq!(step(&choices, 1, false), 3);
    }

    #[test]
    fn touching_a_detail_option_switches_to_a_custom_preset() {
        let mut settings = GameSettings::default();
        Setting::Bloom.change(&mut settings, true);
        assert_eq!(settings.preset, QualityPreset::Custom);
        Setting::Preset.change(&mut settings, true);
        assert_ne!(settings.preset, QualityPreset::Custom);
    }

    #[test]
    fn gameplay_options_do_not_touch_the_graphics_preset() {
        let mut settings = GameSettings::default();
        Setting::GameSpeed.change(&mut settings, true);
        assert_eq!(settings.preset, QualityPreset::High);
        assert_eq!(settings.game_speed, GameSpeed::Fast);
    }
}
