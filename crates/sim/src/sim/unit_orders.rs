use bevy::prelude::*;

use crate::sim::components::*;
use crate::sim::movement::order_move;
use crate::world::map::{GameMap, cell_center};

/// Close enough to an order's destination to call it done; formations and crowding rarely land exactly.
const ARRIVAL_DISTANCE: f32 = 3.0;

fn arrived(position: Vec3, destination: IVec2) -> bool {
    position.xz().distance(cell_center(destination).xz()) < ARRIVAL_DISTANCE
}

/// Turns standing orders into paths: advances through the queue, flips patrols and resumes an
/// attack-move once the fight that interrupted it is over. Harvesters follow their own routine.
pub fn carry_out_orders(
    map: Res<GameMap>,
    mut units: Query<
        (&Transform, &mut Orders, &mut Mobile, Option<&AttackTarget>),
        Without<Harvester>,
    >,
) {
    for (transform, mut orders, mut mobile, attack) in &mut units {
        let fighting = attack.is_some_and(|attack| attack.target.is_some());
        if mobile.is_moving() || fighting {
            continue;
        }
        let position = transform.translation;
        let Some(order) = orders.current else {
            if let Some(next) = orders.queued.pop_front() {
                orders.current = Some(next);
            }
            continue;
        };
        let next_order = if arrived(position, order.destination()) {
            match order {
                Order::Patrol { from, to } => Some(Order::Patrol { from: to, to: from }),
                Order::Move(_) | Order::AttackMove(_) => orders.queued.pop_front(),
            }
        } else {
            Some(order)
        };
        orders.current = next_order;
        let Some(order) = next_order else {
            mobile.pace = None;
            continue;
        };
        if arrived(position, order.destination()) {
            continue;
        }
        let ordered = !order.engages_on_the_way();
        if !order_move(&mut mobile, &map, position, order.destination(), ordered) {
            orders.current = orders.queued.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_orders_wait_behind_the_current_one() {
        let mut orders = Orders::default();
        orders.queue(Order::Move(IVec2::new(1, 1)));
        orders.queue(Order::AttackMove(IVec2::new(5, 5)));
        assert_eq!(orders.current, Some(Order::Move(IVec2::new(1, 1))));
        assert_eq!(
            orders.waypoints().collect::<Vec<_>>(),
            vec![IVec2::new(1, 1), IVec2::new(5, 5)]
        );
        orders.replace_with(Order::Move(IVec2::new(9, 9)));
        assert_eq!(orders.waypoints().count(), 1);
    }

    #[test]
    fn only_plain_moves_ignore_enemies() {
        assert!(!Order::Move(IVec2::ZERO).engages_on_the_way());
        assert!(Order::AttackMove(IVec2::ZERO).engages_on_the_way());
        assert!(
            Order::Patrol {
                from: IVec2::ZERO,
                to: IVec2::ONE
            }
            .engages_on_the_way()
        );
    }
}
