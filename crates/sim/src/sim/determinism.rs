use std::collections::HashMap;
use std::hash::Hasher;

use bevy::ecs::schedule::SingleThreadedExecutor;
use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};

use crate::sim::catalog::Team;
use crate::sim::combat::Projectile;
use crate::sim::components::{Building, Health, Unit};
use crate::sim::economy::Economies;
use crate::sim::sides::Sides;

/// Positions are compared to the millimetre, so rounding noise in rendering-only maths cannot matter.
const CHECKSUM_POSITION_SCALE: f32 = 1000.0;

/// One thread runs the systems in one fixed order, so entities are spawned, and therefore numbered and
/// iterated, the same way every time, and per-thread state such as the map size is always the app's own.
/// It is also cheaper: most systems are chained, and the multi-threaded executor spent more on waking
/// worker threads than it saved (about 13 % more CPU per frame). Call it once every plugin has been added.
pub fn run_schedules_on_one_thread(app: &mut App) {
    let mut schedules = app.world_mut().resource_mut::<Schedules>();
    for (_, schedule) in schedules.iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
}

/// The simulation's only source of chance, seeded by the level, so the same orders replay the same battle.
#[derive(Resource, Deref, DerefMut)]
pub struct SimulationRng(StdRng);

impl SimulationRng {
    pub fn seeded(seed: u64) -> Self {
        Self(StdRng::seed_from_u64(seed))
    }
}

impl Default for SimulationRng {
    fn default() -> Self {
        Self::seeded(0)
    }
}

/// The simulation's own number for a unit or structure, given in the order the simulation spawned them.
/// Bevy's entity ids also count effects that differ between machines, so they cannot order the battle.
#[derive(
    Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize,
)]
pub struct SimId(pub u64);

#[derive(Resource, Default)]
pub struct SimIds {
    next: u64,
}

/// Finds the entity behind a simulation id; units and structures gone from the battle are absent.
#[derive(Resource, Default)]
pub struct SimEntities(HashMap<SimId, Entity>);

impl SimEntities {
    pub fn get(&self, id: SimId) -> Option<Entity> {
        self.0.get(&id).copied()
    }
}

pub fn forget_removed_entity(
    removed: On<Remove<SimId>>,
    ids: Query<&SimId>,
    mut known: ResMut<SimEntities>,
) {
    if let Ok(id) = ids.get(removed.entity) {
        known.0.remove(id);
    }
}

pub fn number_new_unit(added: On<Add<Unit>>, mut registry: Registry) {
    registry.number(added.entity);
}

pub fn number_new_building(added: On<Add<Building>>, mut registry: Registry) {
    registry.number(added.entity);
}

pub fn number_new_projectile(added: On<Add<Projectile>>, mut registry: Registry) {
    registry.number(added.entity);
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct Registry<'w, 's> {
    ids: ResMut<'w, SimIds>,
    entities: ResMut<'w, SimEntities>,
    commands: Commands<'w, 's>,
}

impl Registry<'_, '_> {
    fn number(&mut self, entity: Entity) {
        let id = SimId(self.ids.next);
        self.ids.next += 1;
        self.entities.0.insert(id, entity);
        self.commands.entity(entity).insert(id);
    }
}

/// Query items in the order the simulation spawned them, so the battle cannot depend on how the ECS
/// happens to store entities (presentation adds components at moments that differ between machines).
pub fn in_simulation_order<T>(
    items: impl IntoIterator<Item = T>,
    id: impl Fn(&T) -> SimId,
) -> Vec<T> {
    let mut items: Vec<T> = items.into_iter().collect();
    items.sort_by_key(|item| id(item));
    items
}

/// FNV-1a: stable across runs, builds and machines, unlike the standard library's hasher.
struct StableHasher(u64);

impl Hasher for StableHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// A fingerprint of everything that decides the battle; two machines in step must agree on it every tick.
pub fn simulation_checksum(world: &mut World) -> u64 {
    let mut hasher = StableHasher(0xcbf2_9ce4_8422_2325);
    let mut units: Vec<(SimId, u8, u8, [i32; 3], i32)> = world
        .query::<(&SimId, &Unit, &Team, &Transform, &Health)>()
        .iter(world)
        .map(|(&id, unit, &team, transform, health)| {
            let position = (transform.translation * CHECKSUM_POSITION_SCALE)
                .round()
                .as_ivec3()
                .to_array();
            (
                id,
                unit.0 as u8,
                team.index() as u8,
                position,
                (health.current * 100.0).round() as i32,
            )
        })
        .collect();
    units.sort_by_key(|unit| unit.0);
    for (id, kind, team, position, health) in units {
        hasher.write_u64(id.0);
        hasher.write_u8(kind);
        hasher.write_u8(team);
        for axis in position {
            hasher.write_i32(axis);
        }
        hasher.write_i32(health);
    }
    let mut buildings: Vec<(SimId, IVec2, u8, i32)> = world
        .query::<(&SimId, &Building, &Team, &Health)>()
        .iter(world)
        .map(|(&id, building, &team, health)| {
            (
                id,
                building.origin,
                team.index() as u8,
                (health.current * 100.0).round() as i32,
            )
        })
        .collect();
    buildings.sort_by_key(|building| building.0);
    for (id, origin, team, health) in buildings {
        hasher.write_u64(id.0);
        hasher.write_i32(origin.x);
        hasher.write_i32(origin.y);
        hasher.write_u8(team);
        hasher.write_i32(health);
    }
    let sides = world.get_resource::<Sides>().copied().unwrap_or_default();
    if let Some(economies) = world.get_resource::<Economies>() {
        for team in sides.iter() {
            hasher.write_i32(economies.of(team).credits);
        }
    }
    hasher.finish()
}
