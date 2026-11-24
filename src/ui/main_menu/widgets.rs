use super::*;

pub(super) fn button_node(width: Val) -> Node {
    Node {
        width,
        min_height: Val::Px(32.0),
        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    }
}

pub(super) fn spawn_command_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: &str,
    command: MenuCommand,
) {
    spawn_command_button_sized(parent, fonts, label, command, Val::Percent(100.0));
}

pub(super) fn spawn_command_button_sized(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: &str,
    command: MenuCommand,
    width: Val,
) {
    let (height, size, background) = if command.is_primary() {
        (44.0, 20.0, PRIMARY_IDLE)
    } else {
        (32.0, 17.0, BUTTON_IDLE)
    };
    parent
        .spawn((
            command,
            Hovered::default(),
            Node {
                min_height: Val::Px(height),
                margin: UiRect::top(Val::Px(if command.is_primary() { 6.0 } else { 0.0 })),
                ..button_node(width)
            },
            BackgroundColor(background),
            BorderColor::all(FRAME),
        ))
        .with_children(|button| {
            button.spawn(fonts.heading(label, size, TEXT_MAIN));
        })
        .observe(run_command);
}

pub(super) fn spawn_list_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: &str,
    selection: Selection,
) {
    spawn_list_button_with_badge(parent, fonts, label, selection, None);
}

/// A list entry with a short coloured note on its right, such as a won mission.
pub(super) fn spawn_list_button_with_badge(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: &str,
    selection: Selection,
    badge: Option<(&str, Color)>,
) {
    parent
        .spawn((
            ListButton(selection),
            Hovered::default(),
            Node {
                justify_content: JustifyContent::SpaceBetween,
                ..button_node(Val::Percent(100.0))
            },
            BackgroundColor(BUTTON_IDLE),
            BorderColor::all(FRAME),
        ))
        .with_children(|button| {
            button.spawn(fonts.heading(label, 17.0, TEXT_MAIN));
            if let Some((badge, color)) = badge {
                button.spawn(fonts.heading(badge, 13.0, color));
            }
        })
        .observe(select_entry);
}

/// Labelled values in two columns; clicking a value cycles it forward, right-clicking back.
pub(super) fn spawn_option_grid<M: Component>(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    rows: impl IntoIterator<Item = (&'static str, String, M)>,
) {
    panel
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(24.0),
            row_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|grid| {
            for (label, value, marker) in rows {
                grid.spawn(Node {
                    width: Val::Px(EDITOR_FIELD_WIDTH),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(fonts.text(label, 17.0, TEXT_MAIN));
                    row.spawn((
                        marker,
                        Hovered::default(),
                        button_node(Val::Px(EDITOR_VALUE_WIDTH)),
                        BackgroundColor(BUTTON_IDLE),
                        BorderColor::all(FRAME),
                    ))
                    .with_children(|button| {
                        button.spawn(fonts.heading(value, 16.0, ACCENT));
                    })
                    .observe(change_option);
                });
            }
        });
}

/// A labelled row of buttons of which one is chosen, outlined in the accent colour.
pub(super) fn spawn_choice_row(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: &str,
    choices: impl Iterator<Item = (String, MenuCommand, bool)>,
) {
    panel.spawn(fonts.heading(label, 17.0, TEXT_MAIN));
    panel
        .spawn(Node {
            column_gap: Val::Px(8.0),
            margin: UiRect::bottom(Val::Px(6.0)),
            ..default()
        })
        .with_children(|row| {
            for (text, command, chosen) in choices {
                row.spawn((
                    command,
                    Hovered::default(),
                    Node {
                        min_width: Val::Px(120.0),
                        min_height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        ..button_node(Val::Auto)
                    },
                    BackgroundColor(BUTTON_IDLE),
                    BorderColor::all(if chosen { ACCENT } else { FRAME }),
                ))
                .with_children(|button| {
                    button.spawn(fonts.heading(
                        text,
                        17.0,
                        if chosen { ACCENT } else { TEXT_MAIN },
                    ));
                })
                .observe(run_command);
            }
        });
}

pub(super) fn spawn_text_field(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    label: &str,
    value: String,
    marker: impl Component,
) {
    panel
        .spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(fonts.text(label, 18.0, TEXT_MAIN));
            row.spawn((
                button_node(Val::Px(260.0)),
                BackgroundColor(BUTTON_IDLE),
                BorderColor::all(ACCENT),
            ))
            .with_children(|field| {
                field.spawn((marker, fonts.heading(value, 16.0, ACCENT)));
            });
        });
}
