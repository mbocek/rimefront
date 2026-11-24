use std::collections::HashMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::i18n::{tr, tr_fmt};
use crate::sim::catalog::{Team, UnitKind};
use crate::sim::combat::{DamageDealt, DamageSource, Destroyed};
use crate::sim::components::{Health, Unit};
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::notifications::Notifications;
use crate::sim::player_commands::LocalTeam;
use crate::sim::structures::structure_value;

/// Experience is counted in credits destroyed; a rank is earned at this many times the unit's own price.
const VETERAN_AT_OWN_PRICE: f32 = 1.0;
const ELITE_AT_OWN_PRICE: f32 = 3.0;
const ELITE_HEAL_SHARE_PER_SEC: f32 = 0.01;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serialize, Deserialize)]
pub enum Rank {
    #[default]
    Recruit,
    Veteran,
    Elite,
}

impl Rank {
    pub fn earned(kind: UnitKind, experience: f32) -> Rank {
        let price = kind.spec().cost as f32;
        if experience >= price * ELITE_AT_OWN_PRICE {
            Rank::Elite
        } else if experience >= price * VETERAN_AT_OWN_PRICE {
            Rank::Veteran
        } else {
            Rank::Recruit
        }
    }

    pub fn damage_factor(self) -> f32 {
        match self {
            Rank::Recruit => 1.0,
            Rank::Veteran => 1.2,
            Rank::Elite => 1.4,
        }
    }

    pub fn health_factor(self) -> f32 {
        self.damage_factor()
    }

    pub fn chevrons(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Rank::Recruit => tr("Recruit"),
            Rank::Veteran => tr("Veteran"),
            Rank::Elite => tr("Elite"),
        }
    }
}

#[derive(Component, Clone, Copy, Default, Debug)]
pub struct Veterancy {
    pub experience: f32,
    pub rank: Rank,
}

impl Veterancy {
    /// A veteran coming back from a save: rank and health limit follow from the experience.
    pub fn restored(kind: UnitKind, experience: f32, health: f32) -> (Veterancy, Health) {
        let rank = Rank::earned(kind, experience);
        let veterancy = Veterancy { experience, rank };
        let max = kind.spec().max_health * rank.health_factor();
        (
            veterancy,
            Health {
                current: health.min(max),
                max,
            },
        )
    }
}

/// Who landed the latest hit on each victim, so the kill can be credited when it dies.
#[derive(Resource, Default)]
pub struct LastHits(HashMap<Entity, Entity>);

pub fn record_last_hits(mut hits: MessageReader<DamageDealt>, mut last_hits: ResMut<LastHits>) {
    for hit in hits.read() {
        last_hits.0.insert(hit.victim, hit.shooter);
    }
}

fn bounty(what: DamageSource) -> f32 {
    match what {
        DamageSource::Unit(kind) => kind.spec().cost as f32,
        DamageSource::Building(kind) => structure_value(kind) as f32,
        DamageSource::Support => 0.0,
    }
}

/// Units earn the value of what they destroy; structures that shoot do not gain experience.
pub fn credit_kills(
    mut commands: Commands,
    mut losses: MessageReader<Destroyed>,
    mut last_hits: ResMut<LastHits>,
    mut units: Query<Option<&mut Veterancy>, With<Unit>>,
) {
    for loss in losses.read() {
        let Some(killer) = last_hits.0.remove(&loss.entity) else {
            continue;
        };
        match units.get_mut(killer) {
            Ok(Some(mut veterancy)) => veterancy.experience += bounty(loss.what),
            Ok(None) => {
                commands.entity(killer).insert(Veterancy {
                    experience: bounty(loss.what),
                    rank: Rank::Recruit,
                });
            }
            Err(_) => {}
        }
    }
}

pub fn promote_veterans(
    local: Res<LocalTeam>,
    mut cues: MessageWriter<SoundCue>,
    mut notifications: ResMut<Notifications>,
    mut veterans: Query<(&Unit, &Team, &mut Veterancy, &mut Health), Changed<Veterancy>>,
) {
    for (unit, &team, mut veterancy, mut health) in &mut veterans {
        let earned = Rank::earned(unit.0, veterancy.experience);
        if earned <= veterancy.rank {
            continue;
        }
        let new_max = unit.0.spec().max_health * earned.health_factor();
        health.current += new_max - health.max;
        health.max = new_max;
        veterancy.rank = earned;
        if team == local.0 {
            notifications.push(tr_fmt(
                "{} promoted to {}",
                &[&tr(unit.0.spec().name), &earned.name()],
            ));
            cues.write(SoundCue::interface(Sound::UnitPromoted));
        }
    }
}

pub fn elite_field_repairs(time: Res<Time>, mut elites: Query<(&Veterancy, &mut Health)>) {
    for (veterancy, mut health) in &mut elites {
        if veterancy.rank == Rank::Elite && health.current > 0.0 {
            let healed = health.max * ELITE_HEAL_SHARE_PER_SEC * time.delta_secs();
            health.current = (health.current + healed).min(health.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_follow_destroyed_value_relative_to_own_price() {
        let tank_price = UnitKind::Tank.spec().cost as f32;
        assert_eq!(
            Rank::earned(UnitKind::Tank, tank_price * 0.9),
            Rank::Recruit
        );
        assert_eq!(Rank::earned(UnitKind::Tank, tank_price), Rank::Veteran);
        assert_eq!(Rank::earned(UnitKind::Tank, tank_price * 3.0), Rank::Elite);
    }

    #[test]
    fn a_restored_elite_keeps_its_larger_health_pool() {
        let price = UnitKind::Rifleman.spec().cost as f32;
        let (veterancy, health) = Veterancy::restored(UnitKind::Rifleman, price * 3.0, 60.0);
        assert_eq!(veterancy.rank, Rank::Elite);
        assert_eq!(health.max, UnitKind::Rifleman.spec().max_health * 1.4);
        assert_eq!(health.current, 60.0);
    }
}
