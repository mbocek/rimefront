use bevy::prelude::*;

use crate::missions::levels::ActiveLevel;
use crate::sim::catalog::ArmorClass;
use crate::sim::components::*;
use crate::sim::determinism::SimId;
use crate::sim::sim_math;
use crate::sim::sorties::Sortie;
use crate::sim::spatial_index::SpatialIndex;
use crate::world::map::{CELL_SIZE, GameMap, cell_center, in_far_half, world_to_cell};

const WAYPOINT_REACHED_DISTANCE: f32 = 0.2;
/// Two vehicles heading for the same waypoint from opposite sides push each other off it for ever,
/// so one that stops closing in while this near counts the waypoint as reached.
const WAYPOINT_CROWDED_REACH: f32 = CELL_SIZE * 0.5;
const WAYPOINT_STALL_SECS: f32 = 1.5;
const MEANINGFUL_PROGRESS: f32 = 0.05;
pub const FLIGHT_ALTITUDE: f32 = 6.0;
/// How far a circling aeroplane leans into its turn, its left wing down.
const ORBIT_BANK: f32 = 0.35;
const FLIGHT_NOSE_DOWN: f32 = 0.12;
/// Vehicles drive forward only once roughly facing their next waypoint, so they turn on the spot like tracked vehicles.
const DRIVE_ALIGNMENT_TOLERANCE: f32 = 0.7;
const SEPARATION_STRENGTH: f32 = 2.5;
/// Units on the move yield half as much to a crowd, so they push through it to their waypoint.
const MOVING_SEPARATION_SHARE: f32 = 0.5;
const VEHICLE_HALF_LENGTH_PER_RADIUS: f32 = 1.2;
const VEHICLE_HALF_WIDTH_PER_RADIUS: f32 = 0.75;
const HULL_CLEARANCE: f32 = 0.03;

pub fn order_move(
    mobile: &mut Mobile,
    map: &GameMap,
    from: Vec3,
    goal: IVec2,
    ordered: bool,
) -> bool {
    if mobile.flying {
        mobile.path = [cell_center(goal)].into();
        mobile.ordered_move = ordered;
        return true;
    }
    let Some(cells) = map.find_path(world_to_cell(from), goal) else {
        return false;
    };
    mobile.path = cells.into_iter().skip(1).map(cell_center).collect();
    if mobile.path.is_empty() {
        mobile.path.push_back(cell_center(goal));
    }
    mobile.ordered_move = ordered;
    true
}

pub fn follow_paths(
    time: Res<Time>,
    mut movers: Query<(&mut Mobile, &mut Transform, Option<&Sortie>)>,
) {
    let dt = time.delta_secs();
    for (mut mobile, mut transform, sortie) in &mut movers {
        let Some(&waypoint) = mobile.path.front() else {
            mobile.ordered_move = false;
            continue;
        };
        let to_waypoint = (waypoint - transform.translation).with_y(0.0);
        let distance = to_waypoint.length();
        let crowded_out = stalled_beside(&mut mobile.approach, waypoint, distance, dt);
        if distance < WAYPOINT_REACHED_DISTANCE || crowded_out {
            mobile.path.pop_front();
            continue;
        }

        let desired_yaw = yaw_towards(transform.translation, waypoint);
        mobile.facing = rotate_towards(mobile.facing, desired_yaw, mobile.turn_rate * dt);
        let misalignment = shortest_angle(mobile.facing, desired_yaw).abs();
        let lifting_off = sortie.is_some_and(Sortie::climbing);
        if misalignment < DRIVE_ALIGNMENT_TOLERANCE && !lifting_off {
            let step = (mobile.travel_speed() * dt).min(distance);
            transform.translation += to_waypoint / distance * step;
        }
    }
}

fn stalled_beside(approach: &mut WaypointApproach, waypoint: Vec3, distance: f32, dt: f32) -> bool {
    let new_waypoint = approach.waypoint != waypoint;
    if new_waypoint || distance < approach.closest - MEANINGFUL_PROGRESS {
        *approach = WaypointApproach {
            waypoint,
            closest: distance,
            stalled_secs: 0.0,
        };
        return false;
    }
    approach.stalled_secs += dt;
    approach.stalled_secs > WAYPOINT_STALL_SECS && distance < WAYPOINT_CROWDED_REACH
}

/// Rests every unit on the terrain. Vehicles sample the ground under their nose, tail and flanks,
/// tilt to that plane and sit on the highest supports so crests and banks cannot poke through the hull.
pub fn settle_on_terrain(
    level: Res<ActiveLevel>,
    mut units: Query<(
        &Mobile,
        &mut Transform,
        &Armor,
        &BodyRadius,
        Option<&Sortie>,
    )>,
) {
    let landscape = level.0.landscape;
    for (mobile, mut transform, armor, radius, sortie) in &mut units {
        let heading = Quat::from_rotation_y(mobile.facing);
        let position = transform.translation;
        let center_height = landscape.height(position.x, position.z);
        if mobile.flying {
            let nose_down = if mobile.is_moving() {
                FLIGHT_NOSE_DOWN
            } else {
                0.0
            };
            let banking = sortie.is_some_and(|sortie| sortie.orbit.is_some());
            let roll = if banking { -ORBIT_BANK } else { 0.0 };
            let altitude = sortie.map_or(FLIGHT_ALTITUDE, |sortie| sortie.altitude);
            transform.translation.y = center_height.max(0.0) + altitude;
            transform.rotation =
                heading * Quat::from_rotation_z(-nose_down) * Quat::from_rotation_x(roll);
            continue;
        }
        if armor.0 == ArmorClass::Infantry {
            transform.translation.y = center_height;
            transform.rotation = heading;
            continue;
        }
        let forward = sim_math::facing_direction(mobile.facing);
        let right = forward.cross(Vec3::Y);
        let half_length = radius.0 * VEHICLE_HALF_LENGTH_PER_RADIUS;
        let half_width = radius.0 * VEHICLE_HALF_WIDTH_PER_RADIUS;
        let height_at = |offset: Vec3| {
            let probe = position + offset;
            landscape.height(probe.x, probe.z)
        };
        let (front, back) = (
            height_at(forward * half_length),
            height_at(-forward * half_length),
        );
        let (right_side, left_side) = (
            height_at(right * half_width),
            height_at(-right * half_width),
        );

        let length_axis = forward * (2.0 * half_length) + Vec3::Y * (front - back);
        let width_axis = right * (2.0 * half_width) + Vec3::Y * (right_side - left_side);
        let ground_up = width_axis.cross(length_axis).normalize();
        let ground_up = if ground_up.y < 0.0 {
            -ground_up
        } else {
            ground_up
        };
        let resting_height = center_height
            .max((front + back) * 0.5)
            .max((left_side + right_side) * 0.5);

        transform.translation.y = resting_height + HULL_CLEARANCE;
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, ground_up) * heading;
    }
}

/// Structures are placed at an approximate height; this seats each new one on the real ground.
pub fn seat_new_buildings(
    level: Res<ActiveLevel>,
    mut buildings: Query<&mut Transform, Added<Building>>,
) {
    for mut transform in &mut buildings {
        transform.translation = level.0.landscape.on_ground(transform.translation);
    }
}

/// Where a unit standing exactly on another one is pushed; in the far half the mirror image, so both
/// corners spread their freshly built units the same way.
fn stacked_unit_escape(id: SimId, position: Vec3) -> Vec3 {
    let escape = Vec3::new((id.0 % 3) as f32 - 1.0, 0.0, 1.0).normalize();
    if in_far_half(world_to_cell(position), IVec2::ONE) {
        -escape
    } else {
        escape
    }
}

/// Harvesters shuttle both ways along the same lanes; in one a single cell wide, two driving ones could
/// never push past each other, so they pass through instead.
fn commuting_harvester(mobile: &Mobile, is_harvester: bool) -> bool {
    is_harvester && mobile.is_moving()
}

pub fn separate_units(
    time: Res<Time>,
    map: Res<GameMap>,
    mut units: Query<
        (&SimId, &mut Transform, &BodyRadius, &Mobile, Has<Harvester>),
        Without<crate::sim::transport::Embarked>,
    >,
) {
    let mut snapshot: Vec<(SimId, Vec3, f32, bool, bool)> = units
        .iter()
        .map(|(&id, transform, radius, mobile, is_harvester)| {
            (
                id,
                transform.translation,
                radius.0,
                mobile.flying,
                commuting_harvester(mobile, is_harvester),
            )
        })
        .collect();
    snapshot.sort_by_key(|unit| unit.0);
    let mut nearby = SpatialIndex::default();
    nearby.rebuild(
        snapshot
            .iter()
            .map(|&(_, position, radius, ..)| (position, radius)),
    );
    let dt = time.delta_secs();

    for (&id, mut transform, radius, mobile, is_harvester) in &mut units {
        let commuting = commuting_harvester(mobile, is_harvester);
        let mut push = Vec3::ZERO;
        for &(other, other_position, other_radius, other_flies, other_commuting) in
            nearby.near(&snapshot, transform.translation, radius.0)
        {
            if other == id || other_flies != mobile.flying || (commuting && other_commuting) {
                continue;
            }
            let offset = transform.translation - other_position;
            let overlap = radius.0 + other_radius - offset.length();
            if overlap > 0.0 {
                let away = offset.normalize_or(stacked_unit_escape(id, transform.translation));
                push += away * overlap;
            }
        }
        let resting_factor = if mobile.is_moving() {
            MOVING_SEPARATION_SHARE
        } else {
            1.0
        };
        let candidate = transform.translation + push * SEPARATION_STRENGTH * resting_factor * dt;
        if mobile.flying || map.is_passable(world_to_cell(candidate)) {
            transform.translation = candidate;
        }
    }
}
