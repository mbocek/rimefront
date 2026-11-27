use bevy::prelude::*;

use crate::app::i18n::tr;
use crate::sim::catalog::{BuildingKind, Team};
use crate::sim::player_commands::LocalTeam;
use crate::sim::player_commands::{CommandIssuer, PlayerCommand};
use crate::sim::production::{BuildItem, Lane, Production};
use crate::ui::placement::Placement;
use crate::ui::sidebar::{
    ACCENT, FRAME, GOOD, PANEL, SIDEBAR_WIDTH, TEXT_DIM, UiFonts, icon_path, set_field,
};

const SLOT_ICON_SIZE: f32 = 50.0;
const PROGRESS_BAR_HEIGHT: f32 = 5.0;
const STRUCTURE_LANE_COLOR: Color = Color::srgb(0.96, 0.74, 0.26);
const INFANTRY_LANE_COLOR: Color = Color::srgb(0.55, 0.80, 0.45);
const VEHICLE_LANE_COLOR: Color = Color::srgb(0.40, 0.65, 1.0);
const RESEARCH_LANE_COLOR: Color = Color::srgb(0.75, 0.55, 1.0);
const WAITING_ICON_TINT: Color = Color::srgba(0.75, 0.75, 0.75, 0.7);

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum QueueSlot {
    AwaitingPlacement(BuildingKind),
    Queued { lane: Lane, position: usize },
}

#[derive(Component)]
pub struct QueueBar;

#[derive(Component)]
pub struct QueueSlotProgress(QueueSlot);

fn lane_color(lane: Lane) -> Color {
    match lane {
        Lane::Structures => STRUCTURE_LANE_COLOR,
        Lane::Infantry => INFANTRY_LANE_COLOR,
        Lane::Vehicles => VEHICLE_LANE_COLOR,
        Lane::Research => RESEARCH_LANE_COLOR,
    }
}

fn player_queue(production: &Production, team: Team) -> Vec<(QueueSlot, BuildItem)> {
    let mut entries = Vec::new();
    for lane in Lane::ALL {
        let lane_state = production.lane(team, lane);
        if let Some(kind) = lane_state.ready {
            entries.push((
                QueueSlot::AwaitingPlacement(kind),
                BuildItem::Structure(kind),
            ));
        }
        for (position, &item) in lane_state.queue.iter().enumerate() {
            entries.push((QueueSlot::Queued { lane, position }, item));
        }
    }
    entries
}

pub fn spawn_queue_bar(commands: &mut Commands, fonts: &UiFonts) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Px(0.0),
                right: Val::Px(SIDEBAR_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|anchor| {
            anchor
                .spawn((
                    QueueBar,
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL.with_alpha(0.88)),
                    BorderColor::all(FRAME),
                    Visibility::Hidden,
                ))
                .with_children(|bar| {
                    bar.spawn(fonts.heading(tr("QUEUE"), 15.0, TEXT_DIM));
                });
        });
}

/// Rebuilds the slot row whenever the queue's contents change; progress is animated separately.
pub fn rebuild_queue_bar(
    local: Res<LocalTeam>,
    mut commands: Commands,
    production: Res<Production>,
    fonts: Res<UiFonts>,
    asset_server: Res<AssetServer>,
    mut shown: Local<Vec<(QueueSlot, BuildItem)>>,
    mut bars: Query<(Entity, &mut Visibility), With<QueueBar>>,
    slots: Query<Entity, With<QueueSlot>>,
) {
    let entries = player_queue(&production, local.0);
    if *shown == entries {
        return;
    }
    let Ok((bar, mut visibility)) = bars.single_mut() else {
        return;
    };
    for slot in &slots {
        commands.entity(slot).despawn();
    }
    for &(slot, item) in &entries {
        let icon = asset_server.load(icon_path(item, production.era()));
        spawn_slot(&mut commands, &fonts, icon, bar, slot);
    }
    *visibility = if entries.is_empty() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    *shown = entries;
}

fn spawn_slot(
    commands: &mut Commands,
    fonts: &UiFonts,
    icon: Handle<Image>,
    bar: Entity,
    slot: QueueSlot,
) {
    let (frame_color, icon_tint) = match slot {
        QueueSlot::AwaitingPlacement(_) => (GOOD, Color::WHITE),
        QueueSlot::Queued { lane, position: 0 } => (lane_color(lane), Color::WHITE),
        QueueSlot::Queued { lane, .. } => (lane_color(lane).with_alpha(0.5), WAITING_ICON_TINT),
    };
    commands
        .spawn((
            slot,
            ChildOf(bar),
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(2.0)),
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.11, 0.13, 0.15)),
            BorderColor::all(frame_color),
        ))
        .with_children(|tile| {
            tile.spawn((
                ImageNode::new(icon).with_color(icon_tint),
                Node {
                    width: Val::Px(SLOT_ICON_SIZE),
                    height: Val::Px(SLOT_ICON_SIZE),
                    ..default()
                },
            ));
            tile.spawn((
                Node {
                    width: Val::Px(SLOT_ICON_SIZE),
                    height: Val::Px(PROGRESS_BAR_HEIGHT),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.04, 0.05, 0.05)),
            ))
            .with_children(|track| {
                track.spawn((
                    QueueSlotProgress(slot),
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(frame_color),
                ));
            });
            if matches!(slot, QueueSlot::AwaitingPlacement(_)) {
                tile.spawn((
                    fonts.heading(tr("READY"), 13.0, ACCENT),
                    TextShadow::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(2.0),
                        left: Val::Px(4.0),
                        ..default()
                    },
                ));
            }
        })
        .observe(click_queue_slot);
}

pub fn animate_queue_progress(
    local: Res<LocalTeam>,
    production: Res<Production>,
    mut fills: Query<(&QueueSlotProgress, &mut Node)>,
) {
    for (progress, mut node) in &mut fills {
        let fraction = match progress.0 {
            QueueSlot::AwaitingPlacement(_) => 1.0,
            QueueSlot::Queued { lane, position: 0 } => {
                production.lane(local.0, lane).progress_fraction()
            }
            QueueSlot::Queued { .. } => 0.0,
        };
        set_field(
            &mut node,
            |node| &mut node.width,
            Val::Percent(fraction * 100.0),
        );
    }
}

fn click_queue_slot(
    click: On<PointerClick>,
    slots: Query<&QueueSlot>,
    mut issuer: CommandIssuer,
    mut placement: ResMut<Placement>,
) {
    let Ok(&slot) = slots.get(click.entity) else {
        return;
    };
    match (slot, click.button) {
        (QueueSlot::AwaitingPlacement(kind), PointerButton::Primary) => {
            placement.active = Some(kind)
        }
        (QueueSlot::AwaitingPlacement(kind), PointerButton::Secondary) => {
            issuer.issue(PlayerCommand::CancelReadyStructure);
            if placement.active == Some(kind) {
                placement.active = None;
            }
        }
        (QueueSlot::Queued { lane, position }, PointerButton::Secondary) => {
            issuer.issue(PlayerCommand::CancelQueued { lane, position });
        }
        _ => {}
    }
}
