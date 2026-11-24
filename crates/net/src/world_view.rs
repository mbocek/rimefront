use std::collections::{BTreeMap, HashSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use iron_tide_sim::sim::catalog::{BuildingKind, Diplomacy, Team, UnitKind};
use iron_tide_sim::sim::combat::Targets;
use iron_tide_sim::sim::components::{Building, Health, Unit};
use iron_tide_sim::sim::determinism::SimId;
use iron_tide_sim::sim::economy::Economies;
use iron_tide_sim::sim::fog::Sight;
use iron_tide_sim::sim::transport::Embarked;
use iron_tide_sim::world::map::{MapBounds, chunk_of, map_bounds, world_to_cell};

/// Positions travel in hundredths of a cell, facings in thousandths of a radian, health in percent: finer
/// than anything drawn, coarse enough that a unit standing still sends nothing.
const POSITION_STEPS: f32 = 100.0;
const FACING_STEPS: f32 = 1000.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Thing {
    Unit(UnitKind),
    Building { kind: BuildingKind, origin: IVec2 },
}

/// One unit or structure as a player of a world is shown it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ThingState {
    pub id: SimId,
    pub thing: Thing,
    pub team: Team,
    position: [i32; 3],
    facing: i32,
    pub health_percent: u8,
}

impl ThingState {
    pub fn position(&self) -> Vec3 {
        Vec3::from_array(self.position.map(|axis| axis as f32 / POSITION_STEPS))
    }

    pub fn facing(&self) -> f32 {
        self.facing as f32 / FACING_STEPS
    }
}

/// What changed in a player's view of a world since the last one they were sent.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct ViewChange {
    pub tick: u64,
    /// The world's cells, when they widened since the last view.
    pub bounds: Option<MapBounds>,
    pub credits: i32,
    /// Things that came into view or changed.
    pub shown: Vec<ThingState>,
    /// Things gone from view: out of sight, destroyed or carried off.
    pub hidden: Vec<SimId>,
}

impl ViewChange {
    fn is_empty(&self) -> bool {
        self.bounds.is_none() && self.shown.is_empty() && self.hidden.is_empty()
    }
}

/// The chunks a side and its allies see into: every chunk within sight of one of their units or structures.
pub fn chunks_in_sight(world: &mut World, viewer: Team) -> HashSet<IVec2> {
    let diplomacy = world.resource::<Diplomacy>().clone();
    let mut chunks = HashSet::new();
    let mut eyes = world.query::<(&Team, &Transform, &Sight)>();
    for (&team, transform, sight) in eyes.iter(world) {
        if diplomacy.hostile(team, viewer) {
            continue;
        }
        let reach = IVec2::splat(sight.0.ceil() as i32);
        let cell = world_to_cell(transform.translation);
        let first = chunk_of(cell - reach);
        let last = chunk_of(cell + reach);
        for y in first.y..=last.y {
            for x in first.x..=last.x {
                chunks.insert(IVec2::new(x, y));
            }
        }
    }
    chunks
}

/// Everything `viewer` may see right now: in a chunk its side looks into and not hidden from it by the fog.
pub fn things_in_view(world: &mut World, viewer: Team) -> BTreeMap<SimId, ThingState> {
    let chunks = chunks_in_sight(world, viewer);
    let in_sight_chunk = |position: Vec3| chunks.contains(&chunk_of(world_to_cell(position)));
    let mut shown = BTreeMap::new();
    let mut things = world.query_filtered::<(
        Entity,
        &SimId,
        &Team,
        &Transform,
        &Health,
        Option<&Unit>,
        Option<&Building>,
    ), Without<Embarked>>();
    let targets = world.resource::<Targets>();
    for (entity, &id, &team, transform, health, unit, building) in things.iter(world) {
        let thing = match (unit, building) {
            (Some(unit), _) => Thing::Unit(unit.0),
            (None, Some(building)) => Thing::Building {
                kind: building.kind,
                origin: building.origin,
            },
            (None, None) => continue,
        };
        let seen = targets
            .get(entity)
            .is_some_and(|target| target.seen_by(viewer));
        if !seen || !in_sight_chunk(transform.translation) {
            continue;
        }
        let (facing, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        shown.insert(
            id,
            ThingState {
                id,
                thing,
                team,
                position: (transform.translation * POSITION_STEPS)
                    .round()
                    .as_ivec3()
                    .to_array(),
                facing: (facing * FACING_STEPS).round() as i32,
                health_percent: (health.fraction() * 100.0).round() as u8,
            },
        );
    }
    shown
}

/// What one player of a world was last sent, so the next view carries only what changed.
#[derive(Default)]
pub struct SentView {
    things: BTreeMap<SimId, ThingState>,
    bounds: Option<MapBounds>,
    credits: Option<i32>,
}

impl SentView {
    /// What changed since the last view; `None` when nothing did.
    pub fn next(&mut self, world: &mut World, viewer: Team, tick: u64) -> Option<ViewChange> {
        let now = things_in_view(world, viewer);
        let shown = now
            .values()
            .filter(|state| self.things.get(&state.id) != Some(state))
            .copied()
            .collect();
        let hidden = self
            .things
            .keys()
            .filter(|id| !now.contains_key(id))
            .copied()
            .collect();
        let bounds = Some(map_bounds()).filter(|&bounds| self.bounds != Some(bounds));
        self.bounds = Some(map_bounds());
        self.things = now;
        let credits = world.resource::<Economies>().of(viewer).credits;
        let change = ViewChange {
            tick,
            bounds,
            credits,
            shown,
            hidden,
        };
        let credits_changed = self.credits.replace(credits) != Some(credits);
        (credits_changed || !change.is_empty()).then_some(change)
    }
}

/// A player's copy of the part of a world they see, kept up to date from the views they are sent.
#[derive(Default, Debug)]
pub struct WorldMirror {
    pub tick: u64,
    pub bounds: Option<MapBounds>,
    pub credits: i32,
    pub things: BTreeMap<SimId, ThingState>,
}

impl WorldMirror {
    pub fn apply(&mut self, change: ViewChange) {
        self.tick = change.tick;
        self.credits = change.credits;
        if change.bounds.is_some() {
            self.bounds = change.bounds;
        }
        for id in change.hidden {
            self.things.remove(&id);
        }
        for state in change.shown {
            self.things.insert(state.id, state);
        }
    }
}
