use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::app::settings::GameSettings;
use crate::missions::levels::ActiveLevel;
use crate::view::assets::ModelLibrary;
use crate::view::motion_material::{Motion, MovingMaterial};
use crate::world::scenery::{FlatScenery, sway_amplitude};
use crate::world::season::{Foliage, Season};

/// Trees, rocks and decoration as one merged mesh each. A glTF scene costs a handful of entities per
/// prop (root, nodes, one per material); thousands of grass tufts made that most of the world.
#[derive(Resource, Default)]
pub struct FlatSceneryLibrary {
    sources: HashMap<String, Handle<Gltf>>,
    merged: HashMap<String, Option<MergedModel>>,
}

#[derive(Clone)]
struct MergedModel {
    /// Indexed by `Season::index`: the same mesh with its colours turned for each season.
    seasonal_meshes: Vec<Handle<Mesh>>,
    material: Handle<MovingMaterial>,
    /// How far it leans in the wind when the wind is on.
    lean: f32,
}

/// Deciduous trees and the leafless model each stands as in winter.
const BARE_IN_WINTER: [(&str, &str); 3] = [
    ("tree", "tree_bare"),
    ("tree_broadleaf", "tree_broadleaf_bare"),
    ("tree_birch", "tree_birch_bare"),
];

fn shown_model(model_id: &str, season: Season) -> &str {
    if season != Season::Winter {
        return model_id;
    }
    BARE_IN_WINTER
        .iter()
        .find(|(leafy, _)| *leafy == model_id)
        .map_or(model_id, |(_, bare)| bare)
}

fn leafy_model(model_id: &str) -> &str {
    BARE_IN_WINTER
        .iter()
        .find(|(_, bare)| *bare == model_id)
        .map_or(model_id, |(leafy, _)| leafy)
}

pub fn load_flat_scenery_sources(mut commands: Commands, asset_server: Res<AssetServer>) {
    let bare_models = BARE_IN_WINTER.iter().map(|(_, bare)| bare);
    let sources = crate::world::scenery::FLAT_SCENERY_MODELS
        .iter()
        .chain(bare_models)
        .map(|&id| {
            (
                id.to_string(),
                asset_server.load(format!("models/{id}.glb")),
            )
        })
        .collect();
    commands.insert_resource(FlatSceneryLibrary {
        sources,
        merged: HashMap::new(),
    });
}

pub fn merge_scenery_models(
    mut library: ResMut<FlatSceneryLibrary>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MovingMaterial>>,
) {
    if library.merged.len() == library.sources.len() {
        return;
    }
    let pending: Vec<(String, Handle<Gltf>)> = library
        .sources
        .iter()
        .filter(|(id, _)| !library.merged.contains_key(*id))
        .map(|(id, handle)| (id.clone(), handle.clone()))
        .collect();
    for (id, handle) in pending {
        let Some(gltf) = gltfs.get(&handle) else {
            continue;
        };
        let parts = gltf
            .meshes
            .iter()
            .filter_map(|mesh| gltf_meshes.get(mesh))
            .flat_map(|mesh| mesh.primitives.iter())
            .map(|primitive| {
                let mesh = meshes.get(&primitive.mesh)?;
                let material = primitive
                    .material
                    .as_ref()
                    .and_then(|material| gltf_materials.get(material))?;
                Some((mesh.clone(), material.clone()))
            })
            .collect::<Option<Vec<_>>>();
        let Some(parts) = parts.filter(|parts| !parts.is_empty()) else {
            continue;
        };
        let plant = leafy_model(&id);
        let lean = sway_amplitude(plant).unwrap_or(0.0);
        let merged = merge_parts(&parts).map(|(mesh, material)| MergedModel {
            seasonal_meshes: Season::ALL
                .iter()
                .map(|&season| meshes.add(in_season(&mesh, season, Foliage::of_model(plant))))
                .collect(),
            material: materials.add(MovingMaterial {
                base: material,
                extension: Motion::scenery(lean),
            }),
            lean,
        });
        if merged.is_none() {
            warn!("scenery model {id} cannot be merged; it keeps its full scene");
        }
        library.merged.insert(id, merged);
    }
}

/// One mesh for all parts, each part's material colour baked into its vertices, and one shared material.
fn merge_parts(parts: &[(Mesh, GltfMaterial)]) -> Option<(Mesh, StandardMaterial)> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for (mesh, material) in parts {
        let textured = material.base_color_texture.is_some();
        let opaque = material.alpha_mode == AlphaMode::Opaque;
        if textured || !opaque || mesh.primitive_topology() != PrimitiveTopology::TriangleList {
            return None;
        }
        let Some(VertexAttributeValues::Float32x3(part_positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return None;
        };
        let Some(VertexAttributeValues::Float32x3(part_normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            return None;
        };
        let base = positions.len() as u32;
        let tint = material.base_color.to_linear().to_f32_array();
        let vertex_tints: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
            Some(VertexAttributeValues::Float32x4(own)) => own
                .iter()
                .map(|c| {
                    [
                        c[0] * tint[0],
                        c[1] * tint[1],
                        c[2] * tint[2],
                        c[3] * tint[3],
                    ]
                })
                .collect(),
            _ => vec![tint; part_positions.len()],
        };
        positions.extend_from_slice(part_positions);
        normals.extend_from_slice(part_normals);
        colors.extend(vertex_tints);
        match mesh.indices() {
            Some(part_indices) => indices.extend(part_indices.iter().map(|i| base + i as u32)),
            None => indices.extend(base..base + part_positions.len() as u32),
        }
    }

    let mut merged = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    merged.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    merged.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    merged.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    merged.insert_indices(Indices::U32(indices));

    let roughness = parts
        .iter()
        .map(|(_, m)| m.perceptual_roughness)
        .sum::<f32>()
        / parts.len() as f32;
    let double_sided = parts.iter().any(|(_, m)| m.double_sided);
    let material = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: roughness,
        metallic: 0.0,
        reflectance: parts[0].1.reflectance,
        double_sided,
        cull_mode: if double_sided {
            None
        } else {
            Some(bevy::render::render_resource::Face::Back)
        },
        ..default()
    };
    Some((merged, material))
}

fn in_season(mesh: &Mesh, season: Season, foliage: Foliage) -> Mesh {
    let mut seasonal = mesh.clone();
    let (
        Some(VertexAttributeValues::Float32x4(colors)),
        Some(VertexAttributeValues::Float32x3(normals)),
    ) = (
        mesh.attribute(Mesh::ATTRIBUTE_COLOR),
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
    )
    else {
        return seasonal;
    };
    let turned: Vec<[f32; 4]> = colors
        .iter()
        .zip(normals)
        .map(|(&color, &normal)| season.recolor(color, normal, foliage))
        .collect();
    seasonal.insert_attribute(Mesh::ATTRIBUTE_COLOR, turned);
    seasonal
}

/// Gives flat scenery its merged mesh once it is ready, or the full scene if the model could not be merged.
pub fn dress_flat_scenery(
    mut commands: Commands,
    library: Res<FlatSceneryLibrary>,
    models: Res<ModelLibrary>,
    level: Option<Res<ActiveLevel>>,
    undressed: Query<(Entity, &FlatScenery), (Without<Mesh3d>, Without<WorldAssetRoot>)>,
) {
    let season = level.map_or(Season::Summer, |level| level.0.season);
    for (entity, scenery) in &undressed {
        match library.merged.get(shown_model(scenery.0, season)) {
            Some(Some(model)) => {
                commands.entity(entity).insert((
                    Mesh3d(model.seasonal_meshes[season.index()].clone()),
                    MeshMaterial3d(model.material.clone()),
                ));
            }
            Some(None) => {
                commands
                    .entity(entity)
                    .insert(WorldAssetRoot(models.neutral(scenery.0)));
            }
            None => {}
        }
    }
}

/// Turns the wind in every plant's material on or off with the graphics setting.
pub fn follow_wind_setting(
    settings: Res<GameSettings>,
    library: Res<FlatSceneryLibrary>,
    mut materials: ResMut<Assets<MovingMaterial>>,
) {
    if !settings.is_changed() && !library.is_changed() {
        return;
    }
    for model in library.merged.values().flatten() {
        let lean = if settings.wind { model.lean } else { 0.0 };
        if let Some(mut material) = materials.get_mut(&model.material)
            && material.extension.settings.sway_lean != lean
        {
            material.extension.settings.sway_lean = lean;
        }
    }
}
