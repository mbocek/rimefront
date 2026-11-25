use bevy::prelude::*;

use crate::sim::catalog::Team;
use crate::sim::components::{Building, Unit};
use crate::sim::intel::Intel;

/// The sides in the battle: the four a level names, then every side that joined since, numbered on.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sides {
    count: usize,
}

impl Default for Sides {
    fn default() -> Self {
        Self {
            count: Team::ALL.len(),
        }
    }
}

impl Sides {
    pub fn iter(self) -> impl Iterator<Item = Team> {
        (0..self.count).map(Team::from_index)
    }

    pub fn count(self) -> usize {
        self.count
    }

    pub fn join(&mut self) -> Team {
        assert!(
            self.count < Team::MOST,
            "the battle holds at most {} sides",
            Team::MOST
        );
        self.count += 1;
        Team::from_index(self.count - 1)
    }
}

/// Lets a new side into the battle, in a camp of its own and knowing nothing of the map yet.
pub fn admit_side(world: &mut World) -> Team {
    let team = world.resource_mut::<Sides>().join();
    world.resource_mut::<Intel>().admit(team);
    team
}

/// Whether anything of `team` still stands: a unit, or a structure that keeps a side in the battle.
pub fn side_stands(world: &mut World, team: Team) -> bool {
    let mut units = world.query_filtered::<&Team, With<Unit>>();
    if units.iter(world).any(|&owner| owner == team) {
        return true;
    }
    let mut buildings = world.query::<(&Team, &Building)>();
    buildings
        .iter(world)
        .any(|(&owner, building)| owner == team && building.kind.keeps_a_side_alive())
}

/// A value for every side, kept only for the sides that ever needed one of their own; any other side has
/// the blank value.
#[derive(Clone, PartialEq, Debug)]
pub struct PerSide<T> {
    values: Vec<T>,
    blank: T,
}

impl<T: Clone> PerSide<T> {
    pub fn new(blank: T) -> Self {
        Self {
            values: Vec::new(),
            blank,
        }
    }

    pub fn of(&self, team: Team) -> &T {
        self.values.get(team.index()).unwrap_or(&self.blank)
    }

    pub fn of_mut(&mut self, team: Team) -> &mut T {
        let index = team.index();
        if index >= self.values.len() {
            self.values.resize(index + 1, self.blank.clone());
        }
        &mut self.values[index]
    }

    /// Every side, those to come included, has `value`.
    pub fn set_all(&mut self, value: T) {
        self.values.fill(value.clone());
        self.blank = value;
    }

    /// The values of the sides that have one of their own.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.values.iter_mut()
    }
}

impl<T: Clone + Default> Default for PerSide<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// A set of sides, small enough to copy for every target every tick.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct SideSet([u64; Team::MOST / 64]);

impl SideSet {
    pub fn insert(&mut self, team: Team) {
        self.0[team.index() / 64] |= 1 << (team.index() % 64);
    }

    pub fn contains(&self, team: Team) -> bool {
        self.0[team.index() / 64] & (1 << (team.index() % 64)) != 0
    }
}

impl FromIterator<Team> for SideSet {
    fn from_iter<I: IntoIterator<Item = Team>>(teams: I) -> Self {
        let mut set = SideSet::default();
        for team in teams {
            set.insert(team);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_without_a_value_of_its_own_has_the_blank_one() {
        let mut credits = PerSide::new(100);
        *credits.of_mut(Team::Amber) += 5;
        let late = Team::from_index(40);
        assert_eq!(*credits.of(late), 100);
        *credits.of_mut(late) -= 30;
        assert_eq!(
            [Team::Player, Team::Amber, late].map(|team| *credits.of(team)),
            [100, 105, 70]
        );
    }

    #[test]
    fn a_set_holds_any_side() {
        let last = Team::from_index(Team::MOST - 1);
        let set: SideSet = [Team::Crimson, last].into_iter().collect();
        assert!(set.contains(Team::Crimson) && set.contains(last));
        assert!(!set.contains(Team::Player) && !set.contains(Team::from_index(64)));
    }

    #[test]
    fn joining_sides_are_numbered_after_the_level_ones() {
        let mut sides = Sides::default();
        assert_eq!(sides.join(), Team::from_index(4));
        assert_eq!(sides.iter().count(), 5);
    }
}
