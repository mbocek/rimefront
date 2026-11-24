use bevy::prelude::*;

use crate::sim::abilities::Mine;
use crate::sim::catalog::{ArmorClass, HARVESTER_CAPACITY, Team};
use crate::sim::combat::{DamageSource, Destroyed};
use crate::sim::components::{Building, Harvester, Mobile, Turret, Unit};
use crate::sim::neutral_sites::UnclaimedSite;
use crate::sim::sorties::{ON_THE_PAD_HEIGHT, Sortie};
use crate::view::animation::{animated_building_model, animated_unit_model, spawn_corpse};
use crate::view::assets::{ModelLibrary, named_node};
use crate::view::local_view::LocalView;

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

/// Mines stay hidden until `show_own_mines` decides who may see them.
pub fn hide_new_mine(added: On<Add<Mine>>, mut commands: Commands) {
    commands.entity(added.entity).insert(Visibility::Hidden);
}

/// Mines of the local side and its allies are drawn as small discs; enemy mines stay hidden.
#[derive(Resource)]
pub struct MineLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub fn create_mine_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(MineLook {
        mesh: meshes.add(Cylinder::new(0.35, 0.08)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.24, 0.2),
            emissive: LinearRgba::rgb(0.8, 0.15, 0.05),
            ..default()
        }),
    });
}

pub fn show_own_mines(
    mut commands: Commands,
    look: Res<MineLook>,
    view: LocalView,
    new_mines: Query<(Entity, &Mine), Added<Mine>>,
) {
    for (entity, mine) in &new_mines {
        if view.is_spectating() || !view.is_hostile(mine.team) {
            commands.entity(entity).insert((
                Mesh3d(look.mesh.clone()),
                MeshMaterial3d(look.material.clone()),
                Visibility::Inherited,
            ));
        }
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
