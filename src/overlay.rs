use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

use crate::camera::RtsCamera;
use crate::catalog::{HARVESTER_CAPACITY, Team};
use crate::components::*;
use crate::levels::ActiveLevel;
use crate::local_view::LocalView;
use crate::map::cell_center;
use crate::orders::{MarkerKind, OrderMarker};
use crate::settings::{GameSettings, HealthBars};
use crate::veterancy::{Rank, Veterancy};

const RANK_COLOR: Color = Color::srgb(1.0, 0.82, 0.25);
const CHEVRON_SPACING: f32 = 0.22;
const CHEVRON_HALF_WIDTH: f32 = 0.14;
const HEALTH_BAR_HEIGHT_UNIT: f32 = 2.6;
const HEALTH_BAR_HEIGHT_BUILDING: f32 = 6.8;

const HEALTH_BAR_WIDTH_PX: f32 = 7.0;
const HEALTH_BAR_SEGMENTS: usize = 4;
const CARGO_BAR_HEIGHT: f32 = HEALTH_BAR_HEIGHT_UNIT - 0.45;
const CARGO_COLOR: Color = Color::srgb(0.30, 0.95, 0.90);
const EMPTY_CARGO_COLOR: Color = Color::srgb(0.06, 0.16, 0.18);
const LOST_HEALTH_COLOR: Color = Color::srgb(0.38, 0.07, 0.05);
const SEGMENT_TICK_COLOR: Color = Color::srgb(0.03, 0.03, 0.03);

/// Health bars draw thicker than other markers and always on top of models.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct HealthBarGizmos;

pub fn configure_gizmos(mut config_store: ResMut<GizmoConfigStore>) {
    let (config, _) = config_store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 3.0;
    config.depth_bias = -0.5;
    let (bars, _) = config_store.config_mut::<HealthBarGizmos>();
    bars.line.width = HEALTH_BAR_WIDTH_PX;
    bars.depth_bias = -1.0;
}

fn ground_circle(position: Vec3, radius: f32) -> (Isometry3d, f32) {
    (
        Isometry3d::new(position + Vec3::Y * 0.08, Quat::from_rotation_x(FRAC_PI_2)),
        radius,
    )
}

const SELECTION_COLOR: Color = Color::WHITE;
const SELECTION_MARGIN: f32 = 0.3;
/// How much of each side the corner marks cover.
const SELECTION_CORNER_SHARE: f32 = 0.3;
const MOVE_COLOR: Color = Color::srgb(0.4, 1.0, 0.5);
const ATTACK_COLOR: Color = Color::srgb(1.0, 0.35, 0.2);
const PATROL_COLOR: Color = Color::srgb(0.35, 0.7, 1.0);
const RALLY_COLOR: Color = Color::srgb(1.0, 0.85, 0.3);
const ROUTE_HEIGHT: f32 = 0.15;
const RALLY_FLAG_HEIGHT: f32 = 2.2;

fn marker_color(kind: MarkerKind) -> Color {
    match kind {
        MarkerKind::Move => MOVE_COLOR,
        MarkerKind::Attack => ATTACK_COLOR,
        MarkerKind::Patrol => PATROL_COLOR,
        MarkerKind::Rally => RALLY_COLOR,
    }
}

fn order_color(order: Order) -> Color {
    match order {
        Order::Move(_) => MOVE_COLOR,
        Order::AttackMove(_) => ATTACK_COLOR,
        Order::Patrol { .. } => PATROL_COLOR,
    }
}

/// Selected units show their queued route; selected production buildings show their rally flag.
pub fn draw_planned_routes(
    mut gizmos: Gizmos,
    units: Query<(&Transform, &Orders), With<Selected>>,
    rally_points: Query<(&Transform, &RallyPoint), (With<Selected>, With<Building>)>,
) {
    let lifted = |cell: IVec2| cell_center(cell) + Vec3::Y * ROUTE_HEIGHT;
    for (transform, orders) in &units {
        let mut from = transform.translation + Vec3::Y * ROUTE_HEIGHT;
        let planned = orders.current.iter().chain(orders.queued.iter());
        for &order in planned {
            let to = lifted(order.destination());
            gizmos.line(from, to, order_color(order).with_alpha(0.7));
            if let Order::Patrol { from: start, .. } = order {
                gizmos.line(to, lifted(start), PATROL_COLOR.with_alpha(0.35));
            }
            from = to;
        }
    }
    for (transform, rally) in &rally_points {
        let flag = lifted(rally.0);
        gizmos.line(
            transform.translation + Vec3::Y * ROUTE_HEIGHT,
            flag,
            RALLY_COLOR.with_alpha(0.6),
        );
        gizmos.line(flag, flag + Vec3::Y * RALLY_FLAG_HEIGHT, RALLY_COLOR);
        let (isometry, radius) = ground_circle(flag, 0.6);
        gizmos.circle(isometry, radius, RALLY_COLOR);
    }
}

const WEAPON_RANGE_COLOR: Color = Color::srgba(1.0, 0.35, 0.2, 0.55);
const WEAPON_RANGE_SEGMENTS: usize = 96;
const WEAPON_RANGE_LIFT: f32 = 0.2;

/// Selected defensive structures outline how far they shoot, following the hills.
pub fn draw_weapon_ranges(
    mut gizmos: Gizmos,
    view: LocalView,
    level: Res<ActiveLevel>,
    defenses: Query<(&Transform, &Weapon, &Team), (With<Selected>, With<Building>)>,
) {
    let landscape = level.0.landscape;
    for (transform, weapon, &team) in &defenses {
        let center = transform.translation;
        if !view.reveals(team, center) {
            continue;
        }
        let rim = (0..=WEAPON_RANGE_SEGMENTS).map(|segment| {
            let angle = segment as f32 / WEAPON_RANGE_SEGMENTS as f32 * TAU;
            let x = center.x + angle.cos() * weapon.spec.range;
            let z = center.z + angle.sin() * weapon.spec.range;
            Vec3::new(x, landscape.height(x, z) + WEAPON_RANGE_LIFT, z)
        });
        gizmos.linestrip(rim, WEAPON_RANGE_COLOR);
    }
}

/// Gold rank chevrons stacked just left of the health bar.
fn draw_chevrons(
    bars: &mut Gizmos<HealthBarGizmos>,
    bar_start: Vec3,
    screen_right: Vec3,
    rank: Rank,
) {
    for chevron in 0..rank.chevrons() {
        let tip = bar_start + Vec3::Y * (0.12 + chevron as f32 * CHEVRON_SPACING);
        let wing = screen_right * CHEVRON_HALF_WIDTH - Vec3::Y * CHEVRON_HALF_WIDTH;
        bars.line(
            tip - screen_right * CHEVRON_HALF_WIDTH * 2.0 + wing,
            tip,
            RANK_COLOR,
        );
        bars.line(tip, tip + wing, RANK_COLOR);
    }
}

/// A square on the ground around a selected unit or structure, drawn only at its corners.
fn draw_corner_brackets(gizmos: &mut Gizmos, center: Vec3, half_side: f32) {
    let lifted = center + Vec3::Y * 0.08;
    let arm = half_side * 2.0 * SELECTION_CORNER_SHARE;
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = lifted + Vec3::new(sx * half_side, 0.0, sz * half_side);
        gizmos.line(corner, corner - Vec3::X * sx * arm, SELECTION_COLOR);
        gizmos.line(corner, corner - Vec3::Z * sz * arm, SELECTION_COLOR);
    }
}

pub fn draw_overlays(
    mut gizmos: Gizmos,
    mut bars: Gizmos<HealthBarGizmos>,
    settings: Res<GameSettings>,
    view: LocalView,
    time: Res<Time>,
    mut marker: ResMut<OrderMarker>,
    cameras: Query<&GlobalTransform, With<RtsCamera>>,
    entities: Query<
        (
            &Transform,
            &Health,
            &BodyRadius,
            &Team,
            Option<&Harvester>,
            Option<&Veterancy>,
            Has<Selected>,
            Has<Building>,
        ),
        Without<crate::transport::Embarked>,
    >,
) {
    let Ok(camera) = cameras.single() else { return };
    let screen_right = camera.right().as_vec3();
    let toward_camera = camera.back().as_vec3() * 0.05;

    for (transform, health, radius, &team, harvester, veterancy, selected, is_building) in &entities
    {
        if !view.reveals(team, transform.translation) {
            continue;
        }
        let damaged = health.current < health.max;
        if selected {
            draw_corner_brackets(
                &mut gizmos,
                transform.translation,
                radius.0 + SELECTION_MARGIN,
            );
        }
        let half_width = (radius.0 * 0.9).clamp(0.6, 3.0);
        let bar_span = screen_right * half_width * 2.0;
        if let Some(harvester) = harvester.filter(|_| team == view.team() || selected) {
            let cargo_left =
                transform.translation + Vec3::Y * CARGO_BAR_HEIGHT - screen_right * half_width;
            let cargo = (harvester.cargo as f32 / HARVESTER_CAPACITY as f32).clamp(0.0, 1.0);
            let cargo_end = cargo_left + bar_span * cargo;
            if cargo > 0.0 {
                bars.line(cargo_left, cargo_end, CARGO_COLOR);
            }
            if cargo < 1.0 {
                bars.line(cargo_end, cargo_left + bar_span, EMPTY_CARGO_COLOR);
            }
        }
        let show_health = match settings.health_bars {
            HealthBars::Always => true,
            HealthBars::DamagedOrSelected => selected || damaged,
            HealthBars::SelectedOnly => selected,
        };
        if !show_health {
            continue;
        }
        let height = if is_building {
            HEALTH_BAR_HEIGHT_BUILDING
        } else {
            HEALTH_BAR_HEIGHT_UNIT
        };
        let left = transform.translation + Vec3::Y * height - screen_right * half_width;
        let full = bar_span;
        let fraction = health.fraction();
        let bar_color = match fraction {
            f if f > 0.6 => Color::srgb(0.25, 0.9, 0.3),
            f if f > 0.3 => Color::srgb(0.95, 0.8, 0.2),
            _ => Color::srgb(0.95, 0.25, 0.2),
        };
        let health_end = left + full * fraction;
        if fraction > 0.0 {
            bars.line(left, health_end, bar_color);
        }
        if fraction < 1.0 {
            bars.line(health_end, left + full, LOST_HEALTH_COLOR);
        }
        let rank = veterancy.map_or(Rank::Recruit, |veterancy| veterancy.rank);
        draw_chevrons(&mut bars, left - screen_right * 0.35, screen_right, rank);
        for segment in 1..HEALTH_BAR_SEGMENTS {
            let tick = left + full * (segment as f32 / HEALTH_BAR_SEGMENTS as f32) + toward_camera;
            bars.line(
                tick - Vec3::Y * 0.06,
                tick + Vec3::Y * 0.06,
                SEGMENT_TICK_COLOR,
            );
        }
    }

    if marker.remaining_secs > 0.0 {
        marker.remaining_secs -= time.delta_secs();
        let color = marker_color(marker.kind);
        let (isometry, radius) = ground_circle(marker.position, 0.4 + marker.remaining_secs * 1.5);
        gizmos.circle(isometry, radius, color);
    }
}
