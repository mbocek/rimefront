use super::*;

pub(super) const TILE: Color = Color::srgb(0.13, 0.15, 0.17);
pub(super) const TILE_HOVERED: Color = Color::srgb(0.18, 0.21, 0.24);
pub(super) const TILE_READY: Color = Color::srgb(0.12, 0.30, 0.16);
pub(super) const SHADE: Color = Color::srgba(0.0, 0.0, 0.0, 0.62);
pub(super) const TILE_WIDTH: f32 = 88.0;
pub(super) const TILE_HEIGHT: f32 = 76.0;
pub(super) const TILE_ICON_SIZE: f32 = 42.0;
pub(super) const TOOLTIP_WIDTH: f32 = 300.0;
pub(super) const SHIFT_CLICK_BATCH: u32 = 5;

#[derive(Resource)]
pub struct ActiveTab(pub(super) Lane);

#[derive(Component)]
pub struct BuildTile(BuildItem);

#[derive(Component)]
pub struct TileShade(BuildItem);

#[derive(Component)]
pub struct TileBadge(BuildItem);

#[derive(Component)]
pub struct TileIcon(BuildItem);

#[derive(Component)]
pub struct TabButton(Lane);

pub(super) fn lane_items(lane: Lane, faction: Faction) -> Vec<BuildItem> {
    let items = match lane {
        Lane::Structures => BuildingKind::CONSTRUCTIBLE
            .map(BuildItem::Structure)
            .to_vec(),
        Lane::Infantry => UnitKind::INFANTRY.map(BuildItem::Unit).to_vec(),
        Lane::Vehicles => UnitKind::VEHICLES.map(BuildItem::Unit).to_vec(),
        Lane::Research => Upgrade::ALL.map(BuildItem::Research).to_vec(),
    };
    items
        .into_iter()
        .filter(|item| item.available_to(faction))
        .collect()
}

pub(super) fn lane_title(lane: Lane) -> &'static str {
    match lane {
        Lane::Structures => tr("BASE"),
        Lane::Infantry => tr("INFANTRY"),
        Lane::Vehicles => tr("VEHICLES"),
        Lane::Research => tr("RESEARCH"),
    }
}

pub(super) fn spawn_tabs(sidebar: &mut ChildSpawnerCommands, fonts: &UiFonts) {
    sidebar
        .spawn(Node {
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|tabs| {
            for lane in Lane::ALL {
                tabs.spawn((
                    Hovered::default(),
                    TabButton(lane),
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(30.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(TILE),
                    BorderColor::all(FRAME),
                ))
                .with_children(|tab| {
                    tab.spawn(fonts.heading(lane_title(lane), 15.0, TEXT_MAIN));
                })
                .observe(select_tab);
            }
        });
}

pub(super) fn spawn_build_grid(
    sidebar: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    asset_server: &AssetServer,
    faction: Faction,
) {
    sidebar
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            flex_grow: 1.0,
            align_content: AlignContent::FlexStart,
            ..default()
        })
        .with_children(|grid| {
            for item in Lane::ALL
                .into_iter()
                .flat_map(|lane| lane_items(lane, faction))
            {
                spawn_build_tile(grid, fonts, asset_server, item);
            }
        });
}

pub(super) fn spawn_build_tile(
    grid: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    asset_server: &AssetServer,
    item: BuildItem,
) {
    grid.spawn((
        Hovered::default(),
        BuildTile(item),
        Node {
            width: Val::Px(TILE_WIDTH),
            min_height: Val::Px(TILE_HEIGHT),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(3.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(TILE),
        BorderColor::all(FRAME),
    ))
    .with_children(|tile| {
        tile.spawn((
            TileIcon(item),
            ImageNode::new(asset_server.load(icon_path(item))),
            Node {
                width: Val::Px(TILE_ICON_SIZE),
                height: Val::Px(TILE_ICON_SIZE),
                ..default()
            },
        ));
        tile.spawn((
            fonts.heading(item.name(), 12.0, TEXT_MAIN),
            TextLayout::justify(Justify::Center),
        ));
        tile.spawn((
            fonts.heading(format!("${}", item.cost()), 12.0, ACCENT),
            Node {
                margin: UiRect::top(Val::Auto),
                ..default()
            },
        ));
        tile.spawn((
            TileShade(item),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(0.0),
                ..default()
            },
            BackgroundColor(SHADE),
        ));
        tile.spawn((
            TileBadge(item),
            fonts.heading("", 12.0, GOOD),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(2.0),
                right: Val::Px(4.0),
                ..default()
            },
        ));
    })
    .observe(click_build_tile);
}

#[derive(Component)]
pub struct BuildTooltip;

#[derive(Component)]
pub struct BuildTooltipTitle;

#[derive(Component)]
pub struct BuildTooltipBody;

pub(super) fn spawn_build_tooltip(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            BuildTooltip,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(SIDEBAR_WIDTH + 8.0),
                width: Val::Px(TOOLTIP_WIDTH),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL.with_alpha(0.95)),
            BorderColor::all(FRAME_BRIGHT),
            Pickable::IGNORE,
            GlobalZIndex(10),
        ))
        .with_children(|tooltip| {
            tooltip.spawn((
                BuildTooltipTitle,
                fonts.heading("", 18.0, ACCENT),
                Pickable::IGNORE,
            ));
            tooltip.spawn((
                BuildTooltipBody,
                fonts.text("", 15.0, TEXT_MAIN),
                Pickable::IGNORE,
            ));
        });
}

/// A build tile under the pointer is described in a box beside the sidebar, level with the pointer.
pub fn refresh_build_tooltip(
    local: Res<LocalTeam>,
    cursor: Res<crate::view::camera::Cursor>,
    production: Res<Production>,
    buildings: Query<(&Building, &Team)>,
    hovered_tiles: Query<(&Hovered, &BuildTile)>,
    mut tooltips: Query<&mut Node, With<BuildTooltip>>,
    mut title: Query<&mut Text, (With<BuildTooltipTitle>, Without<BuildTooltipBody>)>,
    mut body: Query<&mut Text, (With<BuildTooltipBody>, Without<BuildTooltipTitle>)>,
) {
    let Ok(mut node) = tooltips.single_mut() else {
        return;
    };
    let hovered = hovered_tiles
        .iter()
        .find(|(hovered, _)| hovered.get())
        .map(|(_, tile)| tile.0);
    let (Some(item), Some(screen)) = (hovered, cursor.screen) else {
        set_field(&mut node, |node| &mut node.display, Display::None);
        return;
    };
    let owned = owned_building_kinds(&buildings, local.0);
    let (heading, details) = describe_item(item, &owned, &production, local.0);
    if let Ok(mut title) = title.single_mut() {
        title.set_if_neq(Text(heading));
    }
    if let Ok(mut body) = body.single_mut() {
        body.set_if_neq(Text(details));
    }
    set_field(&mut node, |node| &mut node.display, Display::Flex);
    set_field(
        &mut node,
        |node| &mut node.top,
        Val::Px((screen.y - 30.0).max(8.0)),
    );
}

pub(super) fn select_tab(
    click: On<PointerClick>,
    tabs: Query<&TabButton>,
    mut active: ResMut<ActiveTab>,
) {
    if let Ok(tab) = tabs.get(click.entity) {
        active.0 = tab.0;
    }
}

pub(super) fn click_build_tile(
    mut click: On<PointerClick>,
    mut cues: MessageWriter<SoundCue>,
    mut issuer: CommandIssuer,
    keys: Res<ButtonInput<KeyCode>>,
    tiles: Query<&BuildTile>,
    production: Res<Production>,
    mut placement: ResMut<Placement>,
    mut notifications: ResMut<Notifications>,
    outcome: Res<GameOutcome>,
    buildings: Query<(&Building, &Team)>,
) {
    let Ok(&BuildTile(item)) = tiles.get(click.entity) else {
        return;
    };
    click.propagate(false);
    if *outcome != GameOutcome::Ongoing {
        return;
    }
    let team = issuer.team();
    if click.button == PointerButton::Secondary {
        issuer.issue(PlayerCommand::Cancel { item });
        if let BuildItem::Structure(kind) = item
            && placement.active == Some(kind)
        {
            placement.active = None;
        }
        return;
    }
    if click.button != PointerButton::Primary {
        return;
    }
    let owned = owned_building_kinds(&buildings, team);
    if let BuildItem::Structure(kind) = item
        && production.lane(team, item.lane()).ready == Some(kind)
    {
        placement.active = Some(kind);
        return;
    }
    if let BuildItem::Structure(kind) = item
        && kind.is_wall()
    {
        let refusal = if !production.allows(team, item) {
            Some(EnqueueRefusal::NotInMission.message())
        } else {
            item.missing_requirement(&owned)
                .map(|missing| tr_fmt("Requires {}", &[&tr(missing.spec().name)]))
        };
        match refusal {
            Some(message) => {
                notifications.push(message);
                cues.write(SoundCue::interface(Sound::UiError));
            }
            None => placement.active = Some(kind),
        }
        return;
    }
    if let Some(missing) = item.missing_requirement(&owned) {
        notifications.push(tr_fmt("Requires {}", &[&tr(missing.spec().name)]));
        return;
    }
    let batch = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        SHIFT_CLICK_BATCH
    } else {
        1
    };
    issuer.issue(PlayerCommand::Enqueue {
        item,
        copies: batch,
    });
}

pub(super) enum TileState {
    NotInMission,
    Locked,
    Ready,
    Building { fraction: f32, queued: usize },
    Queued(usize),
    Unaffordable,
    Available,
    Researched,
}

pub(super) fn tile_state(
    item: BuildItem,
    production: &Production,
    owned: &[BuildingKind],
    credits: i32,
    team: Team,
) -> TileState {
    if !production.allows(team, item) {
        return TileState::NotInMission;
    }
    if let BuildItem::Research(upgrade) = item
        && production.has_researched(team, upgrade)
    {
        return TileState::Researched;
    }
    if item.missing_requirement(owned).is_some() {
        return TileState::Locked;
    }
    let lane = production.lane(team, item.lane());
    if let BuildItem::Structure(kind) = item
        && lane.ready == Some(kind)
    {
        return TileState::Ready;
    }
    let queued = lane.queue.iter().filter(|&&q| q == item).count();
    if lane.queue.front() == Some(&item) {
        return TileState::Building {
            fraction: lane.progress_fraction(),
            queued,
        };
    }
    if queued > 0 {
        return TileState::Queued(queued);
    }
    if credits < item.cost() {
        return TileState::Unaffordable;
    }
    TileState::Available
}

pub fn refresh_build_tiles(
    local: Res<LocalTeam>,
    time: Res<Time>,
    active: Res<ActiveTab>,
    production: Res<Production>,
    economies: Res<Economies>,
    buildings: Query<(&Building, &Team)>,
    mut tiles: Query<(
        &BuildTile,
        &Hovered,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut shades: Query<(&TileShade, &mut Node), Without<BuildTile>>,
    mut badges: Query<(&TileBadge, &mut Text, &mut TextColor)>,
    mut icons: Query<(&TileIcon, &mut ImageNode)>,
    mut tabs: Query<
        (&TabButton, &Hovered, &mut BackgroundColor, &mut BorderColor),
        Without<BuildTile>,
    >,
) {
    let owned = owned_building_kinds(&buildings, local.0);
    let credits = economies.of(local.0).credits;
    let team = local.0;
    let state_of = |item| tile_state(item, &production, &owned, credits, team);
    let pulse = (time.elapsed_secs() * 5.0).sin() * 0.5 + 0.5;

    for (tile, hovered, mut node, mut background, mut border) in &mut tiles {
        let display = if tile.0.lane() == active.0 {
            Display::Flex
        } else {
            Display::None
        };
        set_field(&mut node, |node| &mut node.display, display);
        let hovered = hovered.get();
        let ready = matches!(state_of(tile.0), TileState::Ready);
        background.set_if_neq(BackgroundColor(match (ready, hovered) {
            (true, _) => TILE_READY,
            (false, true) => TILE_HOVERED,
            (false, false) => TILE,
        }));
        border.set_if_neq(BorderColor::all(if ready {
            GOOD.mix(&TEXT_MAIN, pulse * 0.5)
        } else if hovered {
            ACCENT
        } else {
            FRAME
        }));
    }
    for (shade, mut node) in &mut shades {
        let covered = match state_of(shade.0) {
            TileState::NotInMission | TileState::Locked | TileState::Unaffordable => 1.0,
            TileState::Building { fraction, .. } => 1.0 - fraction,
            _ => 0.0,
        };
        set_field(
            &mut node,
            |node| &mut node.height,
            Val::Percent(covered * 100.0),
        );
    }
    for (badge, mut text, mut color) in &mut badges {
        let (label, tint) = match state_of(badge.0) {
            TileState::Ready => (tr("READY").to_string(), GOOD),
            TileState::Building { fraction, queued } if queued > 1 => (
                format!("{}%  x{queued}", (fraction * 100.0) as u32),
                TEXT_MAIN,
            ),
            TileState::Building { fraction, .. } => {
                (format!("{}%", (fraction * 100.0) as u32), TEXT_MAIN)
            }
            TileState::Queued(count) => (format!("x{count}"), TEXT_MAIN),
            TileState::Locked => (tr("LOCKED").to_string(), TEXT_DIM),
            TileState::NotInMission => (tr("N/A").to_string(), TEXT_DIM),
            TileState::Researched => (tr("RESEARCHED").to_string(), GOOD),
            TileState::Unaffordable | TileState::Available => (String::new(), TEXT_MAIN),
        };
        text.set_if_neq(Text(label));
        color.set_if_neq(TextColor(tint));
    }
    for (icon, mut image) in &mut icons {
        let locked = matches!(
            state_of(icon.0),
            TileState::Locked | TileState::NotInMission
        );
        image.color = if locked {
            Color::srgba(0.5, 0.5, 0.5, 0.5)
        } else {
            Color::WHITE
        };
    }
    for (tab, hovered, mut background, mut border) in &mut tabs {
        let has_ready = lane_items(tab.0, production.faction_of(team))
            .into_iter()
            .any(|item| matches!(state_of(item), TileState::Ready));
        let selected = tab.0 == active.0;
        background.0 = if selected {
            PANEL_RAISED
        } else if hovered.get() {
            TILE_HOVERED
        } else {
            TILE
        };
        *border = BorderColor::all(if has_ready {
            GOOD.mix(&TEXT_MAIN, pulse * 0.5)
        } else if selected {
            ACCENT
        } else {
            FRAME
        });
    }
}

pub(super) fn describe_item(
    item: BuildItem,
    owned: &[BuildingKind],
    production: &Production,
    team: Team,
) -> (String, String) {
    let (role, build_secs, extra) = match item {
        BuildItem::Structure(kind) => {
            let spec = kind.spec();
            let power = if spec.power >= 0 {
                tr_fmt("+{} power", &[&spec.power])
            } else {
                tr_fmt("{} power", &[&spec.power])
            };
            (tr(spec.role), spec.build_secs, power)
        }
        BuildItem::Unit(kind) => {
            let spec = kind.spec();
            (
                spec.role,
                spec.build_secs,
                format!("{:.0} HP", spec.max_health),
            )
        }
        BuildItem::Research(upgrade) => {
            let spec = upgrade.spec();
            let status = if production.has_researched(team, upgrade) {
                tr("researched").to_string()
            } else {
                String::new()
            };
            (spec.role, spec.research_secs, status)
        }
    };
    let requirement = if !production.allows(team, item) {
        "\nNot available in this mission".to_string()
    } else {
        item.missing_requirement(owned)
            .map(|missing| format!("\n{}", tr_fmt("Requires {}", &[&tr(missing.spec().name)])))
            .unwrap_or_default()
    };
    let built_at = match item {
        BuildItem::Unit(kind) => format!(
            "\n{}",
            tr_fmt("Built at {}", &[&tr(kind.spec().producer.spec().name)])
        ),
        _ => String::new(),
    };
    (
        format!("{}  ${}", item.name(), item.cost()),
        tr_fmt(
            "{}\n{}s build, {}{}",
            &[
                &role,
                &format!("{build_secs:.0}"),
                &extra,
                &format!("{requirement}{built_at}"),
            ],
        ),
    )
}
