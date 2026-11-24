use bevy::prelude::*;

use crate::sim::catalog::{BuildingKind, Team};
use crate::sim::components::{AttackTarget, Building, Health, Weapon};
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::determinism::{SimId, in_simulation_order};
use crate::sim::economy::Economies;
use crate::sim::player_commands::LocalTeam;
use crate::world::map::{GameMap, footprint_center};

/// Share of its full health a structure regains per second while being repaired.
const REPAIR_SHARE_PER_SEC: f32 = 0.04;
/// Repairing a structure from rubble to full health costs this share of its price.
const REPAIR_COST_SHARE: f32 = 0.5;
const SELL_REFUND_SHARE: f32 = 0.5;
const SELL_ANIMATION_SECS: f32 = 1.0;
/// How far a sold structure sinks into the ground before it disappears.
const SELL_SINK_DEPTH: f32 = 3.0;
/// The Command Center is never bought, but repairing it should still cost something.
const COMMAND_CENTER_VALUE: i32 = 3000;

#[derive(Component, Default)]
pub struct Repairing {
    /// Fractions of a credit owed but not yet charged, so cheap repair steps still add up.
    unpaid: f32,
}

/// Sinks into the ground before it disappears; the refund has already been paid.
#[derive(Component)]
pub struct BeingSold {
    remaining_secs: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum StructureOrderKind {
    ToggleRepair,
    Sell,
}

/// A commander's order for one of its own structures; orders for anyone else's are ignored.
#[derive(Message, Clone, Copy, Debug)]
pub struct StructureOrder {
    pub team: Team,
    pub building: Entity,
    pub kind: StructureOrderKind,
}

pub fn structure_value(kind: BuildingKind) -> i32 {
    match kind {
        BuildingKind::CommandCenter => COMMAND_CENTER_VALUE,
        _ => kind.spec().cost,
    }
}

/// A captured neutral site cannot be sold: it would vanish from a map built around it.
pub fn can_sell(kind: BuildingKind) -> bool {
    kind != BuildingKind::CommandCenter && !kind.is_neutral_site()
}

pub fn sale_refund(kind: BuildingKind) -> i32 {
    (kind.spec().cost as f32 * SELL_REFUND_SHARE).round() as i32
}

/// What it costs to bring a structure back to full health.
pub fn repair_cost(kind: BuildingKind, health: &Health) -> i32 {
    let missing = health.max - health.current;
    (missing * repair_price_per_hit_point(kind, health.max)).ceil() as i32
}

fn repair_price_per_hit_point(kind: BuildingKind, max_health: f32) -> f32 {
    structure_value(kind) as f32 * REPAIR_COST_SHARE / max_health
}

type OrderedStructure<'a> = (
    &'a Building,
    &'a Team,
    &'a Health,
    &'a Transform,
    Has<Repairing>,
    Has<BeingSold>,
);

pub fn carry_out_structure_orders(
    local: Res<LocalTeam>,
    mut commands: Commands,
    mut orders: MessageReader<StructureOrder>,
    mut cues: MessageWriter<SoundCue>,
    mut economies: ResMut<Economies>,
    structures: Query<OrderedStructure>,
) {
    for order in orders.read() {
        let Ok((building, &team, health, transform, repairing, being_sold)) =
            structures.get(order.building)
        else {
            continue;
        };
        if team != order.team || being_sold {
            continue;
        }
        let mut structure = commands.entity(order.building);
        match order.kind {
            StructureOrderKind::ToggleRepair if repairing => {
                structure.remove::<Repairing>();
            }
            StructureOrderKind::ToggleRepair => {
                let standing = health.current > 0.0;
                if standing && health.current < health.max {
                    structure.insert(Repairing::default());
                    if team == local.0 {
                        cues.write(SoundCue::at(Sound::RepairStarted, transform.translation));
                    }
                }
            }
            StructureOrderKind::Sell => {
                if !can_sell(building.kind) {
                    continue;
                }
                economies.of_mut(team).credits += sale_refund(building.kind);
                structure
                    .remove::<(Repairing, Weapon, AttackTarget)>()
                    .insert(BeingSold {
                        remaining_secs: SELL_ANIMATION_SECS,
                    });
                if team == local.0 {
                    cues.write(SoundCue::at(Sound::StructureSold, transform.translation));
                }
            }
        }
    }
}

pub fn repair_structures(
    mut commands: Commands,
    time: Res<Time>,
    mut economies: ResMut<Economies>,
    mut structures: Query<(
        Entity,
        &Building,
        &Team,
        &mut Health,
        &mut Repairing,
        &SimId,
    )>,
) {
    let dt = time.delta_secs();
    let in_order = in_simulation_order(structures.iter_mut(), |item| *item.5);
    for (entity, building, &team, mut health, mut repairing, _) in in_order {
        let destroyed = health.current <= 0.0;
        if destroyed {
            continue;
        }
        let healed = (health.max * REPAIR_SHARE_PER_SEC * dt).min(health.max - health.current);
        let owed =
            repairing.unpaid + healed * repair_price_per_hit_point(building.kind, health.max);
        let charge = owed.floor() as i32;
        let economy = economies.of_mut(team);
        if economy.credits < charge {
            continue;
        }
        economy.credits -= charge;
        repairing.unpaid = owed - charge as f32;
        health.current += healed;
        if health.current >= health.max {
            health.current = health.max;
            commands.entity(entity).remove::<Repairing>();
        }
    }
}

pub fn finish_sales(
    mut commands: Commands,
    time: Res<Time>,
    mut map: ResMut<GameMap>,
    mut sold: Query<(Entity, &Building, &mut BeingSold, &mut Transform)>,
) {
    for (entity, building, mut sale, mut transform) in &mut sold {
        sale.remaining_secs -= time.delta_secs();
        let footprint = building.kind.spec().footprint;
        let sunk = 1.0 - (sale.remaining_secs / SELL_ANIMATION_SECS).clamp(0.0, 1.0);
        transform.translation.y =
            footprint_center(building.origin, footprint).y - sunk * SELL_SINK_DEPTH;
        if sale.remaining_secs > 0.0 {
            continue;
        }
        map.set_building(building.origin, footprint, None);
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_repair_costs_half_the_price() {
        let wrecked = Health {
            current: 0.0,
            max: BuildingKind::Factory.spec().max_health,
        };
        assert_eq!(
            repair_cost(BuildingKind::Factory, &wrecked),
            BuildingKind::Factory.spec().cost / 2
        );
        assert_eq!(
            repair_cost(BuildingKind::Factory, &Health::full(wrecked.max)),
            0
        );
    }

    #[test]
    fn the_command_center_cannot_be_sold() {
        assert!(!can_sell(BuildingKind::CommandCenter));
        assert_eq!(
            sale_refund(BuildingKind::Barracks),
            BuildingKind::Barracks.spec().cost / 2
        );
    }
}
