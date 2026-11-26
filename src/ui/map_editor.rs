use std::path::PathBuf;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::app::settings::custom_levels_directory;

use crate::app::i18n::{tr, tr_fmt, tr_owned};
use crate::app::match_flow::GameState;
use crate::missions::event_editing::{
    ForcePreset, MOST_EVENTS, add_event, adjust_credits, adjust_trigger, cycle_action,
    cycle_action_side, cycle_corner, cycle_preset, cycle_trigger, cycle_trigger_side, main_action,
    message, remove_event, sides, tidy_events,
};
use crate::missions::level_editing::{
    LevelText, add_enemy, cycle_difficulty, cycle_objective, erase_from, move_side, remove_enemy,
    type_into,
};
use crate::missions::level_events::{EventAction, Trigger};
use crate::missions::levels::{ActiveLevel, LevelDefinition, save_custom_level};
use crate::sim::catalog::{BuildingKind, Team};
use crate::ui::sidebar::{ACCENT, FRAME, GOOD, PANEL, SIDEBAR_WIDTH, TEXT_DIM, TEXT_MAIN, UiFonts};
use crate::view::camera::Cursor;
use crate::view::terrain_chunks::{GroundPlan, TerrainChunk, resoil_terrain_chunks};
use crate::world::map::{
    GameMap, cell_center, cells_by_distance, footprint_center, opposite_cell, world_to_cell,
};
use crate::world::map_edits::{Brush, Ground, MapEdit, MapPainter};

const BRUSH_SIZES: [i32; 3] = [0, 1, 2];
const BRUSH_KEYS: [KeyCode; 4] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
];
const BUTTON_IDLE: Color = Color::srgb(0.11, 0.13, 0.15);
const BUTTON_ACTIVE: Color = Color::srgb(0.17, 0.24, 0.32);

/// The open map editor: the level being repainted, where it is saved and how the brush is set.
#[derive(Resource)]
pub struct MapEditor {
    pub brush: Brush,
    /// Cells painted around the one under the cursor; zero paints a single cell.
    pub radius: i32,
    /// Paint the opposite cell too, so both halves of the map stay equally fair.
    pub mirror: bool,
    /// Where the level is saved; none until the first save creates a new custom level file.
    pub file: Option<PathBuf>,
    pub message: Option<String>,
    pub page: EditorPage,
    /// The text the keyboard writes into; brush keys and camera keys rest meanwhile.
    pub typing: Option<LevelText>,
    /// Which of the level's events the EVENTS page shows; the narrow panel edits one at a time.
    pub selected_event: usize,
    panel_outdated: bool,
}

impl MapEditor {
    pub fn new(file: Option<PathBuf>) -> MapEditor {
        MapEditor {
            brush: Brush::Crystal,
            radius: 1,
            mirror: true,
            file,
            message: None,
            page: EditorPage::Paint,
            typing: None,
            selected_event: 0,
            panel_outdated: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorPage {
    Paint,
    Level,
    Events,
}

/// Opens the map editor on a level; `file` is the custom level it overwrites when saved.
pub fn open_map_editor(
    commands: &mut Commands,
    next_state: &mut NextState<GameState>,
    level: LevelDefinition,
    file: Option<PathBuf>,
) {
    commands.insert_resource(ActiveLevel(level));
    commands.insert_resource(MapEditor::new(file));
    next_state.set(GameState::MapEditor);
}

#[derive(Component, Clone, Copy)]
pub enum EditorButton {
    Page(EditorPage),
    Brush(Brush),
    Radius(i32),
    Mirror,
    Type(LevelText),
    Objective,
    MoveSide(usize),
    Difficulty(usize),
    AddEnemy,
    RemoveEnemy,
    Faction,
    StepEvent(i32),
    AddEvent,
    DeleteEvent,
    EventTrigger,
    EventTriggerNumber(i32),
    EventTriggerSide,
    EventAction,
    EventActionSide,
    EventPreset,
    EventCorner,
    EventCredits(i32),
    PlayTest,
    Save,
    Exit,
}

#[derive(Component)]
pub struct MapEditorPanel;

/// Builds the panel again after anything on it changed; it is a few dozen nodes.
pub fn rebuild_map_editor_panel(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    level: Res<ActiveLevel>,
    mut editor: ResMut<MapEditor>,
    panels: Query<Entity, With<MapEditorPanel>>,
) {
    if !editor.panel_outdated {
        return;
    }
    editor.panel_outdated = false;
    for panel in &panels {
        commands.entity(panel).despawn();
    }
    commands
        .spawn((
            MapEditorPanel,
            DespawnOnExit(GameState::MapEditor),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(SIDEBAR_WIDTH),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(7.0),
                border: UiRect::left(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor::all(FRAME),
        ))
        .with_children(|panel| {
            panel.spawn(fonts.heading(tr("MAP EDITOR"), 24.0, ACCENT));
            row(panel, |row| {
                for (page, label) in [
                    (EditorPage::Paint, tr("PAINT")),
                    (EditorPage::Level, tr("LEVEL")),
                    (EditorPage::Events, tr("EVENTS")),
                ] {
                    spawn_button(
                        row,
                        &fonts,
                        label,
                        EditorButton::Page(page),
                        editor.page == page,
                    );
                }
            });
            match editor.page {
                EditorPage::Paint => spawn_paint_page(panel, &fonts, &editor),
                EditorPage::Level => spawn_level_page(panel, &fonts, &level.0, editor.typing),
                EditorPage::Events => spawn_events_page(panel, &fonts, &level.0, &editor),
            }
            spawn_button(panel, &fonts, tr("PLAY IT"), EditorButton::PlayTest, false);
            spawn_button(panel, &fonts, tr("SAVE"), EditorButton::Save, false);
            spawn_button(panel, &fonts, tr("BACK TO MENU"), EditorButton::Exit, false);
            if let Some(message) = &editor.message {
                panel.spawn(fonts.text(message.clone(), 14.0, GOOD));
            }
        });
}

fn spawn_paint_page(panel: &mut ChildSpawnerCommands, fonts: &UiFonts, editor: &MapEditor) {
    panel.spawn(fonts.heading(tr("BRUSH"), 15.0, TEXT_DIM));
    for (index, brush) in Brush::ALL.into_iter().enumerate() {
        let label = format!("{}  {}", index + 1, tr(brush.name()));
        spawn_button(
            panel,
            fonts,
            &label,
            EditorButton::Brush(brush),
            editor.brush == brush,
        );
    }
    panel.spawn(fonts.heading(tr("SIZE"), 15.0, TEXT_DIM));
    row(panel, |row| {
        for radius in BRUSH_SIZES {
            let label = (2 * radius + 1).to_string();
            spawn_button(
                row,
                fonts,
                &label,
                EditorButton::Radius(radius),
                editor.radius == radius,
            );
        }
    });
    let mirror = if editor.mirror {
        tr("Mirror: on")
    } else {
        tr("Mirror: off")
    };
    spawn_button(panel, fonts, mirror, EditorButton::Mirror, editor.mirror);
    panel.spawn(fonts.text(
        tr("Left button paints, right button clears. Bases stay open. Mirroring keeps both halves of the map equally fair."),
        14.0,
        TEXT_DIM,
    ));
}

fn spawn_level_page(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    level: &LevelDefinition,
    typing: Option<LevelText>,
) {
    for (target, label, text) in [
        (LevelText::Name, tr("Name"), &level.name),
        (LevelText::Briefing, tr("Briefing"), &level.briefing),
    ] {
        panel.spawn(fonts.heading(label, 15.0, TEXT_DIM));
        let cursor = if typing == Some(target) { "_" } else { "" };
        spawn_button(
            panel,
            fonts,
            &format!("{text}{cursor}"),
            EditorButton::Type(target),
            typing == Some(target),
        );
    }
    panel.spawn(fonts.heading(tr("Objective"), 15.0, TEXT_DIM));
    spawn_button(
        panel,
        fonts,
        &level.objective.describe(),
        EditorButton::Objective,
        false,
    );
    panel.spawn(fonts.heading(tr("SIDES"), 15.0, TEXT_DIM));
    let faction = tr_owned(&Team::Azure.name_in(level.player_faction));
    let faction_label = tr_fmt("Your faction: {}", &[&faction]);
    spawn_button(panel, fonts, &faction_label, EditorButton::Faction, false);
    let sides = std::iter::once((tr("You").to_string(), level.player.site, None)).chain(
        level.enemies.iter().enumerate().map(|(index, enemy)| {
            let name = tr_fmt("Computer {}", &[&(index + 1)]);
            (name, enemy.force.site, Some((index, enemy.difficulty)))
        }),
    );
    for (side, (name, site, difficulty)) in sides.enumerate() {
        row(panel, |row| {
            row.spawn(fonts.text(name, 15.0, Team::ALL[side].ui_color()));
            spawn_button(row, fonts, site.name(), EditorButton::MoveSide(side), false);
            if let Some((enemy, difficulty)) = difficulty {
                let label = tr_owned(&format!("{difficulty:?}"));
                spawn_button(row, fonts, &label, EditorButton::Difficulty(enemy), false);
            }
        });
    }
    row(panel, |row| {
        spawn_button(row, fonts, tr("+ COMPUTER"), EditorButton::AddEnemy, false);
        spawn_button(
            row,
            fonts,
            tr("- COMPUTER"),
            EditorButton::RemoveEnemy,
            false,
        );
    });
}

fn side_name(level: &LevelDefinition, team: Team) -> String {
    match sides(level).iter().position(|&side| side == team) {
        Some(0) => tr("You").to_string(),
        Some(index) => tr_fmt("Computer {}", &[&index]),
        None => tr_owned(&team.name()),
    }
}

fn trigger_label(level: &LevelDefinition, trigger: Trigger) -> (String, String) {
    match trigger {
        Trigger::AfterMinutes { minutes } => (tr("After minutes").into(), format!("{minutes:.0}")),
        Trigger::StructuresLeft { team, at_most } => (
            tr_fmt("{}: structures left", &[&side_name(level, team)]),
            at_most.to_string(),
        ),
        Trigger::UnitsLeft { team, at_most } => (
            tr_fmt("{}: units left", &[&side_name(level, team)]),
            at_most.to_string(),
        ),
    }
}

fn spawn_events_page(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    level: &LevelDefinition,
    editor: &MapEditor,
) {
    let count = level.events.len();
    let index = editor.selected_event.min(count.saturating_sub(1));
    row(panel, |row| {
        spawn_button(row, fonts, "<", EditorButton::StepEvent(-1), false);
        let shown = if count == 0 {
            tr("No events").to_string()
        } else {
            tr_fmt("Event {} of {}", &[&(index + 1), &count])
        };
        row.spawn(fonts.heading(shown, 15.0, TEXT_MAIN));
        spawn_button(row, fonts, ">", EditorButton::StepEvent(1), false);
    });
    row(panel, |row| {
        if count < MOST_EVENTS {
            spawn_button(row, fonts, tr("+ EVENT"), EditorButton::AddEvent, false);
        }
        if count > 0 {
            spawn_button(row, fonts, tr("DELETE"), EditorButton::DeleteEvent, false);
        }
    });
    let Some(event) = level.events.get(index) else {
        panel.spawn(fonts.text(
            tr("An event happens once during the mission: after some minutes, or when a side is down to a few structures or units. It can show a message and send reinforcements or credits."),
            14.0,
            TEXT_DIM,
        ));
        return;
    };
    panel.spawn(fonts.heading(tr("WHEN"), 15.0, TEXT_DIM));
    let (trigger, number) = trigger_label(level, event.when);
    spawn_button(panel, fonts, &trigger, EditorButton::EventTrigger, false);
    row(panel, |row| {
        spawn_button(row, fonts, "-", EditorButton::EventTriggerNumber(-1), false);
        row.spawn(fonts.heading(number, 15.0, TEXT_MAIN));
        spawn_button(row, fonts, "+", EditorButton::EventTriggerNumber(1), false);
        if !matches!(event.when, Trigger::AfterMinutes { .. }) {
            spawn_button(
                row,
                fonts,
                tr("SIDE"),
                EditorButton::EventTriggerSide,
                false,
            );
        }
    });
    panel.spawn(fonts.heading(tr("MESSAGE"), 15.0, TEXT_DIM));
    let target = LevelText::EventMessage(index);
    let typing = editor.typing == Some(target);
    let written = message(event).unwrap_or_default();
    let shown = match (written.is_empty(), typing) {
        (true, false) => tr("(click to write)").to_string(),
        (_, true) => format!("{written}_"),
        (false, false) => written.to_string(),
    };
    spawn_button(panel, fonts, &shown, EditorButton::Type(target), typing);
    panel.spawn(fonts.heading(tr("THEN"), 15.0, TEXT_DIM));
    let action = main_action(event);
    let action_name = match action {
        Some(EventAction::Reinforcements { .. }) => tr("Reinforcements"),
        Some(EventAction::Credits { .. }) => tr("Credits"),
        _ => tr("Only the message"),
    };
    spawn_button(panel, fonts, action_name, EditorButton::EventAction, false);
    match action {
        Some(EventAction::Reinforcements { team, units, from }) => {
            let preset = ForcePreset::of(units, level.faction_of(*team)).map_or_else(
                || tr_fmt("{} units", &[&units.len()]),
                |p| tr(p.name()).to_string(),
            );
            spawn_button(
                panel,
                fonts,
                &tr_fmt("For: {}", &[&side_name(level, *team)]),
                EditorButton::EventActionSide,
                false,
            );
            spawn_button(panel, fonts, &preset, EditorButton::EventPreset, false);
            spawn_button(
                panel,
                fonts,
                &tr_fmt("From: {}", &[&from.name()]),
                EditorButton::EventCorner,
                false,
            );
        }
        Some(EventAction::Credits { team, amount }) => {
            spawn_button(
                panel,
                fonts,
                &tr_fmt("For: {}", &[&side_name(level, *team)]),
                EditorButton::EventActionSide,
                false,
            );
            row(panel, |row| {
                spawn_button(row, fonts, "-", EditorButton::EventCredits(-1), false);
                row.spawn(fonts.heading(format!("${amount}"), 15.0, TEXT_MAIN));
                spawn_button(row, fonts, "+", EditorButton::EventCredits(1), false);
            });
        }
        _ => {}
    }
}

fn row(parent: &mut ChildSpawnerCommands, children: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn(Node {
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(children);
}

fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: &str,
    button: EditorButton,
    active: bool,
) {
    parent
        .spawn((
            button,
            Node {
                min_height: Val::Px(28.0),
                min_width: Val::Px(44.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if active { BUTTON_ACTIVE } else { BUTTON_IDLE }),
            BorderColor::all(if active { ACCENT } else { FRAME }),
        ))
        .with_children(|button| {
            button.spawn(fonts.heading(label, 15.0, TEXT_MAIN));
        })
        .observe(press_editor_button);
}

fn press_editor_button(
    click: On<PointerClick>,
    buttons: Query<&EditorButton>,
    mut level: ResMut<ActiveLevel>,
    mut editor: ResMut<MapEditor>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(&button) = buttons.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Primary {
        return;
    }
    let level = &mut level.0;
    let clicked_text = matches!(button, EditorButton::Type(_));
    if !clicked_text {
        editor.typing = None;
    }
    match button {
        EditorButton::Page(page) => editor.page = page,
        EditorButton::Brush(brush) => editor.brush = brush,
        EditorButton::Radius(radius) => editor.radius = radius,
        EditorButton::Mirror => editor.mirror = !editor.mirror,
        EditorButton::Type(target) => editor.typing = Some(target),
        EditorButton::Objective => cycle_objective(level),
        EditorButton::MoveSide(side) => move_side(level, side),
        EditorButton::Difficulty(enemy) => cycle_difficulty(level, enemy),
        EditorButton::AddEnemy => {
            add_enemy(level);
        }
        EditorButton::RemoveEnemy => {
            remove_enemy(level);
        }
        EditorButton::Faction => level.player_faction = level.player_faction.opposite(),
        EditorButton::StepEvent(step) => {
            let last = level.events.len().saturating_sub(1);
            editor.selected_event = editor
                .selected_event
                .saturating_add_signed(step as isize)
                .min(last);
        }
        EditorButton::AddEvent => {
            if let Some(index) = add_event(level) {
                editor.selected_event = index;
            }
        }
        EditorButton::DeleteEvent => {
            remove_event(level, editor.selected_event);
            editor.selected_event = editor
                .selected_event
                .min(level.events.len().saturating_sub(1));
        }
        EditorButton::EventTrigger => cycle_trigger(level, editor.selected_event),
        EditorButton::EventTriggerNumber(step) => {
            adjust_trigger(level, editor.selected_event, step)
        }
        EditorButton::EventTriggerSide => cycle_trigger_side(level, editor.selected_event),
        EditorButton::EventAction => cycle_action(level, editor.selected_event),
        EditorButton::EventActionSide => cycle_action_side(level, editor.selected_event),
        EditorButton::EventPreset => cycle_preset(level, editor.selected_event),
        EditorButton::EventCorner => cycle_corner(level, editor.selected_event),
        EditorButton::EventCredits(step) => adjust_credits(level, editor.selected_event, step),
        EditorButton::PlayTest => {
            tidy_events(level);
            next_state.set(GameState::Playing);
        }
        EditorButton::Save => {
            tidy_events(level);
            save_edited_level(&mut editor, level);
        }
        EditorButton::Exit => next_state.set(GameState::MainMenu),
    }
    editor.panel_outdated = true;
}

/// While a text is being written the keyboard belongs to it: brush and camera keys see nothing.
pub fn type_level_text(
    mut typed: MessageReader<KeyboardInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut editor: ResMut<MapEditor>,
    mut level: ResMut<ActiveLevel>,
) {
    let Some(target) = editor.typing else {
        typed.clear();
        return;
    };
    for key in typed.read().filter(|key| key.state.is_pressed()) {
        match &key.logical_key {
            Key::Enter | Key::Escape => editor.typing = None,
            Key::Backspace => erase_from(&mut level.0, target),
            Key::Space => type_into(&mut level.0, target, " "),
            Key::Character(text) => type_into(&mut level.0, target, text),
            _ => {}
        }
        editor.panel_outdated = true;
    }
    keys.reset_all();
}

fn save_edited_level(editor: &mut MapEditor, level: &LevelDefinition) {
    let saved = match &editor.file {
        Some(path) => overwrite_level(path, level).map(|()| path.clone()),
        None => save_custom_level(&custom_levels_directory(), level),
    };
    editor.panel_outdated = true;
    editor.message = Some(match saved {
        Ok(path) => {
            let message = tr_fmt(
                "Saved as {} - it now appears under Custom levels.",
                &[&path.display()],
            );
            editor.file = Some(path);
            message
        }
        Err(error) => tr_fmt("Could not save the level: {}", &[&error]),
    });
}

fn overwrite_level(path: &std::path::Path, level: &LevelDefinition) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(level).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

pub fn leave_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::MainMenu);
    }
}

pub fn choose_brush_by_key(keys: Res<ButtonInput<KeyCode>>, mut editor: ResMut<MapEditor>) {
    if let Some(index) = BRUSH_KEYS.iter().position(|&key| keys.just_pressed(key)) {
        editor.brush = Brush::ALL[index];
        editor.panel_outdated = true;
    }
}

pub fn paint_under_cursor(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<Cursor>,
    editor: Res<MapEditor>,
    mut level: ResMut<ActiveLevel>,
    mut painter: MapPainter,
) {
    let brush = if mouse.pressed(MouseButton::Left) {
        editor.brush
    } else if mouse.pressed(MouseButton::Right) {
        Brush::Clear
    } else {
        return;
    };
    if cursor.over_interface {
        return;
    }
    let Some(ground) = cursor.ground else {
        return;
    };
    let ground_of_level = Ground::of(&level);
    for cell in brush_cells(world_to_cell(ground), editor.radius, editor.mirror) {
        let edit = MapEdit { cell, brush };
        let unchanged = level.0.map_edits.iter().rev().find(|e| e.cell == cell) == Some(&edit);
        if unchanged || !painter.paint(ground_of_level, edit) {
            continue;
        }
        level.0.map_edits.retain(|earlier| earlier.cell != cell);
        level.0.map_edits.push(edit);
    }
}

fn brush_cells(center: IVec2, radius: i32, mirror: bool) -> Vec<IVec2> {
    let reach = radius as f32 + 0.5;
    let mut cells: Vec<IVec2> = cells_by_distance(center, radius)
        .filter(|&cell| (cell - center).as_vec2().length() <= reach)
        .collect();
    if mirror {
        let mirrored: Vec<IVec2> = cells.iter().map(|&cell| opposite_cell(cell)).collect();
        cells.extend(
            mirrored
                .into_iter()
                .filter(|cell| GameMap::in_bounds(*cell)),
        );
        cells.dedup();
    }
    cells
}

/// Rebuilds the ground once a stroke ends, so painted or cleared crystals get or lose their soil.
pub fn resoil_after_stroke(
    mut commands: Commands,
    mut painted: Local<bool>,
    mouse: Res<ButtonInput<MouseButton>>,
    level: Res<ActiveLevel>,
    map: Res<GameMap>,
    plan: Option<Res<GroundPlan>>,
    mut chunks: Query<(Entity, &mut TerrainChunk)>,
) {
    *painted |= level.is_changed();
    let stroke_ended = mouse.any_just_released([MouseButton::Left, MouseButton::Right]);
    if !stroke_ended || !*painted {
        return;
    }
    *painted = false;
    resoil_terrain_chunks(&mut commands, &map, plan.as_deref(), &mut chunks);
}

pub fn draw_brush_outline(cursor: Res<Cursor>, editor: Res<MapEditor>, mut gizmos: Gizmos) {
    if cursor.over_interface {
        return;
    }
    let Some(ground) = cursor.ground else {
        return;
    };
    let color = if editor.brush == Brush::Clear {
        Color::srgb(1.0, 0.4, 0.3)
    } else {
        ACCENT
    };
    for cell in brush_cells(world_to_cell(ground), editor.radius, editor.mirror) {
        let center = cell_center(cell) + Vec3::Y * 0.15;
        gizmos.rect(
            Isometry3d::new(center, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Vec2::splat(1.8),
            color,
        );
    }
}

/// Rings where the bases stand, since nothing can be painted there.
pub fn mark_base_sites(level: Res<ActiveLevel>, mut gizmos: Gizmos) {
    let footprint = BuildingKind::CommandCenter.spec().footprint;
    let sides = std::iter::once((Team::Azure, level.0.player.site)).chain(
        level
            .0
            .opponents()
            .map(|(team, enemy)| (team, enemy.force.site)),
    );
    for (team, site) in sides {
        let center = footprint_center(site.origin(), footprint) + Vec3::Y * 0.3;
        gizmos.circle(
            Isometry3d::new(center, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            6.0,
            team.ui_color(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mirrored_brush_also_paints_the_opposite_cells() {
        let center = IVec2::new(10, 12);
        let cells = brush_cells(center, 0, true);
        assert_eq!(cells, vec![center, opposite_cell(center)]);
        assert_eq!(brush_cells(center, 1, false).len(), 9);
    }
}
