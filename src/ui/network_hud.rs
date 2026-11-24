use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::app::i18n::{tr, tr_fmt};
use crate::net::network::{NetSession, SessionState};
use crate::sim::notifications::Notifications;
use crate::sim::player_commands::LocalTeam;
use crate::sim::player_commands::SimulationTick;
use crate::ui::sidebar::{ACCENT, SIDEBAR_WIDTH, TEXT_DIM, UiFonts, set_field};

/// A short stall is ordinary network jitter; only a longer one is worth telling the player about.
const STALL_WORTH_SHOWING_SECS: f32 = 0.5;

#[derive(Component)]
pub struct LatencyText;

#[derive(Component)]
pub struct WaitingBanner;

pub fn spawn_network_hud(commands: &mut Commands, fonts: &UiFonts) {
    commands.spawn((
        LatencyText,
        fonts.text("", 14.0, TEXT_DIM),
        TextShadow::default(),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            right: Val::Px(SIDEBAR_WIDTH + 12.0),
            ..default()
        },
    ));
    commands.spawn((
        WaitingBanner,
        fonts.heading("", 26.0, ACCENT),
        TextShadow::default(),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(30.0),
            left: Val::Px(0.0),
            right: Val::Px(SIDEBAR_WIDTH),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        TextLayout::justify(Justify::Center),
    ));
}

/// The ping in the corner, and a banner while the battle waits for the other player's orders.
pub fn refresh_network_hud(
    session: Option<Res<NetSession>>,
    tick: Res<SimulationTick>,
    real: Res<Time<Real>>,
    game_time: Res<Time<Virtual>>,
    catching_up: Res<crate::net::network::CatchingUp>,
    mut last_progress: Local<(u64, f32)>,
    mut latency: Query<&mut Text, (With<LatencyText>, Without<WaitingBanner>)>,
    mut banners: Query<(&mut Text, &mut Node), With<WaitingBanner>>,
) {
    let now = real.elapsed_secs();
    if last_progress.0 != tick.0 || game_time.is_paused() {
        *last_progress = (tick.0, now);
    }
    let in_step = session
        .as_ref()
        .is_some_and(|session| session.state == SessionState::InStep);
    let ping = session
        .as_ref()
        .filter(|_| in_step)
        .and_then(|session| session.round_trip())
        .map(|round_trip| tr_fmt("Ping {} ms", &[&round_trip.as_millis()]))
        .unwrap_or_default();
    for mut text in &mut latency {
        text.set_if_neq(Text(ping.clone()));
    }
    let stalled_for = now - last_progress.1;
    let waiting = in_step && stalled_for > STALL_WORTH_SHOWING_SECS;
    let rejoin = session
        .as_ref()
        .and_then(|session| session.waiting_for_rejoin());
    let banner = match (catching_up.0, rejoin) {
        (Some(done), _) => Some(tr_fmt(
            "Catching up with the battle... {} %",
            &[&((done * 100.0) as u32)],
        )),
        (None, Some((team, waited))) => Some(tr_fmt(
            "{} lost the connection; waiting for them to rejoin... {} s",
            &[&tr(team.name()), &waited.as_secs()],
        )),
        (None, None) if waiting => Some(tr_fmt(
            "Waiting for the other player... {} s",
            &[&(stalled_for as u32)],
        )),
        (None, None) => None,
    };
    for (mut text, mut node) in &mut banners {
        let display = if banner.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        set_field(&mut node, |node| &mut node.display, display);
        if let Some(banner) = &banner {
            text.set_if_neq(Text(banner.clone()));
        }
    }
}

/// The chat line being typed during a network battle; `None` while the player is not typing.
#[derive(Resource, Default)]
pub struct BattleChat {
    draft: Option<String>,
}

#[derive(Component)]
pub struct ChatDraftLine;

pub fn spawn_chat_line(commands: &mut Commands, fonts: &UiFonts) {
    commands.spawn((
        ChatDraftLine,
        fonts.heading("", 17.0, ACCENT),
        TextShadow::default(),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(34.0),
            left: Val::Px(12.0),
            display: Display::None,
            ..default()
        },
    ));
}

/// Enter opens the chat line, Enter again sends it and Esc drops it. While it is open the keyboard types
/// instead of commanding, so every other system sees no keys pressed.
pub fn type_battle_chat(
    local: Res<LocalTeam>,
    session: Option<ResMut<NetSession>>,
    mut chat: ResMut<BattleChat>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut typed: MessageReader<KeyboardInput>,
    mut notifications: ResMut<Notifications>,
) {
    let Some(mut session) = session.filter(|session| !session.is_spectating()) else {
        chat.draft = None;
        typed.clear();
        return;
    };
    let Some(draft) = chat.draft.as_mut() else {
        typed.clear();
        if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
            chat.draft = Some(String::new());
            keys.reset_all();
        }
        return;
    };
    let mut finished = false;
    for key in typed.read().filter(|key| key.state.is_pressed()) {
        match &key.logical_key {
            Key::Enter => {
                if let Some(sent) = session.say(local.0, draft) {
                    notifications.push(tr_fmt("You: {}", &[&sent]));
                }
                finished = true;
            }
            Key::Escape => finished = true,
            Key::Backspace => {
                draft.pop();
            }
            Key::Space => draft.push(' '),
            Key::Character(text) => draft.push_str(text),
            _ => {}
        }
    }
    if finished {
        chat.draft = None;
    }
    keys.reset_all();
}

pub fn show_chat(
    session: Option<ResMut<NetSession>>,
    chat: Res<BattleChat>,
    mut notifications: ResMut<Notifications>,
    mut lines: Query<(&mut Text, &mut Node), With<ChatDraftLine>>,
) {
    if let Some(mut session) = session {
        for (team, text) in session.take_chat() {
            notifications.push(tr_fmt("{}: {}", &[&tr(team.name()), &text]));
        }
    }
    for (mut text, mut node) in &mut lines {
        let display = if chat.draft.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        set_field(&mut node, |node| &mut node.display, display);
        if let Some(draft) = &chat.draft {
            text.set_if_neq(Text(tr_fmt("Say: {}_", &[draft])));
        }
    }
}
