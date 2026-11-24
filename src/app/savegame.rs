use crate::missions::level_events::EventsFired;
use crate::sim::abilities::{Mine, bury};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::i18n::{tr, tr_fmt};
use crate::app::match_flow::{GameState, MatchClock};
use crate::app::settings::{GameSettings, game_data_directory};
use crate::missions::levels::{ActiveLevel, LevelDefinition};
use crate::net::network::NetSession;
use crate::sim::ai::AiCommanders;
use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::components::*;
use crate::sim::economy::Economies;
use crate::sim::intel::Intel;
use crate::sim::neutral_sites::{UnclaimedSite, spawn_unclaimed_site};
use crate::sim::notifications::Notifications;
use crate::sim::production::{BuildItem, Lane, Production};
use crate::sim::research::Upgrade;
use crate::sim::spawn::{spawn_building, spawn_unit};
use crate::sim::structures::{BeingSold, Repairing};
use crate::sim::veterancy::Veterancy;
use crate::ui::key_bindings::Action;
use crate::view::assets::ModelLibrary;
use crate::view::camera::RtsCamera;
use crate::world::map::{GameMap, cell_of_index, spawn_crystal_visual};

/// Bumped whenever the save layout changes incompatibly; older files are refused rather than misread.
const SAVE_VERSION: u32 = 2;
const SAVES_DIRECTORY: &str = "saves";
pub const QUICK_SAVE_SLOT: usize = 0;
pub const NAMED_SLOTS: std::ops::RangeInclusive<usize> = 1..=3;

#[derive(Serialize, Deserialize, Clone)]
struct SavedBuilding {
    team: Team,
    kind: BuildingKind,
    origin: [i32; 2],
    health: f32,
    #[serde(default)]
    rally: Option<[i32; 2]>,
    #[serde(default)]
    repairing: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct SavedUnit {
    team: Team,
    kind: UnitKind,
    position: [f32; 3],
    facing: f32,
    health: f32,
    cargo: u32,
    #[serde(default)]
    orders: Vec<Order>,
    #[serde(default)]
    experience: f32,
    #[serde(default)]
    stance: Stance,
}

#[derive(Serialize, Deserialize, Clone)]
struct SavedLane {
    team: Team,
    lane: Lane,
    queue: Vec<BuildItem>,
    progress_secs: f32,
    ready: Option<BuildingKind>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SaveGame {
    version: u32,
    saved_at_unix_secs: u64,
    level: LevelDefinition,
    match_secs: f32,
    credits: Vec<i32>,
    crystal: Vec<u32>,
    buildings: Vec<SavedBuilding>,
    units: Vec<SavedUnit>,
    production: Vec<SavedLane>,
    /// What each team has researched, in `Team::ALL` order.
    #[serde(default)]
    researched: Vec<Vec<Upgrade>>,
    #[serde(default)]
    mines: Vec<Mine>,
    ai: AiCommanders,
    camera_focus: [f32; 3],
    #[serde(default)]
    explored: Vec<bool>,
    /// Saves from before every opponent had its own view kept one shared Syndicate view.
    #[serde(default)]
    explored_by_syndicate: Vec<bool>,
    /// What each opponent has explored, in `Team::OPPONENTS` order.
    #[serde(default)]
    explored_by_opponents: Vec<Vec<bool>>,
    #[serde(default)]
    events_fired: Vec<bool>,
    /// Neutral sites nobody had captured yet: kind and origin.
    #[serde(default)]
    unclaimed_sites: Vec<(BuildingKind, [i32; 2])>,
}

/// A save waiting to be restored once the next match's map has been generated from its level.
#[derive(Resource, Clone)]
pub struct PendingLoad(pub SaveGame);

/// Where save files are written; tests point it at a scratch directory.
#[derive(Resource, Clone)]
pub struct SaveDirectory(pub PathBuf);

impl Default for SaveDirectory {
    fn default() -> Self {
        Self(game_data_directory().join(SAVES_DIRECTORY))
    }
}

#[derive(Resource, Default)]
pub struct SaveRequests {
    pub save: Option<usize>,
    pub load: Option<usize>,
}

fn slot_path(directory: &SaveDirectory, slot: usize) -> PathBuf {
    let file = if slot == QUICK_SAVE_SLOT {
        "quicksave.json".to_string()
    } else {
        format!("slot{slot}.json")
    };
    directory.0.join(file)
}

pub fn slot_label(slot: usize) -> String {
    if slot == QUICK_SAVE_SLOT {
        tr("Quick save").to_string()
    } else {
        tr_fmt("Slot {}", &[&slot])
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn describe_age(saved_at: u64) -> String {
    let minutes = unix_now().saturating_sub(saved_at) / 60;
    match minutes {
        0 => tr("just now").to_string(),
        1..=59 => tr_fmt("{} min ago", &[&minutes]),
        60..=1439 => tr_fmt("{} h ago", &[&(minutes / 60)]),
        _ => tr_fmt("{} days ago", &[&(minutes / 1440)]),
    }
}

/// One-line description of what a slot holds, or `None` when it is empty or unreadable.
pub fn slot_summary(directory: &SaveDirectory, slot: usize) -> Option<String> {
    let json = std::fs::read_to_string(slot_path(directory, slot)).ok()?;
    let save: SaveGame = serde_json::from_str(&json).ok()?;
    let own_units = save
        .units
        .iter()
        .filter(|unit| unit.team == Team::Player)
        .count();
    Some(tr_fmt(
        "{}  -  ${}, {} units",
        &[
            &describe_age(save.saved_at_unix_secs),
            &save.credits[Team::Player.index()],
            &own_units,
        ],
    ))
}

pub fn quick_save_keys(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<GameSettings>,
    session: Option<Res<NetSession>>,
    mut requests: ResMut<SaveRequests>,
) {
    if session.is_some() {
        return;
    }
    if settings.keys.just_pressed(&keys, Action::QuickSave) {
        requests.save = Some(QUICK_SAVE_SLOT);
    }
    if settings.keys.just_pressed(&keys, Action::QuickLoad) {
        requests.load = Some(QUICK_SAVE_SLOT);
    }
}

type SavableBuilding<'a> = (
    &'a Building,
    &'a Team,
    &'a Health,
    Option<&'a RallyPoint>,
    Has<Repairing>,
);

type SavableUnit<'a> = (
    &'a Unit,
    &'a Team,
    &'a Transform,
    &'a Mobile,
    &'a Health,
    Option<&'a Harvester>,
    Option<&'a Orders>,
    Option<&'a Veterancy>,
    Option<&'a Stance>,
);

impl SavedBuilding {
    fn of(
        (building, &team, health, rally, repairing): (
            &Building,
            &Team,
            &Health,
            Option<&RallyPoint>,
            bool,
        ),
    ) -> Self {
        Self {
            team,
            kind: building.kind,
            origin: building.origin.to_array(),
            health: health.current,
            rally: rally.map(|rally| rally.0.to_array()),
            repairing,
        }
    }
}

impl SavedUnit {
    fn of(
        (unit, &team, transform, mobile, health, harvester, orders, veterancy, stance): SavableUnit,
    ) -> Self {
        Self {
            team,
            kind: unit.0,
            position: transform.translation.to_array(),
            facing: mobile.facing,
            health: health.current,
            cargo: harvester.map_or(0, |h| h.cargo),
            orders: orders.map_or_else(Vec::new, |orders| {
                orders
                    .current
                    .iter()
                    .chain(orders.queued.iter())
                    .copied()
                    .collect()
            }),
            experience: veterancy.map_or(0.0, |veterancy| veterancy.experience),
            stance: stance.copied().unwrap_or_default(),
        }
    }
}

fn saved_lanes(production: &Production) -> Vec<SavedLane> {
    Team::ALL
        .into_iter()
        .flat_map(|team| Lane::ALL.map(|lane| (team, lane)))
        .map(|(team, lane)| {
            let state = production.lane(team, lane);
            SavedLane {
                team,
                lane,
                queue: state.queue.iter().copied().collect(),
                progress_secs: state.progress_secs,
                ready: state.ready,
            }
        })
        .collect()
}

fn write_save_file(path: &std::path::Path, save: &SaveGame) -> Result<(), String> {
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .map_err(|error| error.to_string())?;
    let json = serde_json::to_string(save).map_err(|error| error.to_string())?;
    std::fs::write(path, json).map_err(|error| error.to_string())
}

pub fn write_requested_save(
    directory: Res<SaveDirectory>,
    mut requests: ResMut<SaveRequests>,
    mut notifications: ResMut<Notifications>,
    level: Res<ActiveLevel>,
    clock: Res<MatchClock>,
    events_fired: Res<EventsFired>,
    map: Res<GameMap>,
    economies: Res<Economies>,
    production: Res<Production>,
    commanders: Res<AiCommanders>,
    intel: Res<Intel>,
    cameras: Query<&RtsCamera>,
    mines: Query<&Mine>,
    buildings: Query<SavableBuilding, Without<BeingSold>>,
    unclaimed_sites: Query<&Building, With<UnclaimedSite>>,
    units: Query<SavableUnit>,
) {
    let Some(slot) = requests.save.take() else {
        return;
    };
    let save = SaveGame {
        version: SAVE_VERSION,
        saved_at_unix_secs: unix_now(),
        level: level.0.clone(),
        match_secs: clock.elapsed_secs,
        events_fired: events_fired.0.clone(),
        credits: Team::ALL
            .iter()
            .map(|&team| economies.of(team).credits)
            .collect(),
        crystal: map.crystal_amounts(),
        buildings: buildings
            .iter()
            .map(|item| SavedBuilding::of(item))
            .collect(),
        units: units.iter().map(|item| SavedUnit::of(item)).collect(),
        production: saved_lanes(&production),
        researched: Team::ALL
            .map(|team| production.researched(team).to_vec())
            .to_vec(),
        mines: {
            let mut buried: Vec<Mine> = mines.iter().copied().collect();
            buried.sort_by_key(|mine| (mine.cell.y, mine.cell.x));
            buried
        },
        ai: commanders.clone(),
        camera_focus: cameras
            .single()
            .map_or([0.0; 3], |camera| camera.focus.to_array()),
        explored: intel.vision(Team::Player).explored_cells(),
        explored_by_syndicate: Vec::new(),
        unclaimed_sites: unclaimed_sites
            .iter()
            .map(|site| (site.kind, site.origin.to_array()))
            .collect(),
        explored_by_opponents: Team::OPPONENTS
            .map(|opponent| intel.vision(opponent).explored_cells())
            .to_vec(),
    };
    let path = slot_path(&directory, slot);
    match write_save_file(&path, &save) {
        Ok(()) => notifications.push(tr_fmt("Game saved - {}", &[&slot_label(slot)])),
        Err(error) => {
            warn!("could not write {}: {error}", path.display());
            notifications.push(tr("Saving failed"));
        }
    }
}

fn read_save(directory: &SaveDirectory, slot: usize) -> Result<SaveGame, &'static str> {
    let json = std::fs::read_to_string(slot_path(directory, slot))
        .map_err(|_| tr("No saved game in that slot"))?;
    #[derive(Deserialize)]
    struct VersionOnly {
        version: u32,
    }
    let version: VersionOnly = serde_json::from_str(&json).map_err(|_| "Saved game is damaged")?;
    if version.version != SAVE_VERSION {
        return Err(tr("Saved game is from an incompatible version"));
    }
    serde_json::from_str(&json).map_err(|_| "Saved game is damaged")
}

/// Starts a fresh match on the saved level; `restore_pending_load` fills it in once the map exists.
pub fn begin_requested_load(
    mut commands: Commands,
    directory: Res<SaveDirectory>,
    mut requests: ResMut<SaveRequests>,
    notifications: Option<ResMut<Notifications>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(slot) = requests.load.take() else {
        return;
    };
    match read_save(&directory, slot) {
        Ok(save) => {
            commands.insert_resource(ActiveLevel(save.level.clone()));
            commands.insert_resource(PendingLoad(save));
            next_state.set(GameState::Restarting);
        }
        Err(reason) => {
            if let Some(mut notifications) = notifications {
                notifications.push(reason);
            }
            warn!("loading slot {slot} failed: {reason}");
        }
    }
}

pub fn restore_pending_load(
    mut commands: Commands,
    pending: Option<Res<PendingLoad>>,
    models: Res<ModelLibrary>,
    mut map: ResMut<GameMap>,
    mut economies: ResMut<Economies>,
    mut production: ResMut<Production>,
    mut commanders: ResMut<AiCommanders>,
    mut clock: ResMut<MatchClock>,
    mut events_fired: ResMut<EventsFired>,
    mut notifications: ResMut<Notifications>,
    mut intel: ResMut<Intel>,
    mut cameras: Query<&mut RtsCamera>,
) {
    let Some(pending) = pending else { return };
    let save = pending.0.clone();
    commands.remove_resource::<PendingLoad>();
    restore_explored_ground(&mut intel, &save);

    map.reset_for_load(save.crystal);
    let mut rng = rand::rng();
    for (index, amount) in map.crystal_amounts().into_iter().enumerate() {
        if amount > 0 {
            spawn_crystal_visual(&mut commands, &mut rng, cell_of_index(index), amount);
        }
    }
    for saved in &save.buildings {
        respawn_building(&mut commands, &models, &mut map, saved);
    }
    for saved in &save.units {
        respawn_unit(&mut commands, &models, saved);
    }
    for &(kind, origin) in &save.unclaimed_sites {
        spawn_unclaimed_site(
            &mut commands,
            &models,
            &mut map,
            kind,
            IVec2::from_array(origin),
        );
    }
    for (team, credits) in Team::ALL.into_iter().zip(&save.credits) {
        economies.of_mut(team).credits = *credits;
    }
    for (team, enemy) in save.level.opponents() {
        economies.of_mut(team).income_rate = enemy.difficulty.income_rate();
    }
    for saved in save.production {
        let lane = production.lane_mut(saved.team, saved.lane);
        lane.queue = saved.queue.into();
        lane.progress_secs = saved.progress_secs;
        lane.ready = saved.ready;
    }
    for mine in save.mines {
        bury(&mut commands, mine);
    }
    for (team, upgrades) in Team::ALL.into_iter().zip(save.researched) {
        for upgrade in upgrades {
            production.complete_research(team, upgrade);
        }
    }
    *commanders = save.ai;
    clock.elapsed_secs = save.match_secs;
    events_fired.0 = save.events_fired.clone();
    for mut camera in &mut cameras {
        camera.focus = Vec3::from_array(save.camera_focus);
    }
    notifications.push(tr("Game loaded"));
}

fn respawn_building(
    commands: &mut Commands,
    models: &ModelLibrary,
    map: &mut GameMap,
    saved: &SavedBuilding,
) {
    let origin = IVec2::from_array(saved.origin);
    let entity = spawn_building(commands, models, map, saved.kind, saved.team, origin);
    commands.entity(entity).insert(Health {
        current: saved.health,
        max: saved.kind.spec().max_health,
    });
    if let Some(rally) = saved.rally {
        commands
            .entity(entity)
            .insert(RallyPoint(IVec2::from_array(rally)));
    }
    if saved.repairing {
        commands.entity(entity).insert(Repairing::default());
    }
}

fn respawn_unit(commands: &mut Commands, models: &ModelLibrary, saved: &SavedUnit) {
    let position = Vec3::from_array(saved.position);
    let entity = spawn_unit(
        commands,
        models,
        saved.kind,
        saved.team,
        position,
        saved.facing,
    );
    let (veterancy, health) = Veterancy::restored(saved.kind, saved.experience, saved.health);
    commands.entity(entity).insert(health);
    if saved.kind.spec().weapon.is_some() {
        commands.entity(entity).insert(saved.stance);
    }
    if saved.experience > 0.0 {
        commands.entity(entity).insert(veterancy);
    }
    let mut planned = saved.orders.iter().copied();
    commands.entity(entity).insert(Orders {
        current: planned.next(),
        queued: planned.collect(),
    });
    if saved.kind == UnitKind::Harvester {
        commands.entity(entity).insert(Harvester {
            state: HarvestState::SeekingField,
            cargo: saved.cargo,
            preferred_field: None,
        });
    }
}

/// Saves from before every opponent had its own view kept one shared Syndicate view for all of them.
fn restore_explored_ground(intel: &mut Intel, save: &SaveGame) {
    intel
        .vision_mut(Team::Player)
        .restore_explored(&save.explored);
    for (index, opponent) in Team::OPPONENTS.into_iter().enumerate() {
        let explored = save
            .explored_by_opponents
            .get(index)
            .unwrap_or(&save.explored_by_syndicate);
        intel.vision_mut(opponent).restore_explored(explored);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_files_are_distinct() {
        let directory = SaveDirectory(PathBuf::from("/tmp/saves"));
        let paths: Vec<PathBuf> = std::iter::once(QUICK_SAVE_SLOT)
            .chain(NAMED_SLOTS)
            .map(|slot| slot_path(&directory, slot))
            .collect();
        let mut unique = paths.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), paths.len());
    }

    #[test]
    fn save_age_reads_naturally() {
        let now = unix_now();
        assert_eq!(describe_age(now), "just now");
        assert_eq!(describe_age(now - 5 * 60), "5 min ago");
        assert_eq!(describe_age(now - 3 * 3600), "3 h ago");
        assert_eq!(describe_age(now - 2 * 86_400), "2 days ago");
    }

    #[test]
    fn saves_from_other_versions_are_refused() {
        let directory = SaveDirectory(
            std::env::temp_dir().join(format!("iron_tide_version_test_{}", std::process::id())),
        );
        std::fs::create_dir_all(&directory.0).unwrap();
        std::fs::write(slot_path(&directory, 1), r#"{"version": 999}"#).unwrap();
        assert!(read_save(&directory, 1).is_err());
        assert!(
            read_save(&directory, 2).is_err(),
            "an empty slot is not loadable"
        );
        std::fs::remove_dir_all(&directory.0).unwrap();
    }
}
