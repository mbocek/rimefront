use crate::missions::event_editing::{LONGEST_MESSAGE, message_mut};
use crate::missions::levels::{
    BaseSite, Difficulty, EnemySetup, ForceSetup, LevelDefinition, Objective,
};
use crate::sim::catalog::Team;

const LONGEST_NAME: usize = 40;
const LONGEST_BRIEFING: usize = 400;
const OBJECTIVE_CHOICES: usize = 6;

/// The side in a level's list: zero is the player, then the enemies in order.
fn force_mut(level: &mut LevelDefinition, side: usize) -> Option<&mut ForceSetup> {
    match side {
        0 => Some(&mut level.player),
        _ => level
            .enemies
            .get_mut(side - 1)
            .map(|enemy| &mut enemy.force),
    }
}

fn sites_in_use(level: &LevelDefinition) -> Vec<BaseSite> {
    std::iter::once(level.player.site)
        .chain(level.enemies.iter().map(|enemy| enemy.force.site))
        .collect()
}

/// Moves a side to the next corner no other side holds.
pub fn move_side(level: &mut LevelDefinition, side: usize) {
    let taken = sites_in_use(level);
    let Some(force) = force_mut(level, side) else {
        return;
    };
    let current = BaseSite::ALL
        .iter()
        .position(|&site| site == force.site)
        .unwrap_or(0);
    let next_free = (1..BaseSite::ALL.len())
        .map(|step| BaseSite::ALL[(current + step) % BaseSite::ALL.len()])
        .find(|site| !taken.contains(site));
    if let Some(site) = next_free {
        force.site = site;
    }
}

/// Adds an enemy in a free corner; false when every opponent seat is filled.
pub fn add_enemy(level: &mut LevelDefinition) -> bool {
    if level.enemies.len() >= Team::OPPONENTS.len() {
        return false;
    }
    let taken = sites_in_use(level);
    let Some(site) = BaseSite::ALL.into_iter().find(|site| !taken.contains(site)) else {
        return false;
    };
    let mut enemy = EnemySetup::default();
    enemy.force.site = site;
    level.enemies.push(enemy);
    true
}

/// A level keeps at least one enemy.
pub fn remove_enemy(level: &mut LevelDefinition) -> bool {
    if level.enemies.len() <= 1 {
        return false;
    }
    level.enemies.pop();
    true
}

pub fn cycle_difficulty(level: &mut LevelDefinition, enemy: usize) {
    if let Some(enemy) = level.enemies.get_mut(enemy) {
        let next = Difficulty::ALL
            .iter()
            .position(|&difficulty| difficulty == enemy.difficulty)
            .map_or(0, |index| (index + 1) % Difficulty::ALL.len());
        enemy.difficulty = Difficulty::ALL[next];
    }
}

fn objective_choice(level: &LevelDefinition, index: usize) -> Objective {
    match index % OBJECTIVE_CHOICES {
        0 => Objective::DestroyAllEnemies,
        1 => Objective::DestroyCommandCenters,
        2 => Objective::CaptureCommandCenter,
        3 => Objective::Survive { minutes: 10.0 },
        4 => Objective::Survive { minutes: 20.0 },
        _ => Objective::ReachCredits {
            credits: level.player.credits + 10_000,
        },
    }
}

pub fn cycle_objective(level: &mut LevelDefinition) {
    let current =
        (0..OBJECTIVE_CHOICES).position(|index| objective_choice(level, index) == level.objective);
    let next = current.map_or(0, |index| index + 1);
    level.objective = objective_choice(level, next);
}

/// Which line of a level the keyboard writes into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LevelText {
    Name,
    Briefing,
    /// The message of the event at this index.
    EventMessage(usize),
}

impl LevelText {
    pub fn of(self, level: &mut LevelDefinition) -> Option<(&mut String, usize)> {
        match self {
            LevelText::Name => Some((&mut level.name, LONGEST_NAME)),
            LevelText::Briefing => Some((&mut level.briefing, LONGEST_BRIEFING)),
            LevelText::EventMessage(index) => {
                message_mut(level, index).map(|text| (text, LONGEST_MESSAGE))
            }
        }
    }
}

pub fn type_into(level: &mut LevelDefinition, target: LevelText, typed: &str) {
    let Some((text, longest)) = target.of(level) else {
        return;
    };
    for character in typed.chars().filter(|c| !c.is_control()) {
        if text.chars().count() < longest {
            text.push(character);
        }
    }
}

pub fn erase_from(level: &mut LevelDefinition, target: LevelText) {
    if let Some((text, _)) = target.of(level) {
        text.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_move_only_to_free_corners() {
        let mut level = LevelDefinition::default();
        assert_eq!(level.player.site, BaseSite::SouthWest);
        assert_eq!(level.enemies[0].force.site, BaseSite::NorthEast);
        move_side(&mut level, 0);
        assert_eq!(level.player.site, BaseSite::NorthWest);
        move_side(&mut level, 0);
        move_side(&mut level, 0);
        assert_eq!(
            level.player.site,
            BaseSite::SouthWest,
            "skipped the enemy's corner"
        );
        assert!(level.problems().is_empty());
    }

    #[test]
    fn enemies_fill_the_free_corners_and_one_always_stays() {
        let mut level = LevelDefinition::default();
        assert!(add_enemy(&mut level));
        assert!(add_enemy(&mut level));
        assert!(!add_enemy(&mut level));
        assert!(level.problems().is_empty());
        assert!(remove_enemy(&mut level));
        assert!(remove_enemy(&mut level));
        assert!(!remove_enemy(&mut level));
    }

    #[test]
    fn objectives_cycle_back_to_the_start() {
        let mut level = LevelDefinition::default();
        let first = level.objective;
        for _ in 0..OBJECTIVE_CHOICES {
            cycle_objective(&mut level);
        }
        assert_eq!(level.objective, first);
    }

    #[test]
    fn typing_stops_at_the_length_limit() {
        let mut level = LevelDefinition::default();
        level.name.clear();
        type_into(&mut level, LevelText::Name, &"x".repeat(LONGEST_NAME + 5));
        assert_eq!(level.name.len(), LONGEST_NAME);
        erase_from(&mut level, LevelText::Name);
        assert_eq!(level.name.len(), LONGEST_NAME - 1);
    }
}
