use bevy::asset::RenderAssetUsages;
use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor,
    TextureViewDimension,
};
use bevy::shader::ShaderRef;

use crate::missions::levels::ActiveLevel;
use crate::view::fog_view::FogTexture;
use crate::world::map::map_bounds;
use crate::world::season::Season;

/// Order must match the layer indices in `assets/shaders/terrain.wesl` and the weights in `terrain::layer_weights`.
const TERRAIN_LAYERS: [&str; 5] = [
    "leafy_grass",
    "forrest_ground_01",
    "rocks_ground_05",
    "river_small_rocks",
    "gravel_concrete_02",
];
const LAYER_TILE_SIZES: Vec4 = Vec4::new(4.5, 5.0, 7.0, 3.5);
const BASE_PAD_TILE_SIZE: f32 = 5.0;
/// The sources are 2K; the camera never gets close enough to need more than 1K, so the top mip is dropped.
const SKIPPED_SOURCE_MIPS: usize = 1;
const WATER_NORMAL_SIZE: u32 = 256;
const WATER_TILE_METERS: f32 = 9.0;
const WATER_FLOW: Vec2 = Vec2::new(0.012, 0.007);

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainLayers>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TerrainLayers {
    #[uniform(100)]
    pub settings: TerrainSettings,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub albedo_roughness: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    pub normals: Handle<Image>,
    /// Fog of war over the playable map: red channel 0 unexplored, ~0.7 explored, 1 visible.
    #[texture(105)]
    #[sampler(106)]
    pub fog: Handle<Image>,
}

/// The assembled ground material, kept so every later match's terrain can reuse it.
#[derive(Resource)]
pub struct TerrainMaterialHandle(pub Handle<TerrainMaterial>);

pub use uniforms::TerrainSettings;

mod uniforms {
    // The ShaderType derive emits per-field `check` helpers that rustc reports as unused.
    #![allow(dead_code)]
    use bevy::prelude::*;
    use bevy::render::render_resource::ShaderType;

    #[derive(Clone, Copy, Debug, Reflect, ShaderType)]
    pub struct TerrainSettings {
        pub grass_tint: Vec4,
        pub layer_tile_sizes: Vec4,
        /// x, y the map's corner and z, w its size, in metres: the fog texture spans the playable map.
        pub map_area: Vec4,
        pub pad_tile_size: f32,
        pub normal_strength: f32,
        pub macro_variation: f32,
        pub water_level: f32,
        pub snow_cover: f32,
        /// 0 dry, 1 soaked by rain: darker, glossier ground.
        pub wet_ground: f32,
        /// 0 broken clouds casting moving shadows, 1 an even overcast with none.
        pub cloud_cover: f32,
    }
}

impl MaterialExtension for TerrainLayers {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain.wesl".into()
    }
}

#[derive(Component)]
pub struct TerrainSurface;

#[derive(Component)]
pub struct WaterSurface;

#[derive(Resource)]
pub struct TerrainTextureSources {
    albedo: Vec<Handle<Image>>,
    roughness: Vec<Handle<Image>>,
    normal: Vec<Handle<Image>>,
}

pub fn load_terrain_textures(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |layer: &str, map: &str| {
        asset_server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
            })
            .load(format!("textures/terrain/{layer}_{map}.jpg"))
    };
    commands.insert_resource(TerrainTextureSources {
        albedo: TERRAIN_LAYERS
            .iter()
            .map(|layer| load(layer, "diff"))
            .collect(),
        roughness: TERRAIN_LAYERS
            .iter()
            .map(|layer| load(layer, "rough"))
            .collect(),
        normal: TERRAIN_LAYERS
            .iter()
            .map(|layer| load(layer, "nor"))
            .collect(),
    });
}

/// Swaps the placeholder ground material for the textured one once every source image is decoded.
pub fn assemble_terrain_material(
    mut commands: Commands,
    sources: Option<Res<TerrainTextureSources>>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    fog: Res<FogTexture>,
) {
    let Some(sources) = sources else { return };
    let all_handles = sources
        .albedo
        .iter()
        .chain(&sources.roughness)
        .chain(&sources.normal);
    if !all_handles
        .clone()
        .all(|handle| asset_server.is_loaded(handle))
    {
        return;
    }
    let pixels = |handles: &[Handle<Image>]| -> Vec<RgbaLayer> {
        handles
            .iter()
            .map(|handle| RgbaLayer::from_image(images.get(handle).unwrap()))
            .collect()
    };
    let albedo_layers: Vec<RgbaLayer> = pixels(&sources.albedo)
        .into_iter()
        .zip(pixels(&sources.roughness))
        .map(|(mut albedo, roughness)| {
            albedo.replace_alpha_with_red_of(&roughness);
            albedo
        })
        .collect();
    let normal_layers = pixels(&sources.normal);

    let material = TerrainMaterial {
        base: StandardMaterial {
            perceptual_roughness: 0.9,
            ..default()
        },
        extension: TerrainLayers {
            settings: TerrainSettings {
                grass_tint: Season::Summer.grass_tint(),
                layer_tile_sizes: LAYER_TILE_SIZES,
                map_area: map_area(),
                pad_tile_size: BASE_PAD_TILE_SIZE,
                normal_strength: 1.3,
                macro_variation: 0.26,
                water_level: crate::world::terrain::WATER_LEVEL,
                snow_cover: Season::Summer.snow_cover(),
                wet_ground: 0.0,
                cloud_cover: 0.0,
            },
            albedo_roughness: images.add(mipmapped_array(
                &albedo_layers,
                TextureFormat::Rgba8UnormSrgb,
            )),
            normals: images.add(mipmapped_array(&normal_layers, TextureFormat::Rgba8Unorm)),
            fog: fog.0.clone(),
        },
    };
    commands.insert_resource(TerrainMaterialHandle(terrain_materials.add(material)));
    commands.remove_resource::<TerrainTextureSources>();
}

fn map_area() -> Vec4 {
    let bounds = map_bounds();
    let (corner, size) = (bounds.world_min(), bounds.world_size());
    Vec4::new(corner.x, corner.y, size.x, size.y)
}

/// The ground material is shared by every match, so it takes on the season and size of the level played.
pub fn fit_terrain_to_level(
    level: Option<Res<ActiveLevel>>,
    handle: Option<Res<TerrainMaterialHandle>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let (Some(level), Some(handle)) = (level, handle) else {
        return;
    };
    let season = level.0.season;
    let Some(material) = materials.get(&handle.0) else {
        return;
    };
    let settings = material.extension.settings;
    let fitted = settings.grass_tint == season.grass_tint()
        && settings.snow_cover == season.snow_cover()
        && settings.map_area == map_area();
    if fitted {
        return;
    }
    if let Some(mut material) = materials.get_mut(&handle.0) {
        let settings = &mut material.extension.settings;
        settings.grass_tint = season.grass_tint();
        settings.snow_cover = season.snow_cover();
        settings.map_area = map_area();
    }
}

/// Gives each newly spawned terrain the textured material once it exists.
pub fn dress_terrain_surfaces(
    mut commands: Commands,
    material: Option<Res<TerrainMaterialHandle>>,
    bare: Query<
        Entity,
        (
            With<TerrainSurface>,
            Without<MeshMaterial3d<TerrainMaterial>>,
        ),
    >,
) {
    let Some(material) = material else { return };
    for surface in &bare {
        commands
            .entity(surface)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(material.0.clone()));
    }
}

struct RgbaLayer {
    size: u32,
    pixels: Vec<u8>,
}

impl RgbaLayer {
    /// Bevy decodes every 8-bit JPEG, grayscale included, into RGBA8.
    fn from_image(image: &Image) -> Self {
        let size = image.texture_descriptor.size.width;
        Self {
            size,
            pixels: image.data.clone().unwrap_or_default(),
        }
    }

    fn replace_alpha_with_red_of(&mut self, other: &RgbaLayer) {
        for (target, source) in self
            .pixels
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(other.pixels.as_chunks::<4>().0)
        {
            target[3] = source[0];
        }
    }

    fn halved(&self) -> Self {
        let half = (self.size / 2).max(1);
        let mut pixels = vec![0u8; (half * half * 4) as usize];
        let source_row = (self.size * 4) as usize;
        for y in 0..half as usize {
            for x in 0..half as usize {
                for channel in 0..4 {
                    let at = |sx: usize, sy: usize| {
                        self.pixels[sy * source_row + sx * 4 + channel] as u32
                    };
                    let sum = at(2 * x, 2 * y)
                        + at(2 * x + 1, 2 * y)
                        + at(2 * x, 2 * y + 1)
                        + at(2 * x + 1, 2 * y + 1);
                    pixels[(y * half as usize + x) * 4 + channel] = (sum / 4) as u8;
                }
            }
        }
        Self { size: half, pixels }
    }

    fn mip_chain(self) -> Vec<RgbaLayer> {
        let mut chain = vec![self];
        while chain.last().unwrap().size > 1 {
            let next = chain.last().unwrap().halved();
            chain.push(next);
        }
        chain
    }
}

fn mipmapped_array(layers: &[RgbaLayer], format: TextureFormat) -> Image {
    let chains: Vec<Vec<RgbaLayer>> = layers
        .iter()
        .map(|layer| {
            RgbaLayer {
                size: layer.size,
                pixels: layer.pixels.clone(),
            }
            .mip_chain()
            .into_iter()
            .skip(SKIPPED_SOURCE_MIPS)
            .collect()
        })
        .collect();
    let size = chains[0][0].size;
    let mip_count = chains[0].len() as u32;
    let data: Vec<u8> = chains
        .iter()
        .flat_map(|chain| chain.iter().flat_map(|level| level.pixels.iter().copied()))
        .collect();
    let mut image = Image::new_fill(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers.len() as u32,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = mip_count;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    image.sampler = repeating_sampler();
    image
}

fn repeating_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..default()
    })
}

/// Tileable ripple normal map built from waves whose frequencies are whole numbers per tile.
pub fn water_normal_map() -> Image {
    let waves: [(Vec2, f32); 5] = [
        (Vec2::new(3.0, 1.0), 0.35),
        (Vec2::new(-2.0, 4.0), 0.25),
        (Vec2::new(7.0, -5.0), 0.12),
        (Vec2::new(-9.0, -11.0), 0.07),
        (Vec2::new(13.0, 6.0), 0.05),
    ];
    let size = WATER_NORMAL_SIZE;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let uv = Vec2::new(x as f32, y as f32) / size as f32 * std::f32::consts::TAU;
            let slope = waves
                .iter()
                .fold(Vec2::ZERO, |slope, &(frequency, amplitude)| {
                    slope + frequency * amplitude * frequency.dot(uv).cos() * 0.15
                });
            let normal = Vec3::new(-slope.x, -slope.y, 1.0).normalize();
            let encode = |v: f32| ((v * 0.5 + 0.5) * 255.0) as u8;
            pixels.extend_from_slice(&[encode(normal.x), encode(normal.y), encode(normal.z), 255]);
        }
    }
    let base = RgbaLayer { size, pixels };
    let chain = base.mip_chain();
    let mut image = Image::new_fill(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = chain.len() as u32;
    image.data = Some(chain.into_iter().flat_map(|level| level.pixels).collect());
    image.sampler = repeating_sampler();
    image
}

pub type WaterMaterial = ExtendedMaterial<StandardMaterial, FoggedWater>;

/// Darkens the river under the fog of war exactly like the terrain shader does.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FoggedWater {
    #[texture(100)]
    #[sampler(101)]
    pub fog: Handle<Image>,
    /// The fog texture spans the playable map, whose size depends on the level.
    #[uniform(102)]
    pub map_area: Vec4,
}

impl MaterialExtension for FoggedWater {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wesl".into()
    }
}

/// Winter's water: frozen over, which the simulation lets units cross.
#[derive(Component)]
pub struct Ice;

pub fn water_material(
    images: &mut Assets<Image>,
    fog: Handle<Image>,
    frozen: bool,
) -> WaterMaterial {
    let (base_color, perceptual_roughness, reflectance) = if frozen {
        (Color::srgba(0.66, 0.76, 0.82, 0.94), 0.35, 0.45)
    } else {
        (Color::srgba(0.10, 0.27, 0.33, 0.78), 0.04, 0.6)
    };
    WaterMaterial {
        base: StandardMaterial {
            base_color,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness,
            reflectance,
            normal_map_texture: Some(images.add(water_normal_map())),
            ..default()
        },
        extension: FoggedWater {
            fog,
            map_area: map_area(),
        },
    }
}

pub fn animate_water(
    time: Res<Time>,
    water: Query<&MeshMaterial3d<WaterMaterial>, (With<WaterSurface>, Without<Ice>)>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let Ok(handle) = water.single() else { return };
    let Some(mut material) = materials.get_mut(handle) else {
        return;
    };
    let repeats = (map_bounds().world_size() + 2.0 * crate::world::terrain::OUTLANDS_WIDTH)
        / WATER_TILE_METERS;
    let offset = WATER_FLOW * time.elapsed_secs();
    material.base.uv_transform =
        bevy::math::Affine2::from_scale_angle_translation(repeats, 0.0, offset);
}
