use bevy::prelude::*;

/// Metres per second; a blast far off screen reaches the player this much later than it is seen.
const SPEED_OF_SOUND: f32 = 343.0;
/// How much quieter the edge of the screen is than its middle.
const EDGE_DIP: f32 = 0.35;
/// The share of loudness a sound keeps just past the edge of the screen.
const JUST_OFF_SCREEN: f32 = 0.6;
/// Zoomed all the way out, the battle sounds this much quieter than from close up.
const ZOOMED_OUT_DIP: f32 = 0.45;
/// Zoomed out beyond this share of the zoom range, even sounds on screen are heard as from afar.
const FAR_ZOOM: f32 = 0.7;
const PAN_WIDTH: f32 = 0.85;

/// How a sound reaches the player: its loudness, its place in the stereo field and whether it is heard
/// from afar (muffled, a moment after it happened).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Heard {
    pub gain: f32,
    /// -1 is the left speaker, 1 the right one.
    pub pan: f32,
    pub distant: bool,
    pub delay_secs: f32,
}

/// `screen` is the sound's place in normalized device coordinates ([-1, 1] is on screen, `None` behind the
/// camera); `zoom` is 0 fully zoomed in and 1 fully zoomed out; `carry` is how many screen widths beyond
/// the edge the sound still reaches; `metres` is its distance from the middle of the view.
pub fn hear(screen: Option<Vec2>, zoom: f32, carry: f32, metres: f32) -> Option<Heard> {
    let screen = screen?;
    let off_centre = screen.x.abs().max(screen.y.abs());
    let placement = if off_centre <= 1.0 {
        1.0 - EDGE_DIP * off_centre * off_centre
    } else {
        JUST_OFF_SCREEN * (1.0 - (off_centre - 1.0) / carry)
    };
    if placement <= 0.0 {
        return None;
    }
    let zoom = zoom.clamp(0.0, 1.0);
    let gain = placement * (1.0 - ZOOMED_OUT_DIP * zoom);
    let distant = off_centre > 1.0 || zoom > FAR_ZOOM;
    Some(Heard {
        gain,
        pan: screen.x.clamp(-1.0, 1.0) * PAN_WIDTH,
        distant,
        delay_secs: if off_centre > 1.0 {
            metres / SPEED_OF_SOUND
        } else {
            0.0
        },
    })
}

/// Bevy plays spatial sounds through rodio, which sets each ear's volume from its squared distance to the
/// sound and from how much nearer the other ear is (`rodio::source::Spatial::set_positions`). The ears
/// sit `EAR_GAP` apart and every sound is placed one unit in front of them, shifted sideways by its pan.
pub fn ear_volumes(pan: f32) -> (f32, f32) {
    let left_squared = (pan + EAR_HALF_GAP).powi(2) + 1.0;
    let right_squared = (pan - EAR_HALF_GAP).powi(2) + 1.0;
    let (left, right) = (left_squared.sqrt(), right_squared.sqrt());
    let gap = 2.0 * EAR_HALF_GAP;
    let left_side = (((left - right) / gap + 1.0) / 4.0 + 0.5).min(1.0);
    let right_side = (((right - left) / gap + 1.0) / 4.0 + 0.5).min(1.0);
    (
        left_side * (1.0 / left_squared).min(1.0),
        right_side * (1.0 / right_squared).min(1.0),
    )
}

pub const EAR_GAP: f32 = 2.0 * EAR_HALF_GAP;
const EAR_HALF_GAP: f32 = 1.0;

/// The playback volume that keeps the louder ear at the wanted level wherever the sound is panned.
pub fn ear_compensation(pan: f32) -> f32 {
    let (left, right) = ear_volumes(pan);
    1.0 / left.max(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_middle_of_the_screen_is_loudest_and_centred() {
        let middle = hear(Some(Vec2::ZERO), 0.0, 1.0, 0.0).unwrap();
        let edge = hear(Some(Vec2::new(0.95, 0.0)), 0.0, 1.0, 40.0).unwrap();
        assert!(middle.gain > edge.gain);
        assert_eq!(middle.pan, 0.0);
        assert!(edge.pan > 0.7, "a sound on the right comes from the right");
        assert!(!middle.distant && !edge.distant);
    }

    #[test]
    fn off_screen_sounds_are_muffled_late_and_fade_with_their_reach() {
        let just_off = hear(Some(Vec2::new(-1.3, 0.0)), 0.0, 1.0, 120.0).unwrap();
        assert!(just_off.distant);
        assert!(just_off.pan < -0.8);
        assert!((just_off.delay_secs - 120.0 / SPEED_OF_SOUND).abs() < 1e-4);
        assert!(
            hear(Some(Vec2::new(2.5, 0.0)), 0.0, 1.0, 300.0).is_none(),
            "a rifle so far is not heard"
        );
        assert!(
            hear(Some(Vec2::new(2.5, 0.0)), 0.0, 2.0, 300.0).is_some(),
            "a blast carries further"
        );
        assert!(hear(None, 0.0, 3.0, 10.0).is_none());
    }

    #[test]
    fn zooming_out_makes_the_battle_quieter_and_distant() {
        let close = hear(Some(Vec2::ZERO), 0.0, 1.0, 0.0).unwrap();
        let far = hear(Some(Vec2::ZERO), 1.0, 1.0, 0.0).unwrap();
        assert!(far.gain < close.gain * 0.6);
        assert!(far.distant);
    }

    #[test]
    fn a_sound_panned_right_is_louder_in_the_right_ear() {
        let (left, right) = ear_volumes(0.85);
        assert!(right > 2.0 * left);
        let (left, right) = ear_volumes(0.0);
        assert!((left - right).abs() < 1e-6);
    }

    #[test]
    fn the_louder_ear_keeps_the_wanted_level_wherever_the_sound_is() {
        for pan in [-0.85, -0.3, 0.0, 0.5, 0.85] {
            let (left, right) = ear_volumes(pan);
            assert!((left.max(right) * ear_compensation(pan) - 1.0).abs() < 1e-5);
        }
    }
}
