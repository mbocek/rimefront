use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::settings::custom_levels_directory;
use crate::missions::levels::{Difficulty, LevelDefinition, campaign};
use crate::sim::catalog::Faction;

/// Custom campaigns live in this folder inside the custom levels folder, one JSON file each.
const CUSTOM_CAMPAIGNS_FOLDER: &str = "campaigns";
const MY_CAMPAIGN_FILE: &str = "my_campaign.json";

/// A custom campaign as written on disk: its levels are file names in the custom levels folder, in order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CampaignFile {
    pub title: String,
    #[serde(default)]
    pub faction: Faction,
    pub levels: Vec<String>,
}

/// A chain of missions played in order, each unlocked by winning the one before.
#[derive(Clone, Debug)]
pub struct Campaign {
    /// Stable name for remembering progress; the built-in campaigns keep the bare mission names they always had.
    pub key: String,
    pub title: String,
    pub faction: Faction,
    pub missions: Vec<LevelDefinition>,
    built_in: bool,
}

/// Every campaign the player can play: the built-in ones and any custom ones found on disk.
#[derive(Resource, Clone, Debug)]
pub struct CampaignLibrary(pub Vec<Campaign>);

impl Default for CampaignLibrary {
    fn default() -> Self {
        Self(built_in_campaigns())
    }
}

/// The campaign mission being played, so a victory can be remembered and the next mission offered.
#[derive(Resource, Clone, Debug)]
pub struct CampaignRun {
    pub key: String,
    pub mission: usize,
    pub difficulty: Difficulty,
}

impl Campaign {
    /// Where a win of `mission` is remembered among the completed missions.
    pub fn progress_key(&self, mission: usize) -> String {
        let name = &self.missions[mission].name;
        if self.built_in {
            name.clone()
        } else {
            format!("{}/{}", self.key, name)
        }
    }

    pub fn won(&self, mission: usize, completed: &[String]) -> bool {
        completed.contains(&self.progress_key(mission))
    }

    pub fn unlocked(&self, mission: usize, completed: &[String]) -> bool {
        mission == 0 || self.won(mission - 1, completed)
    }

    pub fn won_count(&self, completed: &[String]) -> usize {
        (0..self.missions.len())
            .filter(|&mission| self.won(mission, completed))
            .count()
    }

    /// The first unlocked mission not won yet; `None` once the campaign is finished.
    pub fn next_to_play(&self, completed: &[String]) -> Option<usize> {
        (0..self.missions.len())
            .find(|&mission| !self.won(mission, completed) && self.unlocked(mission, completed))
    }

    /// The mission as it is played at the chosen campaign difficulty.
    pub fn mission_at(&self, mission: usize, difficulty: Difficulty) -> LevelDefinition {
        at_difficulty(&self.missions[mission], difficulty)
    }
}

/// The two built-in campaigns: the Coalition's missions and the Syndicate's, in their authored order.
pub fn built_in_campaigns() -> Vec<Campaign> {
    let missions: Vec<LevelDefinition> = campaign().into_iter().map(|entry| entry.level).collect();
    [
        ("coalition", Faction::Coalition),
        ("syndicate", Faction::Syndicate),
    ]
    .into_iter()
    .map(|(key, faction)| Campaign {
        key: key.to_string(),
        title: faction.title().to_string(),
        faction,
        missions: missions
            .iter()
            .filter(|level| level.player_faction == faction)
            .cloned()
            .collect(),
        built_in: true,
    })
    .collect()
}

pub fn custom_campaigns_directory(levels_directory: &Path) -> PathBuf {
    levels_directory.join(CUSTOM_CAMPAIGNS_FOLDER)
}

/// Every readable custom campaign whose levels all load, sorted by file name.
pub fn custom_campaigns(levels_directory: &Path) -> Vec<Campaign> {
    let Ok(entries) = std::fs::read_dir(custom_campaigns_directory(levels_directory)) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let file: CampaignFile =
                serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
            let missions = file
                .levels
                .iter()
                .map(|name| {
                    let json = std::fs::read_to_string(levels_directory.join(name)).ok()?;
                    let level: LevelDefinition = serde_json::from_str(&json).ok()?;
                    level.problems().is_empty().then_some(level)
                })
                .collect::<Option<Vec<_>>>()?;
            let stem = path.file_stem()?.to_string_lossy();
            (!missions.is_empty()).then(|| Campaign {
                key: format!("custom:{stem}"),
                title: file.title,
                faction: file.faction,
                missions,
                built_in: false,
            })
        })
        .collect()
}

/// Appends a custom level to the player's own campaign file, creating it on first use.
pub fn add_to_my_campaign(
    levels_directory: &Path,
    level_file: &Path,
    faction: Faction,
) -> std::io::Result<usize> {
    let folder = custom_campaigns_directory(levels_directory);
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(MY_CAMPAIGN_FILE);
    let mut file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str::<CampaignFile>(&json).ok())
        .unwrap_or(CampaignFile {
            title: "My campaign".to_string(),
            faction,
            levels: Vec::new(),
        });
    let name = level_file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| std::io::Error::other("the level has no file name"))?;
    if !file.levels.contains(&name) {
        file.levels.push(name);
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&file).map_err(std::io::Error::other)?,
    )?;
    Ok(file.levels.len())
}

/// Reloads the campaigns on returning to the main menu, so custom ones written meanwhile appear.
pub fn reload_campaigns(mut library: ResMut<CampaignLibrary>) {
    let mut campaigns = built_in_campaigns();
    campaigns.extend(custom_campaigns(&custom_levels_directory()));
    library.0 = campaigns;
}

pub fn find_campaign<'a>(campaigns: &'a [Campaign], key: &str) -> Option<&'a Campaign> {
    campaigns.iter().find(|campaign| campaign.key == key)
}

/// Shifts every computer commander of a mission from its authored difficulty by how far the chosen campaign
/// difficulty is from Normal: the first missions stay gentle on Easy and every mission gets harder on Brutal.
pub fn at_difficulty(level: &LevelDefinition, chosen: Difficulty) -> LevelDefinition {
    let rank = |difficulty: Difficulty| {
        Difficulty::ALL
            .iter()
            .position(|&d| d == difficulty)
            .unwrap_or(1) as i32
    };
    let shift = rank(chosen) - rank(Difficulty::Normal);
    let mut level = level.clone();
    for enemy in &mut level.enemies {
        let shifted = (rank(enemy.difficulty) + shift).clamp(0, Difficulty::ALL.len() as i32 - 1);
        enemy.difficulty = Difficulty::ALL[shifted as usize];
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_missions_split_into_a_coalition_and_a_syndicate_campaign() {
        let campaigns = built_in_campaigns();
        assert_eq!(campaigns.len(), 2);
        assert_eq!(campaigns[0].missions.len(), 12);
        assert_eq!(campaigns[1].missions.len(), 8);
        assert!(
            campaigns[1]
                .missions
                .iter()
                .all(|level| level.player_faction == Faction::Syndicate)
        );
    }

    #[test]
    fn a_mission_unlocks_once_the_one_before_is_won() {
        let coalition = &built_in_campaigns()[0];
        let none: Vec<String> = Vec::new();
        assert!(coalition.unlocked(0, &none));
        assert!(!coalition.unlocked(1, &none));
        assert_eq!(coalition.next_to_play(&none), Some(0));
        let first_won = vec![coalition.progress_key(0)];
        assert!(coalition.unlocked(1, &first_won));
        assert_eq!(coalition.next_to_play(&first_won), Some(1));
    }

    #[test]
    fn a_custom_campaign_chains_levels_from_the_custom_folder_and_skips_broken_ones() {
        let folder =
            std::env::temp_dir().join(format!("rimefront_campaigns_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let level = LevelDefinition::default();
        let level_path = crate::missions::levels::save_custom_level(&folder, &level).unwrap();
        assert_eq!(
            add_to_my_campaign(&folder, &level_path, Faction::Coalition).unwrap(),
            1
        );
        assert_eq!(
            add_to_my_campaign(&folder, &level_path, Faction::Coalition).unwrap(),
            1
        );
        std::fs::write(
            custom_campaigns_directory(&folder).join("broken.json"),
            r#"{"title": "Broken", "levels": ["missing.json"]}"#,
        )
        .unwrap();
        let found = custom_campaigns(&folder);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "My campaign");
        assert_eq!(found[0].missions.len(), 1);
        assert!(found[0].progress_key(0).starts_with("custom:my_campaign/"));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn campaign_difficulty_shifts_every_commander_and_stays_within_the_levels() {
        let coalition = &built_in_campaigns()[0];
        let authored = &coalition.missions[0];
        let easy = coalition.mission_at(0, Difficulty::Easy);
        let brutal = coalition.mission_at(0, Difficulty::Brutal);
        for ((original, softer), harder) in authored
            .enemies
            .iter()
            .zip(&easy.enemies)
            .zip(&brutal.enemies)
        {
            assert!(softer.difficulty as usize <= original.difficulty as usize);
            assert!(harder.difficulty as usize >= original.difficulty as usize);
        }
    }
}
