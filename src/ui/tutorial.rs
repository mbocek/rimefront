use bevy::prelude::*;

use crate::app::i18n::{tr, tr_fmt};
use crate::app::settings::GameSettings;
use crate::missions::levels::ActiveLevel;
use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::components::{Building, Mobile, Order, Orders, Selected, Unit};
use crate::sim::cues::{Sound, SoundCue};
use crate::ui::key_bindings::{Action, KeyBindings};
use crate::ui::sidebar::{ACCENT, FRAME_BRIGHT, GOOD, PANEL, TEXT_DIM, TEXT_MAIN, UiFonts};

const PANEL_WIDTH: f32 = 380.0;
const DONE_FLASH_SECS: f32 = 1.2;
/// Leaves the control-group bar in the top left corner uncovered.
const BELOW_GROUP_BAR: f32 = 64.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TutorialStep {
    SelectUnits,
    MoveUnits,
    BuildPowerPlant,
    BuildRefinery,
    BuildBarracks,
    TrainRifleman,
    AttackMove,
    DestroyOutpost,
}

pub const TUTORIAL_STEPS: [TutorialStep; 8] = [
    TutorialStep::SelectUnits,
    TutorialStep::MoveUnits,
    TutorialStep::BuildPowerPlant,
    TutorialStep::BuildRefinery,
    TutorialStep::BuildBarracks,
    TutorialStep::TrainRifleman,
    TutorialStep::AttackMove,
    TutorialStep::DestroyOutpost,
];

impl TutorialStep {
    pub fn instruction(self, keys: &KeyBindings) -> String {
        let fixed = match self {
            TutorialStep::SelectUnits => {
                tr("Select your troops: drag a box around them with the left mouse button.")
            }
            TutorialStep::MoveUnits => {
                tr("Right-click the ground to send the selected troops there.")
            }
            TutorialStep::BuildPowerPlant => tr(
                "Open the BASE tab, click Power Plant, and place it next to your Command Center when it is ready.",
            ),
            TutorialStep::BuildRefinery => tr(
                "Build a Refinery near the crystals. It comes with a harvester that earns credits.",
            ),
            TutorialStep::BuildBarracks => tr("Build Barracks to train infantry."),
            TutorialStep::TrainRifleman => tr("Open the INFANTRY tab and train a Rifleman."),
            TutorialStep::AttackMove => tr(
                "Select your troops and Ctrl + right-click across the river: they fight anything on the way.",
            ),
            TutorialStep::DestroyOutpost => {
                return tr_fmt(
                    "Destroy the Syndicate outpost in the north-east. Press {} any time for all controls.",
                    &[&keys.label(Action::ControlsHelp)],
                );
            }
        };
        fixed.to_string()
    }
}

/// What the player has done so far, gathered from the world each frame.
#[derive(Default, Clone, Copy, Debug)]
pub struct TutorialFacts {
    pub troops_selected: bool,
    pub troops_moving: bool,
    pub power_plant: bool,
    pub refinery: bool,
    pub barracks: bool,
    pub riflemen: usize,
    pub attack_moving: bool,
}

/// Tracks the current step; riflemen are counted against how many the mission started with.
#[derive(Resource)]
pub struct Tutorial {
    pub step: usize,
    starting_riflemen: Option<usize>,
    hidden: bool,
    flash_secs: f32,
}

impl Tutorial {
    fn new() -> Self {
        Self {
            step: 0,
            starting_riflemen: None,
            hidden: false,
            flash_secs: 0.0,
        }
    }

    pub fn current(&self) -> Option<TutorialStep> {
        TUTORIAL_STEPS.get(self.step).copied()
    }

    /// Moves past every step the facts already satisfy; returns whether anything was completed.
    pub fn advance(&mut self, facts: &TutorialFacts) -> bool {
        let starting_riflemen = *self.starting_riflemen.get_or_insert(facts.riflemen);
        let mut advanced = false;
        while let Some(step) = self.current() {
            let done = match step {
                TutorialStep::SelectUnits => facts.troops_selected,
                TutorialStep::MoveUnits => facts.troops_moving,
                TutorialStep::BuildPowerPlant => facts.power_plant,
                TutorialStep::BuildRefinery => facts.refinery,
                TutorialStep::BuildBarracks => facts.barracks,
                TutorialStep::TrainRifleman => facts.riflemen > starting_riflemen,
                TutorialStep::AttackMove => facts.attack_moving,
                TutorialStep::DestroyOutpost => false,
            };
            if !done {
                break;
            }
            self.step += 1;
            advanced = true;
        }
        advanced
    }
}

#[derive(Component)]
pub struct TutorialPanel;

#[derive(Component)]
pub struct TutorialText;

#[derive(Component)]
pub struct TutorialCounter;

pub fn start_tutorial(mut commands: Commands, level: Res<ActiveLevel>, fonts: Res<UiFonts>) {
    commands.remove_resource::<Tutorial>();
    if !level.0.tutorial {
        return;
    }
    commands.insert_resource(Tutorial::new());
    commands
        .spawn((
            TutorialPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(BELOW_GROUP_BAR),
                width: Val::Px(PANEL_WIDTH),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(PANEL.with_alpha(0.92)),
            BorderColor::all(FRAME_BRIGHT),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((TutorialCounter, fonts.heading(tr("TUTORIAL"), 14.0, ACCENT)));
                    header
                        .spawn(fonts.text(tr("hide  ×"), 13.0, TEXT_DIM))
                        .observe(|_: On<PointerClick>, mut tutorial: ResMut<Tutorial>| {
                            tutorial.hidden = true;
                        });
                });
            panel.spawn((TutorialText, fonts.text("", 16.0, TEXT_MAIN)));
        });
}

pub fn follow_tutorial(
    mut cues: MessageWriter<SoundCue>,
    time: Res<Time>,
    tutorial: Option<ResMut<Tutorial>>,
    units: Query<(&Unit, &Team, &Mobile, Option<&Orders>, Has<Selected>)>,
    buildings: Query<(&Building, &Team)>,
) {
    let Some(mut tutorial) = tutorial else { return };
    tutorial.flash_secs = (tutorial.flash_secs - time.delta_secs()).max(0.0);
    let mut facts = TutorialFacts::default();
    for (unit, &team, mobile, orders, selected) in &units {
        if team != Team::Azure || unit.0 == UnitKind::Harvester {
            continue;
        }
        facts.troops_selected |= selected;
        facts.troops_moving |= selected && mobile.is_moving();
        facts.attack_moving |= orders
            .and_then(|orders| orders.current)
            .is_some_and(|order| matches!(order, Order::AttackMove(_)));
        if unit.0 == UnitKind::Rifleman {
            facts.riflemen += 1;
        }
    }
    for (building, &team) in &buildings {
        if team != Team::Azure {
            continue;
        }
        match building.kind {
            BuildingKind::PowerPlant => facts.power_plant = true,
            BuildingKind::Refinery => facts.refinery = true,
            BuildingKind::Barracks => facts.barracks = true,
            _ => {}
        }
    }
    if tutorial.advance(&facts) {
        tutorial.flash_secs = DONE_FLASH_SECS;
        cues.write(SoundCue::interface(Sound::UnitAcknowledge));
    }
}

pub fn refresh_tutorial_panel(
    tutorial: Option<Res<Tutorial>>,
    settings: Res<GameSettings>,
    mut panels: Query<(&mut Visibility, &mut BorderColor), With<TutorialPanel>>,
    mut texts: Query<&mut Text, (With<TutorialText>, Without<TutorialCounter>)>,
    mut counters: Query<&mut Text, (With<TutorialCounter>, Without<TutorialText>)>,
) {
    let Some(tutorial) = tutorial else { return };
    let Ok((mut visibility, mut border)) = panels.single_mut() else {
        return;
    };
    *visibility = if tutorial.hidden {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    *border = BorderColor::all(if tutorial.flash_secs > 0.0 {
        GOOD
    } else {
        FRAME_BRIGHT
    });
    let Some(step) = tutorial.current() else {
        return;
    };
    let counter = tr_fmt(
        "TUTORIAL  {} / {}",
        &[&(tutorial.step + 1), &TUTORIAL_STEPS.len()],
    );
    for mut text in &mut counters {
        if text.0 != counter {
            text.0 = counter.clone();
        }
    }
    for mut text in &mut texts {
        let instruction = step.instruction(&settings.keys);
        if text.0 != instruction {
            text.0 = instruction.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_complete_in_order_and_the_last_one_waits_for_victory() {
        let mut tutorial = Tutorial::new();
        let mut facts = TutorialFacts {
            riflemen: 3,
            ..default()
        };
        assert!(!tutorial.advance(&facts));
        assert_eq!(tutorial.current(), Some(TutorialStep::SelectUnits));

        facts.power_plant = true;
        tutorial.advance(&facts);
        assert_eq!(
            tutorial.current(),
            Some(TutorialStep::SelectUnits),
            "a later step done early must not skip the current one"
        );

        facts.troops_selected = true;
        facts.troops_moving = true;
        tutorial.advance(&facts);
        assert_eq!(tutorial.current(), Some(TutorialStep::BuildRefinery));

        facts.refinery = true;
        facts.barracks = true;
        tutorial.advance(&facts);
        assert_eq!(tutorial.current(), Some(TutorialStep::TrainRifleman));

        facts.riflemen = 4;
        facts.attack_moving = true;
        tutorial.advance(&facts);
        assert_eq!(tutorial.current(), Some(TutorialStep::DestroyOutpost));
    }
}
