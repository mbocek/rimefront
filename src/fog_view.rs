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
use crate::map::{CrystalVisual, footprint_cells, map_cells};
use crate::neutral_sites::UnclaimedSite;
use crate::scenery::Scenery;

const TEXELS_PER_CELL: i32 = 4;
const BLUR_RADIUS: i32 = 3;
fn resolution() -> u32 {
    (map_cells() * TEXELS_PER_CELL) as u32
}

fn texture_extent() -> Extent3d {
    Extent3d {
        width: resolution(),
        height: resolution(),
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

/// Softens the blocky cell grid with a separable box blur so the fog edge reads as a gradient.
fn blurred_shades(fog: &FogOfWar) -> Vec<u8> {
    let size = resolution() as usize;
    let raw: Vec<u32> = (0..size * size)
        .map(|i| {
            let texel = IVec2::new((i % size) as i32, (i / size) as i32);
            shade(fog.sight_at(texel / TEXELS_PER_CELL)) as u32
        })
        .collect();
    box_blur(&raw, size)
}

/// Each output averages the `2 * BLUR_RADIUS + 1` inputs around it along a row, then along a column, the edge
/// repeating beyond the border. A running sum makes each texel cost the same whatever the radius.
fn box_blur(raw: &[u32], size: usize) -> Vec<u8> {
    let taps = (2 * BLUR_RADIUS + 1) as u32;
    let mut horizontal = vec![0u32; raw.len()];
    let mut line = vec![0u32; size];
    for row in 0..size {
        let start = row * size;
        sliding_sums(&raw[start..start + size], &mut line);
        for (column, sum) in line.iter().enumerate() {
            horizontal[start + column] = sum / taps;
        }
    }
    let mut blurred = vec![0u8; raw.len()];
    let mut column_values = vec![0u32; size];
    for column in 0..size {
        for (row, value) in column_values.iter_mut().enumerate() {
            *value = horizontal[row * size + column];
        }
        sliding_sums(&column_values, &mut line);
        for (row, sum) in line.iter().enumerate() {
            blurred[row * size + column] = (sum / taps) as u8;
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

pub fn paint_fog_texture(
    view: LocalView,
    texture: Res<FogTexture>,
    mut painted_revision: Local<u64>,
    mut images: ResMut<Assets<Image>>,
) {
    let fog = view.sight();
    if *painted_revision == fog.revision {
        return;
    }
    let Some(mut image) = images.get_mut(&texture.0) else {
        return;
    };
    if image.texture_descriptor.size != texture_extent() {
        image.resize(texture_extent());
    }
    let Some(pixels) = image.data.as_mut() else {
        return;
    };
    for (texel, shade) in pixels
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(blurred_shades(fog))
    {
        texel[..3].fill(shade);
    }
    *painted_revision = fog.revision;
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
    mut units: Query<(&Team, &Transform, &mut Visibility, Option<&Camouflage>), With<Unit>>,
    mut buildings: Query<
        (Entity, &Team, &Building, &mut Visibility, Has<Discovered>),
        Without<Unit>,
    >,
) {
    for (&team, transform, mut visibility, camouflage) in &mut units {
        let hidden_by_camouflage = camouflage.is_some_and(|camouflage| camouflage.concealed)
            && view.is_hostile(team)
            && !view.is_spectating();
        let wanted = if view.reveals(team, transform.translation) && !hidden_by_camouflage {
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

    fn box_blur_tap_by_tap(raw: &[u32], size: usize) -> Vec<u8> {
        let size = size as i32;
        let at = |values: &[u32], x: i32, y: i32| {
            values[(y.clamp(0, size - 1) * size + x.clamp(0, size - 1)) as usize]
        };
        let taps = (2 * BLUR_RADIUS + 1) as u32;
        let horizontal: Vec<u32> = (0..size * size)
            .map(|i| {
                (-BLUR_RADIUS..=BLUR_RADIUS)
                    .map(|d| at(raw, i % size + d, i / size))
                    .sum::<u32>()
                    / taps
            })
            .collect();
        (0..size * size)
            .map(|i| {
                ((-BLUR_RADIUS..=BLUR_RADIUS)
                    .map(|d| at(&horizontal, i % size, i / size + d))
                    .sum::<u32>()
                    / taps) as u8
            })
            .collect()
    }

    #[test]
    fn the_running_blur_matches_summing_every_tap() {
        let size = 37;
        let shades = [UNEXPLORED_SHADE, EXPLORED_SHADE, VISIBLE_SHADE];
        let raw: Vec<u32> = (0..size * size)
            .map(|i| shades[(i * 7 + i / 5) % shades.len()] as u32)
            .collect();
        assert_eq!(box_blur(&raw, size), box_blur_tap_by_tap(&raw, size));
    }
}
