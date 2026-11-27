use super::*;
use crate::missions::levels::Personality;
use crate::sim::eras::Era;

#[derive(Component)]
pub struct EditorOption(EditorField);

#[derive(Component)]
pub struct RandomOption(RandomField);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RandomField {
    Opponents,
    Difficulty,
    MapSize,
    Credits,
    Faction,
}

impl RandomField {
    const ALL: [RandomField; 5] = [
        RandomField::Opponents,
        RandomField::Difficulty,
        RandomField::MapSize,
        RandomField::Credits,
        RandomField::Faction,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            RandomField::Opponents => tr("Enemy commanders"),
            RandomField::Difficulty => tr("Difficulty"),
            RandomField::MapSize => tr("Map size"),
            RandomField::Credits => tr("Starting credits"),
            RandomField::Faction => tr("Your faction"),
        }
    }

    pub(super) fn value(self, options: &RandomBattleOptions) -> String {
        match self {
            RandomField::Opponents => options.opponents.to_string(),
            RandomField::Difficulty => match options.difficulty {
                DifficultyChoice::Fixed(difficulty) => tr_owned(&format!("{difficulty:?}")),
                DifficultyChoice::Mixed => tr("Mixed").into(),
            },
            RandomField::MapSize => tr_fmt(
                "{} ({} x {} cells)",
                &[
                    &tr_owned(&format!("{:?}", options.map_size)),
                    &options.map_size.cells(),
                    &options.map_size.cells(),
                ],
            ),
            RandomField::Credits => format!("${}", options.credits),
            RandomField::Faction => match options.faction {
                FactionChoice::Fixed(faction) => tr_owned(&Team::Azure.name_in(faction)),
                FactionChoice::Random => tr("Random").into(),
            },
        }
    }

    pub(super) fn change(self, options: &mut RandomBattleOptions, forward: bool) {
        match self {
            RandomField::Opponents => {
                options.opponents = cycle(&[1, 2, 3], options.opponents, forward)
            }
            RandomField::Difficulty => {
                options.difficulty = cycle(&DifficultyChoice::ALL, options.difficulty, forward)
            }
            RandomField::MapSize => {
                options.map_size = cycle(&MapSize::ALL, options.map_size, forward)
            }
            RandomField::Credits => {
                options.credits = cycle(&CREDIT_CHOICES, options.credits, forward)
            }
            RandomField::Faction => {
                let choices = [
                    FactionChoice::Fixed(Faction::Coalition),
                    FactionChoice::Fixed(Faction::Syndicate),
                    FactionChoice::Random,
                ];
                options.faction = cycle(&choices, options.faction, forward);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorField {
    Enemies,
    Difficulty,
    Credits,
    Objective,
    Tech,
    Era,
    MapVariant,
    PlayerSite,
    FogOfWar,
    NeutralSites,
    Personality,
    Alliance,
    Landscape,
    Weather,
    Season,
    MapSize,
    NetworkPlayers,
    Faction,
}

pub(super) const EDITOR_FIELDS: [EditorField; 18] = [
    EditorField::Enemies,
    EditorField::Difficulty,
    EditorField::Personality,
    EditorField::Credits,
    EditorField::Objective,
    EditorField::Era,
    EditorField::Tech,
    EditorField::MapVariant,
    EditorField::PlayerSite,
    EditorField::Faction,
    EditorField::FogOfWar,
    EditorField::NeutralSites,
    EditorField::Alliance,
    EditorField::Landscape,
    EditorField::Weather,
    EditorField::Season,
    EditorField::MapSize,
    EditorField::NetworkPlayers,
];

fn on_or_off(on: bool) -> String {
    if on { tr("On") } else { tr("Off") }.into()
}

pub(super) fn cycle<T: Copy + PartialEq>(choices: &[T], current: T, forward: bool) -> T {
    let index = choices.iter().position(|&c| c == current).unwrap_or(0);
    let next = if forward {
        (index + 1) % choices.len()
    } else {
        (index + choices.len() - 1) % choices.len()
    };
    choices[next]
}

impl EditorField {
    pub(super) fn label(self) -> &'static str {
        match self {
            EditorField::Enemies => tr("Enemy commanders"),
            EditorField::Difficulty => tr("Difficulty"),
            EditorField::Credits => tr("Starting credits"),
            EditorField::Objective => tr("Objective"),
            EditorField::Tech => tr("Available units"),
            EditorField::Era => tr("Age"),
            EditorField::MapVariant => tr("Map variant"),
            EditorField::PlayerSite => tr("Your base"),
            EditorField::FogOfWar => tr("Fog of war"),
            EditorField::NeutralSites => tr("Neutral buildings"),
            EditorField::Personality => tr("Commander style"),
            EditorField::Alliance => tr("Opponents"),
            EditorField::Landscape => tr("Landscape"),
            EditorField::Weather => tr("Weather"),
            EditorField::Season => tr("Season"),
            EditorField::MapSize => tr("Map size"),
            EditorField::NetworkPlayers => tr("Network players"),
            EditorField::Faction => tr("Your faction"),
        }
    }

    pub(super) fn value(self, options: &SkirmishOptions) -> String {
        match self {
            EditorField::Enemies => options.enemies.to_string(),
            EditorField::Difficulty => tr_owned(&format!("{:?}", options.difficulty)),
            EditorField::Credits => format!("${}", options.credits),
            EditorField::Objective => match options.objective {
                ObjectiveKind::Annihilate => tr("Destroy everything").into(),
                ObjectiveKind::Decapitate => tr("Destroy Command Centers").into(),
                ObjectiveKind::Survive => tr("Survive 10 minutes").into(),
                ObjectiveKind::Economy => tr("Stockpile credits").into(),
            },
            EditorField::Tech => options.tech.label().into(),
            EditorField::Era => options.era.label().into(),
            EditorField::MapVariant => tr_fmt("{} of {}", &[&options.map_variant, &MAP_VARIANTS]),
            EditorField::PlayerSite => options.player_site.name().into(),
            EditorField::FogOfWar => on_or_off(options.fog_of_war),
            EditorField::NeutralSites => on_or_off(options.neutral_sites),
            EditorField::Personality => options
                .personality
                .map_or(tr("Mixed"), |personality| personality.name())
                .into(),
            EditorField::Alliance => alliance_name(options.alliance).into(),
            EditorField::Landscape => tr_owned(&format!("{:?}", options.landscape)),
            EditorField::Weather => tr_owned(&format!("{:?}", options.weather)),
            EditorField::Season => tr_owned(&format!("{:?}", options.season)),
            EditorField::MapSize => tr_fmt(
                "{} ({} x {} cells)",
                &[
                    &tr_owned(&format!("{:?}", options.map_size)),
                    &options.map_size.cells(),
                    &options.map_size.cells(),
                ],
            ),
            EditorField::NetworkPlayers => options.network_players.to_string(),
            EditorField::Faction => tr_owned(&Team::Azure.name_in(options.player_faction)),
        }
    }

    pub(super) fn change(self, options: &mut SkirmishOptions, forward: bool) {
        match self {
            EditorField::Enemies => options.enemies = cycle(&[1, 2, 3], options.enemies, forward),
            EditorField::Difficulty => {
                options.difficulty = cycle(&Difficulty::ALL, options.difficulty, forward);
            }
            EditorField::Credits => {
                options.credits = cycle(&CREDIT_CHOICES, options.credits, forward)
            }
            EditorField::Objective => {
                let kinds = [
                    ObjectiveKind::Annihilate,
                    ObjectiveKind::Decapitate,
                    ObjectiveKind::Survive,
                    ObjectiveKind::Economy,
                ];
                options.objective = cycle(&kinds, options.objective, forward);
            }
            EditorField::Tech => {
                options.tech = cycle(
                    &[TechLevel::Basic, TechLevel::Standard, TechLevel::Full],
                    options.tech,
                    forward,
                );
            }
            EditorField::Era => {
                let eras: Vec<Era> = Era::playable().collect();
                options.era = cycle(&eras, options.era, forward);
            }
            EditorField::MapVariant => {
                let variants: Vec<u64> = (1..=MAP_VARIANTS).collect();
                options.map_variant = cycle(&variants, options.map_variant, forward);
            }
            EditorField::PlayerSite => {
                options.player_site = cycle(&BaseSite::ALL, options.player_site, forward)
            }
            EditorField::FogOfWar => options.fog_of_war = !options.fog_of_war,
            EditorField::NeutralSites => options.neutral_sites = !options.neutral_sites,
            EditorField::Personality => {
                let choices: Vec<Option<Personality>> = std::iter::once(None)
                    .chain(Personality::ALL.map(Some))
                    .collect();
                options.personality = cycle(&choices, options.personality, forward);
            }
            EditorField::Alliance => {
                options.alliance = cycle(&Alliance::ALL, options.alliance, forward);
            }
            EditorField::Landscape => {
                options.landscape = cycle(&Landscape::ALL, options.landscape, forward);
            }
            EditorField::Weather => {
                options.weather = cycle(&[Weather::Clear, Weather::Rain], options.weather, forward);
            }
            EditorField::Season => {
                options.season = cycle(&Season::ALL, options.season, forward);
            }
            EditorField::MapSize => {
                options.map_size = cycle(&MapSize::ALL, options.map_size, forward);
            }
            EditorField::NetworkPlayers => {
                let counts: Vec<usize> = (2..=MOST_PLAYERS).collect();
                options.network_players = cycle(&counts, options.network_players, forward);
            }
            EditorField::Faction => options.player_faction = options.player_faction.opposite(),
        }
    }
}

pub(super) fn minutes(secs: f32) -> String {
    format!("{}:{:02}", secs as u32 / 60, secs as u32 % 60)
}

pub(super) fn spawn_editor_fields(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    options: &SkirmishOptions,
    fields: &[EditorField],
) {
    let rows = fields
        .iter()
        .map(|&field| (field.label(), field.value(options), EditorOption(field)));
    spawn_option_grid(panel, fonts, rows);
}

pub(super) fn alliance_name(alliance: Alliance) -> &'static str {
    match alliance {
        Alliance::CoalitionAgainstSyndicate => tr("Allied"),
        Alliance::FreeForAll => tr("Free-for-all"),
        Alliance::TwoAgainstTwo => tr("Two against two"),
    }
}

/// One click to a battle: a side and a difficulty, everything else rolled anew each time.
pub(super) fn spawn_quick_battle(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    menu: &MainMenu,
) {
    panel.spawn(fonts.heading(tr("Quick battle"), 30.0, ACCENT));
    panel.spawn(fonts.text(
        tr("Pick a side and a difficulty. One opponent on a standard map; the land, season, weather and corners are rolled every time."),
        16.0,
        TEXT_DIM,
    ));
    spawn_side_and_difficulty(panel, fonts, menu);
    panel.spawn(Node {
        height: Val::Px(12.0),
        ..default()
    });
    spawn_command_button(
        panel,
        fonts,
        tr("START QUICK BATTLE"),
        MenuCommand::StartQuickBattle,
    );
}

/// The side and difficulty choices the quick battle and the mission generator share.
pub(super) fn spawn_side_and_difficulty(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    menu: &MainMenu,
) {
    let factions = [
        (
            FactionChoice::Fixed(Faction::Coalition),
            tr(Faction::Coalition.title()),
        ),
        (
            FactionChoice::Fixed(Faction::Syndicate),
            tr(Faction::Syndicate.title()),
        ),
        (FactionChoice::Random, tr("RANDOM")),
    ];
    spawn_choice_row(
        panel,
        fonts,
        tr("Your side"),
        factions.into_iter().map(|(choice, label)| {
            (
                label.to_string(),
                MenuCommand::QuickFaction(choice),
                choice == menu.quick_faction,
            )
        }),
    );
    spawn_choice_row(
        panel,
        fonts,
        tr("Difficulty"),
        Difficulty::ALL.into_iter().map(|difficulty| {
            (
                tr_owned(&format!("{difficulty:?}")),
                MenuCommand::QuickDifficulty(difficulty),
                difficulty == menu.quick_difficulty,
            )
        }),
    );
}

/// A whole mission rolled from one seed: the battle, an objective, an operation name and its briefing.
pub(super) fn spawn_generated_mission(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    menu: &MainMenu,
    preview: Option<Handle<Image>>,
) {
    panel.spawn(fonts.heading(tr("Mission generator"), 30.0, ACCENT));
    spawn_side_and_difficulty(panel, fonts, menu);
    let level = menu.generated_mission();
    beside_preview(panel, preview, SMALL_PREVIEW, |column| {
        column.spawn(fonts.heading(&level.name, 20.0, TEXT_MAIN));
        column.spawn(fonts.text(&level.briefing, 15.0, TEXT_DIM));
        spawn_level_facts(column, fonts, &level);
    });
    spawn_command_button(
        panel,
        fonts,
        tr("START MISSION"),
        MenuCommand::StartEditorBattle,
    );
    spawn_command_button(
        panel,
        fonts,
        tr("NEW MISSION"),
        MenuCommand::RerollRandomBattle,
    );
    spawn_command_button(
        panel,
        fonts,
        tr("SAVE AS CUSTOM LEVEL"),
        MenuCommand::SaveEditorLevel,
    );
}

pub(super) fn spawn_random_battle_details(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    menu: &MainMenu,
    preview: Option<Handle<Image>>,
) {
    panel.spawn(fonts.heading(tr("Random battle"), 30.0, ACCENT));
    panel.spawn(fonts.text(
        tr("Choose the opponents, map size and credits; the landscape, season, weather, corners, alliances and the forests, rocks and crystals are rolled. New map rolls them again."),
        15.0,
        TEXT_DIM,
    ));
    let rows = RandomField::ALL.map(|field| {
        (
            field.label(),
            field.value(&menu.random),
            RandomOption(field),
        )
    });
    spawn_option_grid(panel, fonts, rows);
    let level = menu.random_level();
    beside_preview(panel, preview, SMALL_PREVIEW, |column| {
        column.spawn(fonts.heading(&level.name, 20.0, TEXT_MAIN));
        spawn_level_facts(column, fonts, &level);
    });
    spawn_command_button(
        panel,
        fonts,
        tr("START BATTLE"),
        MenuCommand::StartEditorBattle,
    );
    spawn_command_button(panel, fonts, tr("NEW MAP"), MenuCommand::RerollRandomBattle);
    spawn_command_button(
        panel,
        fonts,
        tr("SAVE AS CUSTOM LEVEL"),
        MenuCommand::SaveEditorLevel,
    );
    spawn_command_button(panel, fonts, tr("EDIT MAP"), MenuCommand::EditMap);
}

pub(super) fn change_option(
    click: On<PointerClick>,
    fields: Query<&EditorOption>,
    random_fields: Query<&RandomOption>,
    mut menu: ResMut<MainMenu>,
    mut lobby: ResMut<NetLobby>,
) {
    let forward = click.button != PointerButton::Secondary;
    if let Ok(field) = random_fields.get(click.entity) {
        field.0.change(&mut menu.random, forward);
        menu.message = None;
        menu.needs_rebuild = true;
        return;
    }
    let Ok(field) = fields.get(click.entity) else {
        return;
    };
    field.0.change(&mut menu.options, forward);
    if lobby.is_hosting() {
        let players = menu.options.network_players;
        lobby.reoffer(network_level(&menu.options, players));
    }
    menu.message = None;
    menu.needs_rebuild = true;
}
