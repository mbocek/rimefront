use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::catalog::{Alliance, Faction};
use crate::i18n::{tr, tr_fmt};
use crate::levels::{
    BaseSite, Difficulty, EnemySetup, ForceSetup, LevelDefinition, Objective, Personality, Weather,
};
use crate::map::{MapSize, map_bounds, near_base, near_crystal_field, opposite_cell, set_map_size};
use crate::map_edits::{Brush, MapEdit};
use crate::season::Season;
use crate::terrain::{Landscape, set_terrain_seed};

pub const CREDIT_CHOICES: [i32; 5] = [3000, 5000, 8000, 12_000, 20_000];
const FOREST_CLUMPS: usize = 6;
const ROCK_CLUMPS: usize = 5;
const EXTRA_CRYSTAL_FIELDS: usize = 2;
const RAIN_CHANCE: f64 = 0.25;
/// Clumps stay this far inside the map edge, where the mountains begin.
const EDGE_MARGIN_CELLS: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DifficultyChoice {
    Fixed(Difficulty),
    /// Every opponent gets its own difficulty at random.
    Mixed,
}

impl DifficultyChoice {
    pub const ALL: [DifficultyChoice; 5] = [
        DifficultyChoice::Fixed(Difficulty::Easy),
        DifficultyChoice::Fixed(Difficulty::Normal),
        DifficultyChoice::Fixed(Difficulty::Hard),
        DifficultyChoice::Fixed(Difficulty::Brutal),
        DifficultyChoice::Mixed,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FactionChoice {
    Fixed(Faction),
    Random,
}

/// What the player decides about a random battle; the dice decide the rest.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RandomBattleOptions {
    pub opponents: usize,
    pub difficulty: DifficultyChoice,
    pub map_size: MapSize,
    pub credits: i32,
    pub faction: FactionChoice,
}

impl Default for RandomBattleOptions {
    fn default() -> Self {
        Self {
            opponents: 1,
            difficulty: DifficultyChoice::Fixed(Difficulty::Normal),
            map_size: MapSize::Standard,
            credits: 5000,
            faction: FactionChoice::Fixed(Faction::Coalition),
        }
    }
}

/// Separates the dice for the sides from those for the ground, so changing a side never moves a forest.
const SIDES_DICE: u64 = 0x5eed_51de;
/// The commanders' personalities have their own dice too, so adding them changed no map or side.
const PERSONALITY_DICE: u64 = 0x5717_e5ed;

/// The same options and seed always give the same battle, so a map can be kept while the credits change.
pub fn generate(options: &RandomBattleOptions, seed: u64) -> LevelDefinition {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut sides = StdRng::seed_from_u64(seed ^ SIDES_DICE);
    let mut personalities = StdRng::seed_from_u64(seed ^ PERSONALITY_DICE);
    let landscape = Landscape::ALL[rng.random_range(0..Landscape::ALL.len())];
    let season = Season::ALL[rng.random_range(0..Season::ALL.len())];
    let weather = if rng.random_bool(RAIN_CHANCE) {
        Weather::Rain
    } else {
        Weather::Clear
    };
    let map_edits = scatter_clumps(&mut rng, options.map_size, seed);
    let mut corners = BaseSite::ALL.to_vec();
    shuffle(&mut corners, &mut sides);
    let opponents = options.opponents.clamp(1, 3);
    let player_faction = match options.faction {
        FactionChoice::Fixed(faction) => faction,
        FactionChoice::Random if sides.random_bool(0.5) => Faction::Syndicate,
        FactionChoice::Random => Faction::Coalition,
    };
    let alliance = match opponents {
        1 => Alliance::CoalitionAgainstSyndicate,
        2 => pick(
            &mut sides,
            &[Alliance::CoalitionAgainstSyndicate, Alliance::FreeForAll],
        ),
        _ => pick(&mut sides, &Alliance::ALL),
    };
    let enemies = corners[1..=opponents]
        .iter()
        .map(|&site| EnemySetup {
            force: ForceSetup {
                site,
                credits: options.credits,
                ..default()
            },
            difficulty: match options.difficulty {
                DifficultyChoice::Fixed(difficulty) => difficulty,
                DifficultyChoice::Mixed => pick(
                    &mut sides,
                    &[Difficulty::Easy, Difficulty::Normal, Difficulty::Hard],
                ),
            },
            personality: pick(&mut personalities, &Personality::ALL),
        })
        .collect();
    LevelDefinition {
        name: tr_fmt("Random battle {}", &[&format!("{:04}", seed % 10_000)]),
        briefing: tr("A battlefield rolled by the dice. Destroy every enemy.").to_string(),
        map_seed: seed,
        player: ForceSetup {
            site: corners[0],
            credits: options.credits,
            ..default()
        },
        enemies,
        objective: Objective::DestroyAllEnemies,
        alliance,
        landscape,
        weather,
        season,
        map_size: options.map_size,
        map_edits,
        player_faction,
        terrain_seed: Some(seed),
        events: Vec::new(),
        neutral_sites: true,
        ..default()
    }
}

fn pick<T: Copy>(rng: &mut StdRng, choices: &[T]) -> T {
    choices[rng.random_range(0..choices.len())]
}

fn shuffle<T>(items: &mut [T], rng: &mut StdRng) {
    for index in (1..items.len()).rev() {
        items.swap(index, rng.random_range(0..=index));
    }
}

/// Forests, rock outcrops and extra crystal fields, each with a twin in the opposite half so neither corner
/// is favoured; the map editor's painter later skips water and base sites.
fn scatter_clumps(rng: &mut StdRng, map_size: MapSize, terrain_seed: u64) -> Vec<MapEdit> {
    set_map_size(map_size);
    set_terrain_seed(Some(terrain_seed));
    let bounds = map_bounds();
    let spread = bounds.spread();
    let by_area = |count: usize| (count as f32 * spread.x * spread.y).round() as usize;
    let mut edits = Vec::new();
    for (brush, clumps, radius) in [
        (Brush::Forest, by_area(FOREST_CLUMPS), 1..4),
        (Brush::Rock, by_area(ROCK_CLUMPS), 1..3),
        (Brush::Crystal, EXTRA_CRYSTAL_FIELDS, 2..4),
    ] {
        for _ in 0..clumps {
            let center = IVec2::new(
                rng.random_range(
                    bounds.min.x + EDGE_MARGIN_CELLS
                        ..bounds.min.x + bounds.size.x - EDGE_MARGIN_CELLS,
                ),
                rng.random_range(
                    bounds.min.y + EDGE_MARGIN_CELLS
                        ..bounds.min.y + bounds.size.y - EDGE_MARGIN_CELLS,
                ),
            );
            let radius = rng.random_range(radius.clone());
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let cell = center + IVec2::new(dx, dy);
                    let ragged_edge = (dx * dx + dy * dy) as f32
                        > (radius * radius) as f32 * rng.random_range(0.6..1.2);
                    if ragged_edge || near_base(cell) || near_crystal_field(cell) {
                        continue;
                    }
                    edits.push(MapEdit { cell, brush });
                    edits.push(MapEdit {
                        cell: opposite_cell(cell),
                        brush,
                    });
                }
            }
        }
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_rolls_the_same_battle_and_another_one_differs() {
        let options = RandomBattleOptions::default();
        assert_eq!(generate(&options, 7), generate(&options, 7));
        assert_ne!(
            generate(&options, 7).map_edits,
            generate(&options, 8).map_edits
        );
    }

    #[test]
    fn a_rolled_battle_is_playable_with_the_chosen_sides_and_credits() {
        for seed in 0..40 {
            let options = RandomBattleOptions {
                opponents: 1 + seed as usize % 3,
                credits: 12_000,
                map_size: MapSize::ALL[seed as usize % MapSize::ALL.len()],
                ..default()
            };
            let level = generate(&options, seed);
            assert!(
                level.problems().is_empty(),
                "seed {seed}: {:?}",
                level.problems()
            );
            assert_eq!(level.enemies.len(), options.opponents);
            assert!(
                level
                    .enemies
                    .iter()
                    .all(|enemy| enemy.force.credits == 12_000)
            );
            assert_eq!(level.player.credits, 12_000);
        }
    }

    #[test]
    fn changing_the_credits_keeps_the_map() {
        let poor = generate(&RandomBattleOptions::default(), 3);
        let rich = generate(
            &RandomBattleOptions {
                credits: 20_000,
                ..default()
            },
            3,
        );
        assert_eq!(poor.map_edits, rich.map_edits);
        let mixed = generate(
            &RandomBattleOptions {
                difficulty: DifficultyChoice::Mixed,
                opponents: 3,
                ..default()
            },
            3,
        );
        assert_eq!(poor.map_edits, mixed.map_edits);
        assert_eq!(poor.landscape, rich.landscape);
    }
}
