use crate::abilities::Camouflage;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::catalog::Team;
use crate::combat::Projectile;
use crate::components::{Building, Unit};
use crate::fog::{CellSight, FogOfWar};
use crate::local_view::LocalView;
use crate::map::{CrystalVisual, footprint_cells, map_bounds};
use crate::neutral_sites::UnclaimedSite;
use crate::scenery::Scenery;
use crate::transport::Embarked;

const TEXELS_PER_CELL: i32 = 4;
const BLUR_RADIUS: i32 = 3;
fn resolution() -> UVec2 {
    (map_bounds().size * TEXELS_PER_CELL).as_uvec2()
}

fn texture_extent() -> Extent3d {
    Extent3d {
        width: resolution().x,
        height: resolution().y,
        depth_or_array_layers: 1,
    }
}
const UNEXPLORED_SHADE: u8 = 0;
/// Explored ground stays readable but clearly darker than what units can currently see.
const EXPLORED_SHADE: u8 = 184;
const VISIBLE_SHADE: u8 = 255;

/// The fog of war as a texture the terrain shader darkens the ground with.
#[derive(Resource)]
pub struct FogTexture(pub Handle<Image>);

/// Scenery hidden until the player explores its cell.
#[derive(Component)]
pub struct Shrouded;

/// An enemy structure the player has spotted at least once; it stays on the map afterwards.
#[derive(Component)]
pub struct Discovered;

pub fn create_fog_texture(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        texture_extent(),
        TextureDimension::D2,
        &[VISIBLE_SHADE, VISIBLE_SHADE, VISIBLE_SHADE, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::all(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    commands.insert_resource(FogTexture(images.add(image)));
}

fn shade(sight: CellSight) -> u8 {
    match sight {
        CellSight::Unexplored => UNEXPLORED_SHADE,
        CellSight::Explored => EXPLORED_SHADE,
        CellSight::Visible => VISIBLE_SHADE,
    }
}

/// Repaints the texels from `first` to `last`, softening the blocky cell grid with a separable box blur so the
/// fog edge reads as a gradient. Reading `BLUR_RADIUS` texels around them is enough for them to come out
/// exactly as when the whole texture is painted.
fn paint_texels(fog: &FogOfWar, pixels: &mut [u8], first: IVec2, last: IVec2) {
    let size = resolution().as_ivec2();
    let read_first = (first - IVec2::splat(BLUR_RADIUS)).max(IVec2::ZERO);
    let read_last = (last + IVec2::splat(BLUR_RADIUS)).min(size - IVec2::ONE);
    let read_size = read_last - read_first + IVec2::ONE;
    let first_cell = map_bounds().min;
    let raw: Vec<u32> = (0..read_size.element_product())
        .map(|i| {
            let texel = read_first + IVec2::new(i % read_size.x, i / read_size.x);
            shade(fog.sight_at(first_cell + texel / TEXELS_PER_CELL)) as u32
        })
        .collect();
    let blurred = box_blur(&raw, read_size.as_uvec2());
    for y in first.y..=last.y {
        for x in first.x..=last.x {
            let read = IVec2::new(x, y) - read_first;
            let shade = blurred[(read.y * read_size.x + read.x) as usize];
            let at = ((y * size.x + x) * 4) as usize;
            pixels[at..at + 3].fill(shade);
        }
    }
}

fn paint_everything(fog: &FogOfWar, pixels: &mut [u8]) {
    paint_texels(
        fog,
        pixels,
        IVec2::ZERO,
        resolution().as_ivec2() - IVec2::ONE,
    );
}

/// Repaints the chunks that changed after `revision`, and the blur they spill into their neighbours.
fn paint_changes(fog: &FogOfWar, revision: u64, pixels: &mut [u8]) {
    let bounds = map_bounds();
    let last_texel = resolution().as_ivec2() - IVec2::ONE;
    for chunk in fog.chunks_changed_since(revision) {
        let (first_cell, last_cell) = bounds.cells_of_chunk(chunk);
        let first = (first_cell - bounds.min) * TEXELS_PER_CELL - IVec2::splat(BLUR_RADIUS);
        let last = (last_cell - bounds.min + IVec2::ONE) * TEXELS_PER_CELL - IVec2::ONE
            + IVec2::splat(BLUR_RADIUS);
        paint_texels(fog, pixels, first.max(IVec2::ZERO), last.min(last_texel));
    }
}

/// Each output averages the `2 * BLUR_RADIUS + 1` inputs around it along a row, then along a column, the edge
/// repeating beyond the border. A running sum makes each texel cost the same whatever the radius.
fn box_blur(raw: &[u32], size: UVec2) -> Vec<u8> {
    let (width, height) = (size.x as usize, size.y as usize);
    let taps = (2 * BLUR_RADIUS + 1) as u32;
    let mut horizontal = vec![0u32; raw.len()];
    let mut row_sums = vec![0u32; width];
    for row in 0..height {
        let start = row * width;
        sliding_sums(&raw[start..start + width], &mut row_sums);
        for (column, sum) in row_sums.iter().enumerate() {
            horizontal[start + column] = sum / taps;
        }
    }
    let mut blurred = vec![0u8; raw.len()];
    let mut column_values = vec![0u32; height];
    let mut column_sums = vec![0u32; height];
    for column in 0..width {
        for (row, value) in column_values.iter_mut().enumerate() {
            *value = horizontal[row * width + column];
        }
        sliding_sums(&column_values, &mut column_sums);
        for (row, sum) in column_sums.iter().enumerate() {
            blurred[row * width + column] = (sum / taps) as u8;
        }
    }
    blurred
}

fn sliding_sums(values: &[u32], sums: &mut [u32]) {
    let last = values.len() as i32 - 1;
    let at = |index: i32| values[index.clamp(0, last) as usize];
    let mut sum: u32 = (-BLUR_RADIUS..=BLUR_RADIUS).map(at).sum();
    for (index, slot) in sums.iter_mut().enumerate() {
        *slot = sum;
        let index = index as i32;
        sum = sum + at(index + BLUR_RADIUS + 1) - at(index - BLUR_RADIUS);
    }
}

/// Which fog the texture shows and how far it is painted.
#[derive(Clone, Copy, PartialEq)]
pub struct PaintedFog {
    lineage: u64,
    revision: u64,
}

pub fn paint_fog_texture(
    view: LocalView,
    texture: Res<FogTexture>,
    mut painted: Local<Option<PaintedFog>>,
    mut images: ResMut<Assets<Image>>,
) {
    let fog = view.sight();
    let now = PaintedFog {
        lineage: fog.lineage(),
        revision: fog.revision,
    };
    if *painted == Some(now) {
        return;
    }
    let Some(mut image) = images.get_mut(&texture.0) else {
        return;
    };
    let resized = image.texture_descriptor.size != texture_extent();
    if resized {
        image.resize(texture_extent());
    }
    let Some(pixels) = image.data.as_mut() else {
        return;
    };
    match *painted {
        Some(before) if before.lineage == now.lineage && !resized => {
            paint_changes(fog, before.revision, pixels);
        }
        _ => paint_everything(fog, pixels),
    }
    *painted = Some(now);
}

/// Hides scenery and crystals on ground the player has not explored yet.
pub fn shroud_unexplored_scenery(
    mut commands: Commands,
    view: LocalView,
    scenery: Query<
        (Entity, &Transform),
        Or<(With<Scenery>, With<CrystalVisual>, With<UnclaimedSite>)>,
    >,
) {
    for (entity, transform) in &scenery {
        if !view.sight().is_explored(transform.translation) {
            commands
                .entity(entity)
                .insert((Shrouded, Visibility::Hidden));
        }
    }
}

pub fn reveal_explored_scenery(
    mut commands: Commands,
    view: LocalView,
    mut seen_revision: Local<u64>,
    mut shrouded: Query<(Entity, &Transform, &mut Visibility), With<Shrouded>>,
) {
    let fog = view.sight();
    if *seen_revision == fog.revision {
        return;
    }
    *seen_revision = fog.revision;
    for (entity, transform, mut visibility) in &mut shrouded {
        if fog.is_explored(transform.translation) {
            *visibility = Visibility::Inherited;
            commands.entity(entity).remove::<Shrouded>();
        }
    }
}

/// Shells and bullets show only where the player can see, like the explosions they end in.
pub fn show_seen_projectiles(
    view: LocalView,
    mut projectiles: Query<(&Transform, &mut Visibility), With<Projectile>>,
) {
    for (transform, mut visibility) in &mut projectiles {
        let wanted = if view.sight().is_visible(transform.translation) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(wanted);
    }
}

/// Enemy units show only where the player can see; enemy structures appear once spotted and then stay.
pub fn show_seen_enemies(
    mut commands: Commands,
    view: LocalView,
    mut units: Query<
        (
            &Team,
            &Transform,
            &mut Visibility,
            Option<&Camouflage>,
            Has<Embarked>,
        ),
        With<Unit>,
    >,
    mut buildings: Query<
        (Entity, &Team, &Building, &mut Visibility, Has<Discovered>),
        Without<Unit>,
    >,
) {
    for (&team, transform, mut visibility, camouflage, embarked) in &mut units {
        let hidden_by_camouflage = camouflage.is_some_and(|camouflage| camouflage.concealed)
            && view.is_hostile(team)
            && !view.is_spectating();
        let seen = view.reveals(team, transform.translation) && !hidden_by_camouflage;
        let wanted = if seen && !embarked {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(wanted);
    }
    for (entity, &team, building, mut visibility, discovered) in &mut buildings {
        if !view.is_hostile(team) || discovered {
            continue;
        }
        let spotted = footprint_cells(building.origin, building.kind.spec().footprint)
            .any(|cell| view.sight().sight_at(cell) == CellSight::Visible);
        if spotted {
            commands.entity(entity).insert(Discovered);
            *visibility = Visibility::Inherited;
        } else {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapBounds, cell_center, set_map_bounds};

    fn box_blur_tap_by_tap(raw: &[u32], size: UVec2) -> Vec<u8> {
        let size = size.as_ivec2();
        let at = |values: &[u32], x: i32, y: i32| {
            values[(y.clamp(0, size.y - 1) * size.x + x.clamp(0, size.x - 1)) as usize]
        };
        let taps = (2 * BLUR_RADIUS + 1) as u32;
        let horizontal: Vec<u32> = (0..size.element_product())
            .map(|i| {
                (-BLUR_RADIUS..=BLUR_RADIUS)
                    .map(|d| at(raw, i % size.x + d, i / size.x))
                    .sum::<u32>()
                    / taps
            })
            .collect();
        (0..size.element_product())
            .map(|i| {
                ((-BLUR_RADIUS..=BLUR_RADIUS)
                    .map(|d| at(&horizontal, i % size.x, i / size.x + d))
                    .sum::<u32>()
                    / taps) as u8
            })
            .collect()
    }

    #[test]
    fn repainting_the_changed_chunks_matches_painting_everything() {
        let bounds = MapBounds {
            min: IVec2::new(-20, 7),
            size: IVec2::new(100, 70),
        };
        set_map_bounds(bounds);
        let texel_bytes = resolution().element_product() as usize * 4;
        let viewers = |shift: i32| {
            [IVec2::new(-10, 20), IVec2::new(30, 40), IVec2::new(77, 74)]
                .map(|cell| (cell_center(cell + IVec2::new(shift, 0)), 9.0))
        };
        let mut fog = FogOfWar::new(true);
        fog.reveal_around(viewers(0));
        let mut kept_up = vec![0u8; texel_bytes];
        paint_everything(&fog, &mut kept_up);
        let painted = fog.revision;

        fog.reveal_around(viewers(3));
        paint_changes(&fog, painted, &mut kept_up);

        let mut fresh = vec![0u8; texel_bytes];
        paint_everything(&fog, &mut fresh);
        assert!(kept_up == fresh, "a partly repainted texture differs");
        let changed = fog.chunks_changed_since(painted).count();
        assert!(changed > 0 && changed < bounds.chunk_count());
    }

    #[test]
    fn the_running_blur_matches_summing_every_tap() {
        let size = UVec2::new(37, 23);
        let shades = [UNEXPLORED_SHADE, EXPLORED_SHADE, VISIBLE_SHADE];
        let raw: Vec<u32> = (0..(size.x * size.y) as usize)
            .map(|i| shades[(i * 7 + i / 5) % shades.len()] as u32)
            .collect();
        assert_eq!(box_blur(&raw, size), box_blur_tap_by_tap(&raw, size));
    }
}
