use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::picking::backend::HitData;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;

use crate::camera::{RtsCamera, ray_hit_on_terrain};
use crate::catalog::Team;
use crate::components::{Building, Unit};
use crate::fog::{CellSight, FogOfWar};
use crate::fog_view::Discovered;
use crate::levels::{ActiveLevel, LevelDefinition};
use crate::local_view::LocalView;
use crate::map::{CELL_SIZE, GameMap, footprint_cells, map_cells, map_world_size};
use crate::neutral_sites::UnclaimedSite;
use crate::terrain::GroundLayers;
use crate::ui::SIDEBAR_WIDTH;

pub const PIXELS_PER_CELL: i32 = 3;
fn resolution() -> u32 {
    (map_cells() * PIXELS_PER_CELL) as u32
}
const REFRESH_SECS: f32 = 0.2;
const WATER: [u8; 3] = [38, 78, 104];
const OBSTACLE: [u8; 3] = [34, 62, 30];
const CRYSTAL: [u8; 3] = [90, 230, 210];
const UNCLAIMED: [u8; 3] = [150, 152, 150];
const VIEW_FRAME: [u8; 3] = [240, 240, 240];
const EXPLORED_BRIGHTNESS: f32 = 0.55;

#[derive(Resource)]
pub struct Minimap {
    pub image: Handle<Image>,
    terrain: Vec<u8>,
    refresh_timer: f32,
}

pub fn create_minimap(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    map: Res<GameMap>,
    level: Res<ActiveLevel>,
) {
    let terrain = terrain_pixels(&map, &level.0);
    let image = pixel_image(terrain.clone());
    commands.insert_resource(Minimap {
        image: images.add(image),
        terrain,
        refresh_timer: 0.0,
    });
}

/// The square picture of a map's ground, water, obstacles and crystal, `PIXELS_PER_CELL` per cell.
pub fn pixel_image(pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: resolution(),
            height: resolution(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.sampler = ImageSampler::nearest();
    image
}

pub fn terrain_pixels(map: &GameMap, level: &LevelDefinition) -> Vec<u8> {
    let landscape = level.landscape;
    let ground = GroundLayers::new(landscape);
    let grass = level.season.minimap_grass();
    let mut terrain = Vec::with_capacity((resolution() * resolution() * 4) as usize);
    for py in 0..resolution() {
        for px in 0..resolution() {
            let world =
                (Vec2::new(px as f32, py as f32) + 0.5) * (CELL_SIZE / PIXELS_PER_CELL as f32);
            let cell = (world / CELL_SIZE).floor().as_ivec2();
            let rgb = if landscape.water_depth(world.x / CELL_SIZE, world.y / CELL_SIZE) > 0.25 {
                WATER
            } else if !map.is_passable(cell) {
                OBSTACLE
            } else {
                let crystal = map.crystal_at(cell) > 0;
                let color = ground.minimap_color(world, crystal, grass) * 255.0;
                [color.x as u8, color.y as u8, color.z as u8]
            };
            terrain.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    terrain
}

struct Canvas<'a> {
    pixels: &'a mut [u8],
}

impl Canvas<'_> {
    fn put(&mut self, pixel: IVec2, rgb: [u8; 3]) {
        let inside = pixel.x >= 0
            && pixel.y >= 0
            && pixel.x < resolution() as i32
            && pixel.y < resolution() as i32;
        if inside {
            let at = ((pixel.y as u32 * resolution() + pixel.x as u32) * 4) as usize;
            self.pixels[at..at + 3].copy_from_slice(&rgb);
        }
    }

    fn fill(&mut self, min: IVec2, size: IVec2, rgb: [u8; 3]) {
        for y in 0..size.y {
            for x in 0..size.x {
                self.put(min + IVec2::new(x, y), rgb);
            }
        }
    }

    fn line(&mut self, from: Vec2, to: Vec2, rgb: [u8; 3]) {
        let steps = from.distance(to).ceil().max(1.0) as i32;
        for step in 0..=steps {
            self.put(
                from.lerp(to, step as f32 / steps as f32).round().as_ivec2(),
                rgb,
            );
        }
    }
}

fn to_minimap(world: Vec3) -> Vec2 {
    world.xz() / CELL_SIZE * PIXELS_PER_CELL as f32
}

fn team_rgb(team: Team) -> [u8; 3] {
    let color = team.ui_color().to_srgba();
    [
        (color.red * 255.0) as u8,
        (color.green * 255.0) as u8,
        (color.blue * 255.0) as u8,
    ]
}

/// Black for unexplored ground, dimmed for explored ground the player cannot currently see.
fn darken_by_fog(pixels: &mut [u8], fog: &FogOfWar) {
    for (index, texel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let pixel = IVec2::new(
            index as i32 % resolution() as i32,
            index as i32 / resolution() as i32,
        );
        let factor = match fog.sight_at(pixel / PIXELS_PER_CELL) {
            CellSight::Unexplored => 0.0,
            CellSight::Explored => EXPLORED_BRIGHTNESS,
            CellSight::Visible => continue,
        };
        for channel in &mut texel[..3] {
            *channel = (*channel as f32 * factor) as u8;
        }
    }
}

pub fn redraw_minimap(
    time: Res<Time>,
    map: Res<GameMap>,
    mut minimap: ResMut<Minimap>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<RtsCamera>>,
    view: LocalView,
    level: Res<ActiveLevel>,
    buildings: Query<(&Building, &Team, Has<Discovered>)>,
    unclaimed_sites: Query<&Building, With<UnclaimedSite>>,
    units: Query<(&Transform, &Team), With<Unit>>,
) {
    minimap.refresh_timer -= time.delta_secs();
    if minimap.refresh_timer > 0.0 {
        return;
    }
    minimap.refresh_timer = REFRESH_SECS;
    let Some(mut image) = images.get_mut(&minimap.image) else {
        return;
    };
    let Some(pixels) = image.data.as_mut() else {
        return;
    };
    pixels.copy_from_slice(&minimap.terrain);
    let fog = view.sight();
    darken_by_fog(pixels, fog);
    let mut canvas = Canvas { pixels };

    for cell_y in 0..map_cells() {
        for cell_x in 0..map_cells() {
            let cell = IVec2::new(cell_x, cell_y);
            if map.crystal_at(cell) > 0 && fog.sight_at(cell) != CellSight::Unexplored {
                canvas.fill(cell * PIXELS_PER_CELL + 1, IVec2::ONE, CRYSTAL);
            }
        }
    }
    for site in &unclaimed_sites {
        let footprint = site.kind.spec().footprint;
        if fog.sight_at(site.origin) == CellSight::Unexplored {
            continue;
        }
        for cell in footprint_cells(site.origin, footprint) {
            canvas.fill(
                cell * PIXELS_PER_CELL,
                IVec2::splat(PIXELS_PER_CELL),
                UNCLAIMED,
            );
        }
    }
    for (building, &team, discovered) in &buildings {
        if view.is_hostile(team) && !discovered && fog.enabled() {
            continue;
        }
        for cell in footprint_cells(building.origin, building.kind.spec().footprint) {
            canvas.fill(
                cell * PIXELS_PER_CELL,
                IVec2::splat(PIXELS_PER_CELL),
                team_rgb(team),
            );
        }
    }
    for (transform, &team) in &units {
        if !view.reveals(team, transform.translation) {
            continue;
        }
        let center = to_minimap(transform.translation).floor().as_ivec2();
        canvas.fill(center - IVec2::ONE, IVec2::splat(2), team_rgb(team));
    }

    let (Ok(window), Ok((camera, camera_transform))) = (windows.single(), cameras.single()) else {
        return;
    };
    let playfield = Vec2::new(window.width() - SIDEBAR_WIDTH, window.height());
    let corners = [
        Vec2::ZERO,
        Vec2::new(playfield.x, 0.0),
        playfield,
        Vec2::new(0.0, playfield.y),
    ];
    let view: Option<Vec<Vec2>> = corners
        .iter()
        .map(|&screen| {
            let ray = camera.viewport_to_world(camera_transform, screen).ok()?;
            let hit = ray_hit_on_terrain(ray, level.0.landscape)
                .clamp(Vec3::ZERO, Vec3::splat(map_world_size()).with_y(f32::MAX));
            Some(to_minimap(hit))
        })
        .collect();
    if let Some(view) = view {
        for i in 0..view.len() {
            canvas.line(view[i], view[(i + 1) % view.len()], VIEW_FRAME);
        }
    }
}

/// Where a right click on the minimap sends the selection; `issue_orders` takes it the same frame.
#[derive(Resource, Default)]
pub struct MinimapOrder(pub Option<Vec3>);

pub fn order_on_press(
    press: On<PointerPress>,
    level: Res<ActiveLevel>,
    mut order: ResMut<MinimapOrder>,
) {
    if press.button != PointerButton::Secondary {
        return;
    }
    if let Some(spot) = map_spot(&press.hit) {
        order.0 = Some(level.0.landscape.on_ground(Vec3::new(spot.x, 0.0, spot.y)));
    }
}

pub fn jump_camera_on_press(press: On<PointerPress>, mut cameras: Query<&mut RtsCamera>) {
    if press.button == PointerButton::Primary {
        jump_camera_to(&press.hit, &mut cameras);
    }
}

pub fn jump_camera_on_drag(
    movement: On<PointerMove>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cameras: Query<&mut RtsCamera>,
) {
    if mouse.pressed(MouseButton::Left) {
        jump_camera_to(&movement.hit, &mut cameras);
    }
}

/// The map point under a minimap hit, as world x and z. UI picking reports hits relative to the node, from
/// (-0.5, -0.5) top left to (0.5, 0.5) bottom right.
fn map_spot(hit: &HitData) -> Option<Vec2> {
    let position = hit.position?;
    let normalized = (position.truncate() + Vec2::splat(0.5)).clamp(Vec2::ZERO, Vec2::ONE);
    Some(normalized * map_world_size())
}

fn jump_camera_to(hit: &HitData, cameras: &mut Query<&mut RtsCamera>) {
    let Some(target) = map_spot(hit) else { return };
    for mut camera in cameras.iter_mut() {
        camera.focus.x = target.x;
        camera.focus.z = target.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimap_hit_points_at_the_same_spot_on_the_map() {
        let hit = |x: f32, y: f32| {
            HitData::new(Entity::PLACEHOLDER, 0.0, Some(Vec3::new(x, y, 0.0)), None)
        };
        let size = map_world_size();
        assert_eq!(map_spot(&hit(0.0, 0.0)), Some(Vec2::splat(size * 0.5)));
        assert_eq!(map_spot(&hit(-0.5, 0.5)), Some(Vec2::new(0.0, size)));
        assert_eq!(map_spot(&hit(0.9, -0.9)), Some(Vec2::new(size, 0.0)));
    }
}
