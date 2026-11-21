use bevy::prelude::*;

use crate::catalog::Team;

/// Fires the first time it is asked, then once per interval of game time.
#[derive(Clone, Copy, Default)]
pub struct Cadence {
    countdown_secs: f32,
}

impl Cadence {
    pub fn is_due(&mut self, delta_secs: f32, interval_secs: f32) -> bool {
        self.countdown_secs -= delta_secs;
        if self.countdown_secs > 0.0 {
            return false;
        }
        self.countdown_secs = interval_secs;
        true
    }
}

/// The periodic rhythms of the simulation. They belong to one match, so a rematch starts them afresh like
/// every machine joining it does.
#[derive(Resource, Clone, Default)]
pub struct SimulationCadences {
    pub intel: Cadence,
    pub turret_fortification: Cadence,
    pub engineer_dispatch: Cadence,
    pub minefield_planning: Cadence,
    pub supply_delivery: Cadence,
    /// Indexed by `Team::index`.
    pub harvester_rescue_cooldown_secs: [f32; Team::ALL.len()],
}
