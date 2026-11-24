use bevy::prelude::*;

use crate::sim::catalog::{ArmorClass, Team};
use crate::sim::components::{Armor, Health, Mobile};
use crate::sim::determinism::{SimId, in_simulation_order};
use crate::sim::movement::order_move;
use crate::world::map::{GameMap, world_to_cell};

/// How far a medic or repair truck looks for patients to go over to.
const SEEK_RANGE: f32 = 12.0;
/// Close enough to treat a patient, centre to centre; vehicles are bigger than soldiers.
const INFANTRY_TREATMENT_RANGE: f32 = 2.2;
const VEHICLE_TREATMENT_RANGE: f32 = 3.6;

/// A unit that restores health to wounded friendly units of one armour class around it.
#[derive(Component)]
pub struct Healer {
    pub per_sec: f32,
    pub patients: ArmorClass,
    /// True while actually treating someone, which drives the kneeling animation.
    pub treating: bool,
}

pub fn treat_wounded(
    time: Res<Time>,
    map: Res<GameMap>,
    mut healers: Query<
        (Entity, &Team, &Transform, &mut Healer, &mut Mobile, &SimId),
        Without<crate::sim::transport::Embarked>,
    >,
    mut patients: Query<(Entity, &Team, &Transform, &Armor, &mut Health, &SimId)>,
) {
    let dt = time.delta_secs();
    let in_order = in_simulation_order(healers.iter_mut(), |item| *item.5);
    for (healer_entity, &team, transform, mut healer, mut mobile, _) in in_order {
        let here = transform.translation;
        let patient = patients
            .iter()
            .filter(|(entity, owner, _, armor, health, _)| {
                *entity != healer_entity
                    && **owner == team
                    && armor.0 == healer.patients
                    && health.current < health.max
            })
            .map(|(entity, _, patient_transform, .., &id)| {
                (
                    entity,
                    patient_transform.translation.xz().distance(here.xz()),
                    id,
                )
            })
            .filter(|&(_, distance, _)| distance <= SEEK_RANGE)
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)));

        healer.treating = false;
        let Some((patient, distance, _)) = patient else {
            continue;
        };
        let reach = if healer.patients == ArmorClass::Infantry {
            INFANTRY_TREATMENT_RANGE
        } else {
            VEHICLE_TREATMENT_RANGE
        };
        if distance <= reach {
            if let Ok((.., mut health, _)) = patients.get_mut(patient) {
                health.current = (health.current + healer.per_sec * dt).min(health.max);
                healer.treating = !mobile.is_moving();
            }
        } else if !mobile.ordered_move && !mobile.is_moving() {
            let Ok((_, _, patient_transform, ..)) = patients.get(patient) else {
                continue;
            };
            order_move(
                &mut mobile,
                &map,
                here,
                world_to_cell(patient_transform.translation),
                false,
            );
        }
    }
}
