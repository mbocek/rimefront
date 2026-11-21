use std::collections::HashMap;

use bevy::prelude::*;

use crate::catalog::Team;
use crate::combat::{DamageDealt, DamageSource, Destroyed};
use crate::components::{Building, Unit};
use crate::match_flow::MatchClock;

/// Anything that appears within this long of the start is the starting force, not something built.
const STARTING_FORCE_SECS: f32 = 0.5;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct TeamTally {
    pub units_trained: u32,
    pub units_lost: u32,
    pub structures_built: u32,
    pub structures_lost: u32,
    pub enemies_destroyed: u32,
    pub damage_dealt: f32,
    pub damage_taken: f32,
}

impl TeamTally {
    pub fn add(&mut self, other: &TeamTally) {
        self.units_trained += other.units_trained;
        self.units_lost += other.units_lost;
        self.structures_built += other.structures_built;
        self.structures_lost += other.structures_lost;
        self.enemies_destroyed += other.enemies_destroyed;
        self.damage_dealt += other.damage_dealt;
        self.damage_taken += other.damage_taken;
    }
}

/// How the match went for every team, for the screen at its end.
#[derive(Resource, Default, Debug)]
pub struct MatchStats {
    teams: [TeamTally; 4],
    /// Which team hit each victim last, so its destruction is credited.
    last_attacker: HashMap<Entity, Team>,
}

impl MatchStats {
    pub fn of(&self, team: Team) -> &TeamTally {
        &self.teams[team.index()]
    }

    fn of_mut(&mut self, team: Team) -> &mut TeamTally {
        &mut self.teams[team.index()]
    }
}

pub fn tally_damage(mut hits: MessageReader<DamageDealt>, mut stats: ResMut<MatchStats>) {
    for hit in hits.read() {
        stats.of_mut(hit.attacker).damage_dealt += hit.amount;
        stats.of_mut(hit.team).damage_taken += hit.amount;
        stats.last_attacker.insert(hit.victim, hit.attacker);
    }
}

pub fn tally_losses(mut losses: MessageReader<Destroyed>, mut stats: ResMut<MatchStats>) {
    for loss in losses.read() {
        let tally = stats.of_mut(loss.team);
        match loss.what {
            DamageSource::Unit(_) => tally.units_lost += 1,
            DamageSource::Building(_) => tally.structures_lost += 1,
            DamageSource::Support => {}
        }
        if let Some(attacker) = stats.last_attacker.remove(&loss.entity) {
            stats.of_mut(attacker).enemies_destroyed += 1;
        }
    }
}

pub fn tally_new_forces(
    clock: Res<MatchClock>,
    mut stats: ResMut<MatchStats>,
    units: Query<&Team, Added<Unit>>,
    buildings: Query<&Team, Added<Building>>,
) {
    if clock.elapsed_secs < STARTING_FORCE_SECS {
        return;
    }
    for &team in &units {
        stats.of_mut(team).units_trained += 1;
    }
    for &team in &buildings {
        stats.of_mut(team).structures_built += 1;
    }
}
