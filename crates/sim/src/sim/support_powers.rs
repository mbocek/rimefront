use bevy::prelude::*;
use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::sim::catalog::{BuildingKind, ProjectileStyle, Team, WeaponSpec};
use crate::sim::combat::drop_shell;
use crate::sim::components::Building;
use crate::sim::cues::{Sound, SoundCue};
use crate::sim::determinism::SimulationRng;
use crate::sim::sides::PerSide;
use crate::world::map::cell_center;

const RECON_RADIUS: f32 = 16.0;
const RECON_SECS: f32 = 20.0;
const BARRAGE_WARNING_SECS: f32 = 3.0;
const BARRAGE_SHELLS: usize = 10;
const BARRAGE_SCATTER: f32 = 7.0;
/// Seconds between one shell of a barrage and the next.
const BARRAGE_SHELL_INTERVAL_SECS: f32 = 0.25;

const BARRAGE_SHELL: WeaponSpec = WeaponSpec {
    range: 0.0,
    damage: 70.0,
    reload_secs: 0.0,
    projectile_speed: 45.0,
    splash_radius: 3.0,
    style: ProjectileStyle::ArtilleryShell,
    versus_infantry: 1.0,
    versus_vehicle: 0.8,
    versus_structure: 0.6,
    versus_aircraft: 0.0,
    report: Sound::ShellExplosion,
};

/// Called down from the sidebar onto any spot of the map, then recharged before the next call.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SupportPower {
    /// A reconnaissance flight reveals the ground around the spot for a while.
    ReconSweep,
    /// Shells rain down around the spot after a short warning.
    Barrage,
}

impl SupportPower {
    pub const ALL: [SupportPower; 2] = [SupportPower::ReconSweep, SupportPower::Barrage];

    fn index(self) -> usize {
        match self {
            SupportPower::ReconSweep => 0,
            SupportPower::Barrage => 1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SupportPower::ReconSweep => crate::i18n::tr("Recon flight"),
            SupportPower::Barrage => crate::i18n::tr("Artillery barrage"),
        }
    }

    pub fn recharge_secs(self) -> f32 {
        match self {
            SupportPower::ReconSweep => 120.0,
            SupportPower::Barrage => 180.0,
        }
    }

    /// The structure a side must hold to call this power.
    pub fn requires(self) -> BuildingKind {
        match self {
            SupportPower::ReconSweep => BuildingKind::CommandCenter,
            SupportPower::Barrage => BuildingKind::TechCenter,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ReconFlight {
    team: Team,
    centre: Vec3,
    secs_left: f32,
}

#[derive(Clone, Copy, Debug)]
struct Barrage {
    team: Team,
    centre: Vec3,
    secs_until_next_shell: f32,
    shells_left: usize,
}

/// Every side's recharge of every power, and the flights and barrages under way.
#[derive(Resource, Clone, Debug)]
pub struct SupportPowers {
    /// Seconds until each power is ready again, indexed by power.
    recharge: PerSide<[f32; SupportPower::ALL.len()]>,
    recon_flights: Vec<ReconFlight>,
    barrages: Vec<Barrage>,
}

/// Powers start charging when the match begins, so nobody opens with a barrage.
impl Default for SupportPowers {
    fn default() -> Self {
        let mut recharge = PerSide::new(SupportPower::ALL.map(SupportPower::recharge_secs));
        for team in Team::ALL {
            recharge.of_mut(team);
        }
        Self {
            recharge,
            recon_flights: Vec::new(),
            barrages: Vec::new(),
        }
    }
}

impl SupportPowers {
    pub fn secs_until_ready(&self, team: Team, power: SupportPower) -> f32 {
        self.recharge.of(team)[power.index()].max(0.0)
    }

    /// `team`'s powers start charging; only the sides with a recharge of their own count down.
    pub fn admit(&mut self, team: Team) {
        self.recharge.of_mut(team);
    }

    /// `team`'s powers charge from the start again.
    pub fn forget(&mut self, team: Team) {
        self.recharge.forget(team);
    }

    pub fn set_secs_until_ready(&mut self, team: Team, power: SupportPower, secs: f32) {
        self.recharge.of_mut(team)[power.index()] = secs;
    }

    pub fn is_charged(&self, team: Team, power: SupportPower) -> bool {
        self.secs_until_ready(team, power) <= 0.0
    }

    /// Calls `power` onto `cell` if `team` may; returns whether it was called.
    pub fn call(
        &mut self,
        team: Team,
        power: SupportPower,
        cell: IVec2,
        owned: &[BuildingKind],
    ) -> bool {
        if !self.is_charged(team, power) || !owned.contains(&power.requires()) {
            return false;
        }
        self.recharge.of_mut(team)[power.index()] = power.recharge_secs();
        let centre = cell_center(cell);
        match power {
            SupportPower::ReconSweep => self.recon_flights.push(ReconFlight {
                team,
                centre,
                secs_left: RECON_SECS,
            }),
            SupportPower::Barrage => self.barrages.push(Barrage {
                team,
                centre,
                secs_until_next_shell: BARRAGE_WARNING_SECS,
                shells_left: BARRAGE_SHELLS,
            }),
        }
        true
    }

    /// Spots revealed by recon flights, with the side that flew each and how far it sees.
    pub fn revealed_by_recon(&self) -> impl Iterator<Item = (Team, Vec3, f32)> + '_ {
        self.recon_flights
            .iter()
            .map(|flight| (flight.team, flight.centre, RECON_RADIUS))
    }
}

/// Recharges powers, ends recon flights and drops the shells of barrages as they come due.
pub fn run_support_powers(
    mut commands: Commands,
    time: Res<Time>,
    mut powers: ResMut<SupportPowers>,
    mut rng: ResMut<SimulationRng>,
    mut cues: MessageWriter<SoundCue>,
) {
    let dt = time.delta_secs();
    for team_recharge in powers.recharge.iter_mut() {
        for secs in team_recharge.iter_mut() {
            *secs = (*secs - dt).max(0.0);
        }
    }
    powers.recon_flights.retain_mut(|flight| {
        flight.secs_left -= dt;
        flight.secs_left > 0.0
    });
    for barrage in &mut powers.barrages {
        barrage.secs_until_next_shell -= dt;
        while barrage.secs_until_next_shell <= 0.0 && barrage.shells_left > 0 {
            barrage.secs_until_next_shell += BARRAGE_SHELL_INTERVAL_SECS;
            barrage.shells_left -= 1;
            let offset = Vec3::new(
                rng.random_range(-BARRAGE_SCATTER..BARRAGE_SCATTER),
                0.0,
                rng.random_range(-BARRAGE_SCATTER..BARRAGE_SCATTER),
            );
            let impact = barrage.centre + offset;
            drop_shell(&mut commands, barrage.team, impact, BARRAGE_SHELL);
            cues.write(SoundCue::at(Sound::HowitzerShot, impact));
        }
    }
    powers.barrages.retain(|barrage| barrage.shells_left > 0);
}

/// The kinds of structure `team` holds, for checking what it may call.
pub fn owned_kinds(buildings: &Query<(&Building, &Team)>, team: Team) -> Vec<BuildingKind> {
    buildings
        .iter()
        .filter(|(_, owner)| **owner == team)
        .map(|(building, _)| building.kind)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_power_waits_for_its_charge_and_its_structure() {
        let mut powers = SupportPowers::default();
        let owned = [BuildingKind::CommandCenter];
        assert!(!powers.call(Team::Azure, SupportPower::ReconSweep, IVec2::ZERO, &owned));
        powers.recharge.set_all([0.0; 2]);
        assert!(!powers.call(Team::Azure, SupportPower::Barrage, IVec2::ZERO, &owned));
        assert!(powers.call(Team::Azure, SupportPower::ReconSweep, IVec2::ZERO, &owned));
        assert!(!powers.is_charged(Team::Azure, SupportPower::ReconSweep));
        assert!(powers.is_charged(Team::Crimson, SupportPower::ReconSweep));
    }
}
