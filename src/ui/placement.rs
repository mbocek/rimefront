use bevy::prelude::*;

use crate::sim::catalog::{BuildingKind, Team};
use crate::sim::components::*;
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::economy::Economies;
use crate::sim::intel::Intel;
use crate::sim::player_commands::LocalTeam;
use crate::sim::player_commands::{CommandIssuer, PlayerCommand};
use crate::sim::production::{affordable_wall_line, placement_context, wall_line};
use crate::view::camera::Cursor;
use crate::world::map::{CELL_SIZE, GameMap, cell_center, footprint_center, world_to_cell};

#[derive(Resource, Default)]
pub struct Placement {
    pub active: Option<BuildingKind>,
    ghost: Option<Entity>,
    /// Where the current wall drag started.
    wall_anchor: Option<IVec2>,
}

#[derive(Component)]
pub struct PlacementGhost;

/// One segment of the wall being dragged, shown before the button is released.
#[derive(Component)]
pub struct WallDragGhost;

const WALL_GHOST_HEIGHT: f32 = 1.3;

#[derive(Resource)]
pub struct GhostMaterials {
    valid: Handle<StandardMaterial>,
    invalid: Handle<StandardMaterial>,
    wall_segment: Handle<Mesh>,
}

pub fn start_without_placement(mut commands: Commands) {
    commands.insert_resource(Placement::default());
}

pub fn create_ghost_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let ghost = |color: Color| StandardMaterial {
        base_color: color,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    };
    commands.insert_resource(GhostMaterials {
        valid: materials.add(ghost(Color::srgba(0.2, 1.0, 0.35, 0.35))),
        invalid: materials.add(ghost(Color::srgba(1.0, 0.15, 0.1, 0.4))),
        wall_segment: meshes.add(Cuboid::new(
            CELL_SIZE * 0.9,
            WALL_GHOST_HEIGHT,
            CELL_SIZE * 0.9,
        )),
    });
}

fn placement_origin(cursor_ground: Vec3, footprint: IVec2) -> IVec2 {
    let centered = cursor_ground
        - Vec3::new(footprint.x as f32 - 1.0, 0.0, footprint.y as f32 - 1.0) * CELL_SIZE * 0.5;
    world_to_cell(centered)
}

/// Cells of a straight wall from `from` towards `to`, along whichever axis the drag mostly follows.
pub fn place_structure(
    mut cues: MessageWriter<SoundCue>,
    mut commands: Commands,
    mut issuer: CommandIssuer,
    mut meshes: ResMut<Assets<Mesh>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Res<Cursor>,
    ghost_materials: Res<GhostMaterials>,
    map: Res<GameMap>,
    mut placement: ResMut<Placement>,
    economies: Res<Economies>,
    intel: Res<Intel>,
    buildings: Query<(&Building, &Team)>,
    units: Query<&Transform, With<Unit>>,
    mut ghosts: Query<
        (
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Visibility,
        ),
        (With<PlacementGhost>, Without<Unit>),
    >,
) {
    let Some(kind) = placement.active else {
        if let Some(ghost) = placement.ghost.take() {
            commands.entity(ghost).despawn();
        }
        return;
    };
    if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
        placement.active = None;
        placement.wall_anchor = None;
        return;
    }
    let footprint = kind.spec().footprint;
    let ghost = *placement.ghost.get_or_insert_with(|| {
        let size = footprint.as_vec2() * CELL_SIZE;
        commands
            .spawn((
                PlacementGhost,
                Mesh3d(meshes.add(Cuboid::new(size.x, 0.4, size.y))),
                MeshMaterial3d(ghost_materials.invalid.clone()),
                Transform::default(),
            ))
            .id()
    });
    let Some(ground) = cursor.ground.filter(|_| !cursor.over_interface) else {
        return;
    };
    let origin = placement_origin(ground, footprint);
    let team = issuer.team();
    let known_ground = intel.vision(team);
    let valid =
        placement_context(&map, team, known_ground, &buildings, &units).can_place(kind, origin);
    if let Ok((mut transform, mut material, mut visibility)) = ghosts.get_mut(ghost) {
        // Off the dragged line it reads as one more segment of the wall.
        let dragging_wall = kind.is_wall() && placement.wall_anchor.is_some();
        visibility.set_if_neq(if dragging_wall {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
        transform.translation = footprint_center(origin, footprint) + Vec3::Y * 0.2;
        material.0 = if valid {
            ghost_materials.valid.clone()
        } else {
            ghost_materials.invalid.clone()
        };
    }
    if kind.is_wall() {
        if mouse.just_pressed(MouseButton::Left) {
            placement.wall_anchor = Some(origin);
        }
        let Some(anchor) = placement
            .wall_anchor
            .filter(|_| mouse.just_released(MouseButton::Left))
        else {
            return;
        };
        placement.wall_anchor = None;
        let credits = economies.of(team).credits;
        let segments = {
            let mut context = placement_context(&map, team, known_ground, &buildings, &units);
            affordable_wall_line(&mut context, anchor, origin, credits)
        };
        if segments.is_empty() {
            cues.write(SoundCue::interface(Sound::UiError));
            return;
        }
        issuer.issue(PlayerCommand::LayWall {
            from: anchor,
            to: origin,
        });
        cues.write(SoundCue::at(Sound::BuildingPlaced, cell_center(origin)));
        return;
    }
    if valid && mouse.just_pressed(MouseButton::Left) {
        issuer.issue(PlayerCommand::PlaceStructure { kind, origin });
        cues.write(SoundCue::at(
            Sound::BuildingPlaced,
            footprint_center(origin, footprint),
        ));
        placement.active = None;
    }
}

/// While a wall is dragged, shows every segment along the line: green where one will be built, red where
/// the ground refuses it or the credits run out. It asks the same rules that lay the wall.
pub fn preview_wall_drag(
    mut commands: Commands,
    placement: Res<Placement>,
    cursor: Res<Cursor>,
    map: Res<GameMap>,
    economies: Res<Economies>,
    intel: Res<Intel>,
    local: Res<LocalTeam>,
    ghost_materials: Res<GhostMaterials>,
    buildings: Query<(&Building, &Team)>,
    units: Query<&Transform, With<Unit>>,
    mut ghosts: Query<
        (
            Entity,
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        (With<WallDragGhost>, Without<Unit>),
    >,
) {
    let dragged = placement
        .wall_anchor
        .filter(|_| placement.active.is_some_and(BuildingKind::is_wall))
        .zip(cursor.ground.filter(|_| !cursor.over_interface))
        .map(|(anchor, ground)| (anchor, placement_origin(ground, IVec2::ONE)));
    let segments: Vec<(IVec2, bool)> = match dragged {
        Some((anchor, end)) => {
            let team = local.0;
            let mut context = placement_context(&map, team, intel.vision(team), &buildings, &units);
            let laid = affordable_wall_line(&mut context, anchor, end, economies.of(team).credits);
            wall_line(anchor, end)
                .into_iter()
                .map(|cell| (cell, laid.contains(&cell)))
                .collect()
        }
        None => Vec::new(),
    };
    let mut shown = ghosts.iter_mut();
    for &(cell, will_be_built) in &segments {
        let placed = Transform::from_translation(
            footprint_center(cell, IVec2::ONE) + Vec3::Y * WALL_GHOST_HEIGHT * 0.5,
        );
        let material = if will_be_built {
            ghost_materials.valid.clone()
        } else {
            ghost_materials.invalid.clone()
        };
        match shown.next() {
            Some((_, mut transform, mut current)) => {
                transform.set_if_neq(placed);
                if current.0 != material {
                    current.0 = material;
                }
            }
            None => {
                commands.spawn((
                    WallDragGhost,
                    Mesh3d(ghost_materials.wall_segment.clone()),
                    MeshMaterial3d(material),
                    placed,
                ));
            }
        }
    }
    for (spare, ..) in shown {
        commands.entity(spare).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dragged_wall_shows_red_segments_past_what_the_credits_buy() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        let valid = Handle::<StandardMaterial>::default();
        let invalid = Handle::<StandardMaterial>::from(bevy::asset::uuid::Uuid::from_u128(1));
        world.insert_resource(GhostMaterials {
            valid: valid.clone(),
            invalid: invalid.clone(),
            wall_segment: Handle::default(),
        });
        let anchor = IVec2::new(14, 11);
        world.insert_resource(Placement {
            active: Some(BuildingKind::Wall),
            ghost: None,
            wall_anchor: Some(anchor),
        });
        let end = anchor + IVec2::new(6, 0);
        world.insert_resource(Cursor {
            ground: Some(footprint_center(end, IVec2::ONE)),
            ..default()
        });
        world.insert_resource(GameMap::open_field());
        let mut economies = Economies::default();
        economies.of_mut(Team::Player).credits = BuildingKind::Wall.spec().cost * 3 + 10;
        world.insert_resource(economies);
        world.insert_resource(Intel::new(false));
        world.insert_resource(LocalTeam(Team::Player));
        world.spawn((
            Building {
                kind: BuildingKind::CommandCenter,
                origin: IVec2::new(10, 10),
            },
            Team::Player,
        ));

        world.run_system_once(preview_wall_drag).unwrap();
        let mut ghosts =
            world.query_filtered::<&MeshMaterial3d<StandardMaterial>, With<WallDragGhost>>();
        let shown: Vec<bool> = ghosts
            .iter(&world)
            .map(|material| material.0 == valid)
            .collect();
        assert_eq!(
            shown,
            [true, true, true, false, false, false, false],
            "three segments are affordable"
        );

        world.resource_mut::<Placement>().wall_anchor = None;
        world.run_system_once(preview_wall_drag).unwrap();
        world.flush();
        assert_eq!(
            ghosts.iter(&world).count(),
            0,
            "the preview goes when the button is released"
        );
    }
}
