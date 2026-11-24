use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::app::settings::GameSettings;

/// A game left in the background keeps playing, but needs no more frames than this.
const BACKGROUND_FPS: f32 = 30.0;

fn frame_cap(settings: &GameSettings, focused: bool) -> Option<f32> {
    let chosen = settings.frame_limit.max_fps();
    if focused {
        return chosen;
    }
    Some(chosen.map_or(BACKGROUND_FPS, |fps| fps.min(BACKGROUND_FPS)))
}

/// Sleeps away whatever is left of the frame budget, so the game does not burn CPU on frames nobody sees.
pub fn pace_frames(
    mut frame_started: Local<Option<Instant>>,
    settings: Res<GameSettings>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if crate::app::capture::is_capture_run() {
        return;
    }
    let focused = windows.single().is_ok_and(|window| window.focused);
    if let (Some(fps), Some(started)) = (frame_cap(&settings, focused), *frame_started) {
        let budget = Duration::from_secs_f32(1.0 / fps);
        if let Some(left) = budget.checked_sub(started.elapsed()) {
            std::thread::sleep(left);
        }
    }
    *frame_started = Some(Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::settings::FrameLimit;

    #[test]
    fn the_background_is_held_to_thirty_frames() {
        let mut settings = GameSettings {
            frame_limit: FrameLimit::Unlimited,
            ..default()
        };
        assert_eq!(frame_cap(&settings, true), None);
        assert_eq!(frame_cap(&settings, false), Some(BACKGROUND_FPS));
        settings.frame_limit = FrameLimit::Fps120;
        assert_eq!(frame_cap(&settings, true), Some(120.0));
        assert_eq!(frame_cap(&settings, false), Some(BACKGROUND_FPS));
    }
}
