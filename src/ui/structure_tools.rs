use bevy::picking::hover::Hovered;
use bevy::prelude::*;

use crate::app::i18n::{tr, tr_fmt};
use crate::app::settings::GameSettings;
use crate::sim::catalog::Team;
use crate::sim::components::{Building, Health};
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::determinism::SimId;
use crate::sim::player_commands::LocalTeam;
use crate::sim::player_commands::{CommandIssuer, PlayerCommand};
use crate::sim::structures::{Repairing, StructureOrderKind, can_sell, repair_cost, sale_refund};
use crate::ui::key_bindings::{Action, KeyBindings};
use crate::ui::sidebar::{ACCENT, FRAME, GOOD, TEXT_DIM, TEXT_MAIN, UiFonts, set_field};
use crate::view::camera::Cursor;
use crate::view::local_view::LocalView;
use crate::world::map::{GameMap, footprint_center, world_to_cell};

const BUTTON_IDLE: Color = Color::srgb(0.13, 0.15, 0.17);
const BUTTON_HOVERED: Color = Color::srgb(0.18, 0.21, 0.24);
const BUTTON_ACTIVE: Color = Color::srgb(0.30, 0.24, 0.08);
const HINT_OFFSET: Vec2 = Vec2::new(18.0, 14.0);
const REPAIR_MARKER_COLOR: Color = Color::srgb(0.35, 1.0, 0.45);

/// The repair or sell mode of the mouse; while one is active, left clicks on own structures apply it.
#[derive(Resource, Default)]
pub struct StructureTool(pub Option<StructureOrderKind>);

#[derive(Component)]
pub struct ToolButton(StructureOrderKind);

#[derive(Component)]
pub struct ToolHint;

#[derive(Component)]
pub struct ToolLabel(StructureOrderKind);

fn hotkey(tool: StructureOrderKind) -> Action {
    match tool {
        StructureOrderKind::ToggleRepair => Action::RepairTool,
        StructureOrderKind::Sell => Action::SellTool,
    }
}

fn label(tool: StructureOrderKind, bindings: &KeyBindings) -> String {
    let template = match tool {
        StructureOrderKind::ToggleRepair => "REPAIR  {}",
        StructureOrderKind::Sell => "SELL  {}",
    };
    tr_fmt(template, &[&bindings.label(hotkey(tool))])
}

pub fn label_tool_buttons(
    settings: Res<GameSettings>,
    mut labels: Query<(Ref<ToolLabel>, &mut Text)>,
) {
    for (label_of, mut text) in &mut labels {
        if settings.is_changed() || label_of.is_added() {
            text.0 = label(label_of.0, &settings.keys);
        }
    }
}

pub fn spawn_tool_buttons(panel: &mut ChildSpawnerCommands, fonts: &UiFonts) {
    panel
        .spawn(Node {
            flex_grow: 1.0,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            for tool in [StructureOrderKind::ToggleRepair, StructureOrderKind::Sell] {
                row.spawn((
                    ToolButton(tool),
                    Hovered::default(),
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                    BorderColor::all(FRAME),
                ))
                .with_children(|button| {
                    button.spawn((ToolLabel(tool), fonts.heading("", 13.0, TEXT_MAIN)));
                })
                .observe(
                    move |_: On<PointerClick>,
                          mut active: ResMut<StructureTool>,
                          mut cues: MessageWriter<SoundCue>| {
                        toggle(&mut active, tool);
                        cues.write(SoundCue::interface(Sound::UiClick));
                    },
                );
            }
        });
}

/// Every match starts with the mouse in its normal mode and a hidden hint ready to follow it.
pub fn prepare_structure_tools(mut commands: Commands, fonts: Res<UiFonts>) {
    commands.insert_resource(StructureTool::default());
    commands.spawn((
        ToolHint,
        fonts.heading("", 15.0, ACCENT),
        TextShadow::default(),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn toggle(active: &mut StructureTool, tool: StructureOrderKind) {
    active.0 = if active.0 == Some(tool) {
        None
    } else {
        Some(tool)
    };
}

pub fn switch_tools_by_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    settings: Res<GameSettings>,
    mut active: ResMut<StructureTool>,
) {
    for tool in [StructureOrderKind::ToggleRepair, StructureOrderKind::Sell] {
        if settings.keys.just_pressed(&keys, hotkey(tool)) {
            toggle(&mut active, tool);
        }
    }
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        active.0 = None;
    }
}

fn own_structure_under(
    map: &GameMap,
    cursor: &Cursor,
    structures: &Query<(&Building, &Team, &Health)>,
    team: Team,
) -> Option<Entity> {
    let ground = cursor.ground.filter(|_| !cursor.over_interface)?;
    let entity = map.building_at(world_to_cell(ground))?;
    let (_, &owner, _) = structures.get(entity).ok()?;
    (owner == team).then_some(entity)
}

pub fn apply_structure_tool(
    local: Res<LocalTeam>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<Cursor>,
    map: Res<GameMap>,
    active: Res<StructureTool>,
    structures: Query<(&Building, &Team, &Health)>,
    sim_ids: Query<&SimId>,
    mut issuer: CommandIssuer,
    mut cues: MessageWriter<SoundCue>,
) {
    let Some(tool) = active.0 else { return };
    if !mouse.just_pressed(MouseButton::Left) || cursor.over_interface {
        return;
    }
    let Some(&building) = own_structure_under(&map, &cursor, &structures, local.0)
        .and_then(|entity| sim_ids.get(entity).ok())
    else {
        cues.write(SoundCue::interface(Sound::UiError));
        return;
    };
    issuer.issue(PlayerCommand::OrderStructure {
        building,
        kind: tool,
    });
    cues.write(SoundCue::interface(Sound::UiClick));
}

pub fn refresh_tool_buttons(
    active: Res<StructureTool>,
    mut buttons: Query<(
        &ToolButton,
        &Hovered,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (button, hovered, mut background, mut border) in &mut buttons {
        let selected = active.0 == Some(button.0);
        background.0 = match (selected, hovered.get()) {
            (true, _) => BUTTON_ACTIVE,
            (false, true) => BUTTON_HOVERED,
            (false, false) => BUTTON_IDLE,
        };
        *border = BorderColor::all(if selected { ACCENT } else { FRAME });
    }
}

/// Follows the pointer and tells what a click would do to the structure under it.
pub fn refresh_tool_hint(
    local: Res<LocalTeam>,
    active: Res<StructureTool>,
    cursor: Res<Cursor>,
    map: Res<GameMap>,
    structures: Query<(&Building, &Team, &Health)>,
    mut hints: Query<(&mut Text, &mut TextColor, &mut Node, &mut Visibility), With<ToolHint>>,
) {
    let Ok((mut text, mut color, mut node, mut visibility)) = hints.single_mut() else {
        return;
    };
    let (Some(tool), Some(screen)) = (active.0, cursor.screen) else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };
    visibility.set_if_neq(Visibility::Inherited);
    set_field(
        &mut node,
        |node| &mut node.left,
        Val::Px(screen.x + HINT_OFFSET.x),
    );
    set_field(
        &mut node,
        |node| &mut node.top,
        Val::Px(screen.y + HINT_OFFSET.y),
    );

    let target = own_structure_under(&map, &cursor, &structures, local.0)
        .and_then(|entity| structures.get(entity).ok());
    let (message, tone) = match (tool, target) {
        (StructureOrderKind::ToggleRepair, None) => {
            (tr("Repair: pick a structure").to_string(), TEXT_DIM)
        }
        (StructureOrderKind::Sell, None) => (tr("Sell: pick a structure").to_string(), TEXT_DIM),
        (StructureOrderKind::ToggleRepair, Some((building, _, health))) => {
            let cost = repair_cost(building.kind, health);
            if cost == 0 {
                (tr("Undamaged").to_string(), TEXT_DIM)
            } else {
                (tr_fmt("Repair  ${}", &[&cost]), GOOD)
            }
        }
        (StructureOrderKind::Sell, Some((building, ..))) if can_sell(building.kind) => {
            (tr_fmt("Sell  +${}", &[&sale_refund(building.kind)]), ACCENT)
        }
        (StructureOrderKind::Sell, Some(_)) => (tr("Cannot be sold").to_string(), TEXT_DIM),
    };
    text.set_if_neq(Text(message));
    color.set_if_neq(TextColor(tone));
}

/// A slowly turning green cross above every structure being repaired that the player can see.
pub fn draw_repair_markers(
    mut gizmos: Gizmos,
    time: Res<Time>,
    view: LocalView,
    repairing: Query<&Building, With<Repairing>>,
) {
    let spin = Quat::from_rotation_y(time.elapsed_secs() * 1.5);
    for building in &repairing {
        let footprint = building.kind.spec().footprint;
        let center = footprint_center(building.origin, footprint);
        if !view.sight().is_visible(center) {
            continue;
        }
        let above = center + Vec3::Y * (4.0 + footprint.max_element() as f32);
        for arm in [Vec3::X, Vec3::Z] {
            let reach = spin * arm * 0.9;
            gizmos.line(above - reach, above + reach, REPAIR_MARKER_COLOR);
        }
        gizmos.line(
            above - Vec3::Y * 0.9,
            above + Vec3::Y * 0.9,
            REPAIR_MARKER_COLOR,
        );
    }
}
