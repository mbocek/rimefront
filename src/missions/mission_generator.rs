use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::app::i18n::{tr, tr_fmt};
use crate::missions::levels::{Difficulty, LevelDefinition, Objective};
use crate::missions::random_battle::{self, DifficultyChoice, FactionChoice, RandomBattleOptions};
use crate::sim::catalog::Faction;

/// Keeps the generator's dice apart from the map's, which also start from the seed.
const MISSION_DICE: u64 = 0x0b1e_c71e;

/// Code names stay in English in every language, as code names do; translated words would not agree in gender.
const OPERATION_ADJECTIVES: [&str; 12] = [
    "Iron",
    "Silent",
    "Crimson",
    "Broken",
    "Northern",
    "Burning",
    "Hollow",
    "Grey",
    "Shattered",
    "Last",
    "Falling",
    "Steel",
];
const OPERATION_NOUNS: [&str; 12] = [
    "Dawn", "Anvil", "Harvest", "Crossing", "Ridge", "Lantern", "Spear", "Tide", "Bastion",
    "Ember", "Gate", "Hammer",
];

/// A generated mission: the battle is rolled like a random battle, then given one of the objectives a
/// campaign mission can have, an operation name and a briefing written for that objective.
pub fn generate_mission(
    seed: u64,
    faction: FactionChoice,
    difficulty: Difficulty,
) -> LevelDefinition {
    let options = RandomBattleOptions {
        faction,
        difficulty: DifficultyChoice::Fixed(difficulty),
        ..RandomBattleOptions::default()
    };
    let mut level = random_battle::generate(&options, seed);
    let mut rng = StdRng::seed_from_u64(seed ^ MISSION_DICE);
    let objective = match rng.random_range(0..5) {
        0 => Objective::DestroyAllEnemies,
        1 => Objective::DestroyCommandCenters,
        2 => Objective::CaptureCommandCenter,
        3 => Objective::Survive {
            minutes: rng.random_range(8..=14) as f32,
        },
        _ => Objective::ReachCredits {
            credits: rng.random_range(6..=12) * 1000,
        },
    };
    let adjective = OPERATION_ADJECTIVES[rng.random_range(0..OPERATION_ADJECTIVES.len())];
    let noun = OPERATION_NOUNS[rng.random_range(0..OPERATION_NOUNS.len())];
    level.name = tr_fmt("Operation {} {}", &[&adjective, &noun]);
    level.briefing = briefing(objective, level.player_faction);
    level.objective = objective;
    level
}

fn briefing(objective: Objective, faction: Faction) -> String {
    let enemy = tr(match faction.opposite() {
        Faction::Coalition => "Coalition",
        Faction::Syndicate => "Syndicate",
    });
    match objective {
        Objective::DestroyAllEnemies => tr_fmt(
            "The {} has dug in across the valley. Build up, break through and leave nothing of theirs standing.",
            &[&enemy],
        ),
        Objective::DestroyCommandCenters => tr_fmt(
            "Intelligence has found the {} headquarters. Cut off the head: destroy every enemy Command Center.",
            &[&enemy],
        ),
        Objective::CaptureCommandCenter => tr_fmt(
            "We need the {} command network intact. Fight your way in and capture their Command Center with an engineer.",
            &[&enemy],
        ),
        Objective::Survive { minutes } => tr_fmt(
            "Reinforcements are {} minutes out. The {} will throw everything at you before then: hold the base.",
            &[&minutes, &enemy],
        ),
        Objective::ReachCredits { credits } => tr_fmt(
            "Command wants ${} in the war chest. Mine the fields hard and keep the {} away from the harvesters.",
            &[&credits, &enemy],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_always_gives_the_same_mission_and_seeds_vary_the_objectives() {
        let one = generate_mission(
            7,
            FactionChoice::Fixed(Faction::Coalition),
            Difficulty::Normal,
        );
        let again = generate_mission(
            7,
            FactionChoice::Fixed(Faction::Coalition),
            Difficulty::Normal,
        );
        assert_eq!(one, again);
        let objectives: Vec<_> = (0..40)
            .map(|seed| {
                std::mem::discriminant(
                    &generate_mission(
                        seed,
                        FactionChoice::Fixed(Faction::Coalition),
                        Difficulty::Normal,
                    )
                    .objective,
                )
            })
            .collect();
        let kinds = objectives
            .iter()
            .enumerate()
            .filter(|(index, kind)| !objectives[..*index].contains(kind))
            .count();
        assert_eq!(kinds, 5, "the generator should use every objective");
    }

    #[test]
    fn generated_missions_are_playable_levels() {
        for seed in 0..20 {
            let level = generate_mission(seed, FactionChoice::Random, Difficulty::Hard);
            assert!(
                level.problems().is_empty(),
                "seed {seed}: {:?}",
                level.problems()
            );
            assert!(level.name.starts_with("Operation"));
        }
    }
}
