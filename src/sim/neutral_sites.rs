use bevy::prelude::*;

use crate::sim::cadence::SimulationCadences;
use crate::sim::catalog::{ArmorClass, BuildingKind, Team};
use crate::sim::components::{Armor, BodyRadius, Building, Health, Unit};
use crate::sim::economy::Economies;
use crate::sim::spawn::building_yaw;
use crate::world::map::{
    CELL_SIZE, GameMap, cells_by_distance, footprint_cells, footprint_center, map_bounds,
    near_base, opposite_origin, self_mirrored_origin,
};

const SUPPLY_DELIVERY_SECS: f32 = 5.0;
const SUPPLY_DELIVERY_CREDITS: i32 = 50;
const HOSPITAL_REACH: f32 = 9.0;
const HOSPITAL_HEALING_PER_SEC: f32 = 6.0;
/// How far from its intended spot a site may move to find free, buildable ground, in cells.
const SITE_SEARCH_RADIUS: i32 = 10;
/// Cells of open ground kept round every site.
const SITE_CLEARANCE: i32 = 2;

/// A neutral site nobody holds; it carries no `Team`, so nothing targets it, sees through it or counts it.
#[derive(Component)]
pub struct UnclaimedSite;

pub fn spawn_unclaimed_site(
    commands: &mut Commands,
    map: &mut GameMap,
    kind: BuildingKind,
    origin: IVec2,
) -> Entity {
    let spec = kind.spec();
    let entity = commands
        .spawn((
            Building { kind, origin },
            UnclaimedSite,
            Health::full(spec.max_health),
            BodyRadius(spec.footprint.as_vec2().max_element() * CELL_SIZE * 0.5),
            Armor(ArmorClass::Structure),
            Transform::from_translation(footprint_center(origin, spec.footprint))
                .with_rotation(Quat::from_rotation_y(building_yaw(kind, origin))),
        ))
        .id();
    map.set_building(origin, spec.footprint, Some(entity));
    entity
}

/// Where the sites of a map stand: an Observation Post in the middle, or a mirrored pair of them beside it
/// where the middle is a ford, and mirrored pairs of Supply Depots between the corners and Field Hospitals
/// on the road from each corner to the middle. Mirroring keeps the map as fair as the rest of it.
pub fn place_neutral_sites(commands: &mut Commands, map: &mut GameMap) {
    let bounds = map_bounds();
    let at_share =
        |x: f32, y: f32| bounds.min + (bounds.size.as_vec2() * Vec2::new(x, y)).as_ivec2();
    let post = BuildingKind::ObservationPost;
    let middle = self_mirrored_origin(post.spec().footprint);
    let mut pairs = Vec::new();
    if footprint_is_free(map, post, middle) {
        spawn_unclaimed_site(commands, map, post, middle);
    } else {
        pairs.push((post, at_share(0.4, 0.4)));
    }
    pairs.push((BuildingKind::SupplyDepot, at_share(0.3, 0.3)));
    pairs.push((BuildingKind::FieldHospital, at_share(0.3, 0.68)));
    for (kind, spot) in pairs {
        let Some(origin) = mirrored_spot(map, kind, spot) else {
            continue;
        };
        spawn_unclaimed_site(commands, map, kind, origin);
        let twin = opposite_origin(origin, kind.spec().footprint);
        spawn_unclaimed_site(commands, map, kind, twin);
    }
}

/// The free spot nearest `wanted` whose mirror image is free too.
fn mirrored_spot(map: &GameMap, kind: BuildingKind, wanted: IVec2) -> Option<IVec2> {
    let footprint = kind.spec().footprint;
    cells_by_distance(wanted, SITE_SEARCH_RADIUS).find(|&origin| {
        let twin = opposite_origin(origin, footprint);
        let apart = footprint_cells(origin, footprint)
            .all(|cell| !footprint_cells(twin, footprint).any(|other| other == cell));
        apart && footprint_is_free(map, kind, origin) && footprint_is_free(map, kind, twin)
    })
}

/// Buildable ground away from the bases, with open ground all round, so a site never closes a ford or a
/// pass between rocks and forests.
fn footprint_is_free(map: &GameMap, kind: BuildingKind, origin: IVec2) -> bool {
    let clearance = IVec2::splat(SITE_CLEARANCE);
    let surroundings = kind.spec().footprint + clearance * 2;
    footprint_cells(origin, kind.spec().footprint)
        .all(|cell| map.is_buildable(cell) && !near_base(cell))
        && footprint_cells(origin - clearance, surroundings).all(|cell| map.is_passable(cell))
}

/// Every Supply Depot sends its owner a delivery of credits on the same beat.
pub fn deliver_supplies(
    time: Res<Time>,
    mut cadences: ResMut<SimulationCadences>,
    mut economies: ResMut<Economies>,
    depots: Query<(&Building, &Team)>,
) {
    if !cadences
        .supply_delivery
        .is_due(time.delta_secs(), SUPPLY_DELIVERY_SECS)
    {
        return;
    }
    for (building, &team) in &depots {
        if building.kind == BuildingKind::SupplyDepot {
            economies.of_mut(team).credits += SUPPLY_DELIVERY_CREDITS;
        }
    }
}

/// A Field Hospital mends its owner's wounded infantry standing nearby.
pub fn treat_at_field_hospitals(
    time: Res<Time>,
    hospitals: Query<(&Building, &Team, &Transform)>,
    mut infantry: Query<(&Team, &Transform, &Armor, &mut Health), With<Unit>>,
) {
    let healing = HOSPITAL_HEALING_PER_SEC * time.delta_secs();
    let hospitals: Vec<(Team, Vec3)> = hospitals
        .iter()
        .filter(|(building, ..)| building.kind == BuildingKind::FieldHospital)
        .map(|(_, &team, transform)| (team, transform.translation))
        .collect();
    for (&team, transform, armor, mut health) in &mut infantry {
        let treated = armor.0 == ArmorClass::Infantry
            && health.current < health.max
            && hospitals.iter().any(|&(owner, hospital)| {
                owner == team
                    && hospital.xz().distance(transform.translation.xz()) <= HOSPITAL_REACH
            });
        if treated {
            health.current = (health.current + healing).min(health.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::opposite_cell;

    #[test]
    fn the_middle_post_is_its_own_mirror_image() {
        let post = BuildingKind::ObservationPost;
        let middle = self_mirrored_origin(post.spec().footprint);
        assert_eq!(opposite_origin(middle, post.spec().footprint), middle);
        assert_eq!(
            opposite_cell(middle),
            middle + post.spec().footprint - IVec2::ONE
        );
    }
}
