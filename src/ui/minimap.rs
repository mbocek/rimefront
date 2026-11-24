use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::picking::backend::HitData;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::PrimaryWindow;

use crate::missions::levels::{ActiveLevel, LevelDefinition};
use crate::sim::catalog::Team;
use crate::sim::components::{Building, Unit};
use crate::sim::fog::{CellSight, FogOfWar};
use crate::sim::neutral_sites::UnclaimedSite;
use crate::ui::sidebar::SIDEBAR_WIDTH;
use crate::view::camera::{RtsCamera, ray_hit_on_terrain};
use crate::view::fog_view::Discovered;
use crate::view::local_view::LocalView;
use crate::world::map::{CELL_SIZE, GameMap, footprint_cells, map_bounds};
use crate::world::terrain::GroundLayers;
use crate::world::wilds::WorldWidened;

const MOST_PIXELS_PER_CELL: f32 = 3.0;
/// The picture's longest side; a larger map gets less than a pixel a cell, so drawing it every refresh
/// costs no more than a Vast map, the largest drawn at full detail.
const MOST_PIXELS_PER_SIDE: f32 = 768.0;

pub fn pixels_per_cell() -> f32 {
    (MOST_PIXELS_PER_SIDE / map_bounds().longest_side() as f32).min(MOST_PIXELS_PER_CELL)
}

pub fn resolution() -> UVec2 {
    (map_bounds().size.as_vec2() * pixels_per_cell())
        .ceil()
        .as_uvec2()
}

pub fn cell_pixel(cell: IVec2) -> IVec2 {
    ((cell - map_bounds().min).as_vec2() * pixels_per_cell())
        .floor()
        .as_ivec2()
}

/// The pixels a cell covers on its own; at least one, so small things stay visible on a large map.
pub fn cell_pixels() -> IVec2 {
    IVec2::splat((pixels_per_cell().round() as i32).max(1))
}

/// The cell under a pixel's centre.
fn cell_under_pixel(pixel: IVec2) -> IVec2 {
    map_bounds().min
        + ((pixel.as_vec2() + 0.5) / pixels_per_cell())
            .floor()
            .as_ivec2()
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

/// The picture follows a world that widened during the match, in the same image the sidebar shows.
pub fn follow_widened_world(
    mut widened: MessageReader<WorldWidened>,
    map: Res<GameMap>,
    level: Res<ActiveLevel>,
    mut minimap: ResMut<Minimap>,
    mut images: ResMut<Assets<Image>>,
) {
    if widened.read().last().is_none() {
        return;
    }
    minimap.terrain = terrain_pixels(&map, &level.0);
    if let Some(mut image) = images.get_mut(&minimap.image) {
        *image = pixel_image(minimap.terrain.clone());
    }
    minimap.refresh_timer = 0.0;
}

/// The picture of a map's ground, water, obstacles and crystal, `pixels_per_cell` per cell.
pub fn pixel_image(pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: resolution().x,
            height: resolution().y,
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
    let map_corner = map_bounds().world_min();
    let metres_per_pixel = CELL_SIZE / pixels_per_cell();
    let size = resolution();
    let mut terrain = Vec::with_capacity((size.element_product() * 4) as usize);
    for py in 0..size.y {
        for px in 0..size.x {
            let world = map_corner + (Vec2::new(px as f32, py as f32) + 0.5) * metres_per_pixel;
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
        let size = resolution().as_ivec2();
        let inside = pixel.cmpge(IVec2::ZERO).all() && pixel.cmplt(size).all();
        if inside {
            let at = ((pixel.y * size.x + pixel.x) * 4) as usize;
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
    (world.xz() - map_bounds().world_min()) / CELL_SIZE * pixels_per_cell()
}

fn team_rgb(team: Team) -> [u8; 3] {
    let color = team.ui_color().to_srgba();
    [
        (color.red * 255.0) as u8,
        (color.green * 255.0) as u8,
        (color.blue * 255.0) as u8,
    ]
}

/// The cell under each column and each row of the picture, worked out once a refresh.
struct CellsUnderPixels {
    columns: Vec<i32>,
    rows: Vec<i32>,
}

impl CellsUnderPixels {
    fn new() -> Self {
        let size = resolution().as_ivec2();
        Self {
            columns: (0..size.x)
                .map(|x| cell_under_pixel(IVec2::new(x, 0)).x)
                .collect(),
            rows: (0..size.y)
                .map(|y| cell_under_pixel(IVec2::new(0, y)).y)
                .collect(),
        }
    }

    fn at(&self, pixel: IVec2) -> IVec2 {
        IVec2::new(self.columns[pixel.x as usize], self.rows[pixel.y as usize])
    }
}

/// Marks each cell holding crystal: its middle pixel while a cell has pixels to spare, otherwise the pixels
/// whose centre it lies under, so a large map costs no more than its picture.
fn for_each_crystal_mark(under: &CellsUnderPixels, mut mark: impl FnMut(IVec2, IVec2)) {
    if pixels_per_cell() >= 1.0 {
        for cell in map_bounds().cells() {
            mark(cell, cell_pixel(cell) + cell_pixels() / 2);
        }
        return;
    }
    for (y, _) in under.rows.iter().enumerate() {
        for (x, _) in under.columns.iter().enumerate() {
            let pixel = IVec2::new(x as i32, y as i32);
            mark(under.at(pixel), pixel);
        }
    }
}

/// Black for unexplored ground, dimmed for explored ground the player cannot currently see.
fn darken_by_fog(pixels: &mut [u8], fog: &FogOfWar, under: &CellsUnderPixels) {
    let width = under.columns.len() as i32;
    for (index, texel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let pixel = IVec2::new(index as i32 % width, index as i32 / width);
        let factor = match fog.sight_at(under.at(pixel)) {
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
    let under = CellsUnderPixels::new();
    darken_by_fog(pixels, fog, &under);
    let mut canvas = Canvas { pixels };

    for_each_crystal_mark(&under, |cell, pixel| {
        if map.crystal_at(cell) > 0 && fog.sight_at(cell) != CellSight::Unexplored {
            canvas.put(pixel, CRYSTAL);
        }
    });
    for site in &unclaimed_sites {
        let footprint = site.kind.spec().footprint;
        if fog.sight_at(site.origin) == CellSight::Unexplored {
            continue;
        }
        for cell in footprint_cells(site.origin, footprint) {
            canvas.fill(cell_pixel(cell), cell_pixels(), UNCLAIMED);
        }
    }
    for (building, &team, discovered) in &buildings {
        if view.is_hostile(team) && !discovered && fog.enabled() {
            continue;
        }
        for cell in footprint_cells(building.origin, building.kind.spec().footprint) {
            canvas.fill(cell_pixel(cell), cell_pixels(), team_rgb(team));
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
            let bounds = map_bounds();
            let hit = ray_hit_on_terrain(ray, level.0.landscape).clamp(
                bounds.world_min().extend(f32::MIN).xzy(),
                bounds.world_max().extend(f32::MAX).xzy(),
            );
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
    let bounds = map_bounds();
    Some(bounds.world_min() + normalized * bounds.world_size())
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
    fn maps_up_to_vast_keep_three_pixels_a_cell_and_larger_ones_a_bounded_picture() {
        use crate::world::map::{MapBounds, MapSize, set_map_bounds};
        for size in MapSize::ALL {
            set_map_bounds(MapBounds::square(size.cells()));
            assert_eq!(resolution(), UVec2::splat(size.cells() as u32 * 3));
            assert_eq!(cell_pixels(), IVec2::splat(3));
        }
        set_map_bounds(MapBounds {
            min: IVec2::new(-30, 10),
            size: IVec2::new(1024, 600),
        });
        assert_eq!(resolution(), UVec2::new(768, 450));
        let last_pixel = resolution().as_ivec2() - IVec2::ONE;
        assert_eq!(cell_under_pixel(IVec2::ZERO), map_bounds().min);
        assert_eq!(cell_under_pixel(last_pixel), map_bounds().max());
    }

    #[test]
    fn a_minimap_hit_points_at_the_same_spot_on_the_map() {
        let hit = |x: f32, y: f32| {
            HitData::new(Entity::PLACEHOLDER, 0.0, Some(Vec3::new(x, y, 0.0)), None)
        };
        let bounds = map_bounds();
        let (low, high) = (bounds.world_min(), bounds.world_max());
        assert_eq!(map_spot(&hit(0.0, 0.0)), Some(bounds.world_center()));
        assert_eq!(map_spot(&hit(-0.5, 0.5)), Some(Vec2::new(low.x, high.y)));
        assert_eq!(map_spot(&hit(0.9, -0.9)), Some(Vec2::new(high.x, low.y)));
    }
}
