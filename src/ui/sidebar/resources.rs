use super::*;

pub(super) const POWER_BAR_SCALE: i32 = 400;
#[derive(Component)]
pub struct CreditsText;

#[derive(Component)]
pub struct PowerText;

#[derive(Component)]
pub struct PowerBarFill;

#[derive(Component)]
pub struct PowerBarDemand;

pub(super) fn spawn_resources(sidebar: &mut ChildSpawnerCommands, fonts: &UiFonts) {
    sidebar
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL_RAISED),
            BorderColor::all(FRAME),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((CreditsText, fonts.heading("$ 0", 26.0, ACCENT)));
                    spawn_tool_buttons(row, fonts);
                });
            crate::ui::support_tools::spawn_support_buttons(panel, fonts);
            panel
                .spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        PowerText,
                        fonts.text(tr_fmt("POWER {} used of {}", &[&0, &0]), 14.0, TEXT_MAIN),
                    ));
                    row.spawn((
                        Node {
                            flex_grow: 1.0,
                            height: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.04, 0.05, 0.05)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            PowerBarFill,
                            Node {
                                position_type: PositionType::Absolute,
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(GOOD),
                        ));
                        bar.spawn((
                            PowerBarDemand,
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(2.0),
                                height: Val::Px(12.0),
                                top: Val::Px(-2.0),
                                ..default()
                            },
                            BackgroundColor(TEXT_MAIN),
                        ));
                    });
                });
        });
}

pub fn refresh_resources(
    local: Res<LocalTeam>,
    economies: Res<Economies>,
    mut credits_text: Query<&mut Text, (With<CreditsText>, Without<PowerText>)>,
    mut power_text: Query<(&mut Text, &mut TextColor), (With<PowerText>, Without<CreditsText>)>,
    mut power_fill: Query<
        (&mut Node, &mut BackgroundColor),
        (With<PowerBarFill>, Without<PowerBarDemand>),
    >,
    mut power_demand: Query<&mut Node, (With<PowerBarDemand>, Without<PowerBarFill>)>,
) {
    let economy = economies.of(local.0);
    if let Ok(mut text) = credits_text.single_mut() {
        text.set_if_neq(Text(format!("$ {}", economy.credits)));
    }
    let status_color = if economy.has_low_power() {
        DANGER
    } else if economy.power_supply - economy.power_demand < 25 {
        WARNING
    } else {
        GOOD
    };
    if let Ok((mut text, mut color)) = power_text.single_mut() {
        text.set_if_neq(Text(tr_fmt(
            "POWER {} used of {}",
            &[&economy.power_demand, &economy.power_supply],
        )));
        color.set_if_neq(TextColor(if economy.has_low_power() {
            DANGER
        } else {
            TEXT_MAIN
        }));
    }
    let scale = economy
        .power_supply
        .max(economy.power_demand)
        .max(POWER_BAR_SCALE) as f32;
    if let Ok((mut node, mut color)) = power_fill.single_mut() {
        let supplied = Val::Percent(economy.power_supply as f32 / scale * 100.0);
        set_field(&mut node, |node| &mut node.width, supplied);
        color.set_if_neq(BackgroundColor(status_color));
    }
    if let Ok(mut node) = power_demand.single_mut() {
        let demanded = Val::Percent(economy.power_demand as f32 / scale * 100.0);
        set_field(&mut node, |node| &mut node.left, demanded);
    }
}
