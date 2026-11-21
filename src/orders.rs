use bevy::prelude::*;

use crate::camera::{Cursor, RtsCamera};
use crate::catalog::{ArmorClass, BuildingKind, Diplomacy, Team, UnitKind};
use crate::combat::{TargetInfo, Targets};
use crate::components::*;
use crate::determinism::SimId;
use crate::engineers::{EngineerJob, job_for};
use crate::i18n::{tr, tr_fmt};
use crate::key_bindings::Action;
use crate::map::{GameMap, world_to_cell};
use crate::minimap::MinimapOrder;
use crate::neutral_sites::UnclaimedSite;
use crate::placement::Placement;
use crate::player_commands::LocalTeam;
use crate::player_commands::{CommandIssuer, GroundOrder, PlayerCommand};
use crate::settings::GameSettings;
use crate::sound::{Sound, SoundCue};
use crate::structure_tools::StructureTool;
use crate::transport::{Embarked, Passengers};
use crate::ui::UiFonts;

const DRAG_THRESHOLD_PX: f32 = 8.0;
const SELECTION_CORNER_PX: f32 = 14.0;
const SELECTION_CORNER_THICKNESS_PX: f32 = 3.0;
const CLICK_SLACK: f32 = 0.6;
const ORDER_MARKER_SECS: f32 = 0.6;

#[derive(Resource, Default)]
pub struct DragSelection {
    start: Option<Vec2>,
}

#[derive(Component)]
pub struct SelectionBoxUi;

#[derive(Resource, Default)]
pub struct OrderMarker {
    pub position: Vec3,
    pub remaining_secs: f32,
    pub kind: MarkerKind,
    /// Counts every acknowledged order, so the units' voices can tell a new order from the marker fading.
    pub orders_acknowledged: u32,
}

/// What the last right click ordered, which decides the marker's colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MarkerKind {
    #[default]
    Move,
    Attack,
    Patrol,
    Rally,
}

#[derive(Component)]
pub struct SelectionCount;

pub fn spawn_selection_box(mut commands: Commands, fonts: Res<UiFonts>) {
    let corner = |placement: Node, border: UiRect| {
        (
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(SELECTION_CORNER_PX),
                height: Val::Px(SELECTION_CORNER_PX),
                border,
                ..placement
            },
            BorderColor::all(Color::WHITE),
        )
    };
    let edge = Val::Px(-SELECTION_CORNER_THICKNESS_PX);
    let thick = Val::Px(SELECTION_CORNER_THICKNESS_PX);
    commands
        .spawn((
            SelectionBoxUi,
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.55)),
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
            Visibility::Hidden,
        ))
        .with_children(|frame| {
            frame.spawn(corner(
                Node {
                    left: edge,
                    top: edge,
                    ..default()
                },
                UiRect {
                    left: thick,
                    top: thick,
                    ..default()
                },
            ));
            frame.spawn(corner(
                Node {
                    right: edge,
                    top: edge,
                    ..default()
                },
                UiRect {
                    right: thick,
                    top: thick,
                    ..default()
                },
            ));
            frame.spawn(corner(
                Node {
                    left: edge,
                    bottom: edge,
                    ..default()
                },
                UiRect {
                    left: thick,
                    bottom: thick,
                    ..default()
                },
            ));
            frame.spawn(corner(
                Node {
                    right: edge,
                    bottom: edge,
                    ..default()
                },
                UiRect {
                    right: thick,
                    bottom: thick,
                    ..default()
                },
            ));
            frame.spawn((
                SelectionCount,
                fonts.heading("", 15.0, Color::WHITE),
                TextShadow::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(-22.0),
                    ..default()
                },
            ));
        });
}

/// Where the pointer's line crosses the horizontal plane at `height`.
fn pointer_at_height(ray: Ray3d, height: f32) -> Option<Vec3> {
    let distance = ray.intersect_plane(Vec3::Y * height, InfinitePlane3d::new(Vec3::Y))?;
    Some(ray.get_point(distance))
}

/// What the pointer is on. Aircraft are picked where they are seen, up in the air, not by the ground beneath.
fn target_under(
    targets: &Targets,
    viewer: Team,
    ground: Vec3,
    ray: Option<Ray3d>,
) -> Option<&TargetInfo> {
    let pointed_at = |t: &TargetInfo| {
        let airborne = t.armor == ArmorClass::Aircraft;
        ray.filter(|_| airborne)
            .and_then(|ray| pointer_at_height(ray, t.position.y))
            .unwrap_or(ground)
    };
    targets
        .iter()
        .filter(|t| t.seen_by(viewer))
        .filter(|t| {
            let point = pointed_at(t);
            let offset = (point - t.position).xz().abs();
            let reach = t.radius + CLICK_SLACK;
            if t.is_building {
                offset.max_element() <= t.radius
            } else {
                offset.length() <= reach
            }
        })
        .min_by(|a, b| {
            let distance = |t: &TargetInfo| (pointed_at(t) - t.position).xz().length_squared();
            distance(a).total_cmp(&distance(b))
        })
}

/// A second click on a unit of the same kind within this time selects every one of them on screen.
const DOUBLE_CLICK_SECS: f32 = 0.35;

pub fn select_units(
    local: Res<LocalTeam>,
    time: Res<Time<Real>>,
    mut last_click: Local<Option<(UnitKind, f32)>>,
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Res<Cursor>,
    placement: Res<Placement>,
    tool: Res<StructureTool>,
    targets: Res<Targets>,
    mut drag: ResMut<DragSelection>,
    cameras: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    selectable: Query<(Entity, &Team, &Transform, Option<&Unit>), Without<Embarked>>,
    selected: Query<Entity, With<Selected>>,
    mut selection_box: Query<(&mut Node, &mut Visibility), With<SelectionBoxUi>>,
    mut counts: Query<&mut Text, With<SelectionCount>>,
) {
    let Some(screen) = cursor.screen else { return };
    let pointer_busy = placement.active.is_some() || tool.0.is_some();
    if keys.just_pressed(KeyCode::Escape) && !pointer_busy {
        deselect_all(&mut commands, &selected);
    }
    if pointer_busy {
        drag.start = None;
    } else if mouse.just_pressed(MouseButton::Left) && !cursor.over_interface {
        drag.start = Some(screen);
    }

    let Ok((mut node, mut visibility)) = selection_box.single_mut() else {
        return;
    };
    let Some(start) = drag.start else {
        *visibility = Visibility::Hidden;
        return;
    };
    let min = start.min(screen);
    let max = start.max(screen);
    let dragging = start.distance(screen) > DRAG_THRESHOLD_PX;
    *visibility = if dragging {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    frame_the_box(&mut node, min, max);

    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let inside_box = |position: Vec3| {
        camera
            .world_to_viewport(camera_transform, position)
            .is_ok_and(|projected| projected.cmpge(min).all() && projected.cmple(max).all())
    };
    let own_units_inside =
        |selectable: &Query<(Entity, &Team, &Transform, Option<&Unit>), Without<Embarked>>| {
            selectable
                .iter()
                .filter(|(_, team, transform, unit)| {
                    **team == local.0 && unit.is_some() && inside_box(transform.translation)
                })
                .map(|(entity, ..)| entity)
                .collect::<Vec<_>>()
        };
    if dragging {
        let label = unit_count_label(own_units_inside(&selectable).len());
        for mut text in &mut counts {
            if text.0 != label {
                text.0 = label.clone();
            }
        }
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    drag.start = None;
    *visibility = Visibility::Hidden;

    let additive = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if !additive {
        deselect_all(&mut commands, &selected);
    }
    if dragging {
        for entity in own_units_inside(&selectable) {
            commands.entity(entity).insert(Selected);
        }
    } else if let Some(hit) = cursor
        .ground
        .and_then(|ground| target_under(&targets, local.0, ground, cursor.ray))
        && hit.team == local.0
    {
        commands.entity(hit.entity).insert(Selected);
        let kind = selectable
            .get(hit.entity)
            .ok()
            .and_then(|(.., unit)| unit.map(|unit| unit.0));
        let now = time.elapsed_secs();
        let double_click = kind.is_some()
            && last_click.is_some_and(|(previous, at)| {
                Some(previous) == kind && now - at < DOUBLE_CLICK_SECS
            });
        let whole_type =
            keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) || double_click;
        if let (Some(kind), true) = (kind, whole_type) {
            let on_screen = |position: Vec3| {
                let size = camera.logical_viewport_size().unwrap_or_default();
                camera
                    .world_to_viewport(camera_transform, position)
                    .is_ok_and(|point| point.cmpge(Vec2::ZERO).all() && point.cmple(size).all())
            };
            for (entity, team, transform, unit) in &selectable {
                let same = unit.is_some_and(|unit| unit.0 == kind);
                if *team == local.0 && same && on_screen(transform.translation) {
                    commands.entity(entity).insert(Selected);
                }
            }
        }
        *last_click = kind.map(|kind| (kind, now));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Shortcut {
    order: GroundOrder,
    queued: bool,
}

/// Right click on the ground. Ctrl attack-moves, Alt patrols, Shift queues behind current orders.
fn ground_order_from_keys(keys: &ButtonInput<KeyCode>) -> Shortcut {
    let order = if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        GroundOrder::AttackMove
    } else if keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]) {
        GroundOrder::Patrol
    } else {
        GroundOrder::Move
    };
    Shortcut {
        order,
        queued: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
    }
}

fn produces_units(kind: BuildingKind) -> bool {
    matches!(
        kind,
        BuildingKind::Barracks | BuildingKind::Factory | BuildingKind::Helipad
    )
}

/// Turns the player's clicks and keys into commands; the simulation carries them out on a later tick.
pub fn issue_orders(
    mut cues: MessageWriter<SoundCue>,
    mut issuer: CommandIssuer,
    mouse: Res<ButtonInput<MouseButton>>,
    (keys, settings): (Res<ButtonInput<KeyCode>>, Res<GameSettings>),
    cursor: Res<Cursor>,
    placement: Res<Placement>,
    tool: Res<StructureTool>,
    targets: Res<Targets>,
    mut marker: ResMut<OrderMarker>,
    diplomacy: Res<Diplomacy>,
    mut minimap_order: ResMut<MinimapOrder>,
    selected_units: SelectedUnits,
    selected_buildings: Query<(&SimId, &Building, &Team), With<Selected>>,
    structures: Query<(&SimId, &Building, &Health)>,
    sim_ids: Query<&SimId>,
    (map, unclaimed_sites, carriers): (
        Res<GameMap>,
        Query<&SimId, With<UnclaimedSite>>,
        Query<&SimId, With<Passengers>>,
    ),
) {
    let team = issuer.team();
    let units: Vec<SimId> = selected_units
        .iter()
        .filter(|(_, _, owner)| **owner == team)
        .map(|(&id, ..)| id)
        .collect();
    if settings.keys.just_pressed(&keys, Action::Stop) && !units.is_empty() {
        issuer.issue(PlayerCommand::Stop {
            units: units.clone(),
        });
    }
    let engineers = own_engineers(&selected_units, team);
    let chosen_stance = [
        (Action::StanceAggressive, Stance::Aggressive),
        (Action::StanceHoldPosition, Stance::HoldPosition),
        (Action::StanceHoldFire, Stance::HoldFire),
    ]
    .into_iter()
    .find(|&(action, _)| settings.keys.just_pressed(&keys, action));
    if let Some((_, stance)) = chosen_stance
        && !units.is_empty()
    {
        issuer.issue(PlayerCommand::SetStance {
            units: units.clone(),
            stance,
        });
    }
    let own_carriers: Vec<SimId> = selected_units
        .iter()
        .filter(|(_, unit, owner)| **owner == team && unit.0 == UnitKind::TroopCarrier)
        .map(|(&id, ..)| id)
        .collect();
    if settings.keys.just_pressed(&keys, Action::Unload) && !own_carriers.is_empty() {
        issuer.issue(PlayerCommand::Unload {
            carriers: own_carriers,
        });
    }
    if settings.keys.just_pressed(&keys, Action::LayMine) && !engineers.is_empty() {
        issuer.issue(PlayerCommand::LayMines { units: engineers });
    }
    let from_minimap = minimap_order.0.take();
    if placement.active.is_some() || tool.0.is_some() {
        return;
    }
    let clicked_in_world = !cursor.over_interface && mouse.just_pressed(MouseButton::Right);
    let Some(ground) = from_minimap.or(cursor.ground.filter(|_| clicked_in_world)) else {
        return;
    };
    let hovered = if from_minimap.is_some() {
        None
    } else {
        target_under(&targets, team, ground, cursor.ray).copied()
    };
    let goal = world_to_cell(ground);
    let mut acknowledge = |marker: &mut OrderMarker, position: Vec3, kind: MarkerKind| {
        cues.write(SoundCue::interface(Sound::UnitAcknowledge));
        *marker = OrderMarker {
            position,
            remaining_secs: ORDER_MARKER_SECS,
            kind,
            orders_acknowledged: marker.orders_acknowledged.wrapping_add(1),
        };
    };

    if units.is_empty() {
        let producers: Vec<SimId> = selected_buildings
            .iter()
            .filter(|(_, building, owner)| **owner == team && produces_units(building.kind))
            .map(|(&id, ..)| id)
            .collect();
        if !producers.is_empty() {
            issuer.issue(PlayerCommand::SetRally {
                buildings: producers,
                cell: goal,
            });
            acknowledge(&mut marker, ground, MarkerKind::Rally);
        }
        return;
    }

    let unclaimed_site = map
        .building_at(goal)
        .and_then(|entity| unclaimed_sites.get(entity).ok());
    let engineers = own_engineers(&selected_units, team);
    if let Some(&site) = unclaimed_site
        && !engineers.is_empty()
    {
        issuer.issue(PlayerCommand::SendEngineers {
            units: engineers,
            structure: site,
        });
        acknowledge(&mut marker, ground, MarkerKind::Move);
        return;
    }

    let infantry: Vec<SimId> = selected_units
        .iter()
        .filter(|(_, unit, owner)| **owner == team && unit.0.spec().armor == ArmorClass::Infantry)
        .map(|(&id, ..)| id)
        .collect();
    let own_carrier = hovered
        .filter(|target| target.team == team)
        .and_then(|target| carriers.get(target.entity).ok());
    if let Some(&carrier) = own_carrier
        && !infantry.is_empty()
    {
        issuer.issue(PlayerCommand::Board {
            units: infantry,
            carrier,
        });
        acknowledge(&mut marker, ground, MarkerKind::Move);
        return;
    }

    if let Some(structure) = hovered.filter(|t| t.is_building)
        && let Ok((&id, building, health)) = structures.get(structure.entity)
        && let Some(job) = job_for(
            *diplomacy,
            team,
            Some(structure.team),
            building.kind,
            health,
        )
    {
        let engineers = own_engineers(&selected_units, team);
        if !engineers.is_empty() {
            issuer.issue(PlayerCommand::SendEngineers {
                units: engineers,
                structure: id,
            });
            if job == EngineerJob::Repair {
                acknowledge(&mut marker, structure.position, MarkerKind::Move);
                return;
            }
        }
    }

    if let Some(enemy) = hovered.filter(|t| diplomacy.hostile(t.team, team))
        && let Ok(&target) = sim_ids.get(enemy.entity)
    {
        issuer.issue(PlayerCommand::Attack { units, target });
        acknowledge(&mut marker, enemy.position, MarkerKind::Attack);
        return;
    }

    let refinery = hovered
        .filter(|t| t.team == team)
        .and_then(|t| structures.get(t.entity).ok())
        .filter(|(_, building, _)| building.kind == BuildingKind::Refinery)
        .map(|(&id, ..)| id);
    let shortcut = ground_order_from_keys(&keys);
    issuer.issue(PlayerCommand::Ground {
        units,
        goal,
        order: shortcut.order,
        queued: shortcut.queued,
        refinery,
    });
    acknowledge(&mut marker, ground, marker_for(shortcut.order));
}

type SelectedUnits<'w, 's> =
    Query<'w, 's, (&'static SimId, &'static Unit, &'static Team), With<Selected>>;

fn own_engineers(selected_units: &SelectedUnits, team: Team) -> Vec<SimId> {
    selected_units
        .iter()
        .filter(|(_, unit, owner)| **owner == team && unit.0 == UnitKind::Engineer)
        .map(|(&id, ..)| id)
        .collect()
}

fn marker_for(order: GroundOrder) -> MarkerKind {
    match order {
        GroundOrder::Move => MarkerKind::Move,
        GroundOrder::AttackMove => MarkerKind::Attack,
        GroundOrder::Patrol => MarkerKind::Patrol,
    }
}

fn deselect_all(commands: &mut Commands, selected: &Query<Entity, With<Selected>>) {
    for entity in selected {
        commands.entity(entity).remove::<Selected>();
    }
}

fn frame_the_box(node: &mut Node, min: Vec2, max: Vec2) {
    node.left = Val::Px(min.x);
    node.top = Val::Px(min.y);
    node.width = Val::Px(max.x - min.x);
    node.height = Val::Px(max.y - min.y);
}

fn unit_count_label(count: usize) -> String {
    match count {
        0 => String::new(),
        1 => tr("1 unit").to_string(),
        n => tr_fmt("{} units", &[&n]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_aircraft_is_picked_where_it_flies_not_on_the_ground_beneath() {
        let camera = Vec3::new(0.0, 30.0, 30.0);
        let aircraft = Vec3::new(10.0, 6.0, 10.0);
        let ray = Ray3d::new(camera, Dir3::new(aircraft - camera).unwrap());
        let at_its_height = pointer_at_height(ray, aircraft.y).unwrap();
        assert!(at_its_height.distance(aircraft) < 1e-3);
        let on_the_ground = pointer_at_height(ray, 0.0).unwrap();
        assert!(on_the_ground.xz().distance(aircraft.xz()) > 4.0);
    }
}
