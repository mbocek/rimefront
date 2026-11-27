//! When the age turns: what was queued for the old one is refunded, structures are spared for a while, and
//! units and defences of the old age can be promoted into their successors. See docs/eras.md.

use bevy::prelude::*;

use crate::levels::ActiveLevel;
use crate::match_flow::forbidden_items;
use crate::sim::abilities::Camouflage;
use crate::sim::ai::AiCommanders;
use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::components::*;
use crate::sim::determinism::{SimEntities, SimId};
use crate::sim::economy::Economies;
use crate::sim::eras::Era;
use crate::sim::fog::Sight;
use crate::sim::medical::Healer;
use crate::sim::notifications::{News, SideNews};
use crate::sim::player_commands::SimulationTick;
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

/// The tick the age last turned on; `None` before it ever has.
#[derive(Resource, Clone, Copy, Default, Debug)]
pub struct AgeTurned(pub Option<u64>);

fn ticks_in(secs: f32) -> u64 {
    (secs / crate::SIMULATION_TICK.as_secs_f32()).round() as u64
}

/// A persistent world's age follows its schedule.
pub fn follow_the_age_schedule(tick: Res<SimulationTick>, mut level: ResMut<ActiveLevel>) {
    let Some(schedule) = level.0.ages else {
        return;
    };
    let era = schedule.era_at(tick.0);
    if level.0.era != era {
        level.0.era = era;
    }
}

pub fn turn_the_age(
    level: Res<ActiveLevel>,
    tick: Res<SimulationTick>,
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
        turned.0 = Some(tick.0);
        for team in sides.iter() {
            news.write(News::to(team, SideNews::AgeTurned(era)));
        }
    }
    let calm = turned
        .0
        .is_some_and(|at| tick.0.saturating_sub(at) < ticks_in(CALM_AFTER_THE_AGE_TURNS_SECS));
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

/// When the unit fell or changed hands before its promotion was done, its side gets the price back.
pub fn promote_unit(world: &mut World, team: Team, id: SimId, into: UnitKind) {
    if !make_successor_of_unit(world, team, id, into) {
        refund(world, team, BuildItem::Promotion { unit: id, into });
    }
}

/// The unit becomes its successor where it stands, keeping its share of health, its rank and its orders;
/// whether it still stood, on `team`'s side, to become one.
pub fn make_successor_of_unit(world: &mut World, team: Team, id: SimId, into: UnitKind) -> bool {
    let Some(entity) = own_entity(world, team, id) else {
        return false;
    };
    let mut unit = world.entity_mut(entity);
    let Some(&Unit(from)) = unit
        .get::<Unit>()
        .filter(|unit| unit.0.line() == into.line())
    else {
        return false;
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
    true
}

/// When the defence fell, changed hands or is being sold before it was rebuilt, its side gets the price back.
pub fn promote_structure(world: &mut World, team: Team, id: SimId, into: BuildingKind) {
    if !make_successor_of_structure(world, team, id, into) {
        let item = BuildItem::Fortification {
            structure: id,
            into,
        };
        refund(world, team, item);
    }
}

/// The defence is rebuilt as its successor on the same spot, keeping its share of health; whether it still
/// stood, on `team`'s side and not being sold, to be rebuilt.
pub fn make_successor_of_structure(
    world: &mut World,
    team: Team,
    id: SimId,
    into: BuildingKind,
) -> bool {
    let standing =
        own_entity(world, team, id).filter(|&entity| !world.entity(entity).contains::<BeingSold>());
    let Some(entity) = standing else {
        return false;
    };
    let mut structure = world.entity_mut(entity);
    let Some(&Building { origin, .. }) = structure
        .get::<Building>()
        .filter(|building| building.kind.defence_line() == into.defence_line())
    else {
        return false;
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
    true
}

/// Carries out a promotion at once, wherever the unit stands; whether it was done.
pub fn make_successor(world: &mut World, team: Team, item: BuildItem) -> bool {
    match item {
        BuildItem::Promotion { unit, into } => make_successor_of_unit(world, team, unit, into),
        BuildItem::Fortification { structure, into } => {
            make_successor_of_structure(world, team, structure, into)
        }
        _ => false,
    }
}

fn own_entity(world: &World, team: Team, id: SimId) -> Option<Entity> {
    let entity = world.resource::<SimEntities>().get(id)?;
    (world.get::<Team>(entity) == Some(&team)).then_some(entity)
}

/// How often a computer side looks for what to promote, and what it keeps back for new units meanwhile.
const COMPUTER_PROMOTION_INTERVAL_SECS: f32 = 5.0;
const COMPUTER_PROMOTION_RESERVE: i32 = 500;
const COMPUTER_PROMOTIONS_QUEUED: usize = 2;

/// What `team` would promote into `era`, most wanted first: its defences, then its dearest units. With
/// `at_producers`, only units near a structure that builds their successor.
pub fn promotions_wanted<'a>(
    team: Team,
    era: Era,
    units: impl IntoIterator<Item = (SimId, UnitKind, Team, Vec3)>,
    structures: impl IntoIterator<Item = (SimId, &'a Building, &'a Team)> + Clone,
    at_producers: bool,
) -> Vec<BuildItem> {
    let own_buildings: Vec<(&Building, &Team)> = structures
        .clone()
        .into_iter()
        .filter(|(_, _, owner)| **owner == team)
        .map(|(_, building, owner)| (building, owner))
        .collect();
    let mut defences: Vec<(SimId, BuildItem)> = structures
        .into_iter()
        .filter(|(_, _, owner)| **owner == team)
        .filter_map(|(id, building, _)| {
            let into = building.kind.promoted_into(era)?;
            Some((
                id,
                BuildItem::Fortification {
                    structure: id,
                    into,
                },
            ))
        })
        .collect();
    defences.sort_by_key(|&(id, _)| id);
    let mut army: Vec<(i32, SimId, BuildItem)> = units
        .into_iter()
        .filter(|&(_, _, owner, _)| owner == team)
        .filter_map(|(id, kind, _, position)| {
            let into = kind.promoted_into(era)?;
            let near = !at_producers
                || near_a_producer(position, into, team, own_buildings.iter().copied());
            near.then_some((
                -into.spec().cost,
                id,
                BuildItem::Promotion { unit: id, into },
            ))
        })
        .collect();
    army.sort_by_key(|&(dearest_first, id, _)| (dearest_first, id));
    defences
        .into_iter()
        .map(|(_, item)| item)
        .chain(army.into_iter().map(|(.., item)| item))
        .collect()
}

/// A computer side promotes its defences first, then its dearest units, while it can spare the credits.
pub fn promote_computer_sides(
    commanders: Res<AiCommanders>,
    tick: Res<SimulationTick>,
    mut production: ResMut<Production>,
    mut economies: ResMut<Economies>,
    units: Query<(&SimId, &Unit, &Team, &Transform)>,
    structures: Query<(&SimId, &Building, &Team)>,
) {
    if !tick
        .0
        .is_multiple_of(ticks_in(COMPUTER_PROMOTION_INTERVAL_SECS))
    {
        return;
    }
    let era = production.era();
    for ai in &commanders.0 {
        let team = ai.team;
        let owned: Vec<BuildingKind> = structures
            .iter()
            .filter(|(_, _, owner)| **owner == team)
            .map(|(_, building, _)| building.kind)
            .collect();
        let wanted = promotions_wanted(
            team,
            era,
            units
                .iter()
                .map(|(&id, unit, &owner, transform)| (id, unit.0, owner, transform.translation)),
            structures
                .iter()
                .map(|(&id, building, owner)| (id, building, owner)),
            true,
        );
        for item in wanted {
            let Some(id) = item.promoted() else {
                continue;
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
