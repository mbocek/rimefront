use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::i18n::tr;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Action {
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    RotateLeft,
    RotateRight,
    ResetRotation,
    JumpHome,
    Stop,
    LayMine,
    JumpToAlert,
    RepairTool,
    SellTool,
    QuickSave,
    QuickLoad,
    ControlsHelp,
    GraphicsQuality,
    PerformanceOverlay,
    StanceAggressive,
    StanceHoldPosition,
    StanceHoldFire,
    Unload,
    ReconFlight,
    Barrage,
    Patrol,
}

impl Action {
    pub const ALL: [Action; 25] = [
        Action::ScrollUp,
        Action::ScrollDown,
        Action::ScrollLeft,
        Action::ScrollRight,
        Action::RotateLeft,
        Action::RotateRight,
        Action::ResetRotation,
        Action::JumpHome,
        Action::Stop,
        Action::LayMine,
        Action::JumpToAlert,
        Action::RepairTool,
        Action::SellTool,
        Action::QuickSave,
        Action::QuickLoad,
        Action::ControlsHelp,
        Action::GraphicsQuality,
        Action::PerformanceOverlay,
        Action::StanceAggressive,
        Action::StanceHoldPosition,
        Action::StanceHoldFire,
        Action::Unload,
        Action::ReconFlight,
        Action::Barrage,
        Action::Patrol,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Action::ScrollUp => tr("Scroll up"),
            Action::ScrollDown => tr("Scroll down"),
            Action::ScrollLeft => tr("Scroll left"),
            Action::ScrollRight => tr("Scroll right"),
            Action::RotateLeft => tr("Rotate left"),
            Action::RotateRight => tr("Rotate right"),
            Action::ResetRotation => tr("Reset rotation"),
            Action::JumpHome => tr("Jump to base"),
            Action::Stop => tr("Stop"),
            Action::LayMine => tr("Lay a mine (engineers)"),
            Action::JumpToAlert => tr("Jump to the last attack"),
            Action::RepairTool => tr("Repair mode"),
            Action::SellTool => tr("Sell mode"),
            Action::QuickSave => tr("Quick save"),
            Action::QuickLoad => tr("Quick load"),
            Action::ControlsHelp => tr("Controls overview"),
            Action::GraphicsQuality => tr("Graphics quality"),
            Action::PerformanceOverlay => tr("Performance overlay"),
            Action::StanceAggressive => tr("Stance: aggressive"),
            Action::StanceHoldPosition => tr("Stance: hold position"),
            Action::StanceHoldFire => tr("Stance: hold fire"),
            Action::Unload => tr("Unload carriers"),
            Action::ReconFlight => tr("Recon flight"),
            Action::Barrage => tr("Artillery barrage"),
            Action::Patrol => tr("Patrol (hold with right click)"),
        }
    }

    fn default_key(self) -> KeyCode {
        match self {
            Action::ScrollUp => KeyCode::KeyW,
            Action::ScrollDown => KeyCode::KeyS,
            Action::ScrollLeft => KeyCode::KeyA,
            Action::ScrollRight => KeyCode::KeyD,
            Action::RotateLeft => KeyCode::KeyQ,
            Action::RotateRight => KeyCode::KeyE,
            Action::ResetRotation => KeyCode::KeyR,
            Action::JumpHome => KeyCode::Space,
            Action::Stop => KeyCode::KeyX,
            Action::LayMine => KeyCode::KeyM,
            Action::JumpToAlert => KeyCode::KeyV,
            Action::RepairTool => KeyCode::KeyT,
            Action::SellTool => KeyCode::KeyY,
            Action::QuickSave => KeyCode::F5,
            Action::QuickLoad => KeyCode::F9,
            Action::ControlsHelp => KeyCode::F1,
            Action::GraphicsQuality => KeyCode::F2,
            Action::PerformanceOverlay => KeyCode::F3,
            Action::StanceAggressive => KeyCode::KeyG,
            Action::StanceHoldPosition => KeyCode::KeyH,
            Action::StanceHoldFire => KeyCode::KeyF,
            Action::Unload => KeyCode::KeyU,
            Action::ReconFlight => KeyCode::KeyZ,
            Action::Barrage => KeyCode::KeyC,
            Action::Patrol => KeyCode::KeyP,
        }
    }
}

/// Digits, Esc, F10, Home, arrows and modifiers keep their fixed meanings, so they are not offered.
const BINDABLE_KEYS: [(KeyCode, &str); 58] = [
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F11, "F11"),
    (KeyCode::F12, "F12"),
    (KeyCode::Space, "Space"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "PageUp"),
    (KeyCode::PageDown, "PageDown"),
    (KeyCode::Backquote, "`"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Backslash, "\\"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Quote, "'"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
    (KeyCode::Slash, "/"),
    (KeyCode::NumpadEnter, "NumEnter"),
];

pub fn is_bindable(key: KeyCode) -> bool {
    BINDABLE_KEYS.iter().any(|&(bindable, _)| bindable == key)
}

fn key_name(key: KeyCode) -> &'static str {
    BINDABLE_KEYS
        .iter()
        .find(|&&(bindable, _)| bindable == key)
        .map_or("?", |&(_, name)| name)
}

fn key_from_name(name: &str) -> Option<KeyCode> {
    BINDABLE_KEYS
        .iter()
        .find(|&&(_, bindable)| bindable == name)
        .map(|&(key, _)| key)
}

/// The keys the player moved away from the defaults, saved by key name so the file stays readable.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyBindings(BTreeMap<Action, String>);

impl KeyBindings {
    pub fn key(&self, action: Action) -> KeyCode {
        self.0
            .get(&action)
            .and_then(|name| key_from_name(name))
            .unwrap_or(action.default_key())
    }

    pub fn label(&self, action: Action) -> &'static str {
        tr(key_name(self.key(action)))
    }

    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, action: Action) -> bool {
        keys.pressed(self.key(action))
    }

    pub fn just_pressed(&self, keys: &ButtonInput<KeyCode>, action: Action) -> bool {
        keys.just_pressed(self.key(action))
    }

    /// An action that already uses the key takes over the rebound action's old key instead.
    pub fn bind(&mut self, action: Action, key: KeyCode) {
        let previous = self.key(action);
        let displaced = Action::ALL
            .into_iter()
            .find(|&other| other != action && self.key(other) == key);
        if let Some(other) = displaced {
            self.set(other, previous);
        }
        self.set(action, key);
    }

    fn set(&mut self, action: Action, key: KeyCode) {
        if key == action.default_key() {
            self.0.remove(&action);
        } else {
            self.0.insert(action, key_name(key).to_string());
        }
    }

    pub fn reset(&mut self) {
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_key_is_bindable_and_distinct() {
        let bindings = KeyBindings::default();
        for action in Action::ALL {
            assert!(is_bindable(bindings.key(action)), "{action:?}");
            let sharing = Action::ALL
                .into_iter()
                .filter(|&other| bindings.key(other) == bindings.key(action))
                .count();
            assert_eq!(sharing, 1, "{action:?}");
        }
    }

    #[test]
    fn binding_a_taken_key_swaps_the_two_actions() {
        let mut bindings = KeyBindings::default();
        bindings.bind(Action::Stop, KeyCode::KeyT);
        assert_eq!(bindings.key(Action::Stop), KeyCode::KeyT);
        assert_eq!(bindings.key(Action::RepairTool), KeyCode::KeyX);
        bindings.bind(Action::Stop, KeyCode::KeyX);
        assert_eq!(bindings, KeyBindings::default());
    }

    #[test]
    fn bindings_round_trip_through_json_by_key_name() {
        let mut bindings = KeyBindings::default();
        bindings.bind(Action::QuickSave, KeyCode::F6);
        let json = serde_json::to_string(&bindings).unwrap();
        assert_eq!(json, r#"{"QuickSave":"F6"}"#);
        assert_eq!(
            serde_json::from_str::<KeyBindings>(&json).unwrap(),
            bindings
        );
    }

    #[test]
    fn an_unknown_key_name_falls_back_to_the_default() {
        let bindings: KeyBindings = serde_json::from_str(r#"{"Stop":"Hyper"}"#).unwrap();
        assert_eq!(bindings.key(Action::Stop), KeyCode::KeyX);
    }
}
