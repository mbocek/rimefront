//! Every machine must compute the same battle. This replays a fixed computer-against-computer battle and
//! compares its fingerprints with ones recorded on another kind of computer; CI runs it on x86 Linux.
mod common;

use common::Battle;
use iron_tide::missions::levels::{BaseSite, Difficulty};
use iron_tide::sim::catalog::Team;
use iron_tide::sim::determinism::simulation_checksum;

const FINGERPRINT_FILE: &str = "tests/fingerprints.txt";
const UPDATE_VAR: &str = "UPDATE_FINGERPRINTS";
const SAMPLE_EVERY_SECS: f32 = 30.0;
const SAMPLES: usize = 8;

fn fingerprints_of_the_reference_battle() -> Vec<u64> {
    let mut battle = Battle::standard();
    battle.add_ai(Team::Player, BaseSite::SouthWest.origin(), Difficulty::Hard);
    (0..SAMPLES)
        .map(|_| {
            battle.run_for(SAMPLE_EVERY_SECS);
            simulation_checksum(battle.world())
        })
        .collect()
}

#[test]
fn the_reference_battle_matches_the_recorded_fingerprints() {
    let fingerprints = fingerprints_of_the_reference_battle();
    let written: Vec<String> = fingerprints
        .iter()
        .map(|sum| format!("{sum:016x}"))
        .collect();
    if std::env::var_os(UPDATE_VAR).is_some() {
        std::fs::write(FINGERPRINT_FILE, written.join("\n") + "\n").unwrap();
        return;
    }
    let recorded = std::fs::read_to_string(FINGERPRINT_FILE).unwrap_or_default();
    let recorded: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        recorded, written,
        "the battle differs from the recorded one. If the rules changed on purpose, record it again with \
         {UPDATE_VAR}=1 cargo test --test cross_platform; otherwise this computer calculates differently"
    );
}
