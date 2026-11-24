use bevy::prelude::*;

use crate::sim::catalog::{ArmorClass, BuildingKind, Team, UnitKind};
use crate::sim::components::{Building, Health, Mobile, Unit};
use crate::sim::spawn::exit_cell;
use crate::world::map::{footprint_center, world_to_cell};

const SERVICE_SHARE_PER_SEC: f32 = 0.04;
const OVER_THE_PAD: f32 = 3.0;
/// Reaches every cell touching a 2x2 footprint, diagonal corners included.
const BESIDE_THE_BAY: f32 = 4.5;

/// How far from its centre a structure mends units of a kind it services; `None` when it does not.
pub fn service_radius(structure: BuildingKind, unit: UnitKind) -> Option<f32> {
    match structure {
        BuildingKind::Helipad if unit.flies() => Some(OVER_THE_PAD),
        BuildingKind::RepairBay if unit.spec().armor == ArmorClass::Vehicle => Some(BESIDE_THE_BAY),
        _ => None,
    }
}

/// Where a unit waits to be serviced: on the pad for aircraft, at the bay's door for vehicles.
pub fn waiting_spot(building: &Building) -> IVec2 {
    match building.kind {
        BuildingKind::RepairBay => exit_cell(building),
        _ => world_to_cell(footprint_center(
            building.origin,
            building.kind.spec().footprint,
        )),
    }
}

pub fn service_waiting_units(
    time: Res<Time>,
    structures: Query<(&Building, &Team, &Health), Without<Unit>>,
    mut units: Query<(&Unit, &Team, &Transform, &mut Health, Option<&Mobile>)>,
) {
    let stations: Vec<(BuildingKind, Team, Vec2)> = structures
        .iter()
        .filter(|(_, _, health)| health.current > 0.0)
        .map(|(building, &team, _)| {
            let centre = footprint_center(building.origin, building.kind.spec().footprint);
            (building.kind, team, centre.xz())
        })
        .collect();
    if stations.is_empty() {
        return;
    }
    for (unit, &team, transform, mut health, mobile) in &mut units {
        let waiting = mobile.is_none_or(|mobile| !mobile.is_moving());
        if !waiting || health.current <= 0.0 || health.current >= health.max {
            continue;
        }
        let position = transform.translation.xz();
        let serviced = stations.iter().any(|&(kind, owner, centre)| {
            owner == team
                && service_radius(kind, unit.0)
                    .is_some_and(|radius| position.distance(centre) <= radius)
        });
        if serviced {
            let mended = health.max * SERVICE_SHARE_PER_SEC * time.delta_secs();
            health.current = (health.current + mended).min(health.max);
        }
    }
}
