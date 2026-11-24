use bevy::prelude::*;

use crate::sim::catalog::{BuildingKind, Team};
use crate::sim::components::{AttackTarget, Building, Mobile, Weapon};
use crate::sim::determinism::{SimId, in_simulation_order};
use crate::sim::movement::{FLIGHT_ALTITUDE, order_move};
use crate::sim::sim_math::{atan2, facing_direction};
use crate::world::map::{GameMap, cell_center, footprint_center, world_to_cell};

/// Close enough to a Helipad's centre to be on its pad, where it is also repaired.
const ON_THE_PAD: f32 = 3.0;

/// An aircraft that takes off and lands straight up and down on its Helipad, landing when idle there, and
/// if `rearms`, strikes once and flies home to rearm before the next run.
#[derive(Component, Default)]
pub struct Sortie {
    pub rearms: bool,
    /// Height above the ground; it climbs to `FLIGHT_ALTITUDE` before flying off and sinks onto the pad.
    pub altitude: f32,
    /// Sitting on its pad with nothing to do, so it lands instead of hanging in the air.
    pub parked: bool,
    /// Circling while it waits in the air; an aeroplane cannot stand still.
    pub orbit: Option<Orbit>,
}

/// A holding circle: its centre on the ground plane and where on it the aircraft is, as a `facing_direction`
/// angle from the centre.
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    centre: Vec3,
    angle: f32,
}

const ORBIT_RADIUS: f32 = 5.0;
/// A parked aircraft stands on a Helipad's raised pad, not on the ground beneath it.
pub const ON_THE_PAD_HEIGHT: f32 = 0.35;
/// Metres per second it rises or sinks, straight up and down.
const VERTICAL_SPEED: f32 = 2.5;
/// Close enough to cruising height to start flying forward.
const CLIMB_SLACK: f32 = 0.3;
/// Share of its top speed an aircraft keeps while circling.
const ORBIT_SPEED_SHARE: f32 = 0.5;

/// Sends every aircraft that has dropped its load back to the nearest Helipad of its side and keeps it
/// from taking on targets until it has rearmed there; an idle one at home lands on the pad.
pub fn fly_home_to_rearm(
    map: Res<GameMap>,
    pads: Query<(&Building, &Team)>,
    mut aircraft: Query<(
        &Team,
        &Transform,
        &Weapon,
        &mut AttackTarget,
        &mut Mobile,
        &mut Sortie,
        &SimId,
    )>,
) {
    let mut pads: Vec<(&Building, &Team)> = pads
        .iter()
        .filter(|(building, _)| building.kind == BuildingKind::Helipad)
        .collect();
    pads.sort_by_key(|(building, _)| (building.origin.y, building.origin.x));
    let helipads: Vec<(Team, Vec3)> = pads
        .into_iter()
        .map(|(building, &team)| {
            let footprint = building.kind.spec().footprint;
            (team, footprint_center(building.origin, footprint))
        })
        .collect();
    let in_order = in_simulation_order(aircraft.iter_mut(), |item| *item.6);
    for (&team, transform, weapon, mut attack, mut mobile, mut sortie, _) in in_order {
        let here = transform.translation;
        let home = helipads
            .iter()
            .filter(|(owner, _)| *owner == team)
            .map(|&(_, pad)| pad)
            .min_by(|a, b| {
                a.xz()
                    .distance(here.xz())
                    .total_cmp(&b.xz().distance(here.xz()))
            });
        let at_home = home.is_some_and(|pad| pad.xz().distance(here.xz()) <= ON_THE_PAD);
        sortie.parked = at_home && !mobile.is_moving() && attack.target.is_none();
        if !sortie.rearms || weapon.cooldown <= 0.0 {
            continue;
        }
        attack.clear();
        let Some(pad) = home.filter(|_| !at_home) else {
            continue;
        };
        let pad_cell = world_to_cell(pad);
        if mobile.path.back() != Some(&cell_center(pad_cell)) {
            order_move(&mut mobile, &map, here, pad_cell, false);
        }
    }
}

/// Keeps every waiting aeroplane in the air circling to the left around where it stopped, until it gets an
/// order, a target or lands.
pub fn circle_while_waiting(
    time: Res<Time>,
    mut aircraft: Query<(&mut Transform, &mut Mobile, &AttackTarget, &mut Sortie)>,
) {
    let dt = time.delta_secs();
    for (mut transform, mut mobile, attack, mut sortie) in &mut aircraft {
        let waiting = !sortie.parked && !mobile.is_moving() && attack.target.is_none();
        if !waiting {
            sortie.orbit = None;
            continue;
        }
        let here = transform.translation;
        let orbit = sortie
            .orbit
            .get_or_insert_with(|| entering_orbit(here, mobile.facing));
        orbit.angle = (orbit.angle + mobile.speed * ORBIT_SPEED_SHARE / ORBIT_RADIUS * dt)
            .rem_euclid(std::f32::consts::TAU);
        let on_the_circle = orbit.centre + facing_direction(orbit.angle) * ORBIT_RADIUS;
        transform.translation = on_the_circle.with_y(here.y);
        mobile.facing = orbit.angle + std::f32::consts::FRAC_PI_2;
    }
}

/// The circle that starts where the aircraft is and turns left from the way it faces.
fn entering_orbit(position: Vec3, facing: f32) -> Orbit {
    let centre = position.with_y(0.0)
        + facing_direction(facing + std::f32::consts::FRAC_PI_2) * ORBIT_RADIUS;
    let outward = position.with_y(0.0) - centre;
    Orbit {
        centre,
        angle: atan2(-outward.z, outward.x),
    }
}

/// Lifts every aircraft off its pad towards cruising height when it has somewhere to go, and lets it down
/// onto the pad once it is parked.
pub fn climb_and_descend(time: Res<Time>, mut aircraft: Query<&mut Sortie>) {
    let step = VERTICAL_SPEED * time.delta_secs();
    for mut sortie in &mut aircraft {
        let wanted = if sortie.parked {
            ON_THE_PAD_HEIGHT
        } else {
            FLIGHT_ALTITUDE
        };
        sortie.altitude += (wanted - sortie.altitude).clamp(-step, step);
    }
}

impl Sortie {
    pub fn new(rearms: bool) -> Self {
        Self {
            rearms,
            altitude: ON_THE_PAD_HEIGHT,
            parked: false,
            orbit: None,
        }
    }

    /// Still rising from the pad, so it must not fly forward yet.
    pub fn climbing(&self) -> bool {
        !self.parked && self.altitude < FLIGHT_ALTITUDE - CLIMB_SLACK
    }
}

/// The landing gear legs of an aircraft model, with their rest scale.
#[derive(Component)]
pub struct LandingGear(Vec<(Entity, Vec3)>);

/// Model nodes whose names start with this fold away in flight (tools/blender/irontide_models/units.py).
const LANDING_GEAR_NODE: &str = "LandingGear";
/// Height above the pad by which the gear is fully folded.
const GEAR_FOLDED_ABOVE_PAD: f32 = 1.2;
/// Folded legs keep a sliver of their size, as a zero scale breaks their lighting.
const FOLDED_GEAR_SCALE: f32 = 0.01;

pub fn find_landing_gear(
    mut commands: Commands,
    aircraft: Query<Entity, (With<Sortie>, Without<LandingGear>)>,
    children: Query<&Children>,
    nodes: Query<(&Name, &Transform)>,
) {
    for entity in &aircraft {
        let legs: Vec<(Entity, Vec3)> = children
            .iter_descendants(entity)
            .filter_map(|node| {
                let (name, transform) = nodes.get(node).ok()?;
                name.as_str()
                    .starts_with(LANDING_GEAR_NODE)
                    .then_some((node, transform.scale))
            })
            .collect();
        if !legs.is_empty() {
            commands.entity(entity).insert(LandingGear(legs));
        }
    }
}

/// Folds the gear up into the airframe as the aircraft rises off the pad and lets it down again to land.
pub fn fold_landing_gear(
    aircraft: Query<(&Sortie, &LandingGear)>,
    mut nodes: Query<&mut Transform, Without<Sortie>>,
) {
    for (sortie, gear) in &aircraft {
        let lowered =
            1.0 - ((sortie.altitude - ON_THE_PAD_HEIGHT) / GEAR_FOLDED_ABOVE_PAD).clamp(0.0, 1.0);
        let share = FOLDED_GEAR_SCALE + (1.0 - FOLDED_GEAR_SCALE) * lowered;
        for &(leg, rest) in &gear.0 {
            if let Ok(mut transform) = nodes.get_mut(leg) {
                transform.scale = rest * share;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_orbit_begins_where_the_aircraft_is_and_runs_along_its_heading() {
        let position = Vec3::new(20.0, 6.0, 30.0);
        for facing in [0.0, 1.2, -2.5] {
            let orbit = entering_orbit(position, facing);
            let start = orbit.centre + facing_direction(orbit.angle) * ORBIT_RADIUS;
            assert!(start.xz().distance(position.xz()) < 1e-4, "facing {facing}");
            let tangent = facing_direction(orbit.angle + std::f32::consts::FRAC_PI_2);
            assert!(
                tangent.distance(facing_direction(facing)) < 1e-4,
                "facing {facing}"
            );
        }
    }
}
