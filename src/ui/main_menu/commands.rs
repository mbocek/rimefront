use super::*;

#[derive(Component, Clone, Copy)]
pub enum MenuCommand {
    StartSelected,
    QuickFaction(FactionChoice),
    QuickDifficulty(Difficulty),
    StartQuickBattle,
    StartEditorBattle,
    SaveEditorLevel,
    EditMap,
    RerollRandomBattle,
    WatchReplay,
    CycleLanguage,
    ContinueQuickSave,
    StartCampaignMission,
    CycleCampaignDifficulty,
    ContinueCampaign,
    AddToMyCampaign,
    OpenSettings,
    BackToHub,
    DownloadUpdate,
    HostNetworkBattle,
    JoinNetworkBattle,
    OpenServerRoom,
    PlayWorldAlone,
    EnterServerWorld,
    AcceptNetworkOffer,
    StartNetworkBattle,
    CancelNetworkBattle,
    Quit,
}

impl MenuCommand {
    /// The action a page exists for, drawn larger and in the accent colour.
    pub(super) fn is_primary(self) -> bool {
        matches!(
            self,
            MenuCommand::StartSelected
                | MenuCommand::StartQuickBattle
                | MenuCommand::StartCampaignMission
                | MenuCommand::StartEditorBattle
                | MenuCommand::WatchReplay
                | MenuCommand::HostNetworkBattle
                | MenuCommand::AcceptNetworkOffer
                | MenuCommand::StartNetworkBattle
        )
    }
}

pub(super) fn start_level(
    commands: &mut Commands,
    next_state: &mut NextState<GameState>,
    level: LevelDefinition,
) {
    commands.remove_resource::<CampaignRun>();
    commands.insert_resource(ActiveLevel(level));
    next_state.set(GameState::Playing);
}

pub(super) fn run_command(
    click: On<PointerClick>,
    mut commands: Commands,
    buttons: Query<&MenuCommand>,
    mut menu: ResMut<MainMenu>,
    mut requests: ResMut<SaveRequests>,
    mut next_state: ResMut<NextState<GameState>>,
    mut settings: ResMut<GameSettings>,
    mut updates: ResMut<Updates>,
    mut lobby: ResMut<NetLobby>,
    mut settings_menu: ResMut<MenuState>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(&command) = buttons.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Primary {
        return;
    }
    match command {
        MenuCommand::StartSelected => {
            if let Selection::Level(index) = menu.selection {
                start_level(
                    &mut commands,
                    &mut next_state,
                    menu.entries[index].level.clone(),
                );
            }
        }
        MenuCommand::QuickFaction(faction) => {
            menu.quick_faction = faction;
            menu.needs_rebuild = true;
        }
        MenuCommand::QuickDifficulty(difficulty) => {
            menu.quick_difficulty = difficulty;
            menu.needs_rebuild = true;
        }
        MenuCommand::StartQuickBattle => {
            let options = RandomBattleOptions {
                faction: menu.quick_faction,
                difficulty: DifficultyChoice::Fixed(menu.quick_difficulty),
                ..default()
            };
            let level = random_battle::generate(&options, rand::rng().random());
            start_level(&mut commands, &mut next_state, level);
        }
        MenuCommand::StartEditorBattle => {
            start_level(&mut commands, &mut next_state, menu.level_on_page())
        }
        MenuCommand::RerollRandomBattle => {
            menu.random_seed = rand::rng().random();
            menu.message = None;
            menu.needs_rebuild = true;
        }
        MenuCommand::SaveEditorLevel => save_the_editor_level(&mut menu),
        MenuCommand::EditMap => {
            let (level, file) = menu.level_to_edit();
            open_map_editor(&mut commands, &mut next_state, level, file);
        }
        MenuCommand::WatchReplay => {
            watch_the_selected_replay(&mut commands, &mut next_state, &mut menu)
        }
        MenuCommand::CycleLanguage => {
            switch_to_the_next_language(&mut settings);
            next_state.set(GameState::MainMenu);
        }
        MenuCommand::ContinueQuickSave => requests.load = Some(QUICK_SAVE_SLOT),
        MenuCommand::StartCampaignMission => {
            if let Selection::Mission { campaign, mission } = menu.selection {
                start_campaign_mission(
                    &mut commands,
                    &mut next_state,
                    &mut settings,
                    &menu,
                    campaign,
                    mission,
                );
            }
        }
        MenuCommand::CycleCampaignDifficulty => {
            menu.campaign_difficulty = cycle(&Difficulty::ALL, menu.campaign_difficulty, true);
            settings.campaign_difficulty = menu.campaign_difficulty;
            menu.needs_rebuild = true;
        }
        MenuCommand::AddToMyCampaign => add_the_selected_level_to_my_campaign(&mut menu),
        MenuCommand::ContinueCampaign => {
            if let Some((campaign, mission)) = menu.campaign_to_continue(&settings.last_campaign) {
                start_campaign_mission(
                    &mut commands,
                    &mut next_state,
                    &mut settings,
                    &menu,
                    campaign,
                    mission,
                );
            }
        }
        MenuCommand::OpenSettings => open_settings(&mut settings_menu),
        MenuCommand::BackToHub => menu.show_hub(),
        MenuCommand::DownloadUpdate => download_update(&mut updates),
        MenuCommand::HostNetworkBattle => {
            let players = menu.options.network_players;
            lobby.host(network_level(&menu.options, players), players);
        }
        MenuCommand::JoinNetworkBattle => lobby.join(&settings.join_address),
        MenuCommand::OpenServerRoom => {
            let players = menu.options.network_players;
            lobby.open_room(
                &settings.join_address,
                network_level(&menu.options, players),
                players,
            );
        }
        MenuCommand::PlayWorldAlone => {
            commands.insert_resource(WorldClient::alone(rand::rng().random()));
        }
        MenuCommand::EnterServerWorld => match WorldClient::on_server(&settings.join_address) {
            Ok(client) => commands.insert_resource(client),
            Err(reason) => lobby.fail(reason),
        },
        MenuCommand::AcceptNetworkOffer => lobby.accept_offer(),
        MenuCommand::StartNetworkBattle => lobby.start_battle(),
        MenuCommand::CancelNetworkBattle => lobby.cancel(),
        MenuCommand::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

pub(super) fn save_the_editor_level(menu: &mut MainMenu) {
    let level = menu.level_on_page();
    menu.message = Some(
        match save_custom_level(&custom_levels_directory(), &level) {
            Ok(path) => tr_fmt(
                "Saved as {} - it now appears under Custom levels.",
                &[&path.display()],
            ),
            Err(error) => tr_fmt("Could not save the level: {}", &[&error]),
        },
    );
    menu.entries = all_levels();
    menu.needs_rebuild = true;
}

pub(super) fn watch_the_selected_replay(
    commands: &mut Commands,
    next_state: &mut NextState<GameState>,
    menu: &mut MainMenu,
) {
    let Selection::Replay(index) = menu.selection else {
        return;
    };
    match load_replay(&menu.replays[index].path) {
        Ok(replay) => {
            commands.insert_resource(ActiveLevel(replay.level.clone()));
            commands.insert_resource(ReplayPlayback::new(replay));
            next_state.set(GameState::Replay);
        }
        Err(error) => {
            menu.message = Some(tr_fmt("Could not open the replay: {}", &[&error]));
            menu.needs_rebuild = true;
        }
    }
}

pub(super) fn switch_to_the_next_language(settings: &mut GameSettings) {
    let next = Language::ALL
        .iter()
        .position(|&language| language == settings.language)
        .map_or(0, |index| (index + 1) % Language::ALL.len());
    settings.language = Language::ALL[next];
    set_language(settings.language);
}

pub(super) fn add_the_selected_level_to_my_campaign(menu: &mut MainMenu) {
    let Selection::Level(index) = menu.selection else {
        return;
    };
    let entry = &menu.entries[index];
    let LevelSource::Custom(path) = &entry.source else {
        return;
    };
    let added = add_to_my_campaign(&custom_levels_directory(), path, entry.level.player_faction);
    menu.message = Some(match added {
        Ok(count) => tr_fmt(
            "Added - My campaign now has {} missions under Campaigns.",
            &[&count],
        ),
        Err(error) => tr_fmt("Could not add the level: {}", &[&error]),
    });
    let mut campaigns = built_in_campaigns();
    campaigns.extend(custom_campaigns(&custom_levels_directory()));
    menu.campaigns = campaigns;
    menu.needs_rebuild = true;
}
