use super::*;
use crate::sim::transport::{CARRIER_CAPACITY, Passengers};

#[derive(Component)]
pub struct InfoIcon;

#[derive(Component)]
pub struct InfoTitle;

#[derive(Component)]
pub struct InfoBody;

#[derive(Component)]
pub struct InfoHealthFill;

/// Icons of the selected kinds with their counts, when several units are selected.
#[derive(Component)]
pub struct InfoRoster;

pub(super) fn spawn_info_panel(sidebar: &mut ChildSpawnerCommands, fonts: &UiFonts) {
    sidebar
        .spawn((
            Node {
                min_height: Val::Px(104.0),
                padding: UiRect::all(Val::Px(8.0)),
                column_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL_RAISED),
            BorderColor::all(FRAME),
        ))
        .with_children(|panel| {
            panel.spawn((
                InfoIcon,
                ImageNode::default(),
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    flex_shrink: 0.0,
                    display: Display::None,
                    ..default()
                },
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|column| {
                    column.spawn((InfoTitle, fonts.heading("", 18.0, TEXT_MAIN)));
                    column.spawn((InfoBody, fonts.text("", 14.0, TEXT_DIM)));
                    column.spawn((
                        InfoRoster,
                        Node {
                            flex_wrap: FlexWrap::Wrap,
                            column_gap: Val::Px(4.0),
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                    ));
                    column
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(6.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.04, 0.05, 0.05)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                InfoHealthFill,
                                Node {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(GOOD),
                            ));
                        });
                });
        });
}

pub(super) fn spawn_roster_chip(
    row: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    asset_server: &AssetServer,
    kind: UnitKind,
    count: usize,
) {
    row.spawn((
        Node {
            align_items: AlignItems::Center,
            padding: UiRect::right(Val::Px(5.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(TILE),
        BorderColor::all(FRAME),
    ))
    .with_children(|chip| {
        chip.spawn((
            ImageNode::new(asset_server.load(format!("icons/{}.png", kind.spec().model_id))),
            Node {
                width: Val::Px(30.0),
                height: Val::Px(30.0),
                ..default()
            },
        ));
        chip.spawn(fonts.heading(format!("{count}"), 15.0, TEXT_MAIN));
    });
}

pub(super) fn harvest_activity(state: HarvestState) -> &'static str {
    match state {
        HarvestState::Holding => tr("idle"),
        HarvestState::SeekingField | HarvestState::TravellingToField(_) => {
            tr("heading to crystals")
        }
        HarvestState::Mining { .. } => tr("mining"),
        HarvestState::ReturningToRefinery(_) => tr("returning"),
        HarvestState::Unloading { .. } => tr("unloading"),
    }
}

/// The stance the armed units among a selection share, or that they differ; empty without armed units.
fn stance_line(stances: &[&Stance]) -> String {
    let Some(first) = stances.first() else {
        return String::new();
    };
    if stances.iter().all(|stance| stance == first) {
        tr_fmt("Stance: {}", &[&first.name()])
    } else {
        tr("Stance: mixed").to_string()
    }
}

pub fn refresh_info_panel(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    mut shown_roster: Local<Vec<(UnitKind, usize)>>,
    rosters: Query<Entity, With<InfoRoster>>,
    asset_server: Res<AssetServer>,
    selected: Query<
        (
            Option<&Unit>,
            Option<&Building>,
            &Health,
            Option<&Harvester>,
            Option<&Stance>,
            Option<&Passengers>,
        ),
        With<Selected>,
    >,
    mut icon: Query<(&mut ImageNode, &mut Node), (With<InfoIcon>, Without<InfoHealthFill>)>,
    mut title: Query<&mut Text, (With<InfoTitle>, Without<InfoBody>)>,
    mut body: Query<&mut Text, (With<InfoBody>, Without<InfoTitle>)>,
    mut health_fill: Query<(&mut Node, &mut BackgroundColor), With<InfoHealthFill>>,
    production: Res<Production>,
) {
    let era = production.era();
    let (
        Ok((mut icon_image, mut icon_node)),
        Ok(mut title),
        Ok(mut body),
        Ok((mut health_node, mut health_color)),
    ) = (
        icon.single_mut(),
        title.single_mut(),
        body.single_mut(),
        health_fill.single_mut(),
    )
    else {
        return;
    };
    let selection: Vec<_> = selected.iter().collect();
    let mut roster: Vec<(UnitKind, usize)> = Vec::new();

    let (shown_icon, heading, details, health) = match selection.as_slice() {
        [] => (
            None,
            tr("No selection").into(),
            tr("Select units with a click or a drag box.").into(),
            None,
        ),
        [(unit, building, health, harvester, stance, passengers)] => {
            let name = unit
                .map(|u| tr(u.0.spec().name))
                .or(building.map(|b| tr(b.kind.spec().name)))
                .unwrap_or_default();
            let path = unit
                .map(|u| format!("icons/{}.png", u.0.spec().model_id))
                .or(building.map(|b| format!("icons/{}.png", building_model(b.kind, era))));
            let role = unit
                .map(|u| tr(u.0.spec().role))
                .or(building.map(|b| tr(b.kind.spec().role)))
                .unwrap_or_default();
            let hp = format!("{:.0} / {:.0} HP", health.current.max(0.0), health.max);
            let condition = match harvester {
                Some(harvester) => tr_fmt(
                    "{}\nCargo {} / {}  ({})",
                    &[
                        &hp,
                        &harvester.cargo,
                        &HARVESTER_CAPACITY,
                        &harvest_activity(harvester.state),
                    ],
                ),
                None => hp,
            };
            let mut lines = vec![condition];
            lines.extend(stance.map(|stance| stance_line(&[stance])));
            lines.extend(passengers.map(|riding| {
                tr_fmt(
                    "Carrying {} of {} infantry",
                    &[&riding.0.len(), &CARRIER_CAPACITY],
                )
            }));
            lines.push(role.to_string());
            let details = lines.join("\n");
            (path, name.to_string(), details, Some(health.fraction()))
        }
        many => {
            for kind in many.iter().filter_map(|(unit, ..)| unit.map(|unit| unit.0)) {
                match roster.iter_mut().find(|(listed, _)| *listed == kind) {
                    Some((_, count)) => *count += 1,
                    None => roster.push((kind, 1)),
                }
            }
            roster.sort_by_key(|(kind, _)| UnitKind::ALL.iter().position(|k| k == kind));
            let average =
                many.iter().map(|(_, _, h, ..)| h.fraction()).sum::<f32>() / many.len() as f32;
            let stances: Vec<&Stance> = many.iter().filter_map(|(.., stance, _)| *stance).collect();
            (
                None,
                tr_fmt("{} units", &[&many.len()]),
                stance_line(&stances),
                Some(average),
            )
        }
    };

    title.set_if_neq(Text(heading));
    body.set_if_neq(Text(details));
    if *shown_roster != roster
        && let Ok(roster_node) = rosters.single()
    {
        commands.entity(roster_node).despawn_related::<Children>();
        commands.entity(roster_node).with_children(|row| {
            for &(kind, count) in &roster {
                spawn_roster_chip(row, &fonts, &asset_server, kind, count);
            }
        });
        *shown_roster = roster;
    }
    match shown_icon {
        Some(path) => {
            set_field(
                &mut icon_image,
                |icon| &mut icon.image,
                asset_server.load(path),
            );
            set_field(&mut icon_node, |node| &mut node.display, Display::Flex);
        }
        None => {
            set_field(&mut icon_node, |node| &mut node.display, Display::None);
        }
    }
    let fraction = health.unwrap_or(0.0);
    set_field(
        &mut health_node,
        |node| &mut node.width,
        Val::Percent(fraction * 100.0),
    );
    health_color.set_if_neq(BackgroundColor(match fraction {
        f if f > 0.6 => GOOD,
        f if f > 0.3 => WARNING,
        _ => DANGER,
    }));
}
