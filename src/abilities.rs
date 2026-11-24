use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::catalog::{ArmorClass, ProjectileStyle, Team, UnitKind, WeaponSpec};
use crate::combat::{DamageSource, Shot, Targets, fire_projectile, surface_distance};
use crate::components::Mobile;
use crate::determinism::SimulationRng;
use crate::effects::BlastCue;
use crate::map::cell_center;
use crate::sound::Sound;

pub const MINE_COST: i32 = 100;
const MINE_ARMING_SECS: f32 = 1.5;
/// How close a passing enemy must come, measured to the edge of its body.
const MINE_TRIGGER_DISTANCE: f32 = 0.6;

const MINE_BLAST: WeaponSpec = WeaponSpec {
    range: 2.0,
    damage: 110.0,
    reload_secs: 0.0,
    projectile_speed: 100.0,
    splash_radius: 1.8,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 1.0,
    versus_vehicle: 1.0,
    versus_structure: 0.3,
    versus_aircraft: 0.0,
    report: Sound::ShellExplosion,
};

/// A mine an engineer buried; its own side and allies see it, enemies only when it goes off.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Mine {
    pub team: Team,
    pub cell: IVec2,
    arming_secs: f32,
}

/// Buries a mine in `cell` and charges for it; false when the cell already holds one or the money is short.
pub fn lay_mine(
    commands: &mut Commands,
    credits: &mut i32,
    team: Team,
    cell: IVec2,
    occupied: impl Fn(IVec2) -> bool,
) -> bool {
    if occupied(cell) || *credits < MINE_COST {
        return false;
    }
    *credits -= MINE_COST;
    bury(
        commands,
        Mine {
            team,
            cell,
            arming_secs: MINE_ARMING_SECS,
        },
    );
    true
}

/// Puts a mine in the ground as it is, for a new one or one restored from a save.
pub fn bury(commands: &mut Commands, mine: Mine) {
    commands.spawn((
        mine,
        Transform::from_translation(cell_center(mine.cell)),
        Visibility::Hidden,
    ));
}

pub fn detonate_mines(
    mut commands: Commands,
    mut rng: ResMut<SimulationRng>,
    time: Res<Time>,
    mut blasts: MessageWriter<BlastCue>,
    targets: Res<Targets>,
    mut mines: Query<(Entity, &mut Mine, &Transform)>,
) {
    let mut in_simulation_order: Vec<_> = mines.iter_mut().collect();
    in_simulation_order.sort_by_key(|(_, mine, _)| (mine.cell.y, mine.cell.x));
    for (entity, mut mine, transform) in in_simulation_order {
        if mine.arming_secs > 0.0 {
            mine.arming_secs -= time.delta_secs();
            continue;
        }
        let position = transform.translation;
        let victim = targets
            .near(position, MINE_TRIGGER_DISTANCE)
            .find(|target| {
                targets.hostile(target.team, mine.team)
                    && !target.is_building
                    && target.armor != ArmorClass::Aircraft
                    && surface_distance(position, target) <= MINE_TRIGGER_DISTANCE
            });
        let Some(victim) = victim else {
            continue;
        };
        let shot = Shot {
            team: mine.team,
            source: DamageSource::Unit(UnitKind::Engineer),
            shooter: entity,
            muzzle: position + Vec3::Y * 0.2,
            weapon: MINE_BLAST,
        };
        fire_projectile(&mut commands, &mut rng, &mut blasts, shot, victim);
        commands.entity(entity).despawn();
    }
}

const CONCEAL_AFTER_SECS: f32 = 3.0;
const REVEALED_AFTER_SHOT_SECS: f32 = 2.0;
/// Enemies this close spot a camouflaged unit anyway.
const SPOTTING_DISTANCE: f32 = 8.0;

/// A sniper that keeps still and quiet for a while is hidden from enemies who are not right next to it.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Camouflage {
    still_secs: f32,
    revealed_secs: f32,
    pub concealed: bool,
}

impl Camouflage {
    pub fn betray_by_firing(&mut self) {
        self.revealed_secs = REVEALED_AFTER_SHOT_SECS;
        self.concealed = false;
    }
}

pub fn update_camouflage(
    time: Res<Time>,
    targets: Res<Targets>,
    mut hiders: Query<(&Team, &Transform, &Mobile, &mut Camouflage)>,
) {
    let dt = time.delta_secs();
    for (&team, transform, mobile, mut camouflage) in &mut hiders {
        camouflage.revealed_secs = (camouflage.revealed_secs - dt).max(0.0);
        camouflage.still_secs = if mobile.is_moving() {
            0.0
        } else {
            camouflage.still_secs + dt
        };
        let position = transform.translation;
        let spotted = targets.near(position, SPOTTING_DISTANCE).any(|other| {
            targets.hostile(other.team, team)
                && surface_distance(position, other) <= SPOTTING_DISTANCE
        });
        camouflage.concealed = camouflage.still_secs >= CONCEAL_AFTER_SECS
            && camouflage.revealed_secs <= 0.0
            && !spotted;
    }
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
    view: crate::local_view::LocalView,
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
