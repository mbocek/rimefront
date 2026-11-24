use crate::app::i18n::{tr, tr_fmt, tr_owned};
use crate::app::replay::{ReplayPlayback, list_replays, load_replay, replay_directory};
use crate::missions::campaigns::{CampaignLibrary, CampaignRun, find_campaign};
use crate::missions::level_events::MissionMessage;
use crate::sim::abilities::MINE_COST;
use crate::sim::match_stats::{MatchStats, TeamTally};
use crate::sim::player_commands::LocalTeam;
use crate::sim::player_commands::{CommandIssuer, PlayerCommand};
use crate::sim::research::Upgrade;
use crate::ui::key_bindings::{Action, KeyBindings};
use crate::ui::structure_tools::spawn_tool_buttons;
use crate::view::local_view::LocalView;
use crate::view::sky::{RAINING, WeatherCycle};
use crate::world::season::Season;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};

use crate::app::match_flow::{GameState, MatchClock};
use crate::app::settings::GameSettings;
use crate::missions::levels::{ActiveLevel, LevelDefinition};
use crate::net::network::NetSession;
use crate::sim::catalog::{BuildingKind, Faction, HARVESTER_CAPACITY, Team, UnitKind};
use crate::sim::components::*;
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::economy::Economies;
use crate::sim::notifications::Notifications;
use crate::sim::outcome::{GameOutcome, objective_status};
use crate::sim::production::{BuildItem, EnqueueRefusal, Lane, Production, owned_building_kinds};
use crate::ui::control_groups::spawn_group_bar;
use crate::ui::menu::{MenuPlace, MenuState, open_menu_from_button, spawn_menu};
use crate::ui::minimap::{Minimap, jump_camera_on_drag, jump_camera_on_press, order_on_press};
use crate::ui::network_hud::{spawn_chat_line, spawn_network_hud};
use crate::ui::perf_overlay::spawn_performance_overlay;
use crate::ui::placement::Placement;
use crate::ui::queue_bar::spawn_queue_bar;

mod build_menu;
mod controls_help;
mod info_panel;
mod mission_status;
mod outcome;
mod resources;

pub use build_menu::*;
pub use controls_help::*;
pub use info_panel::*;
pub use mission_status::*;
pub use outcome::*;
pub use resources::*;

pub const SIDEBAR_WIDTH: f32 = 300.0;

pub(crate) const PANEL: Color = Color::srgb(0.065, 0.075, 0.085);
const PANEL_RAISED: Color = Color::srgb(0.10, 0.115, 0.13);
pub(crate) const FRAME: Color = Color::srgb(0.22, 0.25, 0.28);
pub(crate) const FRAME_BRIGHT: Color = Color::srgb(0.38, 0.43, 0.47);
pub(crate) const TEXT_MAIN: Color = Color::srgb(0.90, 0.92, 0.93);
pub(crate) const TEXT_DIM: Color = Color::srgb(0.52, 0.56, 0.60);
pub(crate) const ACCENT: Color = Color::srgb(0.96, 0.74, 0.26);
pub(crate) const GOOD: Color = Color::srgb(0.35, 0.86, 0.42);
const WARNING: Color = Color::srgb(0.95, 0.78, 0.22);
const DANGER: Color = Color::srgb(1.0, 0.33, 0.28);
const PRIMARY_BUTTON: Color = Color::srgb(0.62, 0.45, 0.10);
const MINIMAP_SIZE: f32 = 236.0;
#[derive(Resource)]
pub struct UiFonts {
    regular: Handle<Font>,
    bold: Handle<Font>,
}

impl UiFonts {
    pub(crate) fn text(&self, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (
            Text::new(text),
            TextFont {
                font: FontSource::Handle(self.regular.clone()),
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(color),
        )
    }

    pub(crate) fn heading(&self, text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
        (
            Text::new(text),
            TextFont {
                font: FontSource::Handle(self.bold.clone()),
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(color),
        )
    }
}

pub(crate) fn icon_path(item: BuildItem) -> String {
    let model_id = match item {
        BuildItem::Structure(kind) => kind.spec().model_id,
        BuildItem::Unit(kind) => kind.spec().model_id,
        BuildItem::Research(upgrade) => upgrade.spec().icon_id,
    };
    format!("icons/{model_id}.png")
}

fn framed(width: Val, height: Val) -> Node {
    Node {
        width,
        height,
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    }
}

pub fn load_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(UiFonts {
        regular: asset_server.load("fonts/Rajdhani-SemiBold.ttf"),
        bold: asset_server.load("fonts/Rajdhani-Bold.ttf"),
    });
}

pub fn build_interface(
    local: Res<LocalTeam>,
    view: LocalView,
    production: Res<Production>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    fonts: Res<UiFonts>,
    minimap: Res<Minimap>,
) {
    let fonts = &*fonts;

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(SIDEBAR_WIDTH),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::left(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(FRAME_BRIGHT),
        ))
        .with_children(|sidebar| {
            let faction = production.faction_of(local.0);
            let title = if view.is_spectating() {
                tr("SPECTATOR")
            } else {
                tr(faction.title())
            };
            spawn_header(sidebar, fonts, title, local.0);
            spawn_minimap(sidebar, &minimap);
            if !view.is_spectating() {
                spawn_resources(sidebar, fonts);
                spawn_objective(sidebar, fonts);
                spawn_tabs(sidebar, fonts);
                spawn_build_grid(sidebar, fonts, &asset_server, faction);
            }
            spawn_info_panel(sidebar, fonts);
        });

    spawn_notification_feed(&mut commands, fonts);
    spawn_build_tooltip(&mut commands, fonts);
    spawn_queue_bar(&mut commands, fonts);
    spawn_group_bar(&mut commands, fonts);
    spawn_performance_overlay(&mut commands, fonts);
    spawn_network_hud(&mut commands, fonts);
    spawn_chat_line(&mut commands, fonts);
    spawn_menu(&mut commands, fonts, MenuPlace::Match);
    spawn_controls_help(&mut commands, fonts);
    spawn_outcome_overlay(&mut commands, fonts);
    commands.insert_resource(ActiveTab(Lane::Structures));
}

fn spawn_header(
    sidebar: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    title: &'static str,
    team: Team,
) {
    sidebar
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|header| {
            header.spawn(fonts.heading("IRON TIDE", 30.0, ACCENT));
            header.spawn(fonts.heading(title, 16.0, team.ui_color()));
            header
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(TILE),
                    BorderColor::all(FRAME_BRIGHT),
                ))
                .with_children(|button| {
                    button.spawn(fonts.heading("MENU", 14.0, TEXT_MAIN));
                })
                .observe(|_: On<PointerClick>, menu: ResMut<MenuState>| {
                    open_menu_from_button(menu)
                });
        });
}

fn spawn_minimap(sidebar: &mut ChildSpawnerCommands, minimap: &Minimap) {
    sidebar
        .spawn((
            Node {
                align_self: AlignSelf::Center,
                ..framed(Val::Px(MINIMAP_SIZE + 2.0), Val::Px(MINIMAP_SIZE + 2.0))
            },
            BorderColor::all(FRAME_BRIGHT),
            BackgroundColor(Color::BLACK),
        ))
        .with_children(|frame| {
            frame
                .spawn((
                    ImageNode::new(minimap.image.clone()),
                    Node {
                        width: Val::Px(MINIMAP_SIZE),
                        height: Val::Px(MINIMAP_SIZE),
                        ..default()
                    },
                ))
                .observe(jump_camera_on_press)
                .observe(jump_camera_on_drag)
                .observe(order_on_press);
        });
}

/// Writes a field only when it differs: any write marks the whole component changed, and changed
/// interface nodes make Bevy measure text and lay out the interface again.
pub fn set_field<C, V: PartialEq>(
    component: &mut Mut<C>,
    field: impl FnOnce(&mut C) -> &mut V,
    value: V,
) {
    let slot = field(component.bypass_change_detection());
    if *slot != value {
        *slot = value;
        component.set_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_time_reads_minutes_and_seconds_and_adds_hours_past_one() {
        assert_eq!(match_time(0.0), "0:00");
        assert_eq!(match_time(754.9), "12:34");
        assert_eq!(match_time(3723.0), "1:02:03");
    }

    #[test]
    fn a_unit_tooltip_names_the_building_that_builds_it() {
        let item = BuildItem::Unit(UnitKind::Tank);
        let (_, details) = describe_item(item, &[], &Production::default(), Team::Player);
        let factory = UnitKind::Tank.spec().producer.spec().name;
        assert!(
            details.contains(&format!("Built at {factory}")),
            "{details}"
        );
    }
}
