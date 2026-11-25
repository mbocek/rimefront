use crate::missions::campaigns::{
    Campaign, CampaignLibrary, CampaignRun, add_to_my_campaign, built_in_campaigns,
    custom_campaigns, find_campaign,
};
use crate::missions::mission_generator::generate_mission;
use crate::missions::random_battle::{
    self, CREDIT_CHOICES, DifficultyChoice, FactionChoice, RandomBattleOptions,
};
use crate::net::network::NetSession;
use crate::sim::outcome::GameOutcome;
use crate::ui::map_editor::open_map_editor;
use crate::ui::map_preview::map_preview;
use crate::ui::menu::{MenuPlace, MenuState, close_settings, open_settings, spawn_menu};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use rand::RngExt;

use crate::app::i18n::{Language, set_language, tr, tr_fmt, tr_owned};
use crate::app::match_flow::GameState;
use crate::app::replay::{
    ReplayPlayback, ReplaySummary, list_replays, load_replay, replay_directory,
};
use crate::app::savegame::{QUICK_SAVE_SLOT, SaveDirectory, SaveRequests, slot_summary};
use crate::app::settings::GameSettings;
use crate::app::settings::custom_levels_directory;
use crate::app::updates::{UpdateState, Updates, download_update};
use crate::missions::levels::{
    ActiveLevel, BaseSite, Difficulty, LevelDefinition, LevelEntry, LevelSource, ObjectiveKind,
    SkirmishOptions, TechLevel, Weather, custom_levels, save_custom_level,
};
use crate::net::lobby::{MOST_PLAYERS, NetLobby, network_level};
use crate::net::world_client::{OwnWorld, WorldClient};
use crate::sim::catalog::Team;
use crate::sim::catalog::{Alliance, Faction};
use crate::ui::sidebar::{
    ACCENT, FRAME, FRAME_BRIGHT, GOOD, PANEL, TEXT_DIM, TEXT_MAIN, UiFonts, set_field,
};
use crate::world::map::MapSize;
use crate::world::season::Season;
use crate::world::terrain::Landscape;

mod battle_setup;
mod campaign_map;
mod commands;
mod network_page;
mod update_banner;
mod widgets;

pub use battle_setup::*;
pub use campaign_map::*;
pub use commands::*;
pub use network_page::*;
pub use update_banner::*;
use widgets::*;

const NAV_WIDTH: f32 = 240.0;
const LIST_WIDTH: f32 = 330.0;
const DETAIL_WIDTH: f32 = 640.0;
const REPLAYS_LISTED: usize = 10;
const LARGE_PREVIEW: f32 = 220.0;
const FACT_LABEL_WIDTH: f32 = 130.0;
const SMALL_PREVIEW: f32 = 170.0;
const BODY_GAP: f32 = 14.0;
const EDITOR_FIELD_WIDTH: f32 = 440.0;
const EDITOR_VALUE_WIDTH: f32 = 230.0;
const BUTTON_IDLE: Color = Color::srgb(0.13, 0.15, 0.17);
const BUTTON_HOVERED: Color = Color::srgb(0.19, 0.22, 0.25);
const BUTTON_SELECTED: Color = Color::srgb(0.17, 0.24, 0.32);
const BACKDROP: Color = Color::srgb(0.035, 0.045, 0.055);
/// A battle scene behind the menu, darkened by the backdrop colour at this opacity so text stays readable.
const BACKDROP_IMAGE: &str = "ui/menu_backdrop.jpg";
const BACKDROP_VEIL: f32 = 0.8;
const PRIMARY_IDLE: Color = Color::srgb(0.62, 0.45, 0.10);
const PRIMARY_HOVERED: Color = Color::srgb(0.76, 0.56, 0.14);
const MAP_VARIANTS: u64 = 5;
const HUB_TILE_WIDTH: f32 = 420.0;
const HUB_TILE_HEIGHT: f32 = 120.0;
const HUB_GAP: f32 = 16.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Selection {
    Level(usize),
    /// A mission of the campaign at that index in the campaign list.
    Mission {
        campaign: usize,
        mission: usize,
    },
    Editor,
    Network,
    Replay(usize),
    Quick,
    Generated,
    Random,
    /// A section with nothing in it yet.
    Nothing(MenuSection),
}

/// The groups along the left edge of the main menu; each shows its own list or page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuSection {
    Campaign,
    Quick,
    Generated,
    Random,
    Custom,
    Editor,
    Network,
    Replays,
}

impl MenuSection {
    fn label(self) -> &'static str {
        match self {
            MenuSection::Campaign => tr("CAMPAIGN"),
            MenuSection::Quick => tr("QUICK BATTLE"),
            MenuSection::Generated => tr("MISSION GENERATOR"),
            MenuSection::Random => tr("RANDOM BATTLE"),
            MenuSection::Custom => tr("CUSTOM LEVELS"),
            MenuSection::Editor => tr("BATTLE EDITOR"),
            MenuSection::Network => tr("NETWORK BATTLE"),
            MenuSection::Replays => tr("REPLAYS"),
        }
    }

    /// Sections that are a single page need no list beside them.
    fn has_list(self) -> bool {
        !matches!(
            self,
            MenuSection::Quick
                | MenuSection::Generated
                | MenuSection::Random
                | MenuSection::Editor
                | MenuSection::Network
        )
    }
}

/// The tiles of the start screen; each opens a screen with the sections that belong to it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuGroup {
    Campaigns,
    Skirmish,
    Multiplayer,
    Editor,
    Replays,
}

impl MenuGroup {
    const ALL: [MenuGroup; 5] = [
        MenuGroup::Campaigns,
        MenuGroup::Skirmish,
        MenuGroup::Multiplayer,
        MenuGroup::Editor,
        MenuGroup::Replays,
    ];

    fn title(self) -> &'static str {
        match self {
            MenuGroup::Campaigns => tr("CAMPAIGNS"),
            MenuGroup::Skirmish => tr("SKIRMISH"),
            MenuGroup::Multiplayer => tr("MULTIPLAYER"),
            MenuGroup::Editor => tr("EDITOR"),
            MenuGroup::Replays => tr("REPLAYS"),
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            MenuGroup::Campaigns => tr("Story missions for the Coalition and the Syndicate"),
            MenuGroup::Skirmish => tr("A battle against the computer, rolled or set up your way"),
            MenuGroup::Multiplayer => tr("Battles over the network, up to four players"),
            MenuGroup::Editor => tr("Set up battles, paint maps and play your own levels"),
            MenuGroup::Replays => tr("Watch your recent matches again"),
        }
    }

    fn sections(self) -> &'static [MenuSection] {
        match self {
            MenuGroup::Campaigns => &[MenuSection::Campaign],
            MenuGroup::Skirmish => &[
                MenuSection::Quick,
                MenuSection::Generated,
                MenuSection::Random,
                MenuSection::Custom,
            ],
            MenuGroup::Multiplayer => &[MenuSection::Network],
            MenuGroup::Editor => &[MenuSection::Editor, MenuSection::Custom],
            MenuGroup::Replays => &[MenuSection::Replays],
        }
    }

    fn holding(section: MenuSection) -> MenuGroup {
        MenuGroup::ALL
            .into_iter()
            .find(|group| group.sections().contains(&section))
            .unwrap_or(MenuGroup::Campaigns)
    }
}

/// What the main menu shows: the start screen, or one group's sections.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen {
    Hub,
    Group(MenuGroup),
}

#[derive(Component)]
pub struct SectionButton(MenuSection);

#[derive(Component)]
pub struct HubTile(MenuGroup);

/// Holds the current screen's contents; rebuilt whenever the screen changes.
#[derive(Component)]
pub struct ScreenBody;

#[derive(Resource)]
pub struct MainMenu {
    campaigns: Vec<Campaign>,
    campaign_difficulty: Difficulty,
    quick_faction: FactionChoice,
    quick_difficulty: Difficulty,
    screen: Screen,
    needs_new_screen: bool,
    entries: Vec<LevelEntry>,
    replays: Vec<ReplaySummary>,
    selection: Selection,
    options: SkirmishOptions,
    random: RandomBattleOptions,
    /// The dice of the random battle on show; a new roll draws another.
    random_seed: u64,
    completed_missions: Vec<String>,
    /// The map picture of the level last shown, kept until another level is shown.
    preview: Option<(LevelDefinition, Handle<Image>)>,
    message: Option<String>,
    needs_rebuild: bool,
}

#[derive(Component)]
pub struct MainMenuRoot;

#[derive(Component)]
pub struct LevelList;

#[derive(Component)]
pub struct ListColumn;

#[derive(Component)]
pub struct DetailPanel;

#[derive(Component)]
pub struct ListButton(Selection);

fn all_levels() -> Vec<LevelEntry> {
    custom_levels(&custom_levels_directory())
}

pub fn build_main_menu(
    settings: Res<GameSettings>,
    mut commands: Commands,
    fonts: Res<UiFonts>,
    asset_server: Res<AssetServer>,
    library: Res<CampaignLibrary>,
) {
    // The menu is built on the first frame, before the language from the saved settings is otherwise applied.
    set_language(settings.language);
    let menu = MainMenu {
        campaigns: library.0.clone(),
        campaign_difficulty: settings.campaign_difficulty,
        quick_faction: FactionChoice::Fixed(Faction::Coalition),
        quick_difficulty: Difficulty::Normal,
        screen: Screen::Hub,
        needs_new_screen: true,
        entries: all_levels(),
        replays: list_replays(&replay_directory()),
        selection: Selection::Nothing(MenuSection::Campaign),
        options: SkirmishOptions::default(),
        random: RandomBattleOptions::default(),
        random_seed: rand::rng().random(),
        completed_missions: settings.completed_missions.clone(),
        preview: None,
        message: None,
        needs_rebuild: true,
    };
    commands.spawn((
        DespawnOnExit(GameState::MainMenu),
        ImageNode::new(asset_server.load(BACKDROP_IMAGE)),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(-1),
    ));
    commands
        .spawn((
            MainMenuRoot,
            DespawnOnExit(GameState::MainMenu),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(24.0)),
                row_gap: Val::Px(18.0),
                ..default()
            },
            BackgroundColor(BACKDROP.with_alpha(BACKDROP_VEIL)),
        ))
        .with_children(|root| {
            spawn_title_bar(root, &fonts, &settings);
            root.spawn((
                ScreenBody,
                Node {
                    width: Val::Px(full_width()),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
        });
    spawn_menu(&mut commands, &fonts, MenuPlace::MainMenu);
    commands.insert_resource(menu);
}

fn spawn_title_bar(root: &mut ChildSpawnerCommands, fonts: &UiFonts, settings: &GameSettings) {
    root.spawn(Node {
        width: Val::Px(full_width()),
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|bar| {
        bar.spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|title| {
            title.spawn(fonts.heading("IRON TIDE", 46.0, ACCENT));
            title.spawn(fonts.text(
                format!(
                    "{}  ·  v{}",
                    tr("Real-time strategy"),
                    env!("CARGO_PKG_VERSION")
                ),
                15.0,
                TEXT_DIM,
            ));
        });
        bar.spawn(Node {
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|actions| {
            actions
                .spawn((
                    MenuCommand::DownloadUpdate,
                    UpdateBanner,
                    Hovered::default(),
                    Node {
                        display: Display::None,
                        ..button_node(Val::Auto)
                    },
                    BackgroundColor(BUTTON_IDLE),
                    BorderColor::all(GOOD),
                ))
                .with_children(|button| {
                    button.spawn(fonts.heading("", 16.0, GOOD));
                })
                .observe(run_command);
            let language = format!("{}: {}", tr("Language"), settings.language.native_name());
            spawn_command_button_sized(
                actions,
                fonts,
                &language,
                MenuCommand::CycleLanguage,
                Val::Auto,
            );
            spawn_command_button_sized(
                actions,
                fonts,
                tr("Quit to desktop"),
                MenuCommand::Quit,
                Val::Auto,
            );
        });
    });
}

/// The start screen: a tile for each group, settings, and the quick save to continue from.
fn spawn_hub(
    body: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    quick_save: Option<String>,
    campaign_to_continue: Option<String>,
) {
    body.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(HUB_GAP),
        margin: UiRect::top(Val::Px(40.0)),
        ..default()
    })
    .with_children(|hub| {
        if let Some(summary) = quick_save {
            hub.spawn((
                MenuCommand::ContinueQuickSave,
                Hovered::default(),
                Node {
                    width: Val::Px(HUB_TILE_WIDTH * 2.0 + HUB_GAP),
                    flex_direction: FlexDirection::Column,
                    ..button_node(Val::Auto)
                },
                BackgroundColor(BUTTON_IDLE),
                BorderColor::all(ACCENT),
            ))
            .with_children(|button| {
                button.spawn(fonts.heading(tr("CONTINUE"), 20.0, ACCENT));
                button.spawn(fonts.text(summary, 14.0, TEXT_DIM));
            })
            .observe(run_command);
        }
        if let Some(summary) = campaign_to_continue {
            hub.spawn((
                MenuCommand::ContinueCampaign,
                Hovered::default(),
                Node {
                    width: Val::Px(HUB_TILE_WIDTH * 2.0 + HUB_GAP),
                    flex_direction: FlexDirection::Column,
                    ..button_node(Val::Auto)
                },
                BackgroundColor(BUTTON_IDLE),
                BorderColor::all(ACCENT),
            ))
            .with_children(|button| {
                button.spawn(fonts.heading(tr("CONTINUE CAMPAIGN"), 20.0, ACCENT));
                button.spawn(fonts.text(summary, 14.0, TEXT_DIM));
            })
            .observe(run_command);
        }
        let tiles: Vec<Option<MenuGroup>> =
            MenuGroup::ALL.into_iter().map(Some).chain([None]).collect();
        for row in tiles.chunks(2) {
            hub.spawn(Node {
                column_gap: Val::Px(HUB_GAP),
                ..default()
            })
            .with_children(|row_node| {
                for &tile in row {
                    let (title, blurb) = match tile {
                        Some(group) => (group.title(), group.blurb()),
                        None => (tr("SETTINGS"), tr("Graphics, sound, controls and gameplay")),
                    };
                    let mut button = row_node.spawn((
                        Hovered::default(),
                        Node {
                            width: Val::Px(HUB_TILE_WIDTH),
                            height: Val::Px(HUB_TILE_HEIGHT),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
                            row_gap: Val::Px(6.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(BUTTON_IDLE),
                        BorderColor::all(FRAME),
                    ));
                    button.with_children(|tile_node| {
                        tile_node.spawn(fonts.heading(title, 26.0, TEXT_MAIN));
                        tile_node.spawn(fonts.text(blurb, 15.0, TEXT_DIM));
                    });
                    match tile {
                        Some(group) => {
                            button.insert(HubTile(group)).observe(open_group);
                        }
                        None => {
                            button
                                .insert(MenuCommand::OpenSettings)
                                .observe(run_command);
                        }
                    }
                }
            });
        }
    });
}

/// One group's screen: its sections down the left, a list beside them and the details of what is chosen.
fn spawn_group_screen(body: &mut ChildSpawnerCommands, fonts: &UiFonts, group: MenuGroup) {
    body.spawn(Node {
        width: Val::Px(full_width()),
        column_gap: Val::Px(BODY_GAP),
        align_items: AlignItems::FlexStart,
        ..default()
    })
    .with_children(|body| {
        spawn_section_column(body, fonts, group);
        body.spawn((
            ListColumn,
            Node {
                width: Val::Px(LIST_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(FRAME),
        ))
        .with_children(|column| {
            column.spawn((
                LevelList,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    ..default()
                },
            ));
        });
        body.spawn((
            DetailPanel,
            Node {
                min_width: Val::Px(DETAIL_WIDTH),
                flex_grow: 1.0,
                min_height: Val::Px(420.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(FRAME_BRIGHT),
        ));
    });
}

fn open_group(click: On<PointerClick>, tiles: Query<&HubTile>, mut menu: ResMut<MainMenu>) {
    if let Ok(tile) = tiles.get(click.entity) {
        menu.open_group(tile.0);
    }
}

/// Puts the screen the menu is on into the screen body, after the screen changed.
pub fn rebuild_screen(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    save_directory: Res<SaveDirectory>,
    settings: Res<GameSettings>,
    menu: Option<ResMut<MainMenu>>,
    bodies: Query<Entity, With<ScreenBody>>,
) {
    let (Some(mut menu), Ok(body)) = (menu, bodies.single()) else {
        return;
    };
    if !menu.needs_new_screen {
        return;
    }
    menu.needs_new_screen = false;
    let continue_summary =
        menu.campaign_to_continue(&settings.last_campaign)
            .map(|(campaign, mission)| {
                let chosen = &menu.campaigns[campaign];
                format!(
                    "{}  ·  {}",
                    tr_owned(&chosen.title),
                    tr_owned(&chosen.missions[mission].name)
                )
            });
    commands.entity(body).despawn_related::<Children>();
    let screen = menu.screen;
    commands.entity(body).with_children(|body| match screen {
        Screen::Hub => spawn_hub(
            body,
            &fonts,
            slot_summary(&save_directory, QUICK_SAVE_SLOT),
            continue_summary.clone(),
        ),
        Screen::Group(group) => spawn_group_screen(body, &fonts, group),
    });
    menu.needs_rebuild = true;
}

/// Escape closes the settings, or leaves a group's screen for the start screen.
pub fn step_back_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    menu: Option<ResMut<MainMenu>>,
    mut settings_menu: ResMut<MenuState>,
) {
    let Some(mut menu) = menu else { return };
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if settings_menu.is_open() {
        close_settings(&mut settings_menu);
    } else if menu.screen != Screen::Hub {
        menu.show_hub();
    }
}

fn full_width() -> f32 {
    NAV_WIDTH + LIST_WIDTH + DETAIL_WIDTH + 2.0 * BODY_GAP
}

fn spawn_section_column(body: &mut ChildSpawnerCommands, fonts: &UiFonts, group: MenuGroup) {
    body.spawn(Node {
        width: Val::Px(NAV_WIDTH),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|column| {
        spawn_command_button(column, fonts, tr("< Back"), MenuCommand::BackToHub);
        column.spawn(fonts.heading(group.title(), 24.0, ACCENT));
        let sections = group.sections();
        for &section in sections.iter().filter(|_| sections.len() > 1) {
            column
                .spawn((
                    SectionButton(section),
                    Hovered::default(),
                    Node {
                        min_height: Val::Px(46.0),
                        ..button_node(Val::Percent(100.0))
                    },
                    BackgroundColor(BUTTON_IDLE),
                    BorderColor::all(FRAME),
                ))
                .with_children(|button| {
                    button.spawn(fonts.heading(section.label(), 19.0, TEXT_MAIN));
                })
                .observe(open_section);
        }
    });
}

fn open_section(
    click: On<PointerClick>,
    buttons: Query<&SectionButton>,
    mut menu: ResMut<MainMenu>,
) {
    if let Ok(button) = buttons.get(click.entity) {
        menu.selection = menu.first_in(button.0);
        menu.message = None;
        menu.needs_rebuild = true;
    }
}

fn rebuild_level_list(commands: &mut Commands, list: Entity, fonts: &UiFonts, menu: &MainMenu) {
    commands.entity(list).despawn_related::<Children>();
    commands
        .entity(list)
        .with_children(|list| match menu.section() {
            MenuSection::Campaign => spawn_operations_map(list, fonts, menu),
            MenuSection::Custom => {
                let mut any = false;
                for (index, entry) in menu.entries.iter().enumerate() {
                    if matches!(entry.source, LevelSource::Custom(_)) {
                        any = true;
                        spawn_list_button(
                            list,
                            fonts,
                            &tr_owned(&entry.level.name),
                            Selection::Level(index),
                        );
                    }
                }
                if !any {
                    list.spawn(fonts.text(
                        tr("No custom levels yet - make one in the battle editor."),
                        15.0,
                        TEXT_DIM,
                    ));
                }
            }
            MenuSection::Replays => {
                for (index, replay) in menu.replays.iter().enumerate().take(REPLAYS_LISTED) {
                    let label = format!(
                        "{}  ({})",
                        tr_owned(&replay.title),
                        minutes(replay.recorded_secs)
                    );
                    spawn_list_button(list, fonts, &label, Selection::Replay(index));
                }
                if menu.replays.is_empty() {
                    list.spawn(fonts.text(tr("Every battle is recorded here."), 15.0, TEXT_DIM));
                }
            }
            MenuSection::Quick
            | MenuSection::Generated
            | MenuSection::Random
            | MenuSection::Editor
            | MenuSection::Network => {}
        });
}

/// The facts of a level as label, value and colour, for a two-column table.
fn level_facts(level: &LevelDefinition) -> Vec<(String, String, Color)> {
    let mut facts = vec![
        (
            tr("Objective").to_string(),
            level.objective.describe(),
            ACCENT,
        ),
        (
            tr("Map").to_string(),
            tr_fmt(
                "{}, {}, {}, {} x {} cells",
                &[
                    &tr_owned(&format!("{:?}", level.landscape)),
                    &tr_owned(&format!("{:?}", level.season)),
                    &tr_owned(&format!("{:?}", level.weather)),
                    &level.map_size.cells(),
                    &level.map_size.cells(),
                ],
            ),
            TEXT_MAIN,
        ),
        (
            tr_owned(&Team::Player.name_in(level.player_faction)),
            format!("{}  ·  ${}", level.player.site.name(), level.player.credits),
            Team::Player.ui_color(),
        ),
    ];
    for (team, enemy) in level.opponents() {
        facts.push((
            tr_owned(&team.name_in(level.faction_of(team))),
            format!(
                "{}  ·  {}  ·  {}  ·  ${}",
                enemy.force.site.name(),
                tr_owned(&format!("{:?}", enemy.difficulty)),
                enemy.personality.name(),
                enemy.force.credits
            ),
            team.ui_color(),
        ));
    }
    if level.enemies.len() > 1 {
        facts.push((
            tr("Opponents").to_string(),
            alliance_name(level.effective_alliance()).to_string(),
            TEXT_MAIN,
        ));
    }
    let units = if level.available_units.is_empty() {
        tr("every unit").to_string()
    } else {
        level
            .available_units
            .iter()
            .map(|kind| tr(kind.spec().name))
            .collect::<Vec<_>>()
            .join(", ")
    };
    facts.push((tr("Units").to_string(), units, TEXT_DIM));
    facts
}

fn spawn_level_facts(column: &mut ChildSpawnerCommands, fonts: &UiFonts, level: &LevelDefinition) {
    for (label, value, color) in level_facts(level) {
        column
            .spawn(Node {
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    fonts.heading(label, 14.0, TEXT_DIM),
                    Node {
                        width: Val::Px(FACT_LABEL_WIDTH),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                row.spawn((
                    fonts.text(value, 16.0, color),
                    Node {
                        flex_shrink: 1.0,
                        ..default()
                    },
                ));
            });
    }
}

impl MainMenu {
    fn section(&self) -> MenuSection {
        match self.selection {
            Selection::Level(index) => match self.entries[index].source {
                LevelSource::Custom(_) => MenuSection::Custom,
                LevelSource::Campaign(_) | LevelSource::Skirmish => MenuSection::Campaign,
            },
            Selection::Mission { .. } => MenuSection::Campaign,
            Selection::Editor => MenuSection::Editor,
            Selection::Network => MenuSection::Network,
            Selection::Replay(_) => MenuSection::Replays,
            Selection::Quick => MenuSection::Quick,
            Selection::Generated => MenuSection::Generated,
            Selection::Random => MenuSection::Random,
            Selection::Nothing(section) => section,
        }
    }

    /// What a section shows first: its first entry, or its page.
    fn first_in(&self, section: MenuSection) -> Selection {
        let first_level = |custom: bool| {
            self.entries
                .iter()
                .position(|entry| matches!(entry.source, LevelSource::Custom(_)) == custom)
                .map(Selection::Level)
        };
        let first = match section {
            MenuSection::Campaign => Some(self.campaign_page(0)),
            MenuSection::Custom => first_level(true),
            MenuSection::Quick => Some(Selection::Quick),
            MenuSection::Generated => Some(Selection::Generated),
            MenuSection::Random => Some(Selection::Random),
            MenuSection::Editor => Some(Selection::Editor),
            MenuSection::Network => Some(Selection::Network),
            MenuSection::Replays => (!self.replays.is_empty()).then_some(Selection::Replay(0)),
        };
        first.unwrap_or(Selection::Nothing(section))
    }

    /// A campaign's page opens on the mission to play next, or its first once it is finished.
    fn campaign_page(&self, campaign: usize) -> Selection {
        let mission = self
            .campaigns
            .get(campaign)
            .and_then(|c| c.next_to_play(&self.completed_missions))
            .unwrap_or(0);
        Selection::Mission { campaign, mission }
    }

    /// The campaign played last and its next mission, while it has one.
    fn campaign_to_continue(&self, last: &str) -> Option<(usize, usize)> {
        let campaign = self.campaigns.iter().position(|c| c.key == last)?;
        let mission = self.campaigns[campaign].next_to_play(&self.completed_missions)?;
        Some((campaign, mission))
    }

    fn open_group(&mut self, group: MenuGroup) {
        self.screen = Screen::Group(group);
        self.selection = self.first_in(group.sections()[0]);
        self.message = None;
        self.needs_new_screen = true;
    }

    fn show_hub(&mut self) {
        self.screen = Screen::Hub;
        self.message = None;
        self.needs_new_screen = true;
    }

    /// Opens a section by its English name, for capture runs.
    pub fn open_section_named(&mut self, name: &str) {
        let section = match name {
            "campaign" => MenuSection::Campaign,
            "custom" => MenuSection::Custom,
            "quick" => MenuSection::Quick,
            "generator" => MenuSection::Generated,
            "random" => MenuSection::Random,
            "editor" => MenuSection::Editor,
            "network" => MenuSection::Network,
            "replays" => MenuSection::Replays,
            _ => return,
        };
        let group = MenuGroup::holding(section);
        if self.screen != Screen::Group(group) {
            self.screen = Screen::Group(group);
            self.needs_new_screen = true;
        }
        if self.section() != section || matches!(self.selection, Selection::Nothing(_)) {
            self.selection = self.first_in(section);
            self.needs_rebuild = true;
        }
    }

    /// The level whose map the page shows, if it has one.
    fn previewed_level(&self) -> Option<LevelDefinition> {
        match self.selection {
            Selection::Level(index) => Some(self.entries[index].level.clone()),
            Selection::Mission { campaign, mission } => {
                Some(self.campaigns[campaign].mission_at(mission, self.campaign_difficulty))
            }
            Selection::Random => Some(self.random_level()),
            Selection::Generated => Some(self.generated_mission()),
            Selection::Editor => Some(self.options.to_level()),
            Selection::Network
            | Selection::Replay(_)
            | Selection::Quick
            | Selection::Nothing(_) => None,
        }
    }

    fn update_preview(&mut self, images: &mut Assets<Image>) -> Option<Handle<Image>> {
        let level = self.previewed_level()?;
        let current = self
            .preview
            .as_ref()
            .filter(|(shown, _)| *shown == level)
            .map(|(_, image)| image.clone());
        Some(current.unwrap_or_else(|| {
            let image = images.add(map_preview(&level));
            self.preview = Some((level, image.clone()));
            image
        }))
    }

    fn random_level(&self) -> LevelDefinition {
        random_battle::generate(&self.random, self.random_seed)
    }

    fn generated_mission(&self) -> LevelDefinition {
        generate_mission(self.random_seed, self.quick_faction, self.quick_difficulty)
    }

    /// The battle the editor-like pages describe right now.
    /// The selected custom level and its file, or else the battle on the editor page, which has no file yet.
    fn level_to_edit(&self) -> (LevelDefinition, Option<std::path::PathBuf>) {
        let Selection::Level(index) = self.selection else {
            return (self.level_on_page(), None);
        };
        let entry = &self.entries[index];
        let file = match &entry.source {
            LevelSource::Custom(path) => Some(path.clone()),
            LevelSource::Campaign(_) | LevelSource::Skirmish => None,
        };
        (entry.level.clone(), file)
    }

    fn level_on_page(&self) -> LevelDefinition {
        match self.selection {
            Selection::Random => self.random_level(),
            Selection::Generated => self.generated_mission(),
            _ => self.options.to_level(),
        }
    }

    pub fn show_network_page(&mut self) {
        if self.screen != Screen::Group(MenuGroup::Multiplayer) {
            self.screen = Screen::Group(MenuGroup::Multiplayer);
            self.needs_new_screen = true;
        }
        if self.selection != Selection::Network {
            self.selection = Selection::Network;
            self.needs_rebuild = true;
        }
    }
}

fn rebuild_details(
    commands: &mut Commands,
    panel: Entity,
    fonts: &UiFonts,
    menu: &MainMenu,
    preview: Option<Handle<Image>>,
    join_address: &str,
    lobby: &NetLobby,
) {
    commands.entity(panel).despawn_related::<Children>();
    commands
        .entity(panel)
        .with_children(|panel| match menu.selection {
            Selection::Mission { campaign, mission } => {
                spawn_mission_briefing(panel, fonts, menu, campaign, mission, preview.clone())
            }
            Selection::Level(index) => {
                let level = &menu.entries[index].level;
                panel.spawn(fonts.heading(tr_owned(&level.name), 30.0, ACCENT));
                beside_preview(panel, preview, LARGE_PREVIEW, |column| {
                    column.spawn(fonts.text(tr_owned(&level.briefing), 17.0, TEXT_MAIN));
                    column.spawn(Node {
                        height: Val::Px(4.0),
                        ..default()
                    });
                    spawn_level_facts(column, fonts, level);
                });
                spawn_command_button(panel, fonts, tr("START MISSION"), MenuCommand::StartSelected);
                if matches!(menu.entries[index].source, LevelSource::Custom(_)) {
                    spawn_command_button(panel, fonts, tr("EDIT MAP"), MenuCommand::EditMap);
                    spawn_command_button(
                        panel,
                        fonts,
                        tr("ADD TO MY CAMPAIGN"),
                        MenuCommand::AddToMyCampaign,
                    );
                }
            }
            Selection::Replay(index) => {
                let replay = &menu.replays[index];
                panel.spawn(fonts.heading(tr_owned(&replay.title), 30.0, ACCENT));
                panel.spawn(fonts.text(
                    format!("{}  ·  {}", tr_owned(&replay.outcome), minutes(replay.recorded_secs)),
                    17.0,
                    TEXT_MAIN,
                ));
                panel.spawn(fonts.text(
                    tr(
                        "Watch the whole battle again without the fog of war. Space pauses, + and - change the speed, Esc returns here.",
                    ),
                    15.0,
                    TEXT_DIM,
                ));
                spawn_command_button(panel, fonts, tr("WATCH REPLAY"), MenuCommand::WatchReplay);
            }
            Selection::Network => {
                spawn_network_details(panel, fonts, join_address, lobby, &menu.options)
            }
            Selection::Random => spawn_random_battle_details(panel, fonts, menu, preview),
            Selection::Quick => spawn_quick_battle(panel, fonts, menu),
            Selection::Generated => spawn_generated_mission(panel, fonts, menu, preview),
            Selection::Nothing(section) => {
                panel.spawn(fonts.heading(section.label(), 30.0, ACCENT));
                let hint = match section {
                    MenuSection::Custom => tr(
                        "Levels saved from the battle editor or the map editor appear here. Open the battle editor, set up a battle and save it, or edit its map.",
                    ),
                    _ => tr("Finish or leave a battle and its replay appears here."),
                };
                panel.spawn(fonts.text(hint, 16.0, TEXT_DIM));
            }
            Selection::Editor => {
                panel.spawn(fonts.heading(tr("Battle editor"), 30.0, ACCENT));
                panel.spawn(fonts.text(
                    tr("Left click: next value, right click: previous."),
                    15.0,
                    TEXT_DIM,
                ));
                spawn_editor_fields(panel, fonts, &menu.options, &EDITOR_FIELDS);
                beside_preview(panel, preview, SMALL_PREVIEW, |column| {
                    column.spawn(fonts.text(
                        menu.options.to_level().objective.describe(),
                        16.0,
                        TEXT_DIM,
                    ));
                });
                spawn_command_button(panel, fonts, tr("START BATTLE"), MenuCommand::StartEditorBattle);
                spawn_command_button(
                    panel,
                    fonts,
                    tr("SAVE AS CUSTOM LEVEL"),
                    MenuCommand::SaveEditorLevel,
                );
                spawn_command_button(panel, fonts, tr("EDIT MAP"), MenuCommand::EditMap);
            }
        });
    if let Some(message) = &menu.message {
        commands.entity(panel).with_children(|panel| {
            panel.spawn(fonts.text(message.clone(), 15.0, GOOD));
        });
    }
}

/// Lays `content` out in a column with the map picture to its right.
fn beside_preview(
    panel: &mut ChildSpawnerCommands,
    preview: Option<Handle<Image>>,
    size: f32,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    panel
        .spawn(Node {
            column_gap: Val::Px(16.0),
            align_items: AlignItems::FlexStart,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(content);
            if let Some(image) = preview {
                row.spawn((
                    ImageNode::new(image),
                    Node {
                        width: Val::Px(size),
                        height: Val::Px(size),
                        flex_shrink: 0.0,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(FRAME),
                ));
            }
        });
}

fn select_entry(click: On<PointerClick>, buttons: Query<&ListButton>, mut menu: ResMut<MainMenu>) {
    if let Ok(button) = buttons.get(click.entity) {
        menu.selection = button.0;
        menu.message = None;
        menu.needs_rebuild = true;
    }
}

pub fn refresh_main_menu(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    settings: Res<GameSettings>,
    mut lobby: ResMut<NetLobby>,
    menu: Option<ResMut<MainMenu>>,
    mut images: ResMut<Assets<Image>>,
    lists: Query<Entity, With<LevelList>>,
    mut list_columns: Query<&mut Node, With<ListColumn>>,
    mut sections: Query<(
        &SectionButton,
        &Hovered,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    panels: Query<Entity, With<DetailPanel>>,
    mut hub_tiles: Query<
        (&Hovered, &mut BackgroundColor, &mut BorderColor),
        (
            With<HubTile>,
            Without<SectionButton>,
            Without<ListButton>,
            Without<MenuCommand>,
            Without<EditorOption>,
        ),
    >,
    mut buttons: Query<
        (
            &Hovered,
            &mut BackgroundColor,
            Option<&ListButton>,
            Option<&MenuCommand>,
        ),
        (
            Or<(With<ListButton>, With<MenuCommand>, With<EditorOption>)>,
            Without<SectionButton>,
        ),
    >,
) {
    let Some(mut menu) = menu else { return };
    if lobby.take_change() && menu.selection == Selection::Network {
        menu.needs_rebuild = true;
    }
    if menu.needs_rebuild
        && let (Ok(list), Ok(panel)) = (lists.single(), panels.single())
    {
        menu.needs_rebuild = false;
        let shown = if menu.section().has_list() {
            Display::Flex
        } else {
            Display::None
        };
        for mut column in &mut list_columns {
            set_field(&mut column, |node| &mut node.display, shown);
        }
        rebuild_level_list(&mut commands, list, &fonts, &menu);
        let preview = menu.update_preview(&mut images);
        rebuild_details(
            &mut commands,
            panel,
            &fonts,
            &menu,
            preview,
            &settings.join_address,
            &lobby,
        );
    }
    for (hovered, mut background, mut border) in &mut hub_tiles {
        background.set_if_neq(BackgroundColor(if hovered.get() {
            BUTTON_HOVERED
        } else {
            BUTTON_IDLE
        }));
        border.set_if_neq(BorderColor::all(if hovered.get() { ACCENT } else { FRAME }));
    }
    let open = menu.section();
    for (section, hovered, mut background, mut border) in &mut sections {
        let selected = section.0 == open;
        background.set_if_neq(BackgroundColor(if selected {
            BUTTON_SELECTED
        } else if hovered.get() {
            BUTTON_HOVERED
        } else {
            BUTTON_IDLE
        }));
        border.set_if_neq(BorderColor::all(if selected { ACCENT } else { FRAME }));
    }
    for (hovered, mut background, list_button, command) in &mut buttons {
        let selected = list_button.is_some_and(|button| button.0 == menu.selection);
        let primary = command.is_some_and(|command| command.is_primary());
        background.set_if_neq(BackgroundColor(match (selected, primary, hovered.get()) {
            (true, ..) => BUTTON_SELECTED,
            (false, true, true) => PRIMARY_HOVERED,
            (false, true, false) => PRIMARY_IDLE,
            (false, false, true) => BUTTON_HOVERED,
            (false, false, false) => BUTTON_IDLE,
        }));
    }
}
