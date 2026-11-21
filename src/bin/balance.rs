//! Balance report: equal-cost unit duels and AI-against-AI tournaments, run headless.
//!
//! `cargo run --release --bin balance [-- --matches N] [--personalities]`

use std::collections::HashMap;
use std::sync::Mutex;

use bevy::prelude::*;
use iron_tide::catalog::{Team, UnitKind};
use iron_tide::combat::DamageSource;
use iron_tide::components::{Health, Order, Orders};
use iron_tide::economy::Economies;
use iron_tide::headless::{Battle, BattleSetup};
use iron_tide::levels::{BaseSite, Difficulty, LevelDefinition, Personality};
use iron_tide::map::GameMap;
use iron_tide::outcome::GameOutcome;

const COMBAT_UNITS: [UnitKind; 16] = [
    UnitKind::Rifleman,
    UnitKind::Rocketeer,
    UnitKind::Grenadier,
    UnitKind::Sniper,
    UnitKind::Scout,
    UnitKind::RaiderBike,
    UnitKind::Tank,
    UnitKind::StealthTank,
    UnitKind::Bulwark,
    UnitKind::Artillery,
    UnitKind::Gunship,
    UnitKind::Bomber,
    UnitKind::Fighter,
    UnitKind::FlakTrack,
    UnitKind::FlameTrooper,
    UnitKind::MissileTruck,
];
const DUEL_BUDGET: i32 = 3000;
const DUEL_TIME_LIMIT_SECS: f32 = 150.0;
const WEST_FRONT: IVec2 = IVec2::new(22, 46);
const EAST_FRONT: IVec2 = IVec2::new(38, 46);
const MATCH_TIME_LIMIT_SECS: f32 = 30.0 * 60.0;
const DEFAULT_MATCHES_PER_PAIRING: usize = 12;

fn fogless(level: LevelDefinition) -> LevelDefinition {
    LevelDefinition {
        fog_of_war: false,
        ..level
    }
}

fn army_value(battle: &mut Battle, team: Team) -> f32 {
    let world = battle.world();
    world
        .query::<(&iron_tide::components::Unit, &Team, &Health)>()
        .iter(world)
        .filter(|(_, owner, _)| **owner == team)
        .map(|(unit, _, health)| health.fraction() * unit.0.spec().cost as f32)
        .sum()
}

fn deploy_squad(battle: &mut Battle, kind: UnitKind, team: Team, front: IVec2, enemy_front: IVec2) {
    let count = (DUEL_BUDGET / kind.spec().cost).max(1);
    let columns = 4;
    for i in 0..count {
        let wanted = front + IVec2::new(i % columns - columns / 2, i / columns - 1);
        let cell = battle
            .world()
            .resource::<GameMap>()
            .nearest_passable(wanted)
            .unwrap_or(wanted);
        let unit = battle.spawn_unit(kind, team, cell);
        battle
            .world()
            .get_mut::<Orders>(unit)
            .unwrap()
            .replace_with(Order::AttackMove(enemy_front));
    }
}

/// Fights `attacker` against `defender`, each with the same budget; returns the attacker's margin in [-1, 1].
fn duel(attacker: UnitKind, defender: UnitKind, attacker_west: bool) -> f32 {
    let mut battle = Battle::new(BattleSetup {
        enemy_ai: false,
        starting_forces: false,
        level: Some(fogless(LevelDefinition::default())),
    });
    let (attacker_front, defender_front) = if attacker_west {
        (WEST_FRONT, EAST_FRONT)
    } else {
        (EAST_FRONT, WEST_FRONT)
    };
    deploy_squad(
        &mut battle,
        attacker,
        Team::Player,
        attacker_front,
        defender_front,
    );
    deploy_squad(
        &mut battle,
        defender,
        Team::Crimson,
        defender_front,
        attacker_front,
    );
    let fielded =
        |kind: UnitKind| ((DUEL_BUDGET / kind.spec().cost).max(1) * kind.spec().cost) as f32;
    battle.run_until(DUEL_TIME_LIMIT_SECS, |battle| {
        battle.units(Team::Player, None).is_empty() || battle.units(Team::Crimson, None).is_empty()
    });
    let attacker_left = army_value(&mut battle, Team::Player) / fielded(attacker);
    let defender_left = army_value(&mut battle, Team::Crimson) / fielded(defender);
    attacker_left - defender_left
}

#[derive(Default)]
struct UnitRecord {
    built: u32,
    lost: u32,
    damage: f32,
}

#[derive(Default)]
struct PairingResult {
    wins: u32,
    losses: u32,
    draws: u32,
    south_west_wins: u32,
    north_east_wins: u32,
    decided_secs: Vec<f32>,
    units: HashMap<UnitKind, UnitRecord>,
}

#[derive(Clone, Copy)]
struct Pairing {
    challenger: Difficulty,
    defender: Difficulty,
    fog_of_war: bool,
    challenger_style: Personality,
    defender_style: Personality,
}

const fn pairing(challenger: Difficulty, defender: Difficulty, fog_of_war: bool) -> Pairing {
    Pairing {
        challenger,
        defender,
        fog_of_war,
        challenger_style: Personality::Balanced,
        defender_style: Personality::Balanced,
    }
}

const fn style_against_balanced(style: Personality) -> Pairing {
    Pairing {
        challenger_style: style,
        ..pairing(Difficulty::Normal, Difficulty::Normal, true)
    }
}

const PERSONALITY_PAIRINGS: [Pairing; 4] = [
    style_against_balanced(Personality::Rusher),
    style_against_balanced(Personality::Fortress),
    style_against_balanced(Personality::Armoured),
    style_against_balanced(Personality::AirPower),
];

const PAIRINGS: [Pairing; 6] = [
    pairing(Difficulty::Normal, Difficulty::Normal, true),
    pairing(Difficulty::Hard, Difficulty::Normal, true),
    pairing(Difficulty::Normal, Difficulty::Hard, true),
    pairing(Difficulty::Easy, Difficulty::Normal, true),
    pairing(Difficulty::Brutal, Difficulty::Hard, true),
    pairing(Difficulty::Normal, Difficulty::Normal, false),
];

/// One full AI-against-AI match: the player's side is played by the computer too.
/// The simulation is deterministic, so every match needs its own map seed to be more than a rerun.
fn ai_match(
    pairing: Pairing,
    challenger_site: BaseSite,
    map_seed: u64,
) -> (GameOutcome, f32, HashMap<UnitKind, UnitRecord>) {
    let Pairing {
        challenger,
        defender,
        fog_of_war,
        challenger_style,
        defender_style,
    } = pairing;
    let defender_site = match challenger_site {
        BaseSite::SouthWest => BaseSite::NorthEast,
        _ => BaseSite::SouthWest,
    };
    let mut level = LevelDefinition {
        fog_of_war,
        map_seed,
        ..LevelDefinition::default()
    };
    level.player.site = challenger_site;
    level.enemies[0].force.site = defender_site;
    level.enemies[0].difficulty = defender;
    level.enemies[0].personality = defender_style;
    let mut battle = Battle::new(BattleSetup {
        enemy_ai: true,
        starting_forces: true,
        level: Some(level),
    });
    battle.add_ai_with(
        Team::Player,
        challenger_site.origin(),
        challenger,
        challenger_style,
    );
    battle
        .world()
        .resource_mut::<Economies>()
        .of_mut(Team::Player)
        .income_rate = challenger.income_rate();
    let took = battle.run_until(MATCH_TIME_LIMIT_SECS, |battle| {
        battle.outcome() != GameOutcome::Ongoing
    });

    let ledger = battle.ledger();
    let mut units: HashMap<UnitKind, UnitRecord> = HashMap::new();
    for (&(_, kind), &count) in &ledger.recruited {
        units.entry(kind).or_default().built += count;
    }
    for (&(_, what), &count) in &ledger.lost {
        if let DamageSource::Unit(kind) = what {
            units.entry(kind).or_default().lost += count;
        }
    }
    for (&(_, source), &amount) in &ledger.dealt {
        if let DamageSource::Unit(kind) = source {
            units.entry(kind).or_default().damage += amount;
        }
    }
    (
        battle.outcome(),
        took.unwrap_or(MATCH_TIME_LIMIT_SECS),
        units,
    )
}

/// Map variants as the battle editor offers them; both sides of a pairing play each one.
fn map_seed_of(variant: usize) -> u64 {
    LevelDefinition::default().map_seed + variant as u64 * 7919
}

fn parallel<T: Send, R: Send>(jobs: Vec<T>, run: impl Fn(T) -> R + Sync) -> Vec<R> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    let queue = Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>());
    let results = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let Some((index, job)) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let result = run(job);
                    results.lock().unwrap().push((index, result));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

fn print_duel_matrix() {
    println!("## Equal-cost duels (${DUEL_BUDGET} a side, fog off)\n");
    println!(
        "Row unit attacking column unit; remaining army value of the row minus the column, both sides of the field averaged."
    );
    println!(
        "+100 = the row wins untouched, -100 = the row is wiped out without loss to the column.\n"
    );
    let pairs: Vec<(UnitKind, UnitKind, bool)> = COMBAT_UNITS
        .iter()
        .flat_map(|&a| {
            COMBAT_UNITS
                .iter()
                .flat_map(move |&b| [(a, b, true), (a, b, false)])
        })
        .collect();
    let margins = parallel(pairs.clone(), |(a, b, west)| duel(a, b, west));
    let mut table: HashMap<(UnitKind, UnitKind), Vec<f32>> = HashMap::new();
    for ((a, b, _), margin) in pairs.into_iter().zip(margins) {
        table.entry((a, b)).or_default().push(margin);
    }
    print!("| vs |");
    for b in COMBAT_UNITS {
        print!(" {} |", b.spec().name);
    }
    println!("\n|---|{}", "---|".repeat(COMBAT_UNITS.len()));
    for a in COMBAT_UNITS {
        print!("| **{}** |", a.spec().name);
        for b in COMBAT_UNITS {
            let runs = &table[&(a, b)];
            let mean = runs.iter().sum::<f32>() / runs.len() as f32;
            print!(" {:+.0} |", mean * 100.0);
        }
        println!();
    }
    println!();
}

fn print_tournament(matches: usize, pairings: &[Pairing]) {
    println!(
        "## AI against AI ({matches} matches per pairing, {:.0} min limit)\n",
        MATCH_TIME_LIMIT_SECS / 60.0
    );
    let sides = [BaseSite::SouthWest, BaseSite::NorthEast];
    let jobs: Vec<(usize, Pairing, BaseSite, u64)> = pairings
        .iter()
        .enumerate()
        .flat_map(|(index, &pairing)| {
            (0..matches).map(move |n| (index, pairing, sides[n % 2], map_seed_of(n / 2)))
        })
        .collect();
    let outcomes = parallel(jobs.clone(), |(_, pairing, site, seed)| {
        ai_match(pairing, site, seed)
    });
    let mut results: Vec<PairingResult> =
        pairings.iter().map(|_| PairingResult::default()).collect();
    for ((index, _, site, _), (outcome, secs, units)) in jobs.into_iter().zip(outcomes) {
        let result = &mut results[index];
        let challenger_in_south_west = site == BaseSite::SouthWest;
        match (outcome, challenger_in_south_west) {
            (GameOutcome::Victory, true) | (GameOutcome::Defeat, false) => {
                result.south_west_wins += 1
            }
            (GameOutcome::Victory, false) | (GameOutcome::Defeat, true) => {
                result.north_east_wins += 1
            }
            (GameOutcome::Ongoing, _) => {}
        }
        match outcome {
            GameOutcome::Victory => result.wins += 1,
            GameOutcome::Defeat => result.losses += 1,
            GameOutcome::Ongoing => result.draws += 1,
        }
        if outcome != GameOutcome::Ongoing {
            result.decided_secs.push(secs);
        }
        for (kind, record) in units {
            let total = result.units.entry(kind).or_default();
            total.built += record.built;
            total.lost += record.lost;
            total.damage += record.damage;
        }
    }

    println!(
        "Sides alternate every match; the last column shows whether a base site is favoured.\n"
    );
    println!(
        "| Challenger | vs defender | fog | wins | losses | unfinished | median length | SW : NE site wins |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    for (pairing, result) in pairings.iter().zip(&results) {
        let a = format!("{:?} {:?}", pairing.challenger, pairing.challenger_style);
        let b = format!("{:?} {:?}", pairing.defender, pairing.defender_style);
        let fog = if pairing.fog_of_war { "on" } else { "off" };
        let mut lengths = result.decided_secs.clone();
        lengths.sort_by(f32::total_cmp);
        let median = lengths
            .get(lengths.len() / 2)
            .map_or("-".to_string(), |secs| format!("{:.1} min", secs / 60.0));
        println!(
            "| {a} | {b} | {fog} | {} | {} | {} | {median} | {} : {} |",
            result.wins,
            result.losses,
            result.draws,
            result.south_west_wins,
            result.north_east_wins
        );
    }

    println!("\n### What the AIs built and how it paid off (all pairings, both sides)\n");
    println!("| Unit | built | lost | damage dealt | damage per credit |");
    println!("|---|---|---|---|---|");
    let mut combined: HashMap<UnitKind, UnitRecord> = HashMap::new();
    for result in &results {
        for (&kind, record) in &result.units {
            let total = combined.entry(kind).or_default();
            total.built += record.built;
            total.lost += record.lost;
            total.damage += record.damage;
        }
    }
    for kind in UnitKind::ALL {
        let Some(record) = combined.get(&kind) else {
            continue;
        };
        let spent = (record.built as f32 * kind.spec().cost as f32).max(1.0);
        println!(
            "| {} | {} | {} | {:.0} | {:.2} |",
            kind.spec().name,
            record.built,
            record.lost,
            record.damage,
            record.damage / spent
        );
    }
    println!();
}

fn main() {
    let matches = std::env::args()
        .skip_while(|arg| arg != "--matches")
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_MATCHES_PER_PAIRING);
    let started = std::time::Instant::now();
    println!("# Iron Tide balance report\n");
    if std::env::args().any(|arg| arg == "--personalities") {
        print_tournament(matches, &PERSONALITY_PAIRINGS);
    } else {
        print_duel_matrix();
        print_tournament(matches, &PAIRINGS);
    }
    eprintln!("finished in {:.0} s", started.elapsed().as_secs_f32());
}
