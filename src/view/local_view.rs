use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::net::network::NetSession;
use crate::sim::catalog::{Diplomacy, Team};
use crate::sim::fog::FogOfWar;
use crate::sim::intel::Intel;
use crate::sim::player_commands::LocalTeam;

/// The battle as this machine's player sees it. The simulation keeps every team's view; drawing, sound and
/// the interface must use only the local one, or a networked opponent would see through the fog.
#[derive(SystemParam)]
pub struct LocalView<'w> {
    intel: Res<'w, Intel>,
    local: Res<'w, LocalTeam>,
    diplomacy: Res<'w, Diplomacy>,
    spectator: Option<Res<'w, SpectatorView>>,
}

/// A spectator of a network battle sees the whole map; this is that cleared view.
#[derive(Resource)]
pub struct SpectatorView(FogOfWar);

/// Made once the match's map size is known.
pub fn prepare_spectator_view(mut commands: Commands, session: Option<Res<NetSession>>) {
    if session.is_some_and(|session| session.is_spectating()) {
        commands.insert_resource(SpectatorView(FogOfWar::new(false)));
    } else {
        commands.remove_resource::<SpectatorView>();
    }
}

impl LocalView<'_> {
    pub fn team(&self) -> Team {
        self.local.0
    }

    pub fn sight(&self) -> &FogOfWar {
        match &self.spectator {
            Some(view) => &view.0,
            None => self.intel.vision(self.local.0),
        }
    }

    pub fn is_spectating(&self) -> bool {
        self.spectator.is_some()
    }

    pub fn is_hostile(&self, team: Team) -> bool {
        self.diplomacy.hostile(team, self.local.0)
    }

    /// Whether the local player may know about something of `team` standing at `position`.
    pub fn reveals(&self, team: Team, position: Vec3) -> bool {
        self.is_spectating() || !self.is_hostile(team) || self.sight().is_visible(position)
    }
}
