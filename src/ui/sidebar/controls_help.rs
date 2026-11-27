use super::*;

#[derive(Component)]
pub struct ControlsPanel;

#[derive(Component)]
pub struct ControlsHint;

pub(super) fn spawn_controls_help(commands: &mut Commands, fonts: &UiFonts) {
    commands.spawn((
        ControlsHint,
        fonts.text("", 15.0, TEXT_DIM),
        TextShadow::default(),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(8.0),
            ..default()
        },
    ));
    commands.spawn((
        ControlsPanel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(34.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(10.0)),
            row_gap: Val::Px(2.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL.with_alpha(0.92)),
        BorderColor::all(FRAME_BRIGHT),
        Visibility::Hidden,
    ));
}

pub(super) fn controls_overview(keys: &KeyBindings) -> Vec<(&'static str, String)> {
    let key = |action| keys.label(action);
    vec![
        (
            tr("Select"),
            tr_owned("left click / drag a box (Shift adds)"),
        ),
        (tr("Orders"), tr_owned("right click: move, attack, harvest")),
        (tr("Attack-move"), tr_owned("Ctrl + right click")),
        (
            tr("In formation"),
            tr_owned("Alt (Option) + right click: the group keeps to its slowest unit"),
        ),
        (
            tr("Patrol"),
            tr_fmt("{} + right click", &[&key(Action::Patrol)]),
        ),
        (tr("Queue orders"), tr_owned("Shift + right click")),
        (
            tr("Rally point"),
            tr_owned("select barracks/factory, right click"),
        ),
        (
            tr("Repair / sell"),
            tr_fmt(
                "{} / {} (or the buttons), then click a structure",
                &[&key(Action::RepairTool), &key(Action::SellTool)],
            ),
        ),
        (
            tr("Scroll"),
            tr_fmt(
                "{}{}{}{}, arrows, screen edge, middle-drag",
                &[
                    &key(Action::ScrollUp),
                    &key(Action::ScrollLeft),
                    &key(Action::ScrollDown),
                    &key(Action::ScrollRight),
                ],
            ),
        ),
        (tr("Fast scroll"), tr_owned("hold Shift")),
        (tr("Minimap"), tr_owned("click or drag to jump")),
        (
            tr("Zoom / rotate"),
            tr_fmt(
                "mouse wheel / {}, {} ({} resets)",
                &[
                    &key(Action::RotateLeft),
                    &key(Action::RotateRight),
                    &key(Action::ResetRotation),
                ],
            ),
        ),
        (tr("Home"), tr_fmt("{} or Home", &[&key(Action::JumpHome)])),
        (
            tr("Stop / deselect"),
            format!("{} / Esc", key(Action::Stop)),
        ),
        (
            tr("All of one kind"),
            tr_owned("double-click or Ctrl+click a unit"),
        ),
        (
            tr("Last attack"),
            tr_fmt("{} jumps there", &[&key(Action::JumpToAlert)]),
        ),
        (
            tr("Lay a mine"),
            tr_fmt(
                "{} with engineers selected, ${} each",
                &[&key(Action::LayMine), &MINE_COST],
            ),
        ),
        (
            tr("Stance"),
            tr_fmt(
                "{} aggressive, {} hold position, {} hold fire",
                &[
                    &key(Action::StanceAggressive),
                    &key(Action::StanceHoldPosition),
                    &key(Action::StanceHoldFire),
                ],
            ),
        ),
        (
            tr("Support"),
            tr_fmt(
                "{} recon flight, {} artillery barrage, then click the map",
                &[&key(Action::ReconFlight), &key(Action::Barrage)],
            ),
        ),
        (
            tr("Troop Carrier"),
            tr_fmt(
                "right-click it with infantry to board, {} unloads",
                &[&key(Action::Unload)],
            ),
        ),
        (
            tr("New age"),
            tr_fmt(
                "{} promotes the selection near its producer",
                &[&key(Action::Promote)],
            ),
        ),
        (
            tr("Groups"),
            tr_owned("Ctrl+1-9 save, 1-9 select, twice = jump"),
        ),
        (tr("Add a group"), "Shift+1-9".to_string()),
        (
            tr("Graphics quality"),
            tr_fmt("{}: Low / Medium / High", &[&key(Action::GraphicsQuality)]),
        ),
        (
            tr("Performance"),
            tr_fmt(
                "{} overlay: FPS, CPU, GPU",
                &[&key(Action::PerformanceOverlay)],
            ),
        ),
        (
            tr("Save / load"),
            tr_fmt(
                "{} quick save, {} quick load",
                &[&key(Action::QuickSave), &key(Action::QuickLoad)],
            ),
        ),
        (tr("Menu & settings"), tr_owned("F10 or Esc")),
        (
            tr("Build menu"),
            tr_owned("left click builds (Shift = 5x), right click cancels"),
        ),
        (
            tr("Queue bar"),
            tr_owned("left click places a ready structure, right click cancels"),
        ),
    ]
}

/// The overview names the player's own keys, so it is rewritten whenever the settings change.
pub fn fill_controls_help(
    mut commands: Commands,
    fonts: Option<Res<UiFonts>>,
    settings: Res<GameSettings>,
    panels: Query<(Entity, Ref<ControlsPanel>)>,
    mut hints: Query<&mut Text, With<ControlsHint>>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (panel, added) in &panels {
        if !settings.is_changed() && !added.is_added() {
            continue;
        }
        for mut hint in &mut hints {
            hint.0 = tr_fmt(
                "{}  controls",
                &[&settings.keys.label(Action::ControlsHelp)],
            );
        }
        commands.entity(panel).despawn_related::<Children>();
        commands.entity(panel).with_children(|panel| {
            panel.spawn(fonts.heading(tr("CONTROLS"), 18.0, ACCENT));
            for (action, keys) in controls_overview(&settings.keys) {
                panel
                    .spawn(Node {
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            fonts.heading(action, 15.0, TEXT_MAIN),
                            Node {
                                width: Val::Px(110.0),
                                ..default()
                            },
                        ));
                        row.spawn(fonts.text(keys, 15.0, TEXT_DIM));
                    });
            }
        });
    }
}

pub fn toggle_controls_help(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    mut panels: Query<&mut Visibility, With<ControlsPanel>>,
) {
    if !settings.keys.just_pressed(&keys, Action::ControlsHelp) {
        return;
    }
    for mut visibility in &mut panels {
        *visibility = match *visibility {
            Visibility::Hidden => Visibility::Visible,
            _ => Visibility::Hidden,
        };
    }
}
