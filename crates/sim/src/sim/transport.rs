use bevy::prelude::*;

use crate::sim::catalog::Team;
use crate::sim::components::{AttackTarget, Health, Mobile, Orders, Selected};
use crate::sim::determinism::{SimId, in_simulation_order};
use crate::sim::movement::order_move;
use crate::world::map::{GameMap, cell_center, cells_by_distance, world_to_cell};

pub const CARRIER_CAPACITY: usize = 5;
/// How close to its carrier a soldier climbs in.
const BOARDING_REACH: f32 = 2.6;
const BOARDING_REPATH_SECS: f32 = 1.0;
/// How far from a stopped carrier its passengers may get out, in cells.
const UNLOADING_RADIUS: i32 = 4;
/// Share of their health soldiers keep when they scramble out of a destroyed carrier.
const ESCAPE_HEALTH_SHARE: f32 = 0.5;

/// The soldiers riding in a carrier, in the order they climbed in.
#[derive(Component, Default, Debug)]
pub struct Passengers(pub Vec<Entity>);

impl Passengers {
    pub fn has_room(&self) -> bool {
        self.0.len() < CARRIER_CAPACITY
    }
}

/// A soldier riding in `carrier`: it moves with it, fires from its hatches, and nothing can hit it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Embarked {
    pub carrier: Entity,
}

/// A soldier on its way to climb into `carrier`.
#[derive(Component, Debug)]
pub struct BoardingTask {
    carrier: Entity,
    repath_secs: f32,
}

impl BoardingTask {
    pub fn new(carrier: Entity) -> Self {
        Self {
            carrier,
            repath_secs: 0.0,
        }
    }

    pub fn carrier(&self) -> Entity {
        self.carrier
    }
}

/// A carrier ordered to let its passengers out.
#[derive(Component)]
pub struct Unloading;

pub fn board_carriers(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<GameMap>,
    mut carriers: Query<(&Team, &Transform, &mut Passengers), Without<Embarked>>,
    mut boarders: Query<
        (
            Entity,
            &Team,
            &Transform,
            &mut Mobile,
            &mut BoardingTask,
            Option<&mut AttackTarget>,
            &SimId,
        ),
        Without<Passengers>,
    >,
) {
    for (soldier, &team, transform, mut mobile, mut task, attack, _) in
        in_simulation_order(boarders.iter_mut(), |boarder| *boarder.6)
    {
        let carrier = task.carrier;
        let Ok((&owner, carrier_transform, mut passengers)) = carriers.get_mut(carrier) else {
            commands.entity(soldier).remove::<BoardingTask>();
            continue;
        };
        if owner != team || !passengers.has_room() {
            commands.entity(soldier).remove::<BoardingTask>();
            continue;
        }
        let carrier_position = carrier_transform.translation;
        if transform.translation.xz().distance(carrier_position.xz()) > BOARDING_REACH {
            task.repath_secs -= time.delta_secs();
            if task.repath_secs <= 0.0 || !mobile.is_moving() {
                task.repath_secs = BOARDING_REPATH_SECS;
                let goal = world_to_cell(carrier_position);
                order_move(&mut mobile, &map, transform.translation, goal, true);
            }
            continue;
        }
        passengers.0.push(soldier);
        mobile.stop();
        if let Some(mut attack) = attack {
            attack.clear();
        }
        commands
            .entity(soldier)
            .remove::<BoardingTask>()
            .insert(Embarked { carrier });
    }
}

/// Passengers ride where their carrier is; they never walk off on their own.
pub fn carry_passengers(
    carriers: Query<&Transform, (With<Passengers>, Without<Embarked>)>,
    mut passengers: Query<(&Embarked, &mut Transform, &mut Mobile, Option<&mut Orders>)>,
) {
    for (embarked, mut transform, mut mobile, orders) in &mut passengers {
        let Ok(carrier) = carriers.get(embarked.carrier) else {
            continue;
        };
        transform.translation = carrier.translation;
        if mobile.is_moving() {
            mobile.stop();
        }
        if let Some(mut orders) = orders.filter(|orders| orders.current.is_some()) {
            orders.clear();
        }
    }
}

/// A carrier ordered to unload, or destroyed, lets its passengers out on free ground around it; out of a
/// destroyed one they escape hurt.
pub fn unload_carriers(
    mut commands: Commands,
    map: Res<GameMap>,
    mut carriers: Query<
        (
            Entity,
            &Transform,
            &Health,
            &mut Passengers,
            Has<Unloading>,
            &SimId,
        ),
        Without<Embarked>,
    >,
    mut passengers: Query<(&mut Transform, &mut Health), (With<Embarked>, Without<Passengers>)>,
) {
    for (carrier, transform, health, mut riding, unloading, _) in
        in_simulation_order(carriers.iter_mut(), |carrier| *carrier.5)
    {
        let destroyed = health.current <= 0.0;
        if !unloading && !destroyed {
            continue;
        }
        commands.entity(carrier).remove::<Unloading>();
        let mut exits = cells_by_distance(world_to_cell(transform.translation), UNLOADING_RADIUS)
            .filter(|&cell| map.is_passable(cell));
        for soldier in std::mem::take(&mut riding.0) {
            let Ok((mut soldier_transform, mut soldier_health)) = passengers.get_mut(soldier)
            else {
                continue;
            };
            let exit = exits.next().map_or(transform.translation, |cell| {
                cell_center(cell).with_y(transform.translation.y)
            });
            soldier_transform.translation = exit;
            if destroyed {
                soldier_health.current = (soldier_health.current * ESCAPE_HEALTH_SHARE).max(1.0);
            }
            commands.entity(soldier).remove::<Embarked>();
        }
    }
}

/// Soldiers inside a carrier cannot be picked, even when a control group names them.
pub fn deselect_passengers(
    mut commands: Commands,
    selected: Query<Entity, (With<Embarked>, With<Selected>)>,
) {
    for soldier in &selected {
        commands.entity(soldier).remove::<Selected>();
    }
}
