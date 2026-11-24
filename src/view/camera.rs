use crate::ui::notifications::LastAlert;
use bevy::anti_alias::smaa::Smaa;
use bevy::camera::Hdr;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};
use bevy::window::PrimaryWindow;

use crate::app::match_flow::Persistent;
use crate::app::settings::GameSettings;
use crate::missions::levels::ActiveLevel;
use crate::sim::catalog::{BuildingKind, Team};
use crate::sim::player_commands::LocalTeam;
use crate::ui::key_bindings::{Action, KeyBindings};
use crate::ui::sidebar::SIDEBAR_WIDTH;
use crate::world::map::{footprint_center, map_bounds};
use crate::world::terrain::Landscape;

const PAN_SPEED: f32 = 1.1;
const FAST_PAN_MULTIPLIER: f32 = 2.2;
const EDGE_PAN_MARGIN: f32 = 6.0;
const ROTATE_SPEED: f32 = 1.6;
/// Each mouse wheel notch moves the camera this share of its distance closer or farther.
const ZOOM_PER_NOTCH: f32 = 0.1;
/// Trackpads report scrolling in pixels, many small events per gesture; this many make one notch.
const PIXELS_PER_NOTCH: f32 = 60.0;
const ZOOM_SMOOTHING: f32 = 10.0;
const MIN_DISTANCE: f32 = 12.0;
const MAX_DISTANCE: f32 = 160.0;
const START_DISTANCE: f32 = 40.0;
const PITCH: f32 = 0.95;
const VERTICAL_FOV: f32 = std::f32::consts::FRAC_PI_4;
const FOCUS_MARGIN: f32 = 6.0;
const SKY_COLOR: Color = Color::srgb(0.66, 0.74, 0.82);

#[derive(Component)]
pub struct RtsCamera {
    pub focus: Vec3,
    pub distance: f32,
    pub target_distance: f32,
    yaw: f32,
}

/// Where a capture run's mouse script points, in place of the real cursor; `None` outside such runs.
#[derive(Resource, Default)]
pub struct ScriptedPointer(pub Option<Vec2>);

#[derive(Resource, Default)]
pub struct Cursor {
    pub screen: Option<Vec2>,
    pub ground: Option<Vec3>,
    /// The line from the camera through the pointer, for picking things above the ground such as aircraft.
    pub ray: Option<Ray3d>,
    /// True while the pointer is over any interface element, so world clicks must be ignored.
    pub over_interface: bool,
}

/// Above the local player's own Command Center site.
fn home_focus(level: &ActiveLevel, team: Team) -> Vec3 {
    let site = level.0.force_of(team).unwrap_or(&level.0.player).site;
    footprint_center(site.origin(), BuildingKind::CommandCenter.spec().footprint)
}

pub fn focus_on_player_base(
    level: Res<ActiveLevel>,
    local: Res<LocalTeam>,
    mut cameras: Query<&mut RtsCamera>,
) {
    for mut camera in &mut cameras {
        camera.focus = home_focus(&level, local.0);
    }
}

impl RtsCamera {
    /// 0 fully zoomed in, 1 fully zoomed out.
    pub fn zoom_fraction(&self) -> f32 {
        ((self.distance - MIN_DISTANCE) / (MAX_DISTANCE - MIN_DISTANCE)).clamp(0.0, 1.0)
    }
}

pub fn setup_scene(mut commands: Commands) {
    let middle = map_bounds().world_center();
    let start_focus = Vec3::new(middle.x, 0.0, middle.y);
    commands.spawn((
        Persistent,
        Camera3d::default(),
        Hdr,
        Msaa::Off,
        Smaa::default(),
        ScreenSpaceAmbientOcclusion::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: VERTICAL_FOV,
            far: 600.0,
            ..default()
        }),
        Bloom::NATURAL,
        ColorGrading {
            global: ColorGradingGlobal {
                post_saturation: 1.06,
                ..default()
            },
            midtones: ColorGradingSection {
                contrast: 1.08,
                ..default()
            },
            ..default()
        },
        RtsCamera {
            focus: start_focus,
            distance: START_DISTANCE,
            target_distance: START_DISTANCE,
            yaw: 0.0,
        },
        Transform::default(),
        DistanceFog {
            color: SKY_COLOR,
            directional_light_color: Color::srgba(1.0, 0.92, 0.75, 0.35),
            directional_light_exponent: 24.0,
            falloff: FogFalloff::Linear {
                start: 110.0,
                end: 340.0,
            },
        },
    ));

    commands.spawn((
        Persistent,
        DirectionalLight {
            illuminance: 7_000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.96, 0.9),
            ..default()
        },
        Transform::from_xyz(-48.0, 72.0, 34.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            maximum_distance: 180.0,
            first_cascade_far_bound: 45.0,
            ..default()
        }
        .build(),
    ));
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.72, 0.80, 1.0),
        brightness: 280.0,
        ..default()
    });
    commands.insert_resource(ClearColor(SKY_COLOR));
}

fn keyboard_pan(keys: &ButtonInput<KeyCode>, bindings: &KeyBindings) -> Vec2 {
    let mut pan = Vec2::ZERO;
    if bindings.pressed(keys, Action::ScrollUp) || keys.pressed(KeyCode::ArrowUp) {
        pan.y -= 1.0;
    }
    if bindings.pressed(keys, Action::ScrollDown) || keys.pressed(KeyCode::ArrowDown) {
        pan.y += 1.0;
    }
    if bindings.pressed(keys, Action::ScrollLeft) || keys.pressed(KeyCode::ArrowLeft) {
        pan.x -= 1.0;
    }
    if bindings.pressed(keys, Action::ScrollRight) || keys.pressed(KeyCode::ArrowRight) {
        pan.x += 1.0;
    }
    pan
}

fn edge_pan(window: &Window, cursor: &Cursor) -> Vec2 {
    let Some(screen) = cursor.screen.filter(|_| window.focused) else {
        return Vec2::ZERO;
    };
    let playfield_width = window.width() - SIDEBAR_WIDTH;
    let mut pan = Vec2::ZERO;
    if screen.x < EDGE_PAN_MARGIN {
        pan.x -= 1.0;
    }
    if screen.x > playfield_width - EDGE_PAN_MARGIN && screen.x <= playfield_width {
        pan.x += 1.0;
    }
    if screen.y < EDGE_PAN_MARGIN {
        pan.y -= 1.0;
    }
    if screen.y > window.height() - EDGE_PAN_MARGIN {
        pan.y += 1.0;
    }
    pan
}

pub fn control_camera(
    local: Res<LocalTeam>,
    settings: Res<GameSettings>,
    level: Res<ActiveLevel>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    cursor: Res<Cursor>,
    alert: Option<Res<LastAlert>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mut cameras: Query<(&mut RtsCamera, &mut Transform)>,
) {
    let Ok((mut camera, mut transform)) = cameras.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let dt = time.delta_secs();

    if settings.keys.just_pressed(&keys, Action::JumpHome) || keys.just_pressed(KeyCode::Home) {
        camera.focus = home_focus(&level, local.0);
    }
    if settings.keys.just_pressed(&keys, Action::JumpToAlert)
        && let Some(spot) = alert.as_ref().and_then(|alert| alert.0)
    {
        camera.focus = spot;
    }
    if settings.keys.just_pressed(&keys, Action::ResetRotation) {
        camera.yaw = 0.0;
    }
    let turn = |action| f32::from(u8::from(settings.keys.pressed(&keys, action)));
    camera.yaw += yaw_change(turn(Action::RotateLeft), turn(Action::RotateRight), dt);
    for event in wheel.read() {
        if !cursor.over_interface {
            let notches = match event.unit {
                MouseScrollUnit::Line => event.y,
                MouseScrollUnit::Pixel => event.y / PIXELS_PER_NOTCH,
            };
            camera.target_distance = (camera.target_distance
                * (1.0 - ZOOM_PER_NOTCH).powf(notches))
            .clamp(MIN_DISTANCE, MAX_DISTANCE);
        }
    }
    camera.distance += (camera.target_distance - camera.distance) * (ZOOM_SMOOTHING * dt).min(1.0);

    let yaw_rotation = Quat::from_rotation_y(camera.yaw);
    let fast = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let edge = if settings.edge_scroll {
        edge_pan(window, &cursor)
    } else {
        Vec2::ZERO
    };
    let pan = keyboard_pan(&keys, &settings.keys) + edge;
    let pan_speed = PAN_SPEED
        * settings.scroll_speed_factor()
        * camera.distance
        * if fast { FAST_PAN_MULTIPLIER } else { 1.0 };
    camera.focus +=
        yaw_rotation * Vec3::new(pan.x, 0.0, pan.y).normalize_or_zero() * pan_speed * dt;

    if mouse.pressed(MouseButton::Middle) {
        let meters_per_pixel = 2.0 * camera.distance * (VERTICAL_FOV * 0.5).tan() / window.height();
        let drag = mouse_motion.delta * meters_per_pixel * Vec2::new(1.0, 1.0 / PITCH.sin());
        camera.focus -= yaw_rotation * Vec3::new(drag.x, 0.0, drag.y);
    }

    let bounds = map_bounds();
    let reachable = camera.focus.xz().clamp(
        bounds.world_min() - FOCUS_MARGIN,
        bounds.world_max() + FOCUS_MARGIN,
    );
    camera.focus.x = reachable.x;
    camera.focus.z = reachable.y;
    let ground_under_focus = level
        .0
        .landscape
        .height(camera.focus.x, camera.focus.z)
        .max(0.0);
    camera.focus.y += (ground_under_focus - camera.focus.y) * (dt * 4.0).min(1.0);

    let offset = yaw_rotation * Vec3::new(0.0, PITCH.sin(), PITCH.cos()) * camera.distance;
    *transform =
        Transform::from_translation(camera.focus + offset).looking_at(camera.focus, Vec3::Y);
}

pub fn track_cursor(
    level: Res<ActiveLevel>,
    scripted: Res<ScriptedPointer>,
    hover_map: Res<HoverMap>,
    interface_nodes: Query<(), With<Node>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    mut cursor: ResMut<Cursor>,
) {
    let (Ok(window), Ok((camera, camera_transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    cursor.screen = scripted.0.or_else(|| window.cursor_position());
    let over_sidebar = cursor
        .screen
        .is_some_and(|screen| screen.x > window.width() - SIDEBAR_WIDTH);
    let over_panel = hover_map
        .get(&PointerId::Mouse)
        .is_some_and(|hits| hits.keys().any(|&entity| interface_nodes.contains(entity)));
    cursor.over_interface = over_sidebar || over_panel;
    cursor.ray = cursor
        .screen
        .and_then(|screen| camera.viewport_to_world(camera_transform, screen).ok());
    cursor.ground = cursor
        .ray
        .map(|ray| ray_hit_on_terrain(ray, level.0.landscape));
}

/// Refines a flat-ground hit by re-intersecting at the terrain height under the previous guess.
pub fn ray_hit_on_terrain(ray: Ray3d, landscape: Landscape) -> Vec3 {
    let refinement_steps = 6;
    let mut point = ray.origin;
    let mut ground_height = 0.0;
    for _ in 0..refinement_steps {
        let Some(distance) =
            ray.intersect_plane(Vec3::Y * ground_height, InfinitePlane3d::new(Vec3::Y))
        else {
            break;
        };
        point = ray.get_point(distance);
        ground_height = landscape.height(point.x, point.z);
    }
    point.with_y(ground_height)
}

/// How the camera's yaw changes while the rotate keys are held. Rotating left swings the battlefield to the
/// left, as in most strategy games (issue #1), so the view itself turns right.
fn yaw_change(left: f32, right: f32, dt: f32) -> f32 {
    (right - left) * ROTATE_SPEED * dt
}

/// The ground the corners of the view look at, flattened to sea level; the camera always looks down
/// steeply enough for every corner to reach the ground.
pub fn ground_in_view(camera: &Camera, transform: &GlobalTransform) -> Option<Rect> {
    let size = camera.logical_viewport_size()?;
    let corners = [
        Vec2::ZERO,
        Vec2::new(size.x, 0.0),
        size,
        Vec2::new(0.0, size.y),
    ];
    let mut seen = Rect::EMPTY;
    for corner in corners {
        let ray = camera.viewport_to_world(transform, corner).ok()?;
        let distance = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
        seen = seen.union_point(ray.get_point(distance).xz());
    }
    Some(seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_left_swings_the_battlefield_left_and_rotate_right_back() {
        let left = yaw_change(1.0, 0.0, 0.1);
        let right = yaw_change(0.0, 1.0, 0.1);
        let looking = |yaw: f32| -(Quat::from_rotation_y(yaw) * Vec3::Z);
        assert!(
            looking(left).x > 0.0,
            "the view turns towards what was on the right"
        );
        assert_eq!(left, -right);
        assert_eq!(yaw_change(1.0, 1.0, 0.1), 0.0);
    }
}
