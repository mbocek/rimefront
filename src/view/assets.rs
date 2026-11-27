use std::collections::HashMap;

use bevy::prelude::*;

use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::eras::Era;
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
const PALISADE_LINK_MODEL: &str = "palisade_link";

/// Structures that every era builds wear the look of the age; the modern one is their own model.
const ERA_LOOKS: [(BuildingKind, Era, &str); 28] = [
    (
        BuildingKind::CommandCenter,
        Era::Antiquity,
        "command_center_antiquity",
    ),
    (
        BuildingKind::PowerPlant,
        Era::Antiquity,
        "power_plant_antiquity",
    ),
    (BuildingKind::Refinery, Era::Antiquity, "refinery_antiquity"),
    (BuildingKind::Barracks, Era::Antiquity, "barracks_antiquity"),
    (BuildingKind::Factory, Era::Antiquity, "factory_antiquity"),
    (
        BuildingKind::TechCenter,
        Era::Antiquity,
        "tech_center_antiquity",
    ),
    (
        BuildingKind::RepairBay,
        Era::Antiquity,
        "repair_bay_antiquity",
    ),
    (
        BuildingKind::CommandCenter,
        Era::Gunpowder,
        "command_center_gunpowder",
    ),
    (
        BuildingKind::PowerPlant,
        Era::Gunpowder,
        "power_plant_gunpowder",
    ),
    (BuildingKind::Refinery, Era::Gunpowder, "refinery_gunpowder"),
    (BuildingKind::Barracks, Era::Gunpowder, "barracks_gunpowder"),
    (BuildingKind::Factory, Era::Gunpowder, "factory_gunpowder"),
    (
        BuildingKind::TechCenter,
        Era::Gunpowder,
        "tech_center_gunpowder",
    ),
    (
        BuildingKind::RepairBay,
        Era::Gunpowder,
        "repair_bay_gunpowder",
    ),
    (
        BuildingKind::CommandCenter,
        Era::Industrial,
        "command_center_industrial",
    ),
    (
        BuildingKind::PowerPlant,
        Era::Industrial,
        "power_plant_industrial",
    ),
    (
        BuildingKind::Refinery,
        Era::Industrial,
        "refinery_industrial",
    ),
    (
        BuildingKind::Barracks,
        Era::Industrial,
        "barracks_industrial",
    ),
    (BuildingKind::Factory, Era::Industrial, "factory_industrial"),
    (
        BuildingKind::TechCenter,
        Era::Industrial,
        "tech_center_industrial",
    ),
    (
        BuildingKind::RepairBay,
        Era::Industrial,
        "repair_bay_industrial",
    ),
    (
        BuildingKind::CommandCenter,
        Era::Future,
        "command_center_future",
    ),
    (BuildingKind::PowerPlant, Era::Future, "power_plant_future"),
    (BuildingKind::Refinery, Era::Future, "refinery_future"),
    (BuildingKind::Barracks, Era::Future, "barracks_future"),
    (BuildingKind::Factory, Era::Future, "factory_future"),
    (BuildingKind::TechCenter, Era::Future, "tech_center_future"),
    (BuildingKind::RepairBay, Era::Future, "repair_bay_future"),
];

pub fn building_model(kind: BuildingKind, era: Era) -> &'static str {
    ERA_LOOKS
        .iter()
        .find(|&&(looked, age, _)| looked == kind && age == era)
        .map_or(kind.spec().model_id, |&(.., model)| model)
}
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

    pub fn building(&self, kind: BuildingKind, team: Team, era: Era) -> (WorldAssetRoot, Painted) {
        self.painted(building_model(kind, era), Painted::Side(team))
    }

    /// Half a wall run, joining a wall post to its neighbour.
    pub fn wall_link(&self, wall: BuildingKind, team: Team) -> (WorldAssetRoot, Painted) {
        let link = if wall == BuildingKind::Palisade {
            PALISADE_LINK_MODEL
        } else {
            WALL_LINK_MODEL
        };
        self.painted(link, Painted::Side(team))
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
        .chain(ERA_LOOKS.iter().map(|&(.., model)| model))
        .chain([WALL_LINK_MODEL, PALISADE_LINK_MODEL]);
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
