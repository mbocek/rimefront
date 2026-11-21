use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::catalog::{Diplomacy, Team};
use crate::components::{Building, Unit};
use crate::determinism::SimId;
use crate::fog::{FogOfWar, Sight};
use crate::map::{base_sites, cell_center};

const UPDATE_INTERVAL_SECS: f32 = 0.2;

/// What each team knows: every side sees what its own and its allies' units and buildings see, and
/// enemy structures stay known once spotted.
#[derive(Resource, Clone)]
pub struct Intel {
    /// Indexed by `Team::index`.
    visions: [FogOfWar; Team::ALL.len()],
    /// Keyed by simulation id, so every machine visits them in the same order.
    known_structures: [BTreeMap<SimId, (Entity, Vec3)>; 4],
}

impl Intel {
    pub fn new(fog_of_war: bool) -> Self {
        Self {
            visions: std::array::from_fn(|_| FogOfWar::new(fog_of_war)),
            known_structures: Default::default(),
        }
    }

    pub fn vision(&self, viewer: Team) -> &FogOfWar {
        &self.visions[viewer.index()]
    }

    pub fn vision_mut(&mut self, viewer: Team) -> &mut FogOfWar {
        &mut self.visions[viewer.index()]
    }

    /// Last known positions of enemy structures that `viewer`'s side has spotted and not seen destroyed.
    pub fn known_enemy_structures(&self, viewer: Team) -> impl Iterator<Item = Vec3> + '_ {
        self.known_structures[viewer.index()]
            .values()
            .map(|&(_, position)| position)
    }

    pub fn known_enemy_structure_entities(
        &self,
        viewer: Team,
    ) -> impl Iterator<Item = (Entity, Vec3)> + '_ {
        self.known_structures[viewer.index()].values().copied()
    }

    /// Where to look for an enemy nobody has found: the nearest base site never explored, or failing that
    /// the nearest one out of sight right now.
    pub fn base_site_to_search(&self, viewer: Team, from: Vec3) -> Option<Vec3> {
        let vision = self.vision(viewer);
        let sites: Vec<Vec3> = base_sites()
            .iter()
            .map(|&site| cell_center(site + IVec2::ONE))
            .collect();
        let nearest = |candidates: Vec<Vec3>| {
            candidates
                .into_iter()
                .min_by(|a, b| a.distance(from).total_cmp(&b.distance(from)))
        };
        let unexplored = sites
            .iter()
            .copied()
            .filter(|&site| !vision.is_explored(site))
            .collect();
        let out_of_sight = sites
            .iter()
            .copied()
            .filter(|&site| !vision.is_visible(site))
            .collect();
        nearest(unexplored).or_else(|| nearest(out_of_sight))
    }
}

impl Default for Intel {
    fn default() -> Self {
        Self::new(true)
    }
}

pub fn update_intel(
    time: Res<Time>,
    mut cadences: ResMut<crate::cadence::SimulationCadences>,
    mut intel: ResMut<Intel>,
    diplomacy: Res<Diplomacy>,
    clock: Res<crate::match_flow::MatchClock>,
    forecast: Res<crate::weather::WeatherForecast>,
    support: Res<crate::support_powers::SupportPowers>,
    viewers: Query<(&Team, &Transform, &Sight), Or<(With<Unit>, With<Building>)>>,
    buildings: Query<(Entity, &Building, &Team, &Transform, &SimId)>,
) {
    if !cadences
        .intel
        .is_due(time.delta_secs(), UPDATE_INTERVAL_SECS)
    {
        return;
    }
    let weather_sight = forecast.sight_factor(clock.elapsed_secs);
    for viewer in Team::ALL {
        let allied_eyes = viewers
            .iter()
            .filter(|(team, ..)| !diplomacy.hostile(**team, viewer))
            .map(|(_, transform, sight)| (transform.translation, sight.0 * weather_sight));
        let allied_flights = support
            .revealed_by_recon()
            .filter(|(team, ..)| !diplomacy.hostile(*team, viewer))
            .map(|(_, centre, radius)| (centre, radius));
        intel
            .vision_mut(viewer)
            .reveal_around(allied_eyes.chain(allied_flights));
    }

    let intel = &mut *intel;
    for known in &mut intel.known_structures {
        known.retain(|_, &mut (entity, _)| buildings.contains(entity));
    }
    for (entity, building, &team, transform, &id) in &buildings {
        if building.kind.is_wall() {
            continue;
        }
        let position = transform.translation;
        for viewer in Team::ALL {
            let spotted =
                diplomacy.hostile(team, viewer) && intel.vision(viewer).is_visible(position);
            if spotted {
                intel.known_structures[viewer.index()].insert(id, (entity, position));
            }
        }
    }
}
