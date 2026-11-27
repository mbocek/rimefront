use super::*;

#[derive(Component)]
pub struct NotificationLine(usize);

#[derive(Component)]
pub struct ObjectiveText;

#[derive(Component)]
pub struct MatchTimeText;

/// How long until a world's age turns; empty outside a world with ages.
#[derive(Component)]
pub struct AgeCountdownText;

/// In its last day an age's countdown turns into a warning.
const AGE_ENDING_SECS: u64 = 24 * 3600;

pub(super) fn spawn_objective(sidebar: &mut ChildSpawnerCommands, fonts: &UiFonts) {
    sidebar
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                row_gap: Val::Px(1.0),
                border: UiRect::left(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(PANEL_RAISED),
            BorderColor::all(ACCENT),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn(fonts.heading(tr("OBJECTIVE"), 12.0, TEXT_DIM));
                    header.spawn((MatchTimeText, fonts.heading("0:00", 12.0, TEXT_DIM)));
                });
            panel.spawn((ObjectiveText, fonts.text("", 14.0, TEXT_MAIN)));
            panel.spawn((AgeCountdownText, fonts.text("", 12.0, TEXT_DIM)));
        });
}

pub fn refresh_age_countdown(
    clock: Res<MatchClock>,
    level: Res<ActiveLevel>,
    mut lines: Query<(&mut Text, &mut TextColor), With<AgeCountdownText>>,
) {
    let tick = (clock.elapsed_secs / crate::SIMULATION_TICK.as_secs_f32()) as u64;
    let countdown = level.0.ages.and_then(|ages| {
        let turn = ages.next_turn_after(tick)?;
        let next = level.0.era.next_playable()?;
        let secs_left = (turn - tick) * crate::SIMULATION_TICK.as_millis() as u64 / 1000;
        Some((next, secs_left))
    });
    let (shown, color) = match countdown {
        None => (String::new(), TEXT_DIM),
        Some((next, secs_left)) => (
            tr_fmt("{} in {}", &[&next.label(), &time_left(secs_left)]),
            if secs_left < AGE_ENDING_SECS {
                WARNING
            } else {
                TEXT_DIM
            },
        ),
    };
    for (mut text, mut text_color) in &mut lines {
        if text.0 != shown {
            text.0 = shown.clone();
        }
        text_color.set_if_neq(TextColor(color));
    }
}

fn time_left(secs: u64) -> String {
    let (days, hours, minutes) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60);
    match days {
        0 => format!("{hours}:{minutes:02}:{:02}", secs % 60),
        _ => tr_fmt("{} d {}:{}", &[&days, &hours, &format!("{minutes:02}")]),
    }
}

pub(super) fn spawn_notification_feed(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(14.0),
                left: Val::Px(0.0),
                right: Val::Px(SIDEBAR_WIDTH),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|feed| {
            for index in 0..4 {
                feed.spawn((
                    NotificationLine(index),
                    fonts.heading("", 22.0, ACCENT),
                    TextShadow::default(),
                    Pickable::IGNORE,
                ));
            }
        });
}

pub fn announce_mission_messages(
    mut messages: MessageReader<MissionMessage>,
    mut notifications: ResMut<Notifications>,
) {
    for message in messages.read() {
        notifications.push_mission_message(tr_owned(&message.0));
    }
}

pub fn refresh_notifications(
    notifications: Res<Notifications>,
    mut lines: Query<(&NotificationLine, &mut Text, &mut TextColor)>,
) {
    let visible: Vec<(&str, f32)> = notifications.visible().collect();
    for (line, mut text, mut color) in &mut lines {
        match visible.get(line.0) {
            Some((message, opacity)) => {
                if text.0 != *message {
                    text.0 = (*message).to_string();
                }
                color.set_if_neq(TextColor(ACCENT.with_alpha(*opacity)));
            }
            None => {
                if !text.0.is_empty() {
                    text.0.clear();
                }
            }
        }
    }
}

pub fn refresh_objective(
    local: Res<LocalTeam>,
    level: Res<ActiveLevel>,
    clock: Res<MatchClock>,
    economies: Res<Economies>,
    mut lines: Query<&mut Text, With<ObjectiveText>>,
) {
    let status = objective_status(&level, &clock, &economies, local.0);
    for mut text in &mut lines {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
}

pub fn refresh_match_time(
    clock: Res<MatchClock>,
    level: Res<ActiveLevel>,
    weather: Option<Res<WeatherCycle>>,
    mut lines: Query<&mut Text, (With<MatchTimeText>, Without<ObjectiveText>)>,
) {
    let raining = weather.is_some_and(|weather| weather.now.rainfall > RAINING);
    let time = match_time(clock.elapsed_secs);
    let shown = match (raining, level.0.season) {
        (false, _) => time,
        (true, Season::Summer) => format!("{}  ·  {time}", tr("Rain")),
        (true, Season::Autumn | Season::Spring) => format!("{}  ·  {time}", tr("Rain, mud")),
        (true, Season::Winter) => format!("{}  ·  {time}", tr("Snow")),
    };
    for mut text in &mut lines {
        if text.0 != shown {
            text.0 = shown.clone();
        }
    }
}

pub(super) fn match_time(elapsed_secs: f32) -> String {
    let total = elapsed_secs.max(0.0) as u32;
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
