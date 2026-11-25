use std::collections::HashMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::app::match_flow::GameState;
use crate::sim::catalog::Team;
use crate::sim::components::Building;
use crate::sim::sides::Shelter;
use crate::world::map::footprint_center;

/// Metres the dome reaches past the structures furthest from its middle, and the least it spans.
const DOME_MARGIN: f32 = 10.0;
const SMALLEST_DOME: f32 = 16.0;
/// How often the domes follow the structures they cover; a base changes slowly.
const REFRESH_SECS: f32 = 0.5;

/// The glass dome over a sheltered side's structures: nobody can hurt what is under it while its player is
/// away.
#[derive(Component)]
pub struct ShelterDome(Team);

#[derive(Resource)]
pub struct DomeLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub fn make_dome_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(DomeLook {
        mesh: meshes.add(Sphere::new(1.0).mesh().uv(48, 24)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.45, 0.75, 1.0, 0.16),
            emissive: LinearRgba::rgb(0.05, 0.12, 0.25),
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            unlit: true,
            ..default()
        }),
    });
}

/// Puts a dome over each sheltered side's structures the player knows of, as wide as they spread, and takes
/// it away when the side's player returns.
pub fn cover_sheltered_bases(
    mut commands: Commands,
    time: Res<Time>,
    mut since: Local<f32>,
    shelter: Option<Res<Shelter>>,
    look: Option<Res<DomeLook>>,
    buildings: Query<(&Building, &Team)>,
    mut domes: Query<(Entity, &ShelterDome, &mut Transform)>,
) {
    *since += time.delta_secs();
    if *since < REFRESH_SECS {
        return;
    }
    *since = 0.0;
    let Some(look) = look else { return };
    let mut bases: HashMap<Team, Vec<Vec3>> = HashMap::new();
    if let Some(shelter) = &shelter {
        for (building, &team) in &buildings {
            if shelter.covers(team) {
                let centre = footprint_center(building.origin, building.kind.spec().footprint);
                bases.entry(team).or_default().push(centre);
            }
        }
    }
    let mut covered = Vec::new();
    for (entity, dome, mut transform) in &mut domes {
        match bases.get(&dome.0) {
            Some(structures) => {
                *transform = dome_over(structures);
                covered.push(dome.0);
            }
            None => commands.entity(entity).despawn(),
        }
    }
    for (team, structures) in bases {
        if !covered.contains(&team) {
            commands.spawn((
                ShelterDome(team),
                Mesh3d(look.mesh.clone()),
                MeshMaterial3d(look.material.clone()),
                NotShadowCaster,
                dome_over(&structures),
                DespawnOnExit(GameState::Playing),
            ));
        }
    }
}

/// A half sphere standing on the ground, around the middle of `structures` and reaching past all of them.
fn dome_over(structures: &[Vec3]) -> Transform {
    let middle = structures.iter().copied().sum::<Vec3>() / structures.len() as f32;
    let reach = structures
        .iter()
        .map(|structure| structure.xz().distance(middle.xz()))
        .fold(0.0, f32::max);
    let radius = (reach + DOME_MARGIN).max(SMALLEST_DOME);
    Transform::from_xyz(middle.x, 0.0, middle.z).with_scale(Vec3::new(radius, radius * 0.6, radius))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dome_reaches_past_every_structure_it_covers() {
        let structures = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(30.0, 0.0, 0.0)];
        let dome = dome_over(&structures);
        assert_eq!(dome.translation, Vec3::new(15.0, 0.0, 0.0));
        assert_eq!(dome.scale.x, 15.0 + DOME_MARGIN);
        let lone = dome_over(&[Vec3::new(4.0, 0.0, 4.0)]);
        assert_eq!(lone.scale.x, SMALLEST_DOME);
    }
}
