use bevy::prelude::*;

use crate::i18n::tr_fmt;
use crate::levels::{ActiveLevel, Objective};
use crate::match_flow::MatchClock;
use crate::sim::catalog::{BuildingKind, Diplomacy, Team};
use crate::sim::components::{Building, Unit};
use crate::sim::economy::Economies;
use crate::sim::player_commands::LocalTeam;

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum GameOutcome {
    #[default]
    Ongoing,
    Victory,
    Defeat,
}

/// The player loses once nothing of theirs is left; how they win depends on the mission objective.
pub fn decide_outcome(
    local: Res<LocalTeam>,
    diplomacy: Res<Diplomacy>,
    mut outcome: ResMut<GameOutcome>,
    level: Res<ActiveLevel>,
    clock: Res<MatchClock>,
    economies: Res<Economies>,
    units: Query<&Team, With<Unit>>,
    buildings: Query<(&Team, &Building)>,
) {
    if *outcome != GameOutcome::Ongoing {
        return;
    }
    let standing = |building: &Building| building.kind.keeps_a_side_alive();
    let hostile = |team: &Team| diplomacy.hostile(*team, local.0);
    let own_side_alive = units.iter().any(|team| !hostile(team))
        || buildings
            .iter()
            .any(|(team, building)| !hostile(team) && standing(building));
    if !own_side_alive {
        *outcome = GameOutcome::Defeat;
        return;
    }
    let achieved = match level.0.objective {
        Objective::DestroyAllEnemies => {
            !units.iter().any(hostile)
                && !buildings
                    .iter()
                    .any(|(team, building)| hostile(team) && standing(building))
        }
        Objective::DestroyCommandCenters => !buildings
            .iter()
            .any(|(team, building)| hostile(team) && building.kind == BuildingKind::CommandCenter),
        Objective::CaptureCommandCenter => buildings.iter().any(|(&team, building)| {
            team == local.0
                && building.kind == BuildingKind::CommandCenter
                && building.origin != level.0.player.site.origin()
        }),
        Objective::Survive { minutes } => clock.elapsed_secs >= minutes * 60.0,
        Objective::ReachCredits { credits } => economies.of(local.0).credits >= credits,
        Objective::World => false,
    };
    if achieved {
        *outcome = GameOutcome::Victory;
    }
}

/// One-line progress report for the interface, e.g. "Hold out for 8 minutes - 3:12 left".
pub fn objective_status(
    level: &ActiveLevel,
    clock: &MatchClock,
    economies: &Economies,
    team: Team,
) -> String {
    let objective = level.0.objective;
    let progress = match objective {
        Objective::Survive { minutes } => {
            let left = (minutes * 60.0 - clock.elapsed_secs).max(0.0) as u32;
            tr_fmt(
                "  -  {} left",
                &[&format!("{}:{:02}", left / 60, left % 60)],
            )
        }
        Objective::ReachCredits { credits } => {
            format!("  -  ${} / ${credits}", economies.of(team).credits)
        }
        Objective::DestroyAllEnemies
        | Objective::DestroyCommandCenters
        | Objective::CaptureCommandCenter
        | Objective::World => String::new(),
    };
    format!("{}{progress}", objective.describe())
}
