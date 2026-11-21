use std::collections::HashSet;

use bevy::prelude::*;

use crate::assets::ModelLibrary;
use crate::catalog::{BuildingKind, Diplomacy, Team};
use crate::components::{Building, Health, Mobile, Orders};
use crate::determinism::{SimId, in_simulation_order};
use crate::i18n::{tr, tr_fmt};
use crate::map::{CELL_SIZE, GameMap, footprint_center};
use crate::movement::order_move;
use crate::notifications::Notifications;
use crate::player_commands::LocalTeam;
use crate::sound::{Sound, SoundCue};
use crate::spawn::spawn_building;

/// How close to a structure's walls an engineer has to get before it slips inside.
const ENTRY_REACH: f32 = 1.6;
const REPATH_SECS: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EngineerJob {
    Capture,
    Repair,
}

#[derive(Component, Debug)]
pub struct EngineerTask {
    pub building: Entity,
    pub job: EngineerJob,
    repath_secs: f32,
}

impl EngineerTask {
    pub fn new(building: Entity, job: EngineerJob) -> Self {
        Self {
            building,
            job,
            repath_secs: 0.0,
        }
    }
}

/// What an engineer of `team` would do to a structure: capture an enemy or unclaimed one (`owner` `None`),
/// fully repair a damaged own one.
pub fn job_for(
    diplomacy: Diplomacy,
    team: Team,
    owner: Option<Team>,
    kind: BuildingKind,
    health: &Health,
) -> Option<EngineerJob> {
    if kind.is_wall() {
        return None;
    }
    let Some(owner) = owner else {
        return Some(EngineerJob::Capture);
    };
    if diplomacy.hostile(owner, team) {
        return Some(EngineerJob::Capture);
    }
    (owner == team && health.current < health.max).then_some(EngineerJob::Repair)
}

fn reach_of(building: &Building) -> f32 {
    building.kind.spec().footprint.as_vec2().max_element() * CELL_SIZE * 0.5 + ENTRY_REACH
}

pub fn carry_out_engineer_tasks(
    local: Res<LocalTeam>,
    mut commands: Commands,
    mut cues: MessageWriter<SoundCue>,
    mut notifications: ResMut<Notifications>,
    time: Res<Time>,
    models: Res<ModelLibrary>,
    mut map: ResMut<GameMap>,
    mut engineers: Query<(
        Entity,
        &Team,
        &Transform,
        &mut Mobile,
        &mut EngineerTask,
        Option<&mut Orders>,
        &SimId,
    )>,
    mut buildings: Query<(&Building, Option<&Team>, &mut Health)>,
    diplomacy: Res<Diplomacy>,
) {
    let mut entered = HashSet::new();
    let in_order = in_simulation_order(engineers.iter_mut(), |item| *item.6);
    for (engineer, &team, transform, mut mobile, mut task, orders, _) in in_order {
        let still_standing = !entered.contains(&task.building);
        let Some((building, owner, mut health)) = buildings
            .get_mut(task.building)
            .ok()
            .filter(|_| still_standing)
        else {
            commands.entity(engineer).remove::<EngineerTask>();
            continue;
        };
        let owner = owner.copied();
        if job_for(*diplomacy, team, owner, building.kind, &health) != Some(task.job) {
            commands.entity(engineer).remove::<EngineerTask>();
            continue;
        }
        let footprint = building.kind.spec().footprint;
        let center = footprint_center(building.origin, footprint);
        let distance = transform.translation.xz().distance(center.xz());
        if distance > reach_of(building) {
            task.repath_secs -= time.delta_secs();
            if task.repath_secs <= 0.0 || !mobile.is_moving() {
                task.repath_secs = REPATH_SECS;
                order_move(
                    &mut mobile,
                    &map,
                    transform.translation,
                    building.origin,
                    true,
                );
            }
            if let Some(mut orders) = orders {
                orders.clear();
            }
            continue;
        }

        entered.insert(task.building);
        match task.job {
            EngineerJob::Repair => health.current = health.max,
            EngineerJob::Capture => {
                let (kind, origin, damage) =
                    (building.kind, building.origin, health.max - health.current);
                commands.entity(task.building).despawn();
                map.set_building(origin, footprint, None);
                let captured = spawn_building(&mut commands, &models, &mut map, kind, team, origin);
                commands.entity(captured).insert(Health {
                    current: kind.spec().max_health - damage,
                    max: kind.spec().max_health,
                });
                if owner == Some(local.0) {
                    notifications
                        .push(tr_fmt("{} captured by the enemy", &[&tr(kind.spec().name)]));
                    cues.write(SoundCue::interface(Sound::StructureLost));
                }
            }
        }
        if team == local.0 {
            let message = match task.job {
                EngineerJob::Capture => tr_fmt("{} captured", &[&tr(building.kind.spec().name)]),
                EngineerJob::Repair => tr_fmt("{} repaired", &[&tr(building.kind.spec().name)]),
            };
            notifications.push(message);
            let sound = match task.job {
                EngineerJob::Capture => Sound::StructureCaptured,
                EngineerJob::Repair => Sound::RepairStarted,
            };
            cues.write(SoundCue::at(sound, center));
        }
        commands.entity(engineer).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Alliance;

    #[test]
    fn engineers_capture_unclaimed_sites() {
        let site = BuildingKind::SupplyDepot;
        let intact = Health::full(100.0);
        assert_eq!(
            job_for(Diplomacy::default(), Team::Crimson, None, site, &intact),
            Some(EngineerJob::Capture)
        );
    }

    #[test]
    fn engineers_capture_enemies_repair_friends_and_ignore_walls() {
        let damaged = Health {
            current: 10.0,
            max: 100.0,
        };
        let intact = Health::full(100.0);
        let allied = Diplomacy::default();
        let plant = BuildingKind::PowerPlant;
        assert_eq!(
            job_for(allied, Team::Player, Some(Team::Crimson), plant, &intact),
            Some(EngineerJob::Capture)
        );
        assert_eq!(
            job_for(allied, Team::Player, Some(Team::Player), plant, &damaged),
            Some(EngineerJob::Repair)
        );
        assert_eq!(
            job_for(allied, Team::Player, Some(Team::Player), plant, &intact),
            None
        );
        assert_eq!(
            job_for(
                allied,
                Team::Player,
                Some(Team::Crimson),
                BuildingKind::Wall,
                &intact
            ),
            None
        );
        assert_eq!(
            job_for(allied, Team::Crimson, Some(Team::Amber), plant, &intact),
            None
        );
        let free_for_all = Diplomacy {
            alliance: Alliance::FreeForAll,
        };
        assert_eq!(
            job_for(
                free_for_all,
                Team::Crimson,
                Some(Team::Amber),
                plant,
                &intact
            ),
            Some(EngineerJob::Capture)
        );
    }
}
