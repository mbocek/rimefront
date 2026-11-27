//! When the age turns: what was queued for the old one is refunded, structures are spared for a while, and
//! units and defences of the old age can be promoted into their successors. See docs/eras.md.

use bevy::prelude::*;

use crate::levels::ActiveLevel;
use crate::match_flow::{MatchClock, forbidden_items};
use crate::sim::abilities::Camouflage;
use crate::sim::ai::AiCommanders;
use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::components::*;
use crate::sim::determinism::{SimEntities, SimId};
use crate::sim::economy::Economies;
use crate::sim::fog::Sight;
use crate::sim::medical::Healer;
use crate::sim::notifications::{News, SideNews};
use crate::sim::production::{BuildItem, Production};
use crate::sim::sides::Sides;
use crate::sim::sorties::Sortie;
use crate::sim::spawn::TURRET_TURN_RATE;
use crate::sim::structures::BeingSold;
use crate::sim::veterancy::Veterancy;
use crate::world::map::{CELL_SIZE, footprint_center};

/// For this long after the age turns no structure takes damage, so every side has time to promote.
pub const CALM_AFTER_THE_AGE_TURNS_SECS: f32 = 600.0;
/// How near a unit must stand to a structure that builds its successor to be promoted there.
pub const PROMOTION_REACH_CELLS: f32 = 12.0;

/// When the age last turned, in seconds of the match clock; `None` before it ever has.
#[derive(Resource, Clone, Copy, Default, Debug)]
pub struct AgeTurned(pub Option<f32>);

pub fn turn_the_age(
    level: Res<ActiveLevel>,
    clock: Res<MatchClock>,
    sides: Res<Sides>,
    mut turned: ResMut<AgeTurned>,
    mut production: ResMut<Production>,
    mut economies: ResMut<Economies>,
    mut news: MessageWriter<News>,
) {
    let era = level.0.era;
    if production.era() != era {
        production.set_era(era);
        production.forbid_to_every_side(forbidden_items(&level.0));
        production.refund_other_ages(&mut economies, sides.iter());
        turned.0 = Some(clock.elapsed_secs);
        for team in sides.iter() {
            news.write(News::to(team, SideNews::AgeTurned(era)));
        }
    }
    let calm = turned
        .0
        .is_some_and(|at| clock.elapsed_secs - at < CALM_AFTER_THE_AGE_TURNS_SECS);
    if production.structures_spared() != calm {
        production.spare_structures(calm);
    }
}

/// Whether a structure of `team` that builds `into` stands near enough to `position` to promote there.
pub fn near_a_producer<'a>(
    position: Vec3,
    into: UnitKind,
    team: Team,
    buildings: impl IntoIterator<Item = (&'a Building, &'a Team)>,
) -> bool {
    let producer = into.spec().producer;
    let reach = PROMOTION_REACH_CELLS * CELL_SIZE;
    buildings.into_iter().any(|(building, owner)| {
        *owner == team
            && building.kind == producer
            && footprint_center(building.origin, producer.spec().footprint)
                .with_y(position.y)
                .distance(position)
                <= reach
    })
}

/// The unit becomes its successor where it stands, keeping its share of health, its rank and its orders.
/// When it fell or changed hands before its promotion was done, its side gets the price back.
pub fn promote_unit(world: &mut World, team: Team, id: SimId, into: UnitKind) {
    let item = BuildItem::Promotion { unit: id, into };
    let Some(entity) = own_entity(world, team, id) else {
        refund(world, team, item);
        return;
    };
    let mut unit = world.entity_mut(entity);
    let Some(&Unit(from)) = unit
        .get::<Unit>()
        .filter(|unit| unit.0.line() == into.line())
    else {
        refund(world, team, item);
        return;
    };
    let spec = into.spec();
    let mut health_factor = 1.0;
    if let Some(mut veterancy) = unit.get_mut::<Veterancy>() {
        veterancy.experience *= spec.cost as f32 / from.spec().cost as f32;
        health_factor = veterancy.rank.health_factor();
    }
    let share = unit.get::<Health>().map_or(1.0, Health::fraction);
    let max = spec.max_health * health_factor;
    unit.insert((
        Unit(into),
        Health {
            current: max * share,
            max,
        },
        BodyRadius(spec.radius),
        Sight(spec.sight),
        Armor(spec.armor),
    ));
    let mut facing = 0.0;
    if let Some(mut mobile) = unit.get_mut::<Mobile>() {
        mobile.speed = spec.speed;
        mobile.turn_rate = spec.turn_rate;
        mobile.flying = into.flies();
        facing = mobile.facing;
    }
    arm(&mut unit, spec.weapon, spec.has_turret, facing);
    match spec.heal_per_sec > 0.0 {
        true => unit.insert(Healer {
            per_sec: spec.heal_per_sec,
            patients: spec.armor,
            treating: false,
        }),
        false => unit.remove::<Healer>(),
    };
    match (into.hides_when_still(), unit.contains::<Camouflage>()) {
        (true, false) => {
            unit.insert(Camouflage::default());
        }
        (false, true) => {
            unit.remove::<Camouflage>();
        }
        _ => {}
    }
    match (into.lands_on_pad(), unit.contains::<Sortie>()) {
        (true, false) => {
            unit.insert(Sortie::new(into.rearms_at_pad()));
        }
        (false, true) => {
            unit.remove::<Sortie>();
        }
        _ => {}
    }
    match into.flies() {
        true => unit.insert(Aircraft),
        false => unit.remove::<Aircraft>(),
    };
}

/// The defence is rebuilt as its successor on the same spot, keeping its share of health.
pub fn promote_structure(world: &mut World, team: Team, id: SimId, into: BuildingKind) {
    let item = BuildItem::Fortification {
        structure: id,
        into,
    };
    let standing =
        own_entity(world, team, id).filter(|&entity| !world.entity(entity).contains::<BeingSold>());
    let Some(entity) = standing else {
        refund(world, team, item);
        return;
    };
    let mut structure = world.entity_mut(entity);
    let Some(&Building { origin, .. }) = structure
        .get::<Building>()
        .filter(|building| building.kind.defence_line() == into.defence_line())
    else {
        refund(world, team, item);
        return;
    };
    let spec = into.spec();
    let share = structure.get::<Health>().map_or(1.0, Health::fraction);
    structure.insert((
        Building { kind: into, origin },
        Health {
            current: spec.max_health * share,
            max: spec.max_health,
        },
        BodyRadius(spec.footprint.as_vec2().max_element() * CELL_SIZE * 0.5),
        Sight(spec.sight),
    ));
    arm(&mut structure, spec.weapon, spec.weapon.is_some(), 0.0);
}

fn own_entity(world: &World, team: Team, id: SimId) -> Option<Entity> {
    let entity = world.resource::<SimEntities>().get(id)?;
    (world.get::<Team>(entity) == Some(&team)).then_some(entity)
}

/// How often a computer side looks for what to promote, and what it keeps back for new units meanwhile.
const COMPUTER_PROMOTION_INTERVAL_SECS: f32 = 5.0;
const COMPUTER_PROMOTION_RESERVE: i32 = 500;
const COMPUTER_PROMOTIONS_QUEUED: usize = 2;

/// A computer side promotes its defences first, then its dearest units, while it can spare the credits.
pub fn promote_computer_sides(
    commanders: Res<AiCommanders>,
    clock: Res<MatchClock>,
    time: Res<Time>,
    mut production: ResMut<Production>,
    mut economies: ResMut<Economies>,
    units: Query<(&SimId, &Unit, &Team, &Transform)>,
    structures: Query<(&SimId, &Building, &Team)>,
) {
    let interval = |secs: f32| (secs / COMPUTER_PROMOTION_INTERVAL_SECS).floor();
    let due = interval(clock.elapsed_secs) != interval(clock.elapsed_secs - time.delta_secs());
    if !due {
        return;
    }
    let era = production.era();
    for ai in &commanders.0 {
        let team = ai.team;
        let own_buildings: Vec<(&Building, &Team)> = structures
            .iter()
            .filter(|(_, _, owner)| **owner == team)
            .map(|(_, building, owner)| (building, owner))
            .collect();
        let owned: Vec<BuildingKind> = own_buildings.iter().map(|(b, _)| b.kind).collect();
        let mut defences: Vec<BuildItem> = structures
            .iter()
            .filter(|(_, _, owner)| **owner == team)
            .filter_map(|(&id, building, _)| {
                let into = building.kind.promoted_into(era)?;
                Some(BuildItem::Fortification {
                    structure: id,
                    into,
                })
            })
            .collect();
        defences.sort_by_key(|item| match item {
            BuildItem::Fortification { structure, .. } => *structure,
            _ => SimId(0),
        });
        let mut army: Vec<(i32, SimId, BuildItem)> = units
            .iter()
            .filter(|(_, _, owner, _)| **owner == team)
            .filter_map(|(&id, unit, _, transform)| {
                let into = unit.0.promoted_into(era)?;
                near_a_producer(
                    transform.translation,
                    into,
                    team,
                    own_buildings.iter().copied(),
                )
                .then_some((
                    -into.spec().cost,
                    id,
                    BuildItem::Promotion { unit: id, into },
                ))
            })
            .collect();
        army.sort_by_key(|&(dearest_first, id, _)| (dearest_first, id));
        let wanted = defences
            .into_iter()
            .chain(army.into_iter().map(|(.., item)| item));
        for item in wanted {
            let id = match item {
                BuildItem::Promotion { unit, .. } => unit,
                BuildItem::Fortification { structure, .. } => structure,
                _ => continue,
            };
            let lane_busy =
                production.lane(team, item.lane()).queue.len() >= COMPUTER_PROMOTIONS_QUEUED;
            if lane_busy || production.is_promoting(team, id) {
                continue;
            }
            if economies.of(team).credits < item.cost() + COMPUTER_PROMOTION_RESERVE {
                break;
            }
            let _ = production.enqueue(&mut economies, team, item, &owned);
        }
    }
}

fn arm(
    entity: &mut EntityWorldMut,
    weapon: Option<crate::sim::catalog::WeaponSpec>,
    turret: bool,
    facing: f32,
) {
    match weapon {
        Some(spec) => {
            entity.insert(Weapon {
                spec,
                cooldown: 0.0,
            });
            if !entity.contains::<AttackTarget>() {
                entity.insert(AttackTarget::default());
            }
            if !entity.contains::<Stance>() && entity.contains::<Unit>() {
                entity.insert(Stance::default());
            }
        }
        None => {
            entity.remove::<(Weapon, AttackTarget, Stance)>();
        }
    }
    match turret {
        true if !entity.contains::<Turret>() => {
            entity.insert(Turret::new(facing, TURRET_TURN_RATE));
        }
        true => {}
        false => {
            entity.remove::<Turret>();
        }
    }
}

fn refund(world: &mut World, team: Team, item: BuildItem) {
    world.resource_mut::<Economies>().of_mut(team).credits += item.cost();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_promotion_costs_less_than_building_new() {
        for kind in UnitKind::ALL {
            let Some(into) = kind.promoted_into(crate::sim::eras::Era::Modern) else {
                continue;
            };
            let promotion = BuildItem::Promotion {
                unit: SimId(1),
                into,
            };
            assert!(promotion.cost() < into.spec().cost, "{kind:?}");
            assert!(promotion.build_secs() < into.spec().build_secs, "{kind:?}");
        }
    }
}
