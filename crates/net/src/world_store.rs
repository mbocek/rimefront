use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world_host::PlayerKey;
use iron_tide_sim::levels::ActiveLevel;
use iron_tide_sim::match_flow::MatchClock;
use iron_tide_sim::sim::abilities::{Mine, bury};
use iron_tide_sim::sim::ai::AiCommanders;
use iron_tide_sim::sim::catalog::{BuildingKind, Team};
use iron_tide_sim::sim::components::{Building, Unit};
use iron_tide_sim::sim::determinism::SimId;
use iron_tide_sim::sim::economy::Economies;
use iron_tide_sim::sim::intel::Intel;
use iron_tide_sim::sim::neutral_sites::{UnclaimedSite, spawn_unclaimed_site};
use iron_tide_sim::sim::production::{BuildItem, Lane, Production};
use iron_tide_sim::sim::records::{
    SavableBuilding, SavableUnit, SavedBuilding, SavedUnit, respawn_building, respawn_unit,
};
use iron_tide_sim::sim::research::Upgrade;
use iron_tide_sim::sim::sides::{RetiredSides, Sides};
use iron_tide_sim::sim::structures::BeingSold;
use iron_tide_sim::sim::support_powers::{SupportPower, SupportPowers};
use iron_tide_sim::world::map::{
    CrystalVisual, GameMap, MapBounds, chunk_of, map_bounds, spawn_crystal_visual, world_to_cell,
};
use iron_tide_sim::world::sites::StartingSites;
use iron_tide_sim::world::wilds::{crystal_look, widen_world};

/// The layout of a saved world: what its files hold and in what order. It is the first thing in the world
/// file, so a world saved by another layout is recognised before anything else is read; raising it needs a
/// way to read the layout before (docs/compatibility.md), and `fixtures/world_layout.txt` changes with it.
const STORE_VERSION: u32 = 4;
const WORLD_FILE: &str = "world.bin";
const CHUNKS_DIRECTORY: &str = "chunks";

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct LaneRecord {
    queue: Vec<BuildItem>,
    progress_secs: f32,
    ready: Option<BuildingKind>,
}

/// What a side owns beyond its units and structures.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct SideRecord {
    credits: i32,
    /// In the order of `Lane::ALL`.
    lanes: Vec<LaneRecord>,
    researched: Vec<Upgrade>,
    /// In the order of `SupportPower::ALL`.
    support_recharge_secs: Vec<f32>,
}

/// What the host of a world keeps about its players.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct HostingRecord {
    pub level_side_taken: bool,
    pub players: Vec<(PlayerKey, Team)>,
    pub sites: Vec<(Team, IVec2)>,
    pub away_since: Vec<(Team, u64)>,
    pub sheltered: Vec<Team>,
    pub computer_sides: Vec<(Team, IVec2)>,
}

/// Everything of a world that is not kept by chunk, and the file of every chunk saved with it.
#[derive(Serialize, Deserialize)]
struct WorldRecord {
    version: u32,
    map_seed: u64,
    tick: u64,
    match_secs: f32,
    bounds: MapBounds,
    sides: Vec<SideRecord>,
    retired_sides: Vec<Team>,
    starting_sites: Vec<IVec2>,
    ai: AiCommanders,
    hosting: HostingRecord,
    chunks: Vec<(IVec2, String)>,
}

/// What stands on one chunk of a world: crystal, units, structures, mines and neutral sites, and which
/// of its cells each side has explored.
#[derive(Serialize, Deserialize, Default)]
struct ChunkRecord {
    /// Every cell of the chunk on the map, row by row.
    crystal: Vec<u32>,
    buildings: Vec<SavedBuilding>,
    units: Vec<SavedUnit>,
    mines: Vec<Mine>,
    unclaimed_sites: Vec<(BuildingKind, IVec2)>,
    explored: Vec<(Team, Vec<u8>)>,
}

/// A world read back from its directory, ready to be built again.
pub struct SavedWorld {
    record: WorldRecord,
    chunks: HashMap<IVec2, ChunkRecord>,
}

impl SavedWorld {
    pub fn map_seed(&self) -> u64 {
        self.record.map_seed
    }

    pub fn tick(&self) -> u64 {
        self.record.tick
    }

    pub fn hosting(&self) -> &HostingRecord {
        &self.record.hosting
    }
}

/// How much one save wrote.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SaveReport {
    pub chunks: usize,
    pub chunks_written: usize,
    pub bytes_written: usize,
}

/// A world's directory: the world file, and a file for every chunk. A save writes only the chunks that
/// changed, each to a new file, then the world file naming every chunk's file, then removes the files no
/// longer named, so a save cut short leaves the last whole one. See docs/persistent-world.md.
pub struct WorldStore {
    directory: PathBuf,
    /// The hash and file of every chunk as last written.
    written: HashMap<IVec2, (u64, String)>,
}

impl WorldStore {
    pub fn at(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            written: HashMap::new(),
        }
    }

    fn chunks_directory(&self) -> PathBuf {
        self.directory.join(CHUNKS_DIRECTORY)
    }

    /// The world saved in this directory; `None` when there is none.
    pub fn load(&mut self) -> io::Result<Option<SavedWorld>> {
        let bytes = match std::fs::read(self.directory.join(WORLD_FILE)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let record = read_world_record(&bytes)?;
        let mut chunks = HashMap::new();
        self.written.clear();
        for (chunk, file) in &record.chunks {
            let bytes = std::fs::read(self.chunks_directory().join(file))?;
            self.written.insert(*chunk, (hash_of(&bytes), file.clone()));
            chunks.insert(*chunk, decode(&bytes)?);
        }
        Ok(Some(SavedWorld { record, chunks }))
    }

    pub fn save(
        &mut self,
        world: &mut World,
        tick: u64,
        hosting: HostingRecord,
    ) -> io::Result<SaveReport> {
        std::fs::create_dir_all(self.chunks_directory())?;
        let chunks = chunk_records(world);
        let mut report = SaveReport {
            chunks: chunks.len(),
            ..SaveReport::default()
        };
        let mut written = HashMap::new();
        for (chunk, record) in &chunks {
            let bytes = encode(record)?;
            let hash = hash_of(&bytes);
            let file = match self.written.get(chunk) {
                Some((same, file)) if *same == hash => file.clone(),
                _ => {
                    let file = format!("{}_{}_{tick}.bin", chunk.x, chunk.y);
                    std::fs::write(self.chunks_directory().join(&file), &bytes)?;
                    report.chunks_written += 1;
                    report.bytes_written += bytes.len();
                    file
                }
            };
            written.insert(*chunk, (hash, file));
        }
        let record = world_record(world, tick, hosting, &written);
        let bytes = encode(&record)?;
        let staged = self.directory.join(format!("{WORLD_FILE}.new"));
        std::fs::write(&staged, &bytes)?;
        std::fs::rename(&staged, self.directory.join(WORLD_FILE))?;
        report.bytes_written += bytes.len();
        self.written = written;
        self.remove_unnamed_chunk_files()?;
        Ok(report)
    }

    fn remove_unnamed_chunk_files(&self) -> io::Result<()> {
        let named: Vec<&str> = self
            .written
            .values()
            .map(|(_, file)| file.as_str())
            .collect();
        for entry in std::fs::read_dir(self.chunks_directory())? {
            let entry = entry?;
            let name = entry.file_name();
            if !named.iter().any(|file| name == **file) {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
}

/// The world record of a world file, of this layout. A layout before it is read here and brought up to
/// date once there is one to read.
fn read_world_record(bytes: &[u8]) -> io::Result<WorldRecord> {
    let (layout, _) = postcard::take_from_bytes::<u32>(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    match layout {
        STORE_VERSION => decode(bytes),
        older if older < STORE_VERSION => Err(io::Error::other(format!(
            "the world was saved by layout {older}, which this server no longer reads (it reads \
             {STORE_VERSION})"
        ))),
        newer => Err(io::Error::other(format!(
            "the world was saved by layout {newer}, newer than this server's {STORE_VERSION}: run the \
             server that saved it, or a newer one"
        ))),
    }
}

fn encode<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
    postcard::to_stdvec(value).map_err(io::Error::other)
}

fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> io::Result<T> {
    postcard::from_bytes(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn hash_of(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn chunk_cells(chunk: IVec2) -> impl Iterator<Item = IVec2> {
    let (first, last) = map_bounds().cells_of_chunk(chunk);
    (first.y..=last.y).flat_map(move |y| (first.x..=last.x).map(move |x| IVec2::new(x, y)))
}

/// Every chunk of the map with what stands on it, units and structures in simulation order, so a chunk
/// nothing happened in saves to the same bytes.
fn chunk_records(world: &mut World) -> HashMap<IVec2, ChunkRecord> {
    let bounds = map_bounds();
    let mut chunks: HashMap<IVec2, ChunkRecord> = (0..bounds.chunk_count())
        .map(|index| bounds.chunk_at(index))
        .map(|chunk| (chunk, ChunkRecord::default()))
        .collect();
    let map = world.resource::<GameMap>();
    for (&chunk, record) in &mut chunks {
        record.crystal = chunk_cells(chunk)
            .map(|cell| map.crystal_at(cell))
            .collect();
    }
    let intel = world.resource::<Intel>();
    for viewer in intel.viewers() {
        for (&chunk, record) in &mut chunks {
            if let Some(bits) = intel.vision(viewer).explored_in_chunk(chunk) {
                record.explored.push((viewer, bits));
            }
        }
    }
    let mut buildings = world.query_filtered::<(&SimId, SavableBuilding), Without<BeingSold>>();
    let mut sorted: Vec<_> = buildings.iter(world).collect();
    sorted.sort_by_key(|(id, _)| **id);
    for (_, item) in sorted {
        let saved = SavedBuilding::of(item);
        if let Some(record) = chunks.get_mut(&chunk_of(saved.origin())) {
            record.buildings.push(saved);
        }
    }
    let mut units = world.query::<(&SimId, SavableUnit)>();
    let mut sorted: Vec<_> = units.iter(world).collect();
    sorted.sort_by_key(|(id, _)| **id);
    for (_, item) in sorted {
        let saved = SavedUnit::of(item);
        if let Some(record) = chunks.get_mut(&chunk_of(world_to_cell(saved.position()))) {
            record.units.push(saved);
        }
    }
    let mut mines = world.query::<&Mine>();
    let mut buried: Vec<Mine> = mines.iter(world).copied().collect();
    buried.sort_by_key(|mine| (mine.cell.y, mine.cell.x));
    for mine in buried {
        if let Some(record) = chunks.get_mut(&chunk_of(mine.cell)) {
            record.mines.push(mine);
        }
    }
    let mut sites = world.query_filtered::<&Building, With<UnclaimedSite>>();
    let mut unclaimed: Vec<(BuildingKind, IVec2)> = sites
        .iter(world)
        .map(|site| (site.kind, site.origin))
        .collect();
    unclaimed.sort_by_key(|(_, origin)| (origin.y, origin.x));
    for (kind, origin) in unclaimed {
        if let Some(record) = chunks.get_mut(&chunk_of(origin)) {
            record.unclaimed_sites.push((kind, origin));
        }
    }
    chunks
}

fn world_record(
    world: &World,
    tick: u64,
    hosting: HostingRecord,
    written: &HashMap<IVec2, (u64, String)>,
) -> WorldRecord {
    let economies = world.resource::<Economies>();
    let production = world.resource::<Production>();
    let powers = world.resource::<SupportPowers>();
    let sides = world
        .resource::<Sides>()
        .iter()
        .map(|team| SideRecord {
            credits: economies.of(team).credits,
            lanes: Lane::ALL
                .map(|lane| {
                    let lane = production.lane(team, lane);
                    LaneRecord {
                        queue: lane.queue.iter().copied().collect(),
                        progress_secs: lane.progress_secs,
                        ready: lane.ready,
                    }
                })
                .to_vec(),
            researched: production.researched(team).to_vec(),
            support_recharge_secs: SupportPower::ALL
                .map(|power| powers.secs_until_ready(team, power))
                .to_vec(),
        })
        .collect();
    let mut chunks: Vec<(IVec2, String)> = written
        .iter()
        .map(|(&chunk, (_, file))| (chunk, file.clone()))
        .collect();
    chunks.sort_by_key(|(chunk, _)| (chunk.y, chunk.x));
    WorldRecord {
        version: STORE_VERSION,
        map_seed: world.resource::<ActiveLevel>().0.map_seed,
        tick,
        match_secs: world.resource::<MatchClock>().elapsed_secs,
        bounds: map_bounds(),
        sides,
        retired_sides: world
            .get_resource::<RetiredSides>()
            .map_or_else(Vec::new, |retired| retired.0.clone()),
        starting_sites: world
            .get_resource::<StartingSites>()
            .map_or_else(Vec::new, |sites| sites.claimed.clone()),
        ai: world.resource::<AiCommanders>().clone(),
        hosting,
        chunks,
    }
}

/// Builds the saved world again in `world`, a match on the same seed started without its forces. Returns
/// the cells whose crystal differs from what the seed grows, for players to be told.
pub fn restore(world: &mut World, saved: &SavedWorld) -> Vec<(IVec2, u32)> {
    let record = &saved.record;
    if record.bounds != map_bounds() {
        widen_world(world, record.bounds);
    }
    clear_what_stands(world);
    let mined = restore_crystal(world, &saved.chunks);
    while world.resource::<Sides>().count() < record.sides.len() {
        world.resource_mut::<Sides>().join();
    }
    let teams: Vec<Team> = world.resource::<Sides>().iter().collect();
    let mut intel = world.resource_mut::<Intel>();
    for &team in &teams {
        intel.admit(team);
    }
    for (&chunk, chunk_record) in &saved.chunks {
        for (viewer, bits) in &chunk_record.explored {
            intel.admit(*viewer);
            intel
                .vision_mut(*viewer)
                .restore_explored_in_chunk(chunk, bits);
        }
    }
    restore_sides(world, &teams, &record.sides);
    world.resource_scope(|world, mut map: Mut<GameMap>| {
        let mut commands = world.commands();
        for chunk_record in saved.chunks.values() {
            for building in &chunk_record.buildings {
                respawn_building(&mut commands, &mut map, building);
            }
            for unit in &chunk_record.units {
                respawn_unit(&mut commands, unit);
            }
            for &mine in &chunk_record.mines {
                bury(&mut commands, mine);
            }
            for &(kind, origin) in &chunk_record.unclaimed_sites {
                spawn_unclaimed_site(&mut commands, &mut map, kind, origin);
            }
        }
    });
    world.flush();
    world.resource_mut::<MatchClock>().elapsed_secs = record.match_secs;
    world.insert_resource(RetiredSides(record.retired_sides.clone()));
    world.insert_resource(StartingSites {
        claimed: record.starting_sites.clone(),
    });
    *world.resource_mut::<AiCommanders>() = record.ai.clone();
    let level = world.resource::<ActiveLevel>().0.clone();
    let mut economies = world.resource_mut::<Economies>();
    for (team, enemy) in level.opponents() {
        economies.of_mut(team).income_rate = enemy.difficulty.income_rate();
    }
    mined
}

/// Takes away the units, structures, mines and crystal the seed put down, which the save replaces.
fn clear_what_stands(world: &mut World) {
    let mut standing = world.query_filtered::<Entity, Or<(
        With<Unit>,
        With<Building>,
        With<Mine>,
        With<CrystalVisual>,
    )>>();
    let doomed: Vec<Entity> = standing.iter(world).collect();
    for entity in doomed {
        world.despawn(entity);
    }
}

/// Lays the saved crystal on the map, which also forgets every structure's footprint.
fn restore_crystal(world: &mut World, chunks: &HashMap<IVec2, ChunkRecord>) -> Vec<(IVec2, u32)> {
    let bounds = map_bounds();
    let mut map = world.resource_mut::<GameMap>();
    let mut amounts: HashMap<IVec2, u32> = HashMap::new();
    let mut mined = Vec::new();
    for (&chunk, record) in chunks {
        for (cell, &amount) in chunk_cells(chunk).zip(&record.crystal) {
            amounts.insert(cell, amount);
            if map.crystal_at(cell) != amount {
                mined.push((cell, amount));
            }
        }
    }
    let dense = bounds
        .cells()
        .map(|cell| amounts.get(&cell).copied().unwrap_or(0))
        .collect();
    map.reset_for_load(dense);
    let map_seed = world.resource::<ActiveLevel>().0.map_seed;
    let mut commands = world.commands();
    for (&cell, &amount) in &amounts {
        if amount > 0 {
            spawn_crystal_visual(
                &mut commands,
                &mut crystal_look(map_seed, cell),
                cell,
                amount,
            );
        }
    }
    world.flush();
    mined
}

fn restore_sides(world: &mut World, teams: &[Team], records: &[SideRecord]) {
    for (&team, record) in teams.iter().zip(records) {
        world.resource_mut::<Economies>().of_mut(team).credits = record.credits;
        let mut production = world.resource_mut::<Production>();
        for (lane, saved) in Lane::ALL.into_iter().zip(&record.lanes) {
            let lane = production.lane_mut(team, lane);
            lane.queue = saved.queue.iter().copied().collect();
            lane.progress_secs = saved.progress_secs;
            lane.ready = saved.ready;
        }
        for &upgrade in &record.researched {
            production.complete_research(team, upgrade);
        }
        let mut powers = world.resource_mut::<SupportPowers>();
        for (power, &secs) in SupportPower::ALL
            .into_iter()
            .zip(&record.support_recharge_secs)
        {
            powers.set_secs_until_ready(team, power, secs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT_FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/world_layout.txt");

    fn sample_world() -> WorldRecord {
        WorldRecord {
            version: STORE_VERSION,
            map_seed: 29,
            tick: 1_000_000,
            match_secs: 50_000.5,
            bounds: MapBounds {
                min: IVec2::new(-64, -64),
                size: IVec2::new(192, 192),
            },
            sides: vec![SideRecord {
                credits: 4_200,
                lanes: Lane::ALL
                    .map(|_| LaneRecord {
                        queue: vec![BuildItem::Structure(BuildingKind::Refinery)],
                        progress_secs: 2.5,
                        ready: None,
                    })
                    .to_vec(),
                researched: Upgrade::ALL.to_vec(),
                support_recharge_secs: vec![0.0, 33.0],
            }],
            retired_sides: vec![Team::from_index(9)],
            starting_sites: vec![IVec2::new(100, -20)],
            ai: AiCommanders::default(),
            hosting: HostingRecord {
                level_side_taken: true,
                players: vec![(
                    PlayerKey::from_saved("00112233445566778899aabbccddeeff").unwrap(),
                    Team::from_index(4),
                )],
                sites: vec![(Team::from_index(4), IVec2::new(100, -20))],
                away_since: vec![(Team::from_index(4), 77)],
                sheltered: vec![Team::from_index(4)],
                computer_sides: vec![(Team::Crimson, IVec2::new(54, 8))],
            },
            chunks: vec![(IVec2::new(-1, 2), "-1_2_77.bin".into())],
        }
    }

    fn sample_chunk() -> ChunkRecord {
        let unit: SavedUnit = serde_json::from_str(
            r#"{"team":"Side4","kind":"Tank","position":[1.5,0.0,-3.0],"facing":0.5,"health":300.0,
                "cargo":0,"orders":[],"experience":12.0,"stance":"HoldFire"}"#,
        )
        .unwrap();
        let building: SavedBuilding = serde_json::from_str(
            r#"{"team":"Crimson","kind":"PowerPlant","origin":[3,4],"health":500.0,"rally":[5,6],
                "repairing":true}"#,
        )
        .unwrap();
        ChunkRecord {
            crystal: vec![0, 1_500, 30],
            buildings: vec![building],
            units: vec![unit],
            mines: Vec::new(),
            unclaimed_sites: vec![(BuildingKind::SupplyDepot, IVec2::new(-30, 40))],
            explored: vec![(Team::Player, vec![0b1010_0101, 0xff])],
        }
    }

    fn layout_record() -> String {
        let hex = |bytes: Vec<u8>| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        format!(
            "layout {STORE_VERSION}\n{}\n{}\n",
            hex(encode(&sample_world()).unwrap()),
            hex(encode(&sample_chunk()).unwrap()),
        )
    }

    /// A change to what a saved world holds must come with a new `STORE_VERSION` and a way to read the layout
    /// before; re-record with `UPDATE_WORLD_LAYOUT=1`.
    #[test]
    fn the_layout_of_a_saved_world_changes_only_with_its_number() {
        let record = layout_record();
        if std::env::var_os("UPDATE_WORLD_LAYOUT").is_some() {
            std::fs::write(LAYOUT_FIXTURE, &record).unwrap();
            return;
        }
        let recorded = std::fs::read_to_string(LAYOUT_FIXTURE).unwrap_or_default();
        assert!(
            recorded == record,
            "what a saved world holds changed: raise STORE_VERSION, read the layout before in \
             read_world_record, then re-record with UPDATE_WORLD_LAYOUT=1 cargo test -p iron_tide_net \
             the_layout"
        );
    }

    #[test]
    fn a_world_of_another_layout_is_recognised_before_it_is_read() {
        let mut older = encode(&sample_world()).unwrap();
        older[0] = (STORE_VERSION - 1) as u8;
        let error = read_world_record(&older)
            .err()
            .expect("an older layout was read");
        assert!(error.to_string().contains("no longer reads"), "{error}");
        let mut newer = encode(&sample_world()).unwrap();
        newer[0] = (STORE_VERSION + 1) as u8;
        let error = read_world_record(&newer)
            .err()
            .expect("a newer layout was read");
        assert!(
            error.to_string().contains("newer than this server"),
            "{error}"
        );
        assert_eq!(
            read_world_record(&encode(&sample_world()).unwrap())
                .unwrap()
                .tick,
            1_000_000
        );
    }
}
