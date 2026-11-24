use bevy::prelude::*;
use rand::RngExt;

use crate::app::settings::GameSettings;
use crate::sim::combat::{DamageDealt, Targets};
use crate::sim::components::{BodyRadius, Health};
use crate::sim::player_commands::LocalTeam;
use crate::ui::sidebar::UiFonts;
use crate::view::camera::RtsCamera;
use crate::view::effects::{BlastSize, EffectAssets, spawn_blast};

const NUMBER_LIFETIME_SECS: f32 = 1.1;
/// Rapid hits on the same target within this window add up into one number instead of a stream.
const MERGE_WINDOW_SECS: f32 = 0.4;
const NUMBER_RISE_METERS: f32 = 1.6;
const NUMBER_HEIGHT_UNIT: f32 = 3.2;
const NUMBER_HEIGHT_BUILDING: f32 = 7.6;
const SMALL_HIT_FONT: f32 = 17.0;
const HEAVY_HIT_FONT: f32 = 24.0;
const HEAVY_HIT: f32 = 40.0;
const PLAYER_LOSS_COLOR: Color = Color::srgb(1.0, 0.36, 0.30);
const ENEMY_LOSS_COLOR: Color = Color::srgb(1.0, 0.86, 0.32);

const SMOKING_BELOW: f32 = 0.5;
const BURNING_BELOW: f32 = 0.25;
const SMOKE_INTERVAL_SECS: f32 = 0.55;
const FIRE_INTERVAL_SECS: f32 = 0.3;
const PUFFS_PER_METER_OF_RADIUS: f32 = 0.6;

#[derive(Component)]
pub struct DamageNumber {
    victim: Entity,
    anchor: Vec3,
    total: f32,
    age: f32,
    color: Color,
}

#[derive(Component, Default)]
pub struct DamageSmoke {
    cooldown: f32,
}

fn number_height(is_building: bool) -> f32 {
    if is_building {
        NUMBER_HEIGHT_BUILDING
    } else {
        NUMBER_HEIGHT_UNIT
    }
}

fn format_damage(total: f32) -> String {
    format!("-{}", total.round().max(1.0) as i32)
}

pub fn spawn_damage_numbers(
    local: Res<LocalTeam>,
    mut commands: Commands,
    mut hits: MessageReader<DamageDealt>,
    targets: Res<Targets>,
    fonts: Option<Res<UiFonts>>,
    settings: Res<GameSettings>,
    mut numbers: Query<(&mut DamageNumber, &mut Text, &mut TextFont)>,
) {
    let Some(fonts) = fonts.filter(|_| settings.damage_numbers) else {
        hits.clear();
        return;
    };
    for hit in hits.read() {
        let merged = numbers
            .iter_mut()
            .find(|(number, ..)| number.victim == hit.victim && number.age < MERGE_WINDOW_SECS);
        if let Some((mut number, mut text, mut font)) = merged {
            number.total += hit.amount;
            text.0 = format_damage(number.total);
            if number.total >= HEAVY_HIT {
                font.font_size = bevy::text::FontSize::Px(HEAVY_HIT_FONT);
            }
            continue;
        }
        let Some(victim) = targets.get(hit.victim) else {
            continue;
        };
        if !victim.seen_by(local.0) {
            continue;
        }
        let color = if hit.team == local.0 {
            PLAYER_LOSS_COLOR
        } else {
            ENEMY_LOSS_COLOR
        };
        let size = if hit.amount >= HEAVY_HIT {
            HEAVY_HIT_FONT
        } else {
            SMALL_HIT_FONT
        };
        commands.spawn((
            DamageNumber {
                victim: hit.victim,
                anchor: victim.position + Vec3::Y * number_height(victim.is_building),
                total: hit.amount,
                age: 0.0,
                color,
            },
            fonts.heading(format_damage(hit.amount), size, color),
            TextShadow::default(),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Pickable::IGNORE,
            Visibility::Hidden,
        ));
    }
}

pub fn float_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    targets: Res<Targets>,
    cameras: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut numbers: Query<(
        Entity,
        &mut DamageNumber,
        &mut Node,
        &mut TextColor,
        &mut Visibility,
        &ComputedNode,
    )>,
) {
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    for (entity, mut number, mut node, mut color, mut visibility, computed) in &mut numbers {
        number.age += time.delta_secs();
        if number.age >= NUMBER_LIFETIME_SECS {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(victim) = targets.get(number.victim) {
            number.anchor = victim.position + Vec3::Y * number_height(victim.is_building);
        }
        let progress = number.age / NUMBER_LIFETIME_SECS;
        let floating = number.anchor + Vec3::Y * NUMBER_RISE_METERS * progress;
        let Ok(screen) = camera.world_to_viewport(camera_transform, floating) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let half_size = computed.size() * computed.inverse_scale_factor() * 0.5;
        node.left = Val::Px(screen.x - half_size.x);
        node.top = Val::Px(screen.y - half_size.y);
        let fade = 1.0 - ((progress - 0.6) / 0.4).clamp(0.0, 1.0);
        color.0 = number.color.with_alpha(fade);
        *visibility = Visibility::Inherited;
    }
}

/// Damaged units and structures smoke, and burn once they are close to destruction.
pub fn emit_damage_smoke(
    mut commands: Commands,
    time: Res<Time>,
    effects: Res<EffectAssets>,
    view: crate::view::local_view::LocalView,
    mut damaged: Query<(
        Entity,
        &Health,
        &Transform,
        &BodyRadius,
        Option<&mut DamageSmoke>,
    )>,
) {
    let mut rng = rand::rng();
    for (entity, health, transform, radius, smoke) in &mut damaged {
        let fraction = health.fraction();
        if fraction >= SMOKING_BELOW || !view.sight().is_visible(transform.translation) {
            continue;
        }
        let Some(mut smoke) = smoke else {
            commands.entity(entity).insert(DamageSmoke::default());
            continue;
        };
        smoke.cooldown -= time.delta_secs();
        if smoke.cooldown > 0.0 {
            continue;
        }
        let burning = fraction < BURNING_BELOW;
        smoke.cooldown = if burning {
            FIRE_INTERVAL_SECS
        } else {
            SMOKE_INTERVAL_SECS
        };
        let puffs = (radius.0 * PUFFS_PER_METER_OF_RADIUS).ceil().max(1.0) as usize;
        for _ in 0..puffs {
            let spread = radius.0 * 0.5;
            let offset = Vec3::new(
                rng.random_range(-spread..=spread),
                radius.0 * 0.6 + 0.5,
                rng.random_range(-spread..=spread),
            );
            let size = if burning {
                BlastSize::DamageFire
            } else {
                BlastSize::DamageSmoke
            };
            spawn_blast(
                &mut commands,
                &effects,
                transform.translation + offset,
                size,
            );
        }
    }
}
