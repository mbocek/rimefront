use bevy::prelude::*;

use crate::sim::abilities::Camouflage;
use crate::sim::catalog::{ArmorClass, BuildingKind, Team, UnitKind};
use crate::sim::components::*;
use crate::sim::fog::Sight;
use crate::sim::medical::Healer;
use crate::sim::sorties::Sortie;
use crate::view::animation::{animated_building_model, animated_unit_model};
use crate::view::assets::ModelLibrary;
use crate::view::interpolation::TickTransforms;
use crate::view::wall_joints::WallSegment;
use crate::world::map::{
    CELL_SIZE, GameMap, footprint_center, in_far_half, opposite_cell, opposite_origin,
};

const TURRET_TURN_RATE: f32 = 3.5;

pub fn spawn_unit(
    commands: &mut Commands,
    models: &ModelLibrary,
    kind: UnitKind,
    team: Team,
    position: Vec3,
    facing: f32,
) -> Entity {
    let spec = kind.spec();
    let mut unit = commands.spawn((
        Unit(kind),
        team,
        Health::full(spec.max_health),
        BodyRadius(spec.radius),
        Sight(spec.sight),
        Armor(spec.armor),
        Mobile::for_unit(kind, facing),
        Orders::default(),
        WorldAssetRoot(models.unit(kind, team)),
        animated_unit_model(kind, team),
        Transform::from_translation(position).with_rotation(Quat::from_rotation_y(facing)),
        TickTransforms::default(),
    ));
    if let Some(weapon) = spec.weapon {
        unit.insert((
            Weapon {
                spec: weapon,
                cooldown: 0.0,
            },
            AttackTarget::default(),
            Stance::default(),
        ));
    }
    if spec.has_turret {
        unit.insert(Turret::new(facing, TURRET_TURN_RATE));
    }
    if kind.flies() {
        unit.insert(Aircraft);
    }
    if spec.heal_per_sec > 0.0 {
        unit.insert(Healer {
            per_sec: spec.heal_per_sec,
            patients: spec.armor,
            treating: false,
        });
    }
    if kind.lands_on_pad() {
        unit.insert(Sortie::new(kind.rearms_at_pad()));
    }
    if kind.hides_when_still() {
        unit.insert(Camouflage::default());
    }
    if kind == UnitKind::TroopCarrier {
        unit.insert(crate::sim::transport::Passengers::default());
    }
    if kind == UnitKind::Harvester {
        unit.insert(Harvester {
            state: HarvestState::SeekingField,
            cargo: 0,
            preferred_field: None,
        });
    }
    unit.id()
}

pub fn spawn_building(
    commands: &mut Commands,
    models: &ModelLibrary,
    map: &mut GameMap,
    kind: BuildingKind,
    team: Team,
    origin: IVec2,
) -> Entity {
    let spec = kind.spec();
    let footprint_radius = spec.footprint.as_vec2().max_element() * CELL_SIZE * 0.5;
    let mut building = commands.spawn((
        Building { kind, origin },
        team,
        Health::full(spec.max_health),
        BodyRadius(footprint_radius),
        Sight(spec.sight),
        Armor(ArmorClass::Structure),
        WorldAssetRoot(models.building(kind, team)),
        Transform::from_translation(footprint_center(origin, spec.footprint))
            .with_rotation(Quat::from_rotation_y(building_yaw(kind, origin))),
    ));
    if let Some(weapon) = spec.weapon {
        building.insert((
            Weapon {
                spec: weapon,
                cooldown: 0.0,
            },
            AttackTarget::default(),
            Turret::new(0.0, TURRET_TURN_RATE),
        ));
    }
    if let Some(animated) = animated_building_model(kind, team) {
        building.insert(animated);
    }
    if kind.is_wall() {
        building.insert(WallSegment { cell: origin, team });
    }
    let entity = building.id();
    map.set_building(origin, spec.footprint, Some(entity));
    entity
}

/// Cell just outside the building's +X side, where units leave and harvesters dock.
/// Doors face east in the near half and west in the far half, so every base opens towards the middle.
pub fn exit_cell(building: &Building) -> IVec2 {
    let footprint = building.kind.spec().footprint;
    let east_door = |origin: IVec2| origin + IVec2::new(footprint.x, footprint.y / 2);
    if in_far_half(building.origin, footprint) {
        opposite_cell(east_door(opposite_origin(building.origin, footprint)))
    } else {
        east_door(building.origin)
    }
}

/// Armed structures keep yaw zero: their turret aim is measured from it.
pub(crate) fn building_yaw(kind: BuildingKind, origin: IVec2) -> f32 {
    let spec = kind.spec();
    if spec.weapon.is_none() && in_far_half(origin, spec.footprint) {
        std::f32::consts::PI
    } else {
        0.0
    }
}
