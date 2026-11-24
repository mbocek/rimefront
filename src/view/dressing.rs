use bevy::prelude::*;

use crate::sim::catalog::{ArmorClass, HARVESTER_CAPACITY, Team};
use crate::sim::combat::{DamageSource, Destroyed};
use crate::sim::components::{Building, Harvester, Mobile, Turret, Unit};
use crate::sim::neutral_sites::UnclaimedSite;
use crate::view::animation::{animated_building_model, animated_unit_model, spawn_corpse};
use crate::view::assets::{ModelLibrary, named_node};

pub fn dress_new_unit(
    added: On<Add<Unit>>,
    units: Query<(&Unit, &Team)>,
    models: Res<ModelLibrary>,
    mut commands: Commands,
) {
    let Ok((unit, &team)) = units.get(added.entity) else {
        return;
    };
    commands.entity(added.entity).insert((
        WorldAssetRoot(models.unit(unit.0, team)),
        animated_unit_model(unit.0, team),
    ));
}

/// Structures of a side and the unclaimed sites waiting for one.
pub fn dress_new_building(
    added: On<Add<Building>>,
    buildings: Query<(&Building, Option<&Team>, Has<UnclaimedSite>)>,
    models: Res<ModelLibrary>,
    mut commands: Commands,
) {
    let Ok((building, team, unclaimed)) = buildings.get(added.entity) else {
        return;
    };
    let mut dressed = commands.entity(added.entity);
    match (team, unclaimed) {
        (Some(&team), false) => {
            dressed.insert(WorldAssetRoot(models.building(building.kind, team)));
            if let Some(animated) = animated_building_model(building.kind, team) {
                dressed.insert(animated);
            }
        }
        _ => {
            dressed.insert(WorldAssetRoot(models.unclaimed_site(building.kind)));
        }
    }
}

/// Fallen infantry stays on the ground a while; vehicles leave wrecks through the battlefield's marks.
pub fn lay_corpses(
    mut commands: Commands,
    mut losses: MessageReader<Destroyed>,
    models: Res<ModelLibrary>,
) {
    for loss in losses.read() {
        let DamageSource::Unit(kind) = loss.what else {
            continue;
        };
        if kind.spec().armor != ArmorClass::Infantry {
            continue;
        }
        spawn_corpse(
            &mut commands,
            models.unit(kind, loss.team),
            animated_unit_model(kind, loss.team),
            Transform::from_translation(loss.position).with_rotation(loss.rotation),
        );
    }
}

/// The turret node inside a model, turned to the turret's simulated aim.
#[derive(Component)]
pub struct TurretNode {
    node: Entity,
    base_rotation: Quat,
}

pub fn link_turret_nodes(
    mut commands: Commands,
    unlinked: Query<Entity, (With<Turret>, Without<TurretNode>)>,
    children: Query<&Children>,
    nodes: Query<(&Name, &Transform)>,
) {
    for entity in &unlinked {
        if let Some((node, transform)) = named_node(entity, "Turret", &children, &nodes) {
            commands.entity(entity).insert(TurretNode {
                node,
                base_rotation: transform.rotation,
            });
        }
    }
}

pub fn rotate_turret_nodes(
    turrets: Query<(&Turret, &TurretNode, Option<&Mobile>)>,
    mut nodes: Query<&mut Transform, Without<Turret>>,
) {
    for (turret, link, mobile) in &turrets {
        let Ok(mut node_transform) = nodes.get_mut(link.node) else {
            continue;
        };
        let body_yaw = mobile.map_or(0.0, |m| m.facing);
        node_transform.rotation = Quat::from_rotation_y(turret.yaw - body_yaw) * link.base_rotation;
    }
}

const EMPTY_LOAD_SCALE: f32 = 0.05;
const LOAD_NODE_NAME: &str = "OreLoad";

/// The hopper's ore heap inside the harvester model, scaled up from its floor as cargo accumulates.
#[derive(Component)]
pub struct OreLoadLink {
    node: Entity,
    full_scale: Vec3,
}

pub fn link_ore_load_nodes(
    mut commands: Commands,
    unlinked: Query<Entity, (With<Harvester>, Without<OreLoadLink>)>,
    children: Query<&Children>,
    nodes: Query<(&Name, &Transform)>,
) {
    for harvester in &unlinked {
        let found = crate::view::assets::named_node(harvester, LOAD_NODE_NAME, &children, &nodes)
            .map(|(node, transform)| (node, transform.scale));
        if let Some((node, full_scale)) = found {
            commands
                .entity(harvester)
                .insert(OreLoadLink { node, full_scale });
        }
    }
}

pub fn show_harvester_load(
    harvesters: Query<(&Harvester, &OreLoadLink)>,
    mut nodes: Query<&mut Transform, Without<Harvester>>,
) {
    for (harvester, link) in &harvesters {
        let Ok(mut transform) = nodes.get_mut(link.node) else {
            continue;
        };
        let fill = (harvester.cargo as f32 / HARVESTER_CAPACITY as f32).clamp(0.0, 1.0);
        let height = EMPTY_LOAD_SCALE + (1.0 - EMPTY_LOAD_SCALE) * fill;
        let breadth = 0.8 + 0.2 * fill;
        transform.scale = link.full_scale * Vec3::new(breadth, height, breadth);
    }
}
