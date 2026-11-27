use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::components::*;
use crate::sim::spawn::{spawn_building, spawn_unit};
use crate::sim::structures::Repairing;
use crate::sim::veterancy::Veterancy;
use crate::world::map::GameMap;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SavedBuilding {
    team: Team,
    kind: BuildingKind,
    origin: [i32; 2],
    health: f32,
    #[serde(default)]
    rally: Option<[i32; 2]>,
    #[serde(default)]
    repairing: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SavedUnit {
    team: Team,
    kind: UnitKind,
    position: [f32; 3],
    facing: f32,
    health: f32,
    cargo: u32,
    #[serde(default)]
    orders: Vec<Order>,
    #[serde(default)]
    experience: f32,
    #[serde(default)]
    stance: Stance,
}

pub type SavableBuilding<'a> = (
    &'a Building,
    &'a Team,
    &'a Health,
    Option<&'a RallyPoint>,
    Has<Repairing>,
);

pub type SavableUnit<'a> = (
    &'a Unit,
    &'a Team,
    &'a Transform,
    &'a Mobile,
    &'a Health,
    Option<&'a Harvester>,
    Option<&'a Orders>,
    Option<&'a Veterancy>,
    Option<&'a Stance>,
);

impl SavedBuilding {
    pub fn team(&self) -> Team {
        self.team
    }

    pub fn origin(&self) -> IVec2 {
        IVec2::from_array(self.origin)
    }

    pub fn of(
        (building, &team, health, rally, repairing): (
            &Building,
            &Team,
            &Health,
            Option<&RallyPoint>,
            bool,
        ),
    ) -> Self {
        Self {
            team,
            kind: building.kind,
            origin: building.origin.to_array(),
            health: health.current,
            rally: rally.map(|rally| rally.0.to_array()),
            repairing,
        }
    }
}

impl SavedUnit {
    pub fn team(&self) -> Team {
        self.team
    }

    pub fn position(&self) -> Vec3 {
        Vec3::from_array(self.position)
    }

    pub fn of(
        (unit, &team, transform, mobile, health, harvester, orders, veterancy, stance): SavableUnit,
    ) -> Self {
        Self {
            team,
            kind: unit.0,
            position: transform.translation.to_array(),
            facing: mobile.facing,
            health: health.current,
            cargo: harvester.map_or(0, |h| h.cargo),
            orders: orders.map_or_else(Vec::new, |orders| {
                orders
                    .current
                    .iter()
                    .chain(orders.queued.iter())
                    .copied()
                    .collect()
            }),
            experience: veterancy.map_or(0.0, |veterancy| veterancy.experience),
            stance: stance.copied().unwrap_or_default(),
        }
    }
}

pub fn respawn_building(commands: &mut Commands, map: &mut GameMap, saved: &SavedBuilding) {
    let origin = IVec2::from_array(saved.origin);
    let entity = spawn_building(commands, map, saved.kind, saved.team, origin);
    commands.entity(entity).insert(Health {
        current: saved.health,
        max: saved.kind.spec().max_health,
    });
    if let Some(rally) = saved.rally {
        commands
            .entity(entity)
            .insert(RallyPoint(IVec2::from_array(rally)));
    }
    if saved.repairing {
        commands.entity(entity).insert(Repairing::default());
    }
}

pub fn respawn_unit(commands: &mut Commands, saved: &SavedUnit) {
    let position = Vec3::from_array(saved.position);
    let entity = spawn_unit(commands, saved.kind, saved.team, position, saved.facing);
    let (veterancy, health) = Veterancy::restored(saved.kind, saved.experience, saved.health);
    commands.entity(entity).insert(health);
    if saved.kind.spec().weapon.is_some() {
        commands.entity(entity).insert(saved.stance);
    }
    if saved.experience > 0.0 {
        commands.entity(entity).insert(veterancy);
    }
    let mut planned = saved.orders.iter().copied();
    commands.entity(entity).insert(Orders {
        current: planned.next(),
        queued: planned.collect(),
    });
    if saved.kind.is_harvester() {
        commands.entity(entity).insert(Harvester {
            state: HarvestState::SeekingField,
            cargo: saved.cargo,
            preferred_field: None,
        });
    }
}
