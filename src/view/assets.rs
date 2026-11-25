use std::collections::HashMap;

use bevy::prelude::*;

use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::view::side_tint::Painted;

const NEUTRAL_MODELS: [&str; 22] = [
    "crystal_a",
    "crystal_b",
    "crystal_c",
    "tree",
    "tree_pine",
    "tree_pine_young",
    "tree_broadleaf",
    "tree_birch",
    "tree_dead",
    "rock",
    "rock_a",
    "rock_b",
    "boulder_large",
    "bush",
    "bush_round",
    "bush_flowering",
    "grass_tuft",
    "flowers",
    "pebbles",
    "fallen_log",
    "stump",
    "reeds",
];

const WALL_LINK_MODEL: &str = "wall_link";
/// The grey variant of a neutral site that nobody holds yet (tools/blender/build_models.py).

#[derive(Resource)]
pub struct ModelLibrary {
    scenes: HashMap<String, Handle<WorldAsset>>,
}

impl ModelLibrary {
    /// A unit's model, its side-coloured parts in its side's colour.
    pub fn unit(&self, kind: UnitKind, team: Team) -> (WorldAssetRoot, Painted) {
        self.painted(kind.spec().model_id, Painted::Side(team))
    }

    pub fn building(&self, kind: BuildingKind, team: Team) -> (WorldAssetRoot, Painted) {
        self.painted(kind.spec().model_id, Painted::Side(team))
    }

    /// Half a wall run, joining a wall post to its neighbour.
    pub fn wall_link(&self, team: Team) -> (WorldAssetRoot, Painted) {
        self.painted(WALL_LINK_MODEL, Painted::Side(team))
    }

    pub fn unclaimed_site(&self, kind: BuildingKind) -> (WorldAssetRoot, Painted) {
        self.painted(kind.spec().model_id, Painted::Unclaimed)
    }

    pub fn neutral(&self, model_id: &str) -> Handle<WorldAsset> {
        self.scenes[model_id].clone()
    }

    fn painted(&self, model_id: &str, paint: Painted) -> (WorldAssetRoot, Painted) {
        (WorldAssetRoot(self.scenes[model_id].clone()), paint)
    }
}

fn load_scene(asset_server: &AssetServer, file_stem: &str) -> Handle<WorldAsset> {
    asset_server.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{file_stem}.glb")))
}

fn all_model_keys() -> Vec<String> {
    let team_model_ids = UnitKind::INFANTRY
        .iter()
        .chain(UnitKind::VEHICLES.iter())
        .map(|kind| kind.spec().model_id)
        .chain(
            std::iter::once(BuildingKind::CommandCenter)
                .chain(BuildingKind::CONSTRUCTIBLE)
                .chain(BuildingKind::NEUTRAL_SITES)
                .map(|kind| kind.spec().model_id),
        )
        .chain(std::iter::once(WALL_LINK_MODEL));
    team_model_ids
        .chain(NEUTRAL_MODELS.iter().copied())
        .map(str::to_string)
        .collect()
}

impl ModelLibrary {
    /// Empty handles for every model, for running the simulation without loading or rendering assets.
    pub fn placeholder() -> Self {
        Self {
            scenes: all_model_keys()
                .into_iter()
                .map(|key| (key, Handle::default()))
                .collect(),
        }
    }
}

pub fn load_models(mut commands: Commands, asset_server: Res<AssetServer>) {
    let scenes = all_model_keys()
        .into_iter()
        .map(|key| {
            let scene = load_scene(&asset_server, &key);
            (key, scene)
        })
        .collect::<HashMap<_, _>>();
    commands.insert_resource(ModelLibrary { scenes });
}

/// The node of a spawned model's scene with this name, found once the scene has spawned under `root`.
pub fn named_node<'a>(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    nodes: &'a Query<(&Name, &Transform)>,
) -> Option<(Entity, &'a Transform)> {
    children.iter_descendants(root).find_map(|node| {
        let (node_name, transform) = nodes.get(node).ok()?;
        (node_name.as_str() == name).then_some((node, transform))
    })
}
