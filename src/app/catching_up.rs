use std::time::Duration;

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::app::settings::GameSettings;
use crate::audio::sound::master_volume;
use crate::net::network::{CatchingUp, NetSession};
use crate::sim::player_commands::SimulationTick;

/// A spectator further behind than this (two seconds) runs the battle fast until it has caught up.
const CATCH_UP_FROM_TICKS: u64 = 40;
const CATCH_UP_SPEED: f32 = 40.0;
/// Bevy clamps each frame's scaled game time to `max_delta`, 250 ms by default, which would hold catching up
/// to 5 ticks a frame whatever the speed.
const CATCH_UP_LONGEST_FRAME: Duration = Duration::from_secs(2);
const REGULAR_LONGEST_FRAME: Duration = Duration::from_millis(250);

fn pace_for_catching_up(time: &mut Time<Virtual>) {
    time.set_relative_speed(CATCH_UP_SPEED);
    time.set_max_delta(CATCH_UP_LONGEST_FRAME);
}

fn pace_normally(time: &mut Time<Virtual>, game_speed: f32) {
    time.set_relative_speed(game_speed);
    time.set_max_delta(REGULAR_LONGEST_FRAME);
}

/// A machine that joined late runs the battle many times faster, silently, until it reaches the players.
pub fn catch_up_with_the_battle(
    session: Option<Res<NetSession>>,
    tick: Res<SimulationTick>,
    settings: Res<GameSettings>,
    mut catching_up: ResMut<CatchingUp>,
    mut time: ResMut<Time<Virtual>>,
    mut volume: ResMut<GlobalVolume>,
) {
    let behind = session
        .and_then(|session| session.known_through())
        .map(|known| (known.saturating_sub(tick.0), known));
    let far_behind = behind.filter(|&(ticks, _)| ticks > CATCH_UP_FROM_TICKS);
    match (far_behind, catching_up.0.is_some()) {
        (Some((ticks, known)), _) => {
            catching_up.0 = Some(1.0 - ticks as f32 / known.max(1) as f32);
            pace_for_catching_up(&mut time);
            volume.volume = Volume::SILENT;
        }
        (None, true) => {
            catching_up.0 = None;
            pace_normally(&mut time, settings.game_speed_factor());
            volume.volume = master_volume(&settings);
        }
        (None, false) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::platform::time::Instant;

    fn game_time_of_one_frame(time: &mut Time<Virtual>, frame: Duration) -> Duration {
        let start = Instant::now();
        let mut real = Time::<Real>::new(start);
        real.update_with_instant(start);
        real.update_with_instant(start + frame);
        bevy::time::update_virtual_time(&mut Time::default(), time, &real);
        time.delta()
    }

    #[test]
    fn catching_up_runs_the_full_speed_even_in_a_slow_background_frame() {
        let background_frame = Duration::from_millis(33);
        let mut time = Time::<Virtual>::default();
        pace_for_catching_up(&mut time);
        let game_time = game_time_of_one_frame(&mut time, background_frame);
        assert!(game_time >= background_frame.mul_f32(CATCH_UP_SPEED * 0.99));
        pace_normally(&mut time, 1.0);
        assert_eq!(time.max_delta(), Time::<Virtual>::default().max_delta());
    }
}
