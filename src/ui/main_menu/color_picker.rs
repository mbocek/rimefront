use bevy::camera::RenderTarget;
use bevy::render::render_resource::TextureFormat;

use super::*;
use crate::sim::catalog::{SIDE_PALETTE, SideColor, UnitKind};
use crate::view::assets::ModelLibrary;
use crate::view::side_tint::{Painted, SideColors};

/// Where the menu's tank stands: far above any battle's ground, out of reach of the game's camera.
const PREVIEW_SPOT: Vec3 = Vec3::new(0.0, 5_000.0, 0.0);
const PREVIEW_PIXELS: u32 = 256;
const PREVIEW_SIZE: f32 = 128.0;
const SWATCH: f32 = 24.0;
/// Radians a second the tank turns on its stand.
const TURNTABLE_SPEED: f32 = 0.6;

/// The picture of a tank in the player's colour that the network page shows.
#[derive(Resource)]
pub struct SidePreview {
    image: Handle<Image>,
}

#[derive(Component)]
pub struct SidePreviewCamera;

#[derive(Component)]
pub struct Turntable;

/// A colour to click for the player's side; `None` takes the side's own.
#[derive(Component, Clone, Copy)]
struct ColorSwatch(Option<SideColor>);

/// A tank on a turntable far above the map, and a camera drawing it into a picture for the menu.
pub fn set_up_side_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    models: Res<ModelLibrary>,
) {
    let image = images.add(Image::new_target_texture(
        PREVIEW_PIXELS,
        PREVIEW_PIXELS,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let (scene, _) = models.unit(UnitKind::Tank, Team::Player);
    commands.spawn((
        scene,
        Painted::Preview,
        Turntable,
        Transform::from_translation(PREVIEW_SPOT),
        DespawnOnExit(GameState::MainMenu),
    ));
    commands.spawn((
        PointLight {
            intensity: 8_000_000.0,
            range: 60.0,
            ..default()
        },
        Transform::from_translation(PREVIEW_SPOT + Vec3::new(6.0, 10.0, 8.0)),
        DespawnOnExit(GameState::MainMenu),
    ));
    commands.spawn((
        SidePreviewCamera,
        Camera3d::default(),
        Camera {
            order: -1,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.07, 0.08, 0.09)),
            ..default()
        },
        RenderTarget::from(image.clone()),
        Transform::from_translation(PREVIEW_SPOT + Vec3::new(3.6, 2.6, 3.6))
            .looking_at(PREVIEW_SPOT + Vec3::Y * 0.6, Vec3::Y),
        DespawnOnExit(GameState::MainMenu),
    ));
    commands.insert_resource(SidePreview { image });
}

/// The preview draws only while the network page shows it, in the colour the settings hold.
pub fn run_side_preview(
    time: Res<Time<Real>>,
    menu: Option<Res<MainMenu>>,
    settings: Res<GameSettings>,
    mut colors: ResMut<SideColors>,
    mut cameras: Query<&mut Camera, With<SidePreviewCamera>>,
    mut turntables: Query<&mut Transform, With<Turntable>>,
) {
    let shown = menu.is_some_and(|menu| menu.selection == Selection::Network);
    for mut camera in &mut cameras {
        if camera.is_active != shown {
            camera.is_active = shown;
        }
    }
    if !shown {
        return;
    }
    let wanted = settings.side_color.map(SideColor::color);
    if colors.preview != wanted {
        colors.preview = wanted;
    }
    for mut transform in &mut turntables {
        transform.rotate_y(TURNTABLE_SPEED * time.delta_secs());
    }
}

/// A row of colours to click for the player's side, the side's own first, beside the tank wearing it.
pub(super) fn spawn_color_picker(
    panel: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    chosen: Option<SideColor>,
    preview: Option<&SidePreview>,
) {
    panel.spawn(fonts.heading(tr("Side colour"), 18.0, ACCENT));
    panel
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(12.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            if let Some(preview) = preview {
                row.spawn((
                    ImageNode::new(preview.image.clone()),
                    Node {
                        width: Val::Px(PREVIEW_SIZE),
                        height: Val::Px(PREVIEW_SIZE),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(FRAME),
                ));
            }
            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                max_width: Val::Px(7.0 * (SWATCH + 6.0)),
                ..default()
            })
            .with_children(|swatches| {
                let choices =
                    std::iter::once(None).chain(SIDE_PALETTE.iter().map(|&(_, color)| Some(color)));
                for color in choices {
                    let fill = color.map_or(Team::Player.ui_color(), SideColor::color);
                    let picked = color == chosen;
                    let mut swatch = swatches.spawn((
                        ColorSwatch(color),
                        Node {
                            width: Val::Px(SWATCH),
                            height: Val::Px(SWATCH),
                            border: UiRect::all(Val::Px(if picked { 3.0 } else { 1.0 })),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(fill),
                        BorderColor::all(if picked { ACCENT } else { FRAME }),
                    ));
                    if color.is_none() {
                        swatch.with_child((
                            Node {
                                width: Val::Px(SWATCH * 0.4),
                                height: Val::Px(SWATCH * 0.4),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                            Pickable::IGNORE,
                        ));
                    }
                    swatch.observe(pick_color);
                }
            });
        });
}

fn pick_color(
    click: On<PointerClick>,
    swatches: Query<&ColorSwatch>,
    mut settings: ResMut<GameSettings>,
    mut menu: ResMut<MainMenu>,
) {
    let Ok(&ColorSwatch(color)) = swatches.get(click.entity) else {
        return;
    };
    if click.button != PointerButton::Primary {
        return;
    }
    if settings.side_color != color {
        settings.side_color = color;
        menu.needs_rebuild = true;
    }
}
