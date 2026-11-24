use bevy::prelude::*;

use crate::app::i18n::tr_fmt;
use crate::sim::catalog::Team;
use crate::sim::components::{Selected, Unit};
use crate::sim::notifications::Notifications;
use crate::sim::player_commands::LocalTeam;
use crate::ui::sidebar::{ACCENT, FRAME, PANEL, TEXT_DIM, TEXT_MAIN, UiFonts, set_field};
use crate::view::camera::RtsCamera;

const GROUP_COUNT: usize = 9;
/// A second press of the same group key within this time centers the camera on the group.
const DOUBLE_PRESS_SECS: f32 = 0.4;
const GROUP_KEYS: [KeyCode; GROUP_COUNT] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

#[derive(Resource, Default)]
pub struct ControlGroups {
    members: [Vec<Entity>; GROUP_COUNT],
    last_recall: Option<(usize, f32)>,
}

#[derive(Component)]
pub struct GroupBar;

#[derive(Component)]
pub struct GroupButton(usize);

enum GroupCommand {
    Assign,
    AddToSelection,
    Recall,
}

pub fn start_without_groups(mut commands: Commands) {
    commands.insert_resource(ControlGroups::default());
}

pub fn handle_group_keys(
    local: Res<LocalTeam>,
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut groups: ResMut<ControlGroups>,
    mut notifications: ResMut<Notifications>,
    selected: Query<Entity, (With<Selected>, With<Unit>)>,
    units: Query<(&Team, &Transform), With<Unit>>,
    mut cameras: Query<&mut RtsCamera>,
) {
    let Some(group) = GROUP_KEYS.iter().position(|&key| keys.just_pressed(key)) else {
        return;
    };
    let assign = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let additive = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let command = match (assign, additive) {
        (true, _) => GroupCommand::Assign,
        (false, true) => GroupCommand::AddToSelection,
        (false, false) => GroupCommand::Recall,
    };
    run_group_command(
        local.0,
        &mut commands,
        &time,
        &mut groups,
        &mut notifications,
        &selected,
        &units,
        &mut cameras,
        group,
        command,
    );
}

#[allow(clippy::too_many_arguments)]
fn run_group_command(
    team: Team,
    commands: &mut Commands,
    time: &Time,
    groups: &mut ControlGroups,
    notifications: &mut Notifications,
    selected: &Query<Entity, (With<Selected>, With<Unit>)>,
    units: &Query<(&Team, &Transform), With<Unit>>,
    cameras: &mut Query<&mut RtsCamera>,
    group: usize,
    command: GroupCommand,
) {
    let is_own_unit = |entity: &Entity| units.get(*entity).is_ok_and(|(owner, _)| *owner == team);
    groups.members[group].retain(is_own_unit);

    match command {
        GroupCommand::Assign => {
            groups.members[group] = selected.iter().filter(is_own_unit).collect();
            notifications.push(tr_fmt(
                "Group {} - {} units",
                &[&(group + 1), &groups.members[group].len()],
            ));
        }
        GroupCommand::AddToSelection => {
            for &entity in &groups.members[group] {
                commands.entity(entity).insert(Selected);
            }
        }
        GroupCommand::Recall => {
            if groups.members[group].is_empty() {
                return;
            }
            for entity in selected.iter() {
                commands.entity(entity).remove::<Selected>();
            }
            for &entity in &groups.members[group] {
                commands.entity(entity).insert(Selected);
            }
            let now = time.elapsed_secs();
            let double_press = groups
                .last_recall
                .is_some_and(|(previous, at)| previous == group && now - at < DOUBLE_PRESS_SECS);
            if double_press {
                center_camera_on(&groups.members[group], units, cameras);
            }
            groups.last_recall = Some((group, now));
        }
    }
}

fn center_camera_on(
    members: &[Entity],
    units: &Query<(&Team, &Transform), With<Unit>>,
    cameras: &mut Query<&mut RtsCamera>,
) {
    let positions: Vec<Vec3> = members
        .iter()
        .filter_map(|&e| units.get(e).ok())
        .map(|(_, t)| t.translation)
        .collect();
    if positions.is_empty() {
        return;
    }
    let centroid = positions.iter().sum::<Vec3>() / positions.len() as f32;
    for mut camera in cameras.iter_mut() {
        camera.focus.x = centroid.x;
        camera.focus.z = centroid.z;
    }
}

pub fn spawn_group_bar(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            GroupBar,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                column_gap: Val::Px(4.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|bar| {
            for group in 0..GROUP_COUNT {
                bar.spawn((
                    GroupButton(group),
                    Node {
                        padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(PANEL.with_alpha(0.88)),
                    BorderColor::all(FRAME),
                ))
                .with_children(|button| {
                    button.spawn(fonts.heading("", 15.0, TEXT_MAIN));
                })
                .observe(click_group_button);
            }
        });
}

fn click_group_button(
    click: On<PointerClick>,
    local: Res<LocalTeam>,
    mut commands: Commands,
    time: Res<Time>,
    buttons: Query<&GroupButton>,
    mut groups: ResMut<ControlGroups>,
    mut notifications: ResMut<Notifications>,
    selected: Query<Entity, (With<Selected>, With<Unit>)>,
    units: Query<(&Team, &Transform), With<Unit>>,
    mut cameras: Query<&mut RtsCamera>,
) {
    let Ok(&GroupButton(group)) = buttons.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Primary {
        return;
    }
    run_group_command(
        local.0,
        &mut commands,
        &time,
        &mut groups,
        &mut notifications,
        &selected,
        &units,
        &mut cameras,
        group,
        GroupCommand::Recall,
    );
}

pub fn refresh_group_bar(
    groups: Res<ControlGroups>,
    units: Query<(&Team, &Transform), With<Unit>>,
    selected: Query<(), With<Selected>>,
    mut buttons: Query<(&GroupButton, &mut Node, &mut BorderColor, &Children)>,
    mut labels: Query<(&mut Text, &mut TextColor)>,
) {
    for (button, mut node, mut border, children) in &mut buttons {
        let alive: Vec<Entity> = groups.members[button.0]
            .iter()
            .copied()
            .filter(|&e| units.contains(e))
            .collect();
        let display = if alive.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        set_field(&mut node, |node| &mut node.display, display);
        let active = !alive.is_empty() && alive.iter().all(|&e| selected.contains(e));
        border.set_if_neq(BorderColor::all(if active { ACCENT } else { FRAME }));
        let Some(&label) = children.first() else {
            continue;
        };
        if let Ok((mut text, mut color)) = labels.get_mut(label) {
            text.set_if_neq(Text(format!("{}  {}", button.0 + 1, alive.len())));
            color.set_if_neq(TextColor(if active { ACCENT } else { TEXT_DIM }));
        }
    }
}
