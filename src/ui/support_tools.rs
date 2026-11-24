use bevy::picking::hover::Hovered;
use bevy::prelude::*;

use crate::app::i18n::tr;
use crate::app::settings::GameSettings;
use crate::audio::sound::{Sound, SoundCue};
use crate::sim::catalog::Team;
use crate::sim::components::Building;
use crate::sim::player_commands::{CommandIssuer, PlayerCommand};
use crate::sim::support_powers::{SupportPower, SupportPowers, owned_kinds};
use crate::ui::key_bindings::{Action, KeyBindings};
use crate::ui::sidebar::{ACCENT, FRAME, TEXT_DIM, TEXT_MAIN, UiFonts};
use crate::ui::structure_tools::StructureTool;
use crate::view::camera::Cursor;
use crate::world::map::world_to_cell;

const BUTTON_IDLE: Color = Color::srgb(0.13, 0.15, 0.17);
const BUTTON_HOVERED: Color = Color::srgb(0.18, 0.21, 0.24);
const BUTTON_AIMING: Color = Color::srgb(0.30, 0.24, 0.08);
const AIMING_COLOR: Color = Color::srgb(1.0, 0.55, 0.2);
/// The ring drawn round the pointer while aiming a power, in meters.
const AIMING_RING: f32 = 7.0;

/// The power the player is aiming; its next left click on the map calls it there.
#[derive(Resource, Default)]
pub struct SupportTool(pub Option<SupportPower>);

#[derive(Component)]
pub struct SupportButton(SupportPower);

#[derive(Component)]
pub struct SupportLabel(SupportPower);

fn hotkey(power: SupportPower) -> Action {
    match power {
        SupportPower::ReconSweep => Action::ReconFlight,
        SupportPower::Barrage => Action::Barrage,
    }
}

fn short_name(power: SupportPower) -> &'static str {
    match power {
        SupportPower::ReconSweep => tr("RECON"),
        SupportPower::Barrage => tr("BARRAGE"),
    }
}

/// Without the structure it needs, a power shows only its name, dimmed.
fn label(power: SupportPower, keys: &KeyBindings, secs_left: f32, available: bool) -> String {
    let name = short_name(power);
    if !available {
        return name.to_string();
    }
    if secs_left > 0.0 {
        let secs = secs_left.ceil() as u32;
        return format!("{name}  {}:{:02}", secs / 60, secs % 60);
    }
    format!("{name}  {}", keys.label(hotkey(power)))
}

pub fn spawn_support_buttons(panel: &mut ChildSpawnerCommands, fonts: &UiFonts) {
    panel
        .spawn(Node {
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            for power in SupportPower::ALL {
                row.spawn((
                    SupportButton(power),
                    Hovered::default(),
                    Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(BUTTON_IDLE),
                    BorderColor::all(FRAME),
                ))
                .with_children(|button| {
                    button.spawn((SupportLabel(power), fonts.heading("", 12.0, TEXT_MAIN)));
                })
                .observe(
                    move |_: On<PointerClick>,
                          mut aiming: ResMut<SupportTool>,
                          mut structure_tool: ResMut<StructureTool>,
                          mut cues: MessageWriter<SoundCue>| {
                        aim(&mut aiming, &mut structure_tool, power);
                        cues.write(SoundCue::interface(Sound::UiClick));
                    },
                );
            }
        });
}

pub fn prepare_support_tool(mut commands: Commands) {
    commands.insert_resource(SupportTool::default());
}

/// Aiming a power puts the repair or sell mode away, so one click never does both.
fn aim(aiming: &mut SupportTool, structure_tool: &mut StructureTool, power: SupportPower) {
    aiming.0 = if aiming.0 == Some(power) {
        None
    } else {
        Some(power)
    };
    if aiming.0.is_some() {
        structure_tool.0 = None;
    }
}

pub fn aim_support_by_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    settings: Res<GameSettings>,
    mut aiming: ResMut<SupportTool>,
    mut structure_tool: ResMut<StructureTool>,
) {
    for power in SupportPower::ALL {
        if settings.keys.just_pressed(&keys, hotkey(power)) {
            aim(&mut aiming, &mut structure_tool, power);
        }
    }
    if structure_tool.0.is_some() {
        aiming.0 = None;
    }
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        aiming.0 = None;
    }
}

pub fn call_aimed_support(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<Cursor>,
    powers: Res<SupportPowers>,
    buildings: Query<(&Building, &Team)>,
    mut aiming: ResMut<SupportTool>,
    mut issuer: CommandIssuer,
    mut cues: MessageWriter<SoundCue>,
) {
    let Some(power) = aiming.0 else { return };
    if !mouse.just_pressed(MouseButton::Left) || cursor.over_interface {
        return;
    }
    let Some(ground) = cursor.ground else { return };
    let team = issuer.team();
    let ready =
        powers.is_charged(team, power) && owned_kinds(&buildings, team).contains(&power.requires());
    if !ready {
        cues.write(SoundCue::interface(Sound::UiError));
        return;
    }
    issuer.issue(PlayerCommand::CallSupport {
        power,
        cell: world_to_cell(ground),
    });
    cues.write(SoundCue::interface(Sound::UiClick));
    aiming.0 = None;
}

pub fn refresh_support_buttons(
    settings: Res<GameSettings>,
    powers: Res<SupportPowers>,
    aiming: Res<SupportTool>,
    local: Res<crate::sim::player_commands::LocalTeam>,
    buildings: Query<(&Building, &Team)>,
    mut buttons: Query<(
        &SupportButton,
        &Hovered,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut labels: Query<(&SupportLabel, &mut Text, &mut TextColor)>,
) {
    let owned = owned_kinds(&buildings, local.0);
    for (button, hovered, mut background, mut border) in &mut buttons {
        let aimed = aiming.0 == Some(button.0);
        background.set_if_neq(BackgroundColor(match (aimed, hovered.get()) {
            (true, _) => BUTTON_AIMING,
            (false, true) => BUTTON_HOVERED,
            (false, false) => BUTTON_IDLE,
        }));
        border.set_if_neq(BorderColor::all(if aimed { ACCENT } else { FRAME }));
    }
    for (label_of, mut text, mut color) in &mut labels {
        let power = label_of.0;
        let secs_left = powers.secs_until_ready(local.0, power);
        let available = owned.contains(&power.requires());
        text.set_if_neq(Text(label(power, &settings.keys, secs_left, available)));
        let ready = available && secs_left <= 0.0;
        color.set_if_neq(TextColor(if ready { ACCENT } else { TEXT_DIM }));
    }
}

/// A ring round the pointer shows where an aimed power would land.
pub fn draw_support_aim(mut gizmos: Gizmos, aiming: Res<SupportTool>, cursor: Res<Cursor>) {
    let (Some(_), Some(ground)) = (aiming.0, cursor.ground) else {
        return;
    };
    if cursor.over_interface {
        return;
    }
    let ring = Isometry3d::new(
        ground + Vec3::Y * 0.2,
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
    );
    gizmos.circle(ring, AIMING_RING, AIMING_COLOR);
}
