use std::collections::HashMap;

use bevy::prelude::*;

use crate::sim::catalog::{BuildingKind, Team, UnitKind};

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
const UNCLAIMED_VARIANT: &str = "neutral";

#[derive(Resource)]
pub struct ModelLibrary {
    scenes: HashMap<String, Handle<WorldAsset>>,
}

impl ModelLibrary {
    pub fn unit(&self, kind: UnitKind, team: Team) -> Handle<WorldAsset> {
        self.team_model(kind.spec().model_id, team)
    }

    pub fn building(&self, kind: BuildingKind, team: Team) -> Handle<WorldAsset> {
        self.team_model(kind.spec().model_id, team)
    }

    /// Half a wall run, joining a wall post to its neighbour.
    pub fn wall_link(&self, team: Team) -> Handle<WorldAsset> {
        self.team_model(WALL_LINK_MODEL, team)
    }

    pub fn unclaimed_site(&self, kind: BuildingKind) -> Handle<WorldAsset> {
        self.scenes[&format!("{}_{UNCLAIMED_VARIANT}", kind.spec().model_id)].clone()
    }

    pub fn neutral(&self, model_id: &str) -> Handle<WorldAsset> {
        self.scenes[model_id].clone()
    }

    fn team_model(&self, model_id: &str, team: Team) -> Handle<WorldAsset> {
        self.scenes[&team_model_key(model_id, team)].clone()
    }
}

fn team_model_key(model_id: &str, team: Team) -> String {
    format!("{model_id}_{}", team.model_variant())
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
    let unclaimed_sites = BuildingKind::NEUTRAL_SITES
        .map(|kind| format!("{}_{UNCLAIMED_VARIANT}", kind.spec().model_id));
    team_model_ids
        .flat_map(|model_id| Team::ALL.map(|team| team_model_key(model_id, team)))
        .chain(unclaimed_sites)
        .chain(NEUTRAL_MODELS.iter().map(|id| id.to_string()))
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
