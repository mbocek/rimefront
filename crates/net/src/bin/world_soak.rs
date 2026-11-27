//! A persistent world run as fast as it goes, with computer-driven players who enter and leave it, to find
//! what grows or slows over days of world time:
//! `rimefront-world-soak [--players 20] [--days 10] [--chance N] [--seed MAP_SEED] [--difficulty normal]
//! [--dir DIR] [--out soak.csv] [--era-hours 168] [--first-era antiquity]`. `--chance` picks the map, when the players come and go, and how hard and
//! in what manner each one plays; without it a run picks its own and prints it, so a run can be repeated.
//! Writes a line of measurements for every hour of world time; see docs/persistent-world.md.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use rimefront_net::link::encode;
use rimefront_net::network::{Arrival, Message};
use rimefront_net::world_host::{
    LocalConnection, PlayerKey, WORLD_PROTOCOL, WorldConnection, WorldHost, local_connection,
};
use rimefront_net::world_store::WorldStore;
use rimefront_sim::SIMULATION_TICK;
use rimefront_sim::levels::ActiveLevel;
use rimefront_sim::levels::{Difficulty, Personality};
use rimefront_sim::sim::ai::{AiCommander, AiCommanders};
use rimefront_sim::sim::catalog::{BuildingKind, Team};
use rimefront_sim::sim::components::{Building, Unit};
use rimefront_sim::sim::eras::{AgeSchedule, Era};
use rimefront_sim::sim::sides::side_stands;
use rimefront_sim::world::map::{CrystalVisual, map_bounds, world_to_cell};

const TICKS_PER_SECOND: u64 = 1000 / SIMULATION_TICK.as_millis() as u64;
const TICKS_PER_MINUTE: u64 = 60 * TICKS_PER_SECOND;
const TICKS_PER_HOUR: u64 = 60 * TICKS_PER_MINUTE;
const TICKS_PER_DAY: u64 = 24 * TICKS_PER_HOUR;
const SAVE_EVERY_TICKS: u64 = 30 * TICKS_PER_SECOND;
/// The tick budget of a world at 20 ticks a second.
const TICK_BUDGET: Duration = Duration::from_millis(50);
const SLOW_TICK: Duration = Duration::from_millis(20);
/// Keeps the host's own saving out of the way, so every save is the soak's and is timed.
const NEVER: Duration = Duration::from_secs(100 * 365 * 24 * 3600);

fn argument_after<T: std::str::FromStr>(flag: &str) -> Option<T> {
    let arguments: Vec<String> = std::env::args().collect();
    arguments
        .iter()
        .position(|argument| argument == flag)
        .and_then(|at| arguments.get(at + 1))
        .and_then(|value| value.parse().ok())
}

fn difficulty_named(name: &str) -> Option<Difficulty> {
    Difficulty::ALL
        .into_iter()
        .find(|difficulty| format!("{difficulty:?}").eq_ignore_ascii_case(name))
}

/// A chance of its own for a run not given one.
fn fresh_chance() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos() as u64)
}

/// A player whose side a computer commander plays while they are connected, and who comes and goes: mostly
/// for hours, now and then for days, and once in a while long enough for their base to decay.
struct SoakPlayer {
    key: PlayerKey,
    difficulty: Difficulty,
    personality: Personality,
    connection: Option<LocalConnection>,
    team: Option<Team>,
    next_change: u64,
    bytes: usize,
    falls: u32,
}

impl SoakPlayer {
    fn new(rng: &mut StdRng, difficulty: Option<Difficulty>) -> Self {
        let any_difficulty = Difficulty::ALL[rng.random_range(0..Difficulty::ALL.len())];
        Self {
            key: PlayerKey::new_random(),
            difficulty: difficulty.unwrap_or(any_difficulty),
            personality: Personality::ALL[rng.random_range(0..Personality::ALL.len())],
            connection: None,
            team: None,
            next_change: rng.random_range(0..TICKS_PER_HOUR),
            bytes: 0,
            falls: 0,
        }
    }

    fn enter(&mut self, host: &mut WorldHost) {
        let (player, world_end) = local_connection();
        host.connect(Box::new(world_end));
        player.send(&Message::EnterWorld {
            version: env!("CARGO_PKG_VERSION").into(),
            protocol: WORLD_PROTOCOL,
            player: self.key.clone(),
            color: None,
        });
        self.connection = Some(player);
        self.team = None;
    }

    fn leave(&mut self, host: &mut WorldHost) {
        if let Some(team) = self.team.take() {
            hand_back_side(host, team);
        }
        self.connection = None;
    }

    fn time_online(rng: &mut StdRng) -> u64 {
        rng.random_range(TICKS_PER_HOUR..6 * TICKS_PER_HOUR)
    }

    fn time_away(rng: &mut StdRng) -> u64 {
        match rng.random_range(0..10) {
            0 => rng.random_range(3 * TICKS_PER_DAY..6 * TICKS_PER_DAY),
            1 | 2 => rng.random_range(12 * TICKS_PER_HOUR..2 * TICKS_PER_DAY),
            _ => rng.random_range(TICKS_PER_HOUR..12 * TICKS_PER_HOUR),
        }
    }

    fn follow_schedule(&mut self, host: &mut WorldHost, rng: &mut StdRng) {
        let tick = host.tick();
        if tick < self.next_change {
            return;
        }
        if self.connection.is_some() {
            self.leave(host);
            self.next_change = tick + Self::time_away(rng);
        } else {
            self.enter(host);
            self.next_change = tick + Self::time_online(rng);
        }
    }

    /// Reads what the world sent; once it lets the player in, a commander takes over their side.
    fn read_views(&mut self, host: &mut WorldHost) {
        let Some(connection) = &mut self.connection else {
            return;
        };
        for arrival in connection.drain() {
            let Arrival::Message(message) = arrival else {
                continue;
            };
            self.bytes += encode(&message).map_or(0, |bytes| bytes.len());
            if let Message::Entered { team, .. } = message {
                self.team = Some(team);
                let commander = AiCommander::new(team, IVec2::ZERO, self.difficulty)
                    .with_personality(self.personality);
                take_over_side(host, commander);
            }
        }
    }

    /// A player whose side fell leaves for a while and enters again later, to a new start.
    fn leave_if_fallen(&mut self, host: &mut WorldHost, rng: &mut StdRng) {
        let Some(team) = self.team else { return };
        if side_stands(host.world(), team) {
            return;
        }
        self.falls += 1;
        self.leave(host);
        self.next_change =
            host.tick() + rng.random_range(10 * TICKS_PER_MINUTE..2 * TICKS_PER_HOUR);
    }
}

/// Hands the side to `commander`, based where the side has its Command Center or else its first unit.
fn take_over_side(host: &mut WorldHost, mut commander: AiCommander) {
    let team = commander.team;
    let world = host.world();
    let base = world
        .query::<(&Building, &Team)>()
        .iter(world)
        .filter(|(_, owner)| **owner == team)
        .min_by_key(|(building, _)| building.kind != BuildingKind::CommandCenter)
        .map(|(building, _)| building.origin)
        .or_else(|| {
            world
                .query_filtered::<(&Transform, &Team), With<Unit>>()
                .iter(world)
                .find(|(_, owner)| **owner == team)
                .map(|(transform, _)| world_to_cell(transform.translation))
        });
    let Some(base) = base else { return };
    let mut commanders = world.resource_mut::<AiCommanders>();
    if commanders.0.iter().all(|commander| commander.team != team) {
        commander.base = base;
        commanders.0.push(commander);
    }
}

fn hand_back_side(host: &mut WorldHost, team: Team) {
    host.world()
        .resource_mut::<AiCommanders>()
        .0
        .retain(|commander| commander.team != team);
}

/// What an hour of world time cost and left behind.
#[derive(Default)]
struct HourOfWorld {
    ticks: u64,
    stepping: Duration,
    slowest: Duration,
    slow_ticks: u64,
    over_budget: u64,
    saves: u32,
    saving: Duration,
    slowest_save: Duration,
    bytes_saved: usize,
}

impl HourOfWorld {
    fn note_tick(&mut self, took: Duration) {
        self.ticks += 1;
        self.stepping += took;
        self.slowest = self.slowest.max(took);
        self.slow_ticks += u64::from(took > SLOW_TICK);
        self.over_budget += u64::from(took > TICK_BUDGET);
    }

    fn note_save(&mut self, took: Duration, bytes: usize) {
        self.saves += 1;
        self.saving += took;
        self.slowest_save = self.slowest_save.max(took);
        self.bytes_saved += bytes;
    }
}

const CSV_HEADER: &str = "world_hours,wall_secs,tick_avg_ms,tick_max_ms,ticks_over_20ms,ticks_over_50ms,\
units,structures,entities,sides_standing,players_online,player_falls,world_cells,crystal_cells,\
bytes_per_player_s,save_avg_ms,save_max_ms,saved_kb,store_mb,peak_rss_mb,era";

struct WorldCensus {
    units: usize,
    structures: usize,
    entities: usize,
    sides_standing: usize,
    crystal_cells: usize,
}

fn census(host: &mut WorldHost) -> WorldCensus {
    let world = host.world();
    let mut sides: Vec<Team> = world
        .query_filtered::<&Team, Or<(With<Unit>, With<Building>)>>()
        .iter(world)
        .copied()
        .collect();
    sides.sort_by_key(|team| team.index());
    sides.dedup();
    WorldCensus {
        units: world.query_filtered::<(), With<Unit>>().iter(world).count(),
        structures: world
            .query_filtered::<(), With<Building>>()
            .iter(world)
            .count(),
        entities: world.query::<Entity>().iter(world).count(),
        sides_standing: sides.len(),
        crystal_cells: world
            .query_filtered::<(), With<CrystalVisual>>()
            .iter(world)
            .count(),
    }
}

fn directory_bytes(directory: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(metadata) if metadata.is_dir() => directory_bytes(&entry.path()),
            Ok(metadata) => metadata.len(),
            Err(_) => 0,
        })
        .sum()
}

/// The most memory this process has held so far, as the system counts it.
fn peak_resident_megabytes() -> f64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage only writes the struct it is given.
    let usage = unsafe {
        libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr());
        usage.assume_init()
    };
    let bytes_per_unit = if cfg!(target_os = "macos") {
        1.0
    } else {
        1024.0
    };
    usage.ru_maxrss as f64 * bytes_per_unit / (1024.0 * 1024.0)
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn main() -> std::io::Result<()> {
    let players = argument_after::<usize>("--players").unwrap_or(20);
    let days = argument_after::<f64>("--days").unwrap_or(10.0);
    let chance = argument_after::<u64>("--chance").unwrap_or_else(fresh_chance);
    let mut rng = StdRng::seed_from_u64(chance);
    let seed = argument_after::<u64>("--seed").unwrap_or_else(|| rng.random_range(0..1_000_000));
    let difficulty =
        argument_after::<String>("--difficulty").and_then(|name| difficulty_named(&name));
    let directory = argument_after::<PathBuf>("--dir")
        .unwrap_or_else(|| std::env::temp_dir().join(format!("rimefront-soak-{seed}")));
    let out = argument_after::<PathBuf>("--out").unwrap_or_else(|| PathBuf::from("soak.csv"));
    let last_tick = (days * TICKS_PER_DAY as f64) as u64;

    if directory.exists() {
        std::fs::remove_dir_all(&directory)?;
    }
    let first = argument_after::<String>("--first-era")
        .and_then(|name| Era::named(&name))
        .unwrap_or(Era::Antiquity);
    let era_hours = argument_after::<u32>("--era-hours").unwrap_or(7 * 24);
    let ages = AgeSchedule {
        first,
        era_hours,
        finale_hours: era_hours,
        turn_hour: 20,
    };
    let mut host = WorldHost::with_ages(seed, Some(ages));
    host.keep_in(WorldStore::at(&directory), NEVER)?;
    let mut soak_players: Vec<SoakPlayer> = (0..players)
        .map(|_| SoakPlayer::new(&mut rng, difficulty))
        .collect();
    let mut csv = BufWriter::new(File::create(&out)?);
    writeln!(csv, "{CSV_HEADER}")?;
    eprintln!(
        "Soaking world {seed} with {players} players for {days} days (--chance {chance}); \
         saving in {}, measuring into {}",
        directory.display(),
        out.display()
    );
    for (index, player) in soak_players.iter().enumerate() {
        eprintln!(
            "player {index}: {:?}, {:?}",
            player.difficulty, player.personality
        );
    }

    let started = Instant::now();
    let mut hour = HourOfWorld::default();
    let mut bytes_counted_from: HashMap<usize, usize> = HashMap::new();
    while host.tick() < last_tick {
        for player in &mut soak_players {
            player.follow_schedule(&mut host, &mut rng);
        }
        let before = host.tick();
        let step = Instant::now();
        host.step(SIMULATION_TICK);
        hour.note_tick(step.elapsed());
        for player in &mut soak_players {
            player.read_views(&mut host);
        }
        let tick = host.tick();
        let crossed = |every: u64| tick / every != before / every;
        if crossed(TICKS_PER_MINUTE) {
            for player in &mut soak_players {
                player.leave_if_fallen(&mut host, &mut rng);
            }
        }
        if crossed(SAVE_EVERY_TICKS) {
            let save = Instant::now();
            let report = host.save_in_background()?;
            hour.note_save(save.elapsed(), report.bytes_written);
        }
        if crossed(TICKS_PER_HOUR) {
            let world_hours = tick / TICKS_PER_HOUR;
            let online = soak_players
                .iter()
                .filter(|player| player.connection.is_some())
                .count();
            let bytes: usize = soak_players
                .iter()
                .enumerate()
                .map(|(index, player)| {
                    let before = bytes_counted_from.insert(index, player.bytes).unwrap_or(0);
                    player.bytes - before
                })
                .sum();
            let falls: u32 = soak_players.iter().map(|player| player.falls).sum();
            let counted = census(&mut host);
            let bounds = map_bounds().size;
            let line = format!(
                "{world_hours},{:.0},{:.3},{:.2},{},{},{},{},{},{},{online},{falls},{},{},{:.0},{:.2},{:.2},{:.0},{:.2},{:.0},{:?}",
                started.elapsed().as_secs_f64(),
                millis(hour.stepping) / hour.ticks.max(1) as f64,
                millis(hour.slowest),
                hour.slow_ticks,
                hour.over_budget,
                counted.units,
                counted.structures,
                counted.entities,
                counted.sides_standing,
                bounds.x * bounds.y,
                counted.crystal_cells,
                bytes as f64 / online.max(1) as f64 / 3600.0,
                millis(hour.saving) / f64::from(hour.saves.max(1)),
                millis(hour.slowest_save),
                hour.bytes_saved as f64 / 1024.0,
                directory_bytes(&directory) as f64 / (1024.0 * 1024.0),
                peak_resident_megabytes(),
                host.world().resource::<ActiveLevel>().0.era,
            );
            writeln!(csv, "{line}")?;
            csv.flush()?;
            eprintln!(
                "day {:.2}: tick {:.2} ms avg, {:.1} ms max; {} units, {} structures, {} sides, \
                 {online} online; {:.0} s so far",
                world_hours as f64 / 24.0,
                millis(hour.stepping) / hour.ticks.max(1) as f64,
                millis(hour.slowest),
                counted.units,
                counted.structures,
                counted.sides_standing,
                started.elapsed().as_secs_f64(),
            );
            hour = HourOfWorld::default();
        }
    }
    Ok(())
}
