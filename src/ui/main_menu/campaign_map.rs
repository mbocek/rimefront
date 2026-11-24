use super::*;

pub(super) const MISSION_NODE: f32 = 34.0;
pub(super) const MISSION_ROW: f32 = 52.0;
pub(super) const ROUTE_SWING: f32 = 70.0;

/// The campaign tabs and the route of the chosen campaign's missions, one node per mission joined by a line,
/// zig-zagging down the column: won ones ticked, the next one lit, locked ones dimmed.
pub(super) fn spawn_operations_map(
    list: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    menu: &MainMenu,
) {
    let Selection::Mission {
        campaign: shown,
        mission: chosen,
    } = menu.selection
    else {
        return;
    };
    list.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(6.0),
        row_gap: Val::Px(6.0),
        margin: UiRect::bottom(Val::Px(8.0)),
        ..default()
    })
    .with_children(|tabs| {
        for (index, campaign) in menu.campaigns.iter().enumerate() {
            let progress = format!(
                "{}  {}/{}",
                tr_owned(&campaign.title),
                campaign.won_count(&menu.completed_missions),
                campaign.missions.len()
            );
            tabs.spawn((
                ListButton(menu.campaign_page(index)),
                Hovered::default(),
                button_node(Val::Auto),
                BackgroundColor(BUTTON_IDLE),
                BorderColor::all(if index == shown { ACCENT } else { FRAME }),
            ))
            .with_children(|tab| {
                tab.spawn(fonts.heading(progress, 15.0, TEXT_MAIN));
            })
            .observe(select_entry);
        }
    });
    let campaign = &menu.campaigns[shown];
    let completed = &menu.completed_missions;
    let next = campaign.next_to_play(completed);
    let swing = |mission: usize| {
        if mission.is_multiple_of(2) {
            0.0
        } else {
            ROUTE_SWING
        }
    };
    let height = MISSION_ROW * campaign.missions.len() as f32;
    list.spawn(Node {
        height: Val::Px(height),
        ..default()
    })
    .with_children(|route| {
        for mission in 1..campaign.missions.len() {
            let from = Vec2::new(swing(mission - 1), MISSION_ROW * (mission - 1) as f32);
            let to = Vec2::new(swing(mission), MISSION_ROW * mission as f32);
            for step in 1..8 {
                let at = from.lerp(to, step as f32 / 8.0) + Vec2::splat(MISSION_NODE / 2.0 - 2.0);
                let reached = campaign.won(mission - 1, completed);
                route.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(at.x),
                        top: Val::Px(at.y),
                        width: Val::Px(4.0),
                        height: Val::Px(4.0),
                        ..default()
                    },
                    BackgroundColor(if reached { ACCENT } else { FRAME }),
                ));
            }
        }
        for (mission, level) in campaign.missions.iter().enumerate() {
            let won = campaign.won(mission, completed);
            let unlocked = campaign.unlocked(mission, completed);
            let (mark, mark_color) = match (won, unlocked) {
                (true, _) => (format!("{}", mission + 1), GOOD),
                (false, true) => (format!("{}", mission + 1), ACCENT),
                (false, false) => ("·".to_string(), TEXT_DIM),
            };
            let label_color = if unlocked { TEXT_MAIN } else { TEXT_DIM };
            route
                .spawn((
                    ListButton(Selection::Mission {
                        campaign: shown,
                        mission,
                    }),
                    Hovered::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(swing(mission)),
                        top: Val::Px(MISSION_ROW * mission as f32),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        padding: UiRect::right(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                    BorderColor::all(if mission == chosen {
                        ACCENT
                    } else if Some(mission) == next {
                        GOOD
                    } else {
                        FRAME
                    }),
                ))
                .with_children(|node| {
                    node.spawn(Node {
                        width: Val::Px(MISSION_NODE),
                        height: Val::Px(MISSION_NODE),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|badge| {
                        badge.spawn(fonts.heading(mark, 18.0, mark_color));
                    });
                    node.spawn(fonts.heading(short_mission_name(level), 15.0, label_color));
                    if won {
                        node.spawn(fonts.heading(tr("WON"), 12.0, GOOD));
                    }
                })
                .observe(select_entry);
        }
    });
}

/// A mission's name without the number its title starts with; the node shows the number.
pub(super) fn short_mission_name(level: &LevelDefinition) -> String {
    let name = tr_owned(&level.name);
    match name.split_once(". ") {
        Some((number, rest)) if number.len() <= 3 => rest.to_string(),
        _ => name,
    }
}

/// The briefing beside the route: the story, the objective and map, the campaign difficulty and the start.
pub(super) fn spawn_mission_briefing(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    menu: &MainMenu,
    campaign: usize,
    mission: usize,
    preview: Option<Handle<Image>>,
) {
    let chosen = &menu.campaigns[campaign];
    let level = chosen.mission_at(mission, menu.campaign_difficulty);
    panel.spawn(fonts.text(
        tr_fmt(
            "{} campaign  ·  mission {} of {}",
            &[
                &tr_owned(&chosen.title),
                &(mission + 1),
                &chosen.missions.len(),
            ],
        ),
        15.0,
        TEXT_DIM,
    ));
    panel.spawn(fonts.heading(tr_owned(&level.name), 30.0, ACCENT));
    beside_preview(panel, preview, LARGE_PREVIEW, |column| {
        column.spawn(fonts.text(tr_owned(&level.briefing), 17.0, TEXT_MAIN));
        column.spawn(Node {
            height: Val::Px(4.0),
            ..default()
        });
        spawn_level_facts(column, fonts, &level);
    });
    let difficulty = format!(
        "{}: {}",
        tr("Campaign difficulty"),
        tr_owned(&format!("{:?}", menu.campaign_difficulty))
    );
    spawn_command_button(
        panel,
        fonts,
        &difficulty,
        MenuCommand::CycleCampaignDifficulty,
    );
    if chosen.unlocked(mission, &menu.completed_missions) {
        spawn_command_button(
            panel,
            fonts,
            tr("START MISSION"),
            MenuCommand::StartCampaignMission,
        );
    } else {
        panel.spawn(fonts.text(
            tr_fmt(
                "Locked: win {} first.",
                &[&tr_owned(&chosen.missions[mission - 1].name)],
            ),
            16.0,
            TEXT_DIM,
        ));
    }
}

/// A campaign mission won on this computer is remembered and marked in the main menu.
pub fn remember_won_missions(
    outcome: Res<GameOutcome>,
    run: Option<Res<CampaignRun>>,
    library: Res<CampaignLibrary>,
    session: Option<Res<NetSession>>,
    mut settings: ResMut<GameSettings>,
) {
    if !outcome.is_changed() || *outcome != GameOutcome::Victory || session.is_some() {
        return;
    }
    let Some(run) = run else { return };
    let Some(campaign) = find_campaign(&library.0, &run.key) else {
        return;
    };
    let key = campaign.progress_key(run.mission);
    if !settings.completed_missions.contains(&key) {
        settings.completed_missions.push(key);
    }
}

pub(super) fn start_campaign_mission(
    commands: &mut Commands,
    next_state: &mut NextState<GameState>,
    settings: &mut GameSettings,
    menu: &MainMenu,
    campaign: usize,
    mission: usize,
) {
    let chosen = &menu.campaigns[campaign];
    if !chosen.unlocked(mission, &menu.completed_missions) {
        return;
    }
    start_level(
        commands,
        next_state,
        chosen.mission_at(mission, menu.campaign_difficulty),
    );
    commands.insert_resource(CampaignRun {
        key: chosen.key.clone(),
        mission,
        difficulty: menu.campaign_difficulty,
    });
    settings.last_campaign = chosen.key.clone();
}
