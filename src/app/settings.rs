use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use serde::{Deserialize, Serialize};

use crate::app::i18n::{Language, set_language};
use crate::missions::levels::Difficulty;
use crate::sim::catalog::SideColor;
use crate::ui::key_bindings::KeyBindings;

const GAME_DIRECTORY: &str = "rimefront";
/// What the game's data directory was called before the game was renamed (docs/rename.md).
const GAME_DIRECTORY_BEFORE_RENAME: &str = "iron_tide";
const CUSTOM_LEVELS_DIRECTORY: &str = "levels";
const SETTINGS_FILE: &str = "settings.json";
const QUALITY_VAR: &str = "RIMEFRONT_QUALITY";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum QualityPreset {
    Low,
    Medium,
    High,
    Custom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ShadowQuality {
    Off,
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum VegetationDetail {
    Off,
    Near,
    Far,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GameSpeed {
    Slow,
    Normal,
    Fast,
    Fastest,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ScrollSpeed {
    Slow,
    Normal,
    Fast,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum FrameLimit {
    Fps30,
    Fps60,
    Fps120,
    Unlimited,
}

impl FrameLimit {
    pub fn max_fps(self) -> Option<f32> {
        match self {
            FrameLimit::Fps30 => Some(30.0),
            FrameLimit::Fps60 => Some(60.0),
            FrameLimit::Fps120 => Some(120.0),
            FrameLimit::Unlimited => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HealthBars {
    Always,
    DamagedOrSelected,
    SelectedOnly,
}

/// Everything the player can configure; persisted as JSON in the user's config directory.
#[derive(Resource, Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    pub preset: QualityPreset,
    pub shadows: ShadowQuality,
    pub ambient_occlusion: bool,
    pub bloom: bool,
    pub anti_aliasing: bool,
    pub vegetation: VegetationDetail,
    pub wind: bool,
    pub vsync: bool,
    pub frame_limit: FrameLimit,
    pub fullscreen: bool,
    pub game_speed: GameSpeed,
    pub edge_scroll: bool,
    pub scroll_speed: ScrollSpeed,
    pub damage_numbers: bool,
    pub health_bars: HealthBars,
    pub performance_overlay: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub effects_volume: f32,
    /// The radio operator and the units' replies.
    pub voice_volume: f32,
    pub language: Language,
    pub keys: KeyBindings,
    /// Ask GitHub at start-up whether a newer version has been released.
    pub check_for_updates: bool,
    /// The colour of the player's side; `None` takes the side's own.
    pub side_color: Option<SideColor>,
    /// The host the player last joined for a network battle.
    pub join_address: String,
    /// What the player is called in a world's hall of fame; empty leaves them "Player N".
    pub nickname: String,
    /// Names of the campaign missions the player has won.
    pub completed_missions: Vec<String>,
    /// How hard campaign missions are played, against their authored difficulty.
    pub campaign_difficulty: Difficulty,
    /// The campaign played last, which the main menu offers to continue; empty before any.
    pub last_campaign: String,
    /// Field tips the player has already read, by their stored name.
    pub seen_tips: Vec<String>,
}

impl Default for GameSettings {
    fn default() -> Self {
        let mut settings = Self {
            preset: QualityPreset::High,
            shadows: ShadowQuality::High,
            ambient_occlusion: true,
            bloom: true,
            anti_aliasing: true,
            vegetation: VegetationDetail::Far,
            wind: true,
            vsync: true,
            frame_limit: FrameLimit::Fps60,
            fullscreen: false,
            game_speed: GameSpeed::Normal,
            edge_scroll: true,
            scroll_speed: ScrollSpeed::Normal,
            damage_numbers: true,
            health_bars: HealthBars::DamagedOrSelected,
            performance_overlay: false,
            master_volume: 0.4,
            music_volume: 0.6,
            effects_volume: 0.8,
            voice_volume: 0.8,
            language: Language::English,
            keys: KeyBindings::default(),
            check_for_updates: true,
            side_color: None,
            join_address: "127.0.0.1".to_string(),
            nickname: String::new(),
            completed_missions: Vec::new(),
            campaign_difficulty: Difficulty::Normal,
            last_campaign: String::new(),
            seen_tips: Vec::new(),
        };
        settings.apply_preset(QualityPreset::High);
        settings
    }
}

impl GameSettings {
    pub fn world_nickname(&self) -> Option<String> {
        let nickname = self.nickname.trim();
        (!nickname.is_empty()).then(|| nickname.to_string())
    }

    /// Sets every detail option to the preset's values; `Custom` leaves them untouched.
    pub fn apply_preset(&mut self, preset: QualityPreset) {
        self.preset = preset;
        let (shadows, ambient_occlusion, bloom, vegetation, wind) = match preset {
            QualityPreset::Low => (
                ShadowQuality::Low,
                false,
                false,
                VegetationDetail::Off,
                false,
            ),
            QualityPreset::Medium => (
                ShadowQuality::Medium,
                false,
                true,
                VegetationDetail::Near,
                true,
            ),
            QualityPreset::High => (ShadowQuality::High, true, true, VegetationDetail::Far, true),
            QualityPreset::Custom => return,
        };
        self.shadows = shadows;
        self.ambient_occlusion = ambient_occlusion;
        self.bloom = bloom;
        self.vegetation = vegetation;
        self.wind = wind;
    }

    pub fn shadow_map_size(&self) -> usize {
        match self.shadows {
            ShadowQuality::Off | ShadowQuality::Low => 1024,
            ShadowQuality::Medium => 2048,
            ShadowQuality::High => 4096,
        }
    }

    pub fn shadow_cascades(&self) -> usize {
        match self.shadows {
            ShadowQuality::Off | ShadowQuality::Low => 2,
            ShadowQuality::Medium => 3,
            ShadowQuality::High => 4,
        }
    }

    pub fn shadow_distance(&self) -> f32 {
        match self.shadows {
            ShadowQuality::Off | ShadowQuality::Low => 90.0,
            ShadowQuality::Medium => 130.0,
            ShadowQuality::High => 180.0,
        }
    }

    /// Distance from the camera focus within which ground clutter is drawn; `None` hides it entirely.
    pub fn fine_decor_range(&self) -> Option<f32> {
        match self.vegetation {
            VegetationDetail::Off => None,
            VegetationDetail::Near => Some(55.0),
            VegetationDetail::Far => Some(85.0),
        }
    }

    /// Plants farther than this from the camera focus hold still, which saves transform updates.
    pub fn game_speed_factor(&self) -> f32 {
        match self.game_speed {
            GameSpeed::Slow => 0.75,
            GameSpeed::Normal => 1.0,
            GameSpeed::Fast => 1.5,
            GameSpeed::Fastest => 2.0,
        }
    }

    pub fn scroll_speed_factor(&self) -> f32 {
        match self.scroll_speed {
            ScrollSpeed::Slow => 0.6,
            ScrollSpeed::Normal => 1.0,
            ScrollSpeed::Fast => 1.6,
        }
    }
}

fn data_directories() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn game_data_directory() -> PathBuf {
    data_directories().join(GAME_DIRECTORY)
}

/// The first time the renamed game starts, the settings, saves, replays, levels and worlds kept under its old
/// name move to the new one, so nobody loses them.
pub fn bring_over_data_from_before_the_rename() {
    let old = data_directories().join(GAME_DIRECTORY_BEFORE_RENAME);
    if let Err(error) = move_game_data(&old, &game_data_directory()) {
        warn!(
            "could not bring over the data in {}: {error}",
            old.display()
        );
    }
}

/// Moves `old` to `new` unless `new` is there already; across volumes, where a move cannot be, copies it and
/// leaves `old` in place. Whether anything was brought over.
fn move_game_data(old: &Path, new: &Path) -> std::io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(old, new).is_err() {
        copy_directory(old, new)?;
    }
    Ok(true)
}

fn copy_directory(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn custom_levels_directory() -> PathBuf {
    game_data_directory().join(CUSTOM_LEVELS_DIRECTORY)
}

fn settings_path() -> PathBuf {
    game_data_directory().join(SETTINGS_FILE)
}

/// Reads saved settings, falling back to defaults when the file is missing or unreadable.
pub fn load_settings() -> GameSettings {
    let mut settings: GameSettings = std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    let override_preset = match std::env::var(QUALITY_VAR).as_deref() {
        Ok("low") => Some(QualityPreset::Low),
        Ok("medium") => Some(QualityPreset::Medium),
        Ok("high") => Some(QualityPreset::High),
        _ => None,
    };
    if let Some(preset) = override_preset {
        settings.apply_preset(preset);
    }
    // Screens built before the first frame already need the language.
    set_language(settings.language);
    settings
}

pub fn persist_settings(settings: Res<GameSettings>) {
    if !settings.is_changed() || settings.is_added() {
        return;
    }
    let path = settings_path();
    let written = std::fs::create_dir_all(game_data_directory())
        .map_err(|error| error.to_string())
        .and_then(|_| serde_json::to_string_pretty(&*settings).map_err(|error| error.to_string()))
        .and_then(|json| std::fs::write(&path, json).map_err(|error| error.to_string()));
    if let Err(error) = written {
        warn!("could not save settings to {}: {error}", path.display());
    }
}

pub fn apply_language(settings: Res<GameSettings>) {
    if settings.is_changed() {
        set_language(settings.language);
    }
}

pub fn apply_window_settings(
    settings: Res<GameSettings>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !settings.is_changed() || crate::app::capture::is_capture_run() {
        return;
    }
    for mut window in &mut windows {
        window.present_mode = if settings.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
        window.mode = if settings.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
    }
}

/// Game speed scales virtual time, which every simulation system reads.
pub fn apply_game_speed(settings: Res<GameSettings>, mut time: ResMut<Time<Virtual>>) {
    if settings.is_changed() {
        time.set_relative_speed(settings.game_speed_factor());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_kept_under_the_old_name_moves_to_the_new_one_once() {
        let root = std::env::temp_dir().join(format!("rimefront_rename_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (old, new) = (
            root.join(GAME_DIRECTORY_BEFORE_RENAME),
            root.join(GAME_DIRECTORY),
        );
        std::fs::create_dir_all(old.join("saves")).unwrap();
        std::fs::write(old.join(SETTINGS_FILE), "{}").unwrap();
        std::fs::write(old.join("saves/slot1.json"), "saved").unwrap();

        assert!(move_game_data(&old, &new).unwrap());
        assert!(!old.exists());
        assert_eq!(
            std::fs::read_to_string(new.join("saves/slot1.json")).unwrap(),
            "saved"
        );

        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(SETTINGS_FILE), "older").unwrap();
        assert!(
            !move_game_data(&old, &new).unwrap(),
            "the new directory was replaced"
        );
        assert_eq!(
            std::fs::read_to_string(new.join(SETTINGS_FILE)).unwrap(),
            "{}"
        );

        let copied = root.join("copied");
        copy_directory(&new, &copied).unwrap();
        assert_eq!(
            std::fs::read_to_string(copied.join("saves/slot1.json")).unwrap(),
            "saved"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn presets_drive_every_detail_option() {
        let mut settings = GameSettings::default();
        settings.apply_preset(QualityPreset::Low);
        assert_eq!(settings.shadows, ShadowQuality::Low);
        assert!(!settings.ambient_occlusion && !settings.bloom && !settings.wind);
        assert_eq!(settings.fine_decor_range(), None);
        settings.apply_preset(QualityPreset::High);
        assert!(settings.ambient_occlusion && settings.bloom);
        assert!(settings.fine_decor_range().unwrap() > 60.0);
    }

    #[test]
    fn custom_preset_keeps_hand_picked_values() {
        let mut settings = GameSettings {
            shadows: ShadowQuality::Off,
            ..Default::default()
        };
        settings.apply_preset(QualityPreset::Custom);
        assert_eq!(settings.shadows, ShadowQuality::Off);
    }

    #[test]
    fn settings_survive_a_file_round_trip_and_tolerate_missing_fields() {
        let settings = GameSettings {
            game_speed: GameSpeed::Fastest,
            health_bars: HealthBars::Always,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<GameSettings>(&json).unwrap(),
            settings
        );
        let partial: GameSettings = serde_json::from_str(r#"{"vsync": false}"#).unwrap();
        assert!(!partial.vsync);
        assert_eq!(partial.game_speed, GameSpeed::Normal);
    }
}
