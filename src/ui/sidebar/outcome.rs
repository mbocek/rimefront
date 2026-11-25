use super::*;
use crate::app::match_flow::MirroredBattle;
use crate::view::side_tint::SideColors;

#[derive(Component)]
pub struct OutcomeOverlay;

#[derive(Component)]
pub struct OutcomeTitle;

#[derive(Component)]
pub struct OutcomeStats;

pub(super) fn spawn_outcome_overlay(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            OutcomeOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                right: Val::Px(SIDEBAR_WIDTH),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            Visibility::Hidden,
        ))
        .with_children(|overlay| {
            overlay.spawn((
                OutcomeTitle,
                fonts.heading("", 96.0, ACCENT),
                TextShadow::default(),
            ));
            overlay.spawn((
                OutcomeSubtitle,
                fonts.heading("", 22.0, TEXT_MAIN),
                TextShadow::default(),
            ));
            overlay.spawn((
                OutcomeStats,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    min_width: Val::Px(460.0),
                    ..default()
                },
                BackgroundColor(PANEL.with_alpha(0.9)),
                BorderColor::all(FRAME_BRIGHT),
            ));
            overlay.spawn((
                OutcomeButtons,
                Node {
                    column_gap: Val::Px(12.0),
                    ..default()
                },
            ));
        });
}

#[derive(Component, Clone, Copy)]
pub enum OutcomeAction {
    /// The campaign mission at this index comes next.
    NextMission(usize),
    Restart,
    WatchReplay,
    MainMenu,
}

impl OutcomeAction {
    pub(super) fn label(self) -> &'static str {
        match self {
            OutcomeAction::NextMission(_) => tr("NEXT MISSION"),
            OutcomeAction::Restart => tr("PLAY AGAIN"),
            OutcomeAction::WatchReplay => tr("WATCH REPLAY"),
            OutcomeAction::MainMenu => tr("MAIN MENU"),
        }
    }
}

#[derive(Component)]
pub struct OutcomeSubtitle;

#[derive(Component)]
pub struct OutcomeButtons;

/// What the player can do once the battle is decided; a network battle cannot be replayed on the spot.
pub(super) fn outcome_actions(
    outcome: GameOutcome,
    run: Option<&CampaignRun>,
    library: &CampaignLibrary,
    networked: bool,
) -> Vec<OutcomeAction> {
    let mut actions = Vec::new();
    let next_mission = run.and_then(|run| {
        let campaign = find_campaign(&library.0, &run.key)?;
        Some(run.mission + 1).filter(|&next| next < campaign.missions.len())
    });
    if let (GameOutcome::Victory, Some(next), false) = (outcome, next_mission, networked) {
        actions.push(OutcomeAction::NextMission(next));
    }
    if !networked {
        actions.push(OutcomeAction::Restart);
    }
    actions.extend([OutcomeAction::WatchReplay, OutcomeAction::MainMenu]);
    actions
}

/// The mission's name, and in a campaign how far it has come and what victory opened next.
pub(super) fn campaign_debriefing(
    level: &LevelDefinition,
    outcome: GameOutcome,
    run: Option<&CampaignRun>,
    library: &CampaignLibrary,
) -> String {
    let name = tr_owned(&level.name);
    let Some((run, campaign)) =
        run.and_then(|run| Some((run, find_campaign(&library.0, &run.key)?)))
    else {
        return name;
    };
    let won_now = usize::from(outcome == GameOutcome::Victory);
    let progress = tr_fmt(
        "{} campaign: mission {} of {}",
        &[
            &tr_owned(&campaign.title),
            &(run.mission + 1),
            &campaign.missions.len(),
        ],
    );
    let next = match campaign.missions.get(run.mission + 1) {
        Some(next) if won_now == 1 => tr_fmt("Unlocked: {}", &[&tr_owned(&next.name)]),
        None if won_now == 1 => tr("Campaign complete!").to_string(),
        _ => String::new(),
    };
    [name, progress, next]
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("  ·  ")
}

pub(super) fn spawn_outcome_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    action: OutcomeAction,
    primary: bool,
) {
    parent
        .spawn((
            action,
            Hovered::default(),
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if primary { PRIMARY_BUTTON } else { TILE }),
            BorderColor::all(FRAME_BRIGHT),
        ))
        .with_children(|button| {
            button.spawn(fonts.heading(action.label(), 18.0, TEXT_MAIN));
        })
        .observe(run_outcome_action);
}

pub fn highlight_outcome_buttons(
    mut buttons: Query<(&Hovered, &mut BorderColor), With<OutcomeAction>>,
) {
    for (hovered, mut border) in &mut buttons {
        border.set_if_neq(BorderColor::all(if hovered.get() {
            ACCENT
        } else {
            FRAME_BRIGHT
        }));
    }
}

pub(super) fn run_outcome_action(
    click: On<PointerClick>,
    mut commands: Commands,
    actions: Query<&OutcomeAction>,
    mut notifications: ResMut<Notifications>,
    mut next_state: ResMut<NextState<GameState>>,
    run: Option<ResMut<CampaignRun>>,
    library: Res<CampaignLibrary>,
) {
    let Ok(&action) = actions.get(click.entity) else {
        return;
    };
    match action {
        OutcomeAction::NextMission(index) => {
            let Some(mut run) = run else { return };
            if let Some(campaign) = find_campaign(&library.0, &run.key) {
                run.mission = index;
                commands.insert_resource(ActiveLevel(campaign.mission_at(index, run.difficulty)));
                next_state.set(GameState::Restarting);
            }
        }
        OutcomeAction::Restart => next_state.set(GameState::Restarting),
        OutcomeAction::MainMenu => next_state.set(GameState::MainMenu),
        OutcomeAction::WatchReplay => {
            let newest = list_replays(&replay_directory())
                .into_iter()
                .next()
                .and_then(|summary| load_replay(&summary.path).ok());
            match newest {
                Some(replay) => {
                    commands.insert_resource(ActiveLevel(replay.level.clone()));
                    commands.insert_resource(ReplayPlayback::new(replay));
                    next_state.set(GameState::Replay);
                }
                None => notifications.push(tr("The replay could not be found")),
            }
        }
    }
}

/// The rows of the end-of-match table: what the player did against what all opponents did together.
pub(super) fn outcome_rows(
    stats: &MatchStats,
    economies: &Economies,
    team: Team,
) -> Vec<(&'static str, String, String)> {
    let player = *stats.of(team);
    let mut opponents = TeamTally::default();
    let mut opponents_harvested = 0;
    for team in Team::OPPONENTS {
        opponents.add(stats.of(team));
        opponents_harvested += economies.of(team).harvested;
    }
    let count = |value: u32| value.to_string();
    let amount = |value: f32| format!("{:.0}", value);
    vec![
        (
            tr("Units trained"),
            count(player.units_trained),
            count(opponents.units_trained),
        ),
        (
            tr("Units lost"),
            count(player.units_lost),
            count(opponents.units_lost),
        ),
        (
            tr("Structures built"),
            count(player.structures_built),
            count(opponents.structures_built),
        ),
        (
            tr("Structures lost"),
            count(player.structures_lost),
            count(opponents.structures_lost),
        ),
        (
            tr("Enemies destroyed"),
            count(player.enemies_destroyed),
            count(opponents.enemies_destroyed),
        ),
        (
            tr("Damage dealt"),
            amount(player.damage_dealt),
            amount(opponents.damage_dealt),
        ),
        (
            tr("Credits harvested"),
            format!("${}", economies.of(team).harvested),
            format!("${opponents_harvested}"),
        ),
    ]
}

pub(super) fn spawn_outcome_table(
    table: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    rows: Vec<(&'static str, String, String)>,
    match_secs: f32,
    yours: Color,
) {
    let row = |table: &mut ChildSpawnerCommands, cells: [(String, Color); 3]| {
        table
            .spawn(Node {
                column_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|line| {
                for (index, (text, color)) in cells.into_iter().enumerate() {
                    let width = if index == 0 { 220.0 } else { 100.0 };
                    line.spawn(Node {
                        width: Val::Px(width),
                        justify_content: if index == 0 {
                            JustifyContent::FlexStart
                        } else {
                            JustifyContent::FlexEnd
                        },
                        ..default()
                    })
                    .with_children(|cell| {
                        cell.spawn(fonts.text(text, 18.0, color));
                    });
                }
            });
    };
    let minutes = match_secs as u32 / 60;
    let seconds = match_secs as u32 % 60;
    let time = format!("{minutes}:{seconds:02}");
    table.spawn(fonts.heading(tr_fmt("MATCH TIME  {}", &[&time]), 18.0, ACCENT));
    row(
        table,
        [
            (String::new(), TEXT_DIM),
            (tr("You").into(), yours),
            (tr("Enemies").into(), Team::Crimson.ui_color()),
        ],
    );
    for (label, player, opponents) in rows {
        row(
            table,
            [
                (label.into(), TEXT_DIM),
                (player, TEXT_MAIN),
                (opponents, TEXT_MAIN),
            ],
        );
    }
}

pub fn show_outcome(
    local: Res<LocalTeam>,
    view: LocalView,
    mut commands: Commands,
    fonts: Res<UiFonts>,
    outcome: Res<GameOutcome>,
    stats: Res<MatchStats>,
    economies: Res<Economies>,
    clock: Res<MatchClock>,
    mut overlay: Query<&mut Visibility, With<OutcomeOverlay>>,
    mut title: Query<(&mut Text, &mut TextColor), (With<OutcomeTitle>, Without<OutcomeSubtitle>)>,
    mut subtitle: Query<&mut Text, (With<OutcomeSubtitle>, Without<OutcomeTitle>)>,
    level: Res<ActiveLevel>,
    (session, in_a_world, colors): (
        Option<Res<NetSession>>,
        Option<Res<MirroredBattle>>,
        Res<SideColors>,
    ),
    (run, library): (Option<Res<CampaignRun>>, Res<CampaignLibrary>),
    button_rows: Query<Entity, With<OutcomeButtons>>,
    tables: Query<Entity, With<OutcomeStats>>,
) {
    if !outcome.is_changed() {
        return;
    }
    let enemy_side = if local.0 == Team::Player {
        Team::Crimson
    } else {
        Team::Player
    };
    let banner = match *outcome {
        GameOutcome::Ongoing => None,
        _ if view.is_spectating() => Some((tr("THE BATTLE IS OVER"), local.0)),
        GameOutcome::Victory => Some((tr("VICTORY"), local.0)),
        GameOutcome::Defeat => Some((tr("DEFEAT"), enemy_side)),
    };
    if let Some((message, team)) = banner
        && let Ok((mut text, mut color)) = title.single_mut()
    {
        text.0 = message.into();
        color.0 = colors.ui_of(team);
    }
    if banner.is_some() {
        if let Ok(mut subtitle) = subtitle.single_mut() {
            subtitle.0 = match in_a_world {
                Some(_) => {
                    tr("Nothing of your side stands. Enter the world again to start anew.").into()
                }
                None => campaign_debriefing(&level.0, *outcome, run.as_deref(), &library),
            };
        }
        if let Ok(row) = button_rows.single() {
            let actions = match in_a_world {
                Some(_) => vec![OutcomeAction::MainMenu],
                None => outcome_actions(*outcome, run.as_deref(), &library, session.is_some()),
            };
            commands.entity(row).despawn_related::<Children>();
            commands.entity(row).with_children(|row| {
                for (index, action) in actions.into_iter().enumerate() {
                    spawn_outcome_button(row, &fonts, action, index == 0);
                }
            });
        }
    }
    if banner.is_some()
        && let Ok(table) = tables.single()
    {
        let rows = outcome_rows(&stats, &economies, local.0);
        commands.entity(table).despawn_related::<Children>();
        commands.entity(table).with_children(|table| {
            spawn_outcome_table(
                table,
                &fonts,
                rows,
                clock.elapsed_secs,
                colors.ui_of(local.0),
            );
        });
    }
    if let Ok(mut visibility) = overlay.single_mut() {
        *visibility = if banner.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
