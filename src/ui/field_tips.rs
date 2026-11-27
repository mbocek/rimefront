use bevy::prelude::*;

use crate::app::i18n::{tr, tr_fmt};
use crate::app::settings::GameSettings;
use crate::sim::catalog::{BuildingKind, Team, UnitKind};
use crate::sim::components::{Building, Selected, Unit};
use crate::sim::cues::{Sound, SoundCue};
use crate::ui::key_bindings::{Action, KeyBindings};
use crate::ui::sidebar::{ACCENT, FRAME_BRIGHT, PANEL, TEXT_DIM, TEXT_MAIN, UiFonts};
use crate::view::local_view::LocalView;

const PANEL_WIDTH: f32 = 380.0;
/// Leaves the control-group bar in the top left corner uncovered.
const BELOW_GROUP_BAR: f32 = 64.0;
const SHOWN_FOR_SECS: f32 = 20.0;

/// A rule explained once, the first time the player runs into it; remembered across matches in the settings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldTip {
    Mines,
    Research,
    Camouflage,
}

impl FieldTip {
    const ALL: [FieldTip; 3] = [FieldTip::Mines, FieldTip::Research, FieldTip::Camouflage];

    /// The name stored in the settings file; it must never change, or players see the tip again.
    fn id(self) -> &'static str {
        match self {
            FieldTip::Mines => "mines",
            FieldTip::Research => "research",
            FieldTip::Camouflage => "camouflage",
        }
    }

    fn title(self) -> String {
        match self {
            FieldTip::Mines => tr("MINES"),
            FieldTip::Research => tr("RESEARCH"),
            FieldTip::Camouflage => tr("CAMOUFLAGE"),
        }
        .to_string()
    }

    fn text(self, keys: &KeyBindings) -> String {
        match self {
            FieldTip::Mines => tr_fmt(
                "Engineers lay mines: select them and press {} to bury one where they stand ($100). The enemy cannot see your mines, and you cannot see theirs.",
                &[&keys.label(Action::LayMine)],
            ),
            FieldTip::Research => tr(
                "The Tech Center opens the RESEARCH tab: upgrades for your whole army. Research takes time, and the Tech Center must stand until it is done.",
            )
            .to_string(),
            FieldTip::Camouflage => tr(
                "A sniper that keeps still for a few seconds blends into the ground and cannot be targeted, until it fires or an enemy comes within 8 m.",
            )
            .to_string(),
        }
    }
}

/// What the player has run into so far in this match.
#[derive(Default, Clone, Copy, Debug)]
pub struct Encounters {
    pub engineer_selected: bool,
    pub tech_center_owned: bool,
    pub sniper_met: bool,
}

/// The tips the player has just earned and not yet read, in a fixed order.
pub fn tips_due(encounters: Encounters, seen: &[String]) -> Vec<FieldTip> {
    FieldTip::ALL
        .into_iter()
        .filter(|tip| match tip {
            FieldTip::Mines => encounters.engineer_selected,
            FieldTip::Research => encounters.tech_center_owned,
            FieldTip::Camouflage => encounters.sniper_met,
        })
        .filter(|tip| !seen.iter().any(|id| id == tip.id()))
        .collect()
}

#[derive(Component)]
pub struct TipPanel {
    shown_secs: f32,
}

#[derive(Component)]
pub struct TipTitle;

#[derive(Component)]
pub struct TipText;

pub fn spawn_tip_panel(mut commands: Commands, fonts: Res<UiFonts>) {
    commands
        .spawn((
            TipPanel {
                shown_secs: SHOWN_FOR_SECS,
            },
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
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|header| {
                    header.spawn((TipTitle, fonts.heading("", 14.0, ACCENT)));
                    header
                        .spawn(fonts.text(tr("got it  ×"), 13.0, TEXT_DIM))
                        .observe(|_: On<PointerClick>, mut panels: Query<&mut TipPanel>| {
                            for mut panel in &mut panels {
                                panel.shown_secs = SHOWN_FOR_SECS;
                            }
                        });
                });
            panel.spawn((TipText, fonts.text("", 16.0, TEXT_MAIN)));
        });
}

/// Shows each tip once, the first time the player meets what it explains, one at a time.
pub fn offer_field_tips(
    time: Res<Time<Real>>,
    view: LocalView,
    mut settings: ResMut<GameSettings>,
    mut cues: MessageWriter<SoundCue>,
    units: Query<(&Unit, &Team, &Transform, Has<Selected>)>,
    buildings: Query<(&Building, &Team)>,
    mut panels: Query<(&mut TipPanel, &mut Visibility)>,
    mut titles: Query<&mut Text, (With<TipTitle>, Without<TipText>)>,
    mut texts: Query<&mut Text, (With<TipText>, Without<TipTitle>)>,
) {
    let Ok((mut panel, mut visibility)) = panels.single_mut() else {
        return;
    };
    panel.shown_secs += time.delta_secs();
    let showing = panel.shown_secs < SHOWN_FOR_SECS;
    if showing || view.is_spectating() {
        visibility.set_if_neq(if showing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        return;
    }
    visibility.set_if_neq(Visibility::Hidden);

    let own = view.team();
    let mut encounters = Encounters::default();
    for (unit, &team, transform, selected) in &units {
        let is_sniper = unit.0 == UnitKind::Sniper;
        if team == own {
            encounters.engineer_selected |= selected && unit.0.is_engineer();
            encounters.sniper_met |= is_sniper;
        } else if is_sniper && view.is_hostile(team) {
            encounters.sniper_met |= view.sight().is_visible(transform.translation);
        }
    }
    encounters.tech_center_owned = buildings
        .iter()
        .any(|(building, &team)| team == own && building.kind == BuildingKind::TechCenter);

    let Some(&tip) = tips_due(encounters, &settings.seen_tips).first() else {
        return;
    };
    settings.seen_tips.push(tip.id().to_string());
    panel.shown_secs = 0.0;
    visibility.set_if_neq(Visibility::Inherited);
    for mut title in &mut titles {
        title.0 = tip.title();
    }
    for mut text in &mut texts {
        text.0 = tip.text(&settings.keys);
    }
    cues.write(SoundCue::interface(Sound::UnitAcknowledge));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tip_comes_once_and_only_after_its_encounter() {
        let nothing = Encounters::default();
        assert!(tips_due(nothing, &[]).is_empty());
        let engineer = Encounters {
            engineer_selected: true,
            ..default()
        };
        assert_eq!(tips_due(engineer, &[]), vec![FieldTip::Mines]);
        assert!(tips_due(engineer, &["mines".to_string()]).is_empty());
    }

    #[test]
    fn several_encounters_queue_their_tips_in_order() {
        let everything = Encounters {
            engineer_selected: true,
            tech_center_owned: true,
            sniper_met: true,
        };
        assert_eq!(
            tips_due(everything, &["research".to_string()]),
            vec![FieldTip::Mines, FieldTip::Camouflage]
        );
    }
}
