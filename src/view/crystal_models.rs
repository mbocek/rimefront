use std::collections::HashMap;

use bevy::asset::{AssetId, RenderAssetUsages};
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::view::motion_material::{Motion, MovingMaterial};
use crate::world::map::{CRYSTAL_MODELS, CrystalVisual};

const FLOATING_SHARD: &str = "FloatingShard";
const GLOWING_CORE: &str = "CrystalCore";

/// Each crystal model as one mesh per material, with floating shards apart so they hover on the GPU.
/// As a glTF scene a crystal was some fifteen entities with animated transforms, and a map has hundreds.
#[derive(Resource, Default)]
pub struct CrystalModelLibrary {
    sources: HashMap<&'static str, Handle<Gltf>>,
    merged: HashMap<&'static str, Vec<CrystalPart>>,
}

#[derive(Clone)]
struct CrystalPart {
    mesh: Handle<Mesh>,
    material: Handle<MovingMaterial>,
    /// Floating shards and glowing cores add little to a shadow and would make every crystal a dozen casters.
    casts_shadow: bool,
}

/// Where a vertex's shard hovers: its pivot on the ground plane and (phase, spin direction); zero spin stays put.
#[derive(Clone, Copy, Default)]
struct Hover {
    pivot: [f32; 2],
    shard: [f32; 2],
}

#[derive(Default)]
struct PartBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    pivots: Vec<[f32; 2]>,
    shards: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

pub fn load_crystal_models(
    mut library: ResMut<CrystalModelLibrary>,
    asset_server: Res<AssetServer>,
) {
    for id in CRYSTAL_MODELS {
        library
            .sources
            .insert(id, asset_server.load(format!("models/{id}.glb")));
    }
}

pub fn merge_crystal_models(
    mut library: ResMut<CrystalModelLibrary>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    gltf_materials: Res<Assets<GltfMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MovingMaterial>>,
) {
    if library.merged.len() == library.sources.len() {
        return;
    }
    for id in CRYSTAL_MODELS {
        if library.merged.contains_key(id) {
            continue;
        }
        let Some(gltf) = library.sources.get(id).and_then(|handle| gltfs.get(handle)) else {
            continue;
        };
        let Some(groups) = gather_parts(gltf, &gltf_nodes, &gltf_meshes, &meshes) else {
            continue;
        };
        let core = gltf.named_materials.get(GLOWING_CORE).map(Handle::id);
        let mut parts = Vec::new();
        for ((material_id, floating), builder) in groups {
            let Some(source) = gltf_materials.get(material_id) else {
                continue;
            };
            let glows = Some(material_id) == core;
            let material = materials.add(MovingMaterial {
                base: standard_material(source),
                extension: Motion::hovering(glows),
            });
            parts.push(CrystalPart {
                mesh: meshes.add(builder.into_mesh()),
                material,
                casts_shadow: !floating && !glows,
            });
        }
        library.merged.insert(id, parts);
    }
}

/// The model's primitives grouped by material and by whether they float, in model space.
fn gather_parts(
    gltf: &Gltf,
    gltf_nodes: &Assets<GltfNode>,
    gltf_meshes: &Assets<GltfMesh>,
    meshes: &Assets<Mesh>,
) -> Option<HashMap<(AssetId<GltfMaterial>, bool), PartBuilder>> {
    let nodes: Vec<&GltfNode> = gltf
        .nodes
        .iter()
        .map(|handle| gltf_nodes.get(handle))
        .collect::<Option<_>>()?;
    let shard_count = nodes
        .iter()
        .filter(|node| node.name.starts_with(FLOATING_SHARD))
        .count()
        .max(1);
    let mut groups: HashMap<(AssetId<GltfMaterial>, bool), PartBuilder> = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(mesh) = node.mesh.as_ref() else {
            continue;
        };
        let placement = model_space_placement(gltf, &nodes, index);
        let hover = node
            .name
            .strip_prefix(FLOATING_SHARD)
            .and_then(|number| number.parse::<usize>().ok())
            .map(|number| Hover {
                pivot: [placement.translation().x, placement.translation().z],
                shard: [
                    number as f32 / shard_count as f32,
                    if number % 2 == 0 { -1.0 } else { 1.0 },
                ],
            });
        for primitive in &gltf_meshes.get(mesh)?.primitives {
            let material = primitive.material.as_ref()?.id();
            let source = meshes.get(&primitive.mesh)?;
            groups
                .entry((material, hover.is_some()))
                .or_default()
                .append(source, &placement, hover.unwrap_or_default())?;
        }
    }
    (!groups.is_empty()).then_some(groups)
}

fn model_space_placement(gltf: &Gltf, nodes: &[&GltfNode], index: usize) -> GlobalTransform {
    let own_id = gltf.nodes[index].id();
    let parent = nodes
        .iter()
        .position(|candidate| candidate.children.iter().any(|child| child.id() == own_id));
    let local = GlobalTransform::from(nodes[index].transform);
    match parent {
        Some(parent) => model_space_placement(gltf, nodes, parent) * local,
        None => local,
    }
}

impl PartBuilder {
    fn append(&mut self, mesh: &Mesh, placement: &GlobalTransform, hover: Hover) -> Option<()> {
        if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
            return None;
        }
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return None;
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            return None;
        };
        let base = self.positions.len() as u32;
        let rotation = placement.rotation();
        self.positions.extend(
            positions
                .iter()
                .map(|&p| placement.transform_point(Vec3::from(p)).to_array()),
        );
        self.normals.extend(
            normals
                .iter()
                .map(|&n| (rotation * Vec3::from(n)).to_array()),
        );
        self.pivots
            .extend(std::iter::repeat_n(hover.pivot, positions.len()));
        self.shards
            .extend(std::iter::repeat_n(hover.shard, positions.len()));
        match mesh.indices() {
            Some(indices) => self.indices.extend(indices.iter().map(|i| base + i as u32)),
            None => self.indices.extend(base..base + positions.len() as u32),
        }
        Some(())
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.pivots)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.shards)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

fn standard_material(source: &GltfMaterial) -> StandardMaterial {
    StandardMaterial {
        base_color: source.base_color,
        emissive: source.emissive,
        perceptual_roughness: source.perceptual_roughness,
        metallic: source.metallic,
        reflectance: source.reflectance,
        double_sided: source.double_sided,
        cull_mode: if source.double_sided {
            None
        } else {
            Some(bevy::render::render_resource::Face::Back)
        },
        ..default()
    }
}

/// Gives each new crystal deposit its merged parts once they are ready.
pub fn dress_crystals(
    mut commands: Commands,
    library: Res<CrystalModelLibrary>,
    undressed: Query<(Entity, &CrystalVisual), Without<Children>>,
) {
    for (crystal, visual) in &undressed {
        let Some(parts) = library.merged.get(visual.model) else {
            continue;
        };
        commands.entity(crystal).with_children(|deposit| {
            for part in parts {
                let mut child = deposit.spawn((
                    Mesh3d(part.mesh.clone()),
                    MeshMaterial3d(part.material.clone()),
                ));
                if !part.casts_shadow {
                    child.insert(NotShadowCaster);
                }
            }
        });
    }
}
