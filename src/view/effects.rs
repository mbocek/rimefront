use bevy::prelude::*;
use rand::RngExt;

use crate::sim::catalog::ProjectileStyle;
use crate::sim::combat::Projectile;
use crate::sim::cues::{BlastCue, BlastSize};
use crate::view::local_view::LocalView;

#[derive(Resource)]
pub struct EffectAssets {
    tracer_mesh: Handle<Mesh>,
    shell_mesh: Handle<Mesh>,
    blast_mesh: Handle<Mesh>,
    tracer_material: Handle<StandardMaterial>,
    shell_material: Handle<StandardMaterial>,
    fire_material: Handle<StandardMaterial>,
    flash_material: Handle<StandardMaterial>,
    smoke_material: Handle<StandardMaterial>,
    sparkle_mesh: Handle<Mesh>,
    sparkle_material: Handle<StandardMaterial>,
}

/// A tiny glowing mote drifting up from crystal ore.
#[derive(Component)]
pub struct Sparkle {
    age: f32,
    lifetime: f32,
    velocity: Vec3,
    peak_scale: f32,
}

#[derive(Component)]
pub struct Blast {
    age: f32,
    lifetime: f32,
    peak_scale: f32,
    rise_speed: f32,
}

#[derive(Component)]
pub struct FadingLight {
    age: f32,
    lifetime: f32,
    peak_intensity: f32,
}

pub fn create_effect_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let glowing = |materials: &mut Assets<StandardMaterial>, color: Color, emissive: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: color,
            emissive,
            ..default()
        })
    };
    let tracer_material = glowing(
        &mut materials,
        Color::srgb(1.0, 0.9, 0.5),
        LinearRgba::rgb(12.0, 8.0, 2.0),
    );
    let shell_material = glowing(
        &mut materials,
        Color::srgb(1.0, 0.7, 0.3),
        LinearRgba::rgb(10.0, 4.0, 1.0),
    );
    let fire_material = glowing(
        &mut materials,
        Color::srgb(1.0, 0.55, 0.15),
        LinearRgba::rgb(18.0, 6.0, 1.0),
    );
    let flash_material = glowing(
        &mut materials,
        Color::srgb(1.0, 0.95, 0.7),
        LinearRgba::rgb(30.0, 24.0, 10.0),
    );
    let sparkle_material = glowing(
        &mut materials,
        Color::srgb(0.5, 1.0, 0.95),
        LinearRgba::rgb(4.0, 16.0, 14.0),
    );
    let smoke_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.18, 0.17, 0.16, 0.65),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.insert_resource(EffectAssets {
        tracer_mesh: meshes.add(Capsule3d::new(0.05, 0.6)),
        shell_mesh: meshes.add(Sphere::new(0.14).mesh().ico(2).unwrap()),
        blast_mesh: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        tracer_material,
        shell_material,
        fire_material,
        flash_material,
        smoke_material,
        sparkle_mesh: meshes.add(Sphere::new(1.0).mesh().ico(0).unwrap()),
        sparkle_material,
    });
}

pub fn spawn_sparkle(
    commands: &mut Commands,
    effects: &EffectAssets,
    position: Vec3,
    velocity: Vec3,
    size: f32,
) {
    let mut rng = rand::rng();
    commands.spawn((
        Sparkle {
            age: 0.0,
            lifetime: rng.random_range(0.9..1.6),
            velocity,
            peak_scale: size,
        },
        Mesh3d(effects.sparkle_mesh.clone()),
        MeshMaterial3d(effects.sparkle_material.clone()),
        Transform::from_translation(position).with_scale(Vec3::splat(0.001)),
    ));
}

pub fn animate_sparkles(
    mut commands: Commands,
    time: Res<Time>,
    mut sparkles: Query<(Entity, &mut Sparkle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut sparkle, mut transform) in &mut sparkles {
        sparkle.age += dt;
        let progress = sparkle.age / sparkle.lifetime;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation += sparkle.velocity * dt;
        let twinkle =
            (progress * std::f32::consts::PI).sin() * (0.7 + 0.3 * (sparkle.age * 23.0).sin());
        transform.scale = Vec3::splat((sparkle.peak_scale * twinkle).max(0.001));
    }
}

pub fn show_blasts(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    view: LocalView,
    mut cues: MessageReader<BlastCue>,
) {
    let seen = cues
        .read()
        .filter(|cue| view.sight().is_visible(cue.position));
    for cue in seen {
        spawn_blast(&mut commands, &effects, cue.position, cue.size);
    }
}

pub fn dress_projectiles(
    mut commands: Commands,
    effects: Res<EffectAssets>,
    projectiles: Query<(Entity, &Projectile), Added<Projectile>>,
) {
    for (entity, projectile) in &projectiles {
        commands
            .entity(entity)
            .insert(effects.projectile_visual(projectile.style()));
    }
}

impl EffectAssets {
    pub fn projectile_visual(
        &self,
        style: ProjectileStyle,
    ) -> (Mesh3d, MeshMaterial3d<StandardMaterial>) {
        match style {
            ProjectileStyle::Missile => (
                Mesh3d(self.tracer_mesh.clone()),
                MeshMaterial3d(self.shell_material.clone()),
            ),
            ProjectileStyle::Bullet => (
                Mesh3d(self.tracer_mesh.clone()),
                MeshMaterial3d(self.tracer_material.clone()),
            ),
            ProjectileStyle::Shell | ProjectileStyle::ArtilleryShell => (
                Mesh3d(self.shell_mesh.clone()),
                MeshMaterial3d(self.shell_material.clone()),
            ),
            ProjectileStyle::Flame => (
                Mesh3d(self.blast_mesh.clone()),
                MeshMaterial3d(self.fire_material.clone()),
            ),
        }
    }
}

pub fn spawn_blast(
    commands: &mut Commands,
    effects: &EffectAssets,
    position: Vec3,
    size: BlastSize,
) {
    let mut rng = rand::rng();
    let (fireballs, scale, smoke_puffs, light_intensity): (u32, f32, u32, f32) = match size {
        BlastSize::MuzzleFlash => (1, 0.25, 0, 0.0),
        BlastSize::BulletImpact => (1, 0.18, 1, 0.0),
        BlastSize::ShellImpact => (2, 0.9, 2, 60_000.0),
        BlastSize::VehicleWreck => (4, 1.6, 5, 400_000.0),
        BlastSize::BuildingCollapse => (9, 2.6, 10, 1_500_000.0),
        BlastSize::DamageSmoke => (0, 0.55, 1, 0.0),
        BlastSize::DamageFire => (1, 0.45, 1, 0.0),
    };
    let spread = scale * 0.8;
    let random_offset = |rng: &mut rand::rngs::ThreadRng, spread: f32| {
        Vec3::new(
            rng.random_range(-spread..=spread),
            rng.random_range(0.0..=spread),
            rng.random_range(-spread..=spread),
        )
    };

    let core_material = if size == BlastSize::MuzzleFlash {
        effects.flash_material.clone()
    } else {
        effects.fire_material.clone()
    };
    for _ in 0..fireballs {
        commands.spawn((
            Blast {
                age: 0.0,
                lifetime: rng.random_range(0.25..0.55) * scale.max(0.4),
                peak_scale: scale * rng.random_range(0.7..1.2),
                rise_speed: 0.6,
            },
            Mesh3d(effects.blast_mesh.clone()),
            MeshMaterial3d(core_material.clone()),
            Transform::from_translation(position + random_offset(&mut rng, spread))
                .with_scale(Vec3::splat(0.01)),
        ));
    }
    for _ in 0..smoke_puffs {
        commands.spawn((
            Blast {
                age: 0.0,
                lifetime: rng.random_range(1.2..2.6),
                peak_scale: scale * rng.random_range(0.8..1.5),
                rise_speed: rng.random_range(0.8..1.8),
            },
            Mesh3d(effects.blast_mesh.clone()),
            MeshMaterial3d(effects.smoke_material.clone()),
            Transform::from_translation(position + random_offset(&mut rng, spread * 1.5))
                .with_scale(Vec3::splat(0.01)),
        ));
    }
    if light_intensity > 0.0 {
        commands.spawn((
            PointLight {
                intensity: light_intensity,
                color: Color::srgb(1.0, 0.6, 0.25),
                range: scale * 14.0,
                shadow_maps_enabled: false,
                ..default()
            },
            FadingLight {
                age: 0.0,
                lifetime: 0.45,
                peak_intensity: light_intensity,
            },
            Transform::from_translation(position + Vec3::Y * 1.5),
        ));
    }
}

pub fn animate_blasts(
    mut commands: Commands,
    time: Res<Time>,
    mut blasts: Query<(Entity, &mut Blast, &mut Transform)>,
    mut lights: Query<(Entity, &mut FadingLight, &mut PointLight)>,
) {
    let dt = time.delta_secs();
    for (entity, mut blast, mut transform) in &mut blasts {
        blast.age += dt;
        let progress = blast.age / blast.lifetime;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let swell = (progress * 4.0).min(1.0);
        let fade = 1.0 - progress.powf(2.0);
        transform.scale = Vec3::splat((blast.peak_scale * swell * fade).max(0.01));
        transform.translation.y += blast.rise_speed * dt;
    }
    for (entity, mut light, mut point_light) in &mut lights {
        light.age += dt;
        let progress = light.age / light.lifetime;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        point_light.intensity = light.peak_intensity * (1.0 - progress);
    }
}
