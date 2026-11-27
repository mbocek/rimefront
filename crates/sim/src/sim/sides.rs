use bevy::prelude::*;

use crate::sim::abilities::Mine;
use crate::sim::ai::AiCommanders;
use crate::sim::cadence::SimulationCadences;
use crate::sim::catalog::{Diplomacy, Team};
use crate::sim::components::{Building, Unit};
use crate::sim::economy::Economies;
use crate::sim::intel::Intel;
use crate::sim::match_stats::MatchStats;
use crate::sim::production::Production;
use crate::sim::support_powers::SupportPowers;
use crate::world::map::GameMap;

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

/// Sides that left the battle for good, whose numbers new sides take again before the count grows.
#[derive(Resource, Clone, PartialEq, Eq, Default, Debug)]
pub struct RetiredSides(pub Vec<Team>);

/// Lets a new side into the battle, in a camp of its own and knowing nothing of the map yet; it takes the
/// lowest number a side retired, if any did.
pub fn admit_side(world: &mut World) -> Team {
    let reused = world
        .get_resource_mut::<RetiredSides>()
        .and_then(|mut retired| {
            retired.0.sort();
            (!retired.0.is_empty()).then(|| retired.0.remove(0))
        });
    let team = reused.unwrap_or_else(|| world.resource_mut::<Sides>().join());
    world.resource_mut::<Intel>().admit(team);
    world.resource_mut::<SupportPowers>().admit(team);
    team
}

/// Takes a side out of the battle for good: whatever of it is left (walls, mines) goes, and it owns,
/// knows and has made nothing any more. Its number is given out again unless a level names it, since a
/// level's sides keep the camps the level put them in.
pub fn retire_side(world: &mut World, team: Team) {
    let mut owned = world.query::<(Entity, &Team)>();
    let left: Vec<Entity> = owned
        .iter(world)
        .filter(|(_, owner)| **owner == team)
        .map(|(entity, _)| entity)
        .collect();
    let mut mines = world.query::<(Entity, &Mine)>();
    let buried: Vec<Entity> = mines
        .iter(world)
        .filter(|(_, mine)| mine.team == team)
        .map(|(entity, _)| entity)
        .collect();
    for entity in left.into_iter().chain(buried) {
        if let Some(building) = world.get::<Building>(entity) {
            let (origin, footprint) = (building.origin, building.kind.spec().footprint);
            world
                .resource_mut::<GameMap>()
                .set_building(origin, footprint, None);
        }
        world.despawn(entity);
    }
    world.resource_mut::<Economies>().forget(team);
    world.resource_mut::<Production>().forget(team);
    world.resource_mut::<SupportPowers>().forget(team);
    world.resource_mut::<MatchStats>().forget(team);
    world
        .resource_mut::<SimulationCadences>()
        .harvester_rescue_cooldown_secs
        .forget(team);
    world.resource_mut::<Intel>().forget(team);
    world.resource_mut::<Shelter>().0.remove(team);
    world.resource_mut::<Diplomacy>().leave_camp(team);
    if let Some(mut offers) = world.get_resource_mut::<crate::sim::alliances::AllianceOffers>() {
        offers.forget(team);
    }
    world
        .resource_mut::<AiCommanders>()
        .0
        .retain(|ai| ai.team != team);
    if team.index() >= Team::ALL.len() {
        world.get_resource_or_init::<RetiredSides>().0.push(team);
    }
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

    /// `team` has the blank value again.
    pub fn forget(&mut self, team: Team) {
        if let Some(value) = self.values.get_mut(team.index()) {
            *value = self.blank.clone();
        }
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

    pub fn remove(&mut self, team: Team) {
        self.0[team.index() / 64] &= !(1 << (team.index() % 64));
    }

    /// Every side in the set, lowest number first.
    pub fn iter(&self) -> impl Iterator<Item = Team> + '_ {
        (0..Team::MOST)
            .filter(|&index| self.0[index / 64] & (1 << (index % 64)) != 0)
            .map(Team::from_index)
    }

    pub fn union(mut self, other: SideSet) -> SideSet {
        for (word, other) in self.0.iter_mut().zip(other.0) {
            *word |= other;
        }
        self
    }
}

/// Sides out of the fighting while their players are away: nobody can hurt or capture what is theirs,
/// and it fires at nobody.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Shelter(pub SideSet);

impl Shelter {
    pub fn covers(&self, team: Team) -> bool {
        self.0.contains(team)
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
            [Team::Azure, Team::Amber, late].map(|team| *credits.of(team)),
            [100, 105, 70]
        );
    }

    #[test]
    fn a_set_holds_any_side() {
        let last = Team::from_index(Team::MOST - 1);
        let set: SideSet = [Team::Crimson, last].into_iter().collect();
        assert!(set.contains(Team::Crimson) && set.contains(last));
        assert!(!set.contains(Team::Azure) && !set.contains(Team::from_index(64)));
    }

    #[test]
    fn joining_sides_are_numbered_after_the_level_ones() {
        let mut sides = Sides::default();
        assert_eq!(sides.join(), Team::from_index(4));
        assert_eq!(sides.iter().count(), 5);
    }
}
