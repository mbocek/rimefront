use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::input::ButtonState;
use bevy::input::mouse::MouseButtonInput;
use bevy::picking::PickingSystems;
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::PrimaryWindow;

use crate::app::capture::CaptureTarget;
use crate::view::camera::ScriptedPointer;

/// `RIMEFRONT_CAPTURE_POINTER="2 move 640 400; 2.2 press left; 2.4 release left; 3 shot"` plays a mouse script
/// in a capture run: seconds since start, then `move x y` (interface pixels of the 1600 x 900 capture),
/// `press` / `release` `left` or `right`, or `shot`, which saves `<capture name>-<n>.png`.
pub const CAPTURE_POINTER_VAR: &str = "RIMEFRONT_CAPTURE_POINTER";

#[derive(Clone, Copy, PartialEq, Debug)]
enum PointerStep {
    Move(Vec2),
    Press(MouseButton),
    Release(MouseButton),
    Shot,
}

#[derive(Resource)]
struct PointerScript {
    steps: Vec<(f32, PointerStep)>,
    next: usize,
    shots_taken: usize,
    shot_stem: String,
}

fn parse_script(script: &str) -> Result<Vec<(f32, PointerStep)>, String> {
    script
        .split(';')
        .map(str::trim)
        .filter(|step| !step.is_empty())
        .map(|step| {
            let words: Vec<&str> = step.split_whitespace().collect();
            let number = |index: usize| -> Result<f32, String> {
                words
                    .get(index)
                    .and_then(|word| word.parse().ok())
                    .ok_or_else(|| format!("expected a number in `{step}`"))
            };
            let button = || match words.get(2) {
                Some(&"right") => Ok(MouseButton::Right),
                Some(&"left") | None => Ok(MouseButton::Left),
                Some(other) => Err(format!("unknown button `{other}` in `{step}`")),
            };
            let action = match words.get(1) {
                Some(&"move") => PointerStep::Move(Vec2::new(number(2)?, number(3)?)),
                Some(&"press") => PointerStep::Press(button()?),
                Some(&"release") => PointerStep::Release(button()?),
                Some(&"shot") => PointerStep::Shot,
                _ => return Err(format!("unknown step `{step}`")),
            };
            Ok((number(0)?, action))
        })
        .collect()
}

pub fn add_pointer_script(app: &mut App, capture_path: &str) {
    let Ok(script) = std::env::var(CAPTURE_POINTER_VAR) else {
        return;
    };
    let steps = match parse_script(&script) {
        Ok(steps) => steps,
        Err(problem) => panic!("{CAPTURE_POINTER_VAR}: {problem}"),
    };
    let shot_stem = capture_path
        .strip_suffix(".png")
        .unwrap_or(capture_path)
        .to_string();
    app.insert_resource(PointerScript {
        steps,
        next: 0,
        shots_taken: 0,
        shot_stem,
    })
    .add_systems(
        PreUpdate,
        play_pointer_script
            .after(bevy::input::InputSystems)
            .before(PickingSystems::ProcessInput),
    );
}

#[allow(clippy::too_many_arguments)]
fn play_pointer_script(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut script: ResMut<PointerScript>,
    target: Option<Res<CaptureTarget>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut scripted: ResMut<ScriptedPointer>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut mouse_messages: MessageWriter<MouseButtonInput>,
    mut pointer_inputs: MessageWriter<PointerInput>,
) {
    let Some(image) = target.map(|target| target.0.clone()) else {
        return;
    };
    let Ok(window) = windows.single() else { return };
    let location = |position: Vec2, image: &Handle<Image>| Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget {
            handle: image.clone(),
            scale_factor: 1.0,
        }),
        position,
    };
    while let Some(&(at, step)) = script.steps.get(script.next) {
        if time.elapsed_secs() < at {
            break;
        }
        script.next += 1;
        let position = scripted.0.unwrap_or_default();
        match step {
            PointerStep::Move(to) => {
                scripted.0 = Some(to);
                pointer_inputs.write(PointerInput::new(
                    PointerId::Mouse,
                    location(to, &image),
                    PointerAction::Move {
                        delta: to - position,
                    },
                ));
            }
            PointerStep::Press(button) | PointerStep::Release(button) => {
                let pressed = matches!(step, PointerStep::Press(_));
                if pressed {
                    mouse.press(button);
                } else {
                    mouse.release(button);
                }
                mouse_messages.write(MouseButtonInput {
                    button,
                    state: if pressed {
                        ButtonState::Pressed
                    } else {
                        ButtonState::Released
                    },
                    window,
                });
                let picking_button = if button == MouseButton::Right {
                    PointerButton::Secondary
                } else {
                    PointerButton::Primary
                };
                pointer_inputs.write(PointerInput::new(
                    PointerId::Mouse,
                    location(position, &image),
                    if pressed {
                        PointerAction::Press(picking_button)
                    } else {
                        PointerAction::Release(picking_button)
                    },
                ));
            }
            PointerStep::Shot => {
                script.shots_taken += 1;
                let path = format!("{}-{}.png", script.shot_stem, script.shots_taken);
                commands
                    .spawn(Screenshot::image(image.clone()))
                    .observe(save_to_disk(path));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_script_reads_moves_buttons_and_shots() {
        let steps =
            parse_script("1 move 640 400; 1.2 press right; 1.4 release right; 2 shot").unwrap();
        assert_eq!(
            steps,
            vec![
                (1.0, PointerStep::Move(Vec2::new(640.0, 400.0))),
                (1.2, PointerStep::Press(MouseButton::Right)),
                (1.4, PointerStep::Release(MouseButton::Right)),
                (2.0, PointerStep::Shot),
            ]
        );
        assert!(parse_script("1 wiggle").is_err());
    }
}
