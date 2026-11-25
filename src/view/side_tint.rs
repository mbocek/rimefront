use std::collections::HashMap;

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

use crate::app::match_flow::MirroredBattle;
use crate::app::settings::GameSettings;
use crate::net::network::NetSession;
use crate::sim::catalog::Team;
use crate::sim::player_commands::LocalTeam;

/// A model's own material with its side's colour put where the model has the key colour; see
/// assets/shaders/side_tint.wesl.
pub type SideTintedMaterial = ExtendedMaterial<StandardMaterial, SideTint>;

/// Bindless like the standard material, so every side's units still draw in one batch.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[data(100, SideTintSettings, binding_array(101))]
#[bindless(index_table(range(100..101), binding(102)))]
pub struct SideTint {
    pub settings: SideTintSettings,
}

impl From<&SideTint> for SideTintSettings {
    fn from(tint: &SideTint) -> Self {
        tint.settings
    }
}

pub use uniforms::SideTintSettings;

mod uniforms {
    // The ShaderType derive emits per-field `check` helpers that rustc reports as unused.
    #![allow(dead_code)]
    use bevy::prelude::*;
    use bevy::render::render_resource::ShaderType;

    /// Same layout as `SideTintSettings` in assets/shaders/side_tint.wesl.
    #[derive(Clone, Copy, Debug, Reflect, ShaderType)]
    pub struct SideTintSettings {
        pub color: Vec4,
    }
}

impl MaterialExtension for SideTint {
    fn fragment_shader() -> ShaderRef {
        "shaders/side_tint.wesl".into()
    }
}

/// Whose colour a model's side-coloured surfaces take; put on the root of the model of a unit or structure.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Painted {
    Side(Team),
    /// A neutral site nobody holds.
    Unclaimed,
    /// The model the menu shows the player's chosen colour on.
    Preview,
}

const UNCLAIMED_GREY: Color = Color::linear_rgb(0.40, 0.41, 0.40);

/// The colour of every side's models: what its player chose, or else the side's own.
#[derive(Resource, Default, Debug)]
pub struct SideColors {
    chosen: HashMap<Team, Color>,
    /// The colour the menu's preview shows; `None` the player's side's own.
    pub preview: Option<Color>,
}

impl SideColors {
    pub fn of(&self, paint: Painted) -> Color {
        match paint {
            Painted::Side(team) => self
                .chosen
                .get(&team)
                .copied()
                .unwrap_or_else(|| team.model_tint()),
            Painted::Unclaimed => UNCLAIMED_GREY,
            Painted::Preview => self.preview.unwrap_or_else(|| Team::Player.model_tint()),
        }
    }

    pub fn choose(&mut self, team: Team, color: Color) {
        self.chosen.insert(team, color);
    }

    /// The side's colour on the minimap and in the sidebar.
    pub fn ui_of(&self, team: Team) -> Color {
        self.chosen
            .get(&team)
            .copied()
            .unwrap_or_else(|| team.ui_color())
    }
}

/// A new battle starts with every side in its own colour and the player's in the one they chose; a network
/// battle tells the other players, and a world sends every side's colour itself.
pub fn paint_sides_for_a_new_battle(
    mut colors: ResMut<SideColors>,
    settings: Res<GameSettings>,
    local: Res<LocalTeam>,
    session: Option<ResMut<NetSession>>,
    world: Option<Res<MirroredBattle>>,
) {
    colors.chosen.clear();
    let Some(color) = settings.side_color.filter(|_| world.is_none()) else {
        return;
    };
    colors.choose(local.0, color.color());
    if let Some(mut session) = session
        && !session.command_timing().spectating
    {
        session.paint(local.0, color);
    }
}

/// The colours the other players of a network battle chose.
pub fn paint_other_players_sides(
    session: Option<ResMut<NetSession>>,
    mut colors: ResMut<SideColors>,
) {
    let Some(mut session) = session else { return };
    for (team, color) in session.take_paints() {
        colors.choose(team, color.color());
    }
}

/// A replay shows every side in its own colour.
pub fn forget_chosen_colors(mut colors: ResMut<SideColors>) {
    colors.chosen.clear();
}

/// A part painted in its side's colour: the model's own material, to paint it again when the colour changes.
#[derive(Component, Clone, Debug)]
pub struct PaintedPart {
    paint: Painted,
    own: Handle<StandardMaterial>,
}

/// One tinted material for every model material and colour, made the first time a part needs it.
#[derive(Resource, Default)]
pub struct SideTintedMaterials(
    HashMap<(AssetId<StandardMaterial>, [u32; 3]), Handle<SideTintedMaterial>>,
);

impl SideTintedMaterials {
    fn for_colour(
        &mut self,
        own: &Handle<StandardMaterial>,
        color: Color,
        standard: &Assets<StandardMaterial>,
        tinted: &mut Assets<SideTintedMaterial>,
    ) -> Option<Handle<SideTintedMaterial>> {
        let linear = color.to_linear();
        let key = (
            own.id(),
            [linear.red, linear.green, linear.blue].map(f32::to_bits),
        );
        if let Some(handle) = self.0.get(&key) {
            return Some(handle.clone());
        }
        let base = standard.get(own)?.clone();
        let handle = tinted.add(SideTintedMaterial {
            base,
            extension: SideTint {
                settings: SideTintSettings {
                    color: linear.to_vec4(),
                },
            },
        });
        self.0.insert(key, handle.clone());
        Some(handle)
    }
}

/// Gives the parts of a model under `Painted` their side's colour, as the model's scene puts them down.
pub fn paint_new_parts(
    mut commands: Commands,
    parts: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
    parents: Query<&ChildOf>,
    paints: Query<&Painted>,
    colors: Res<SideColors>,
    standard: Res<Assets<StandardMaterial>>,
    mut tinted: ResMut<Assets<SideTintedMaterial>>,
    mut made: ResMut<SideTintedMaterials>,
) {
    for (part, material) in &parts {
        let Some(&paint) = parents
            .iter_ancestors(part)
            .find_map(|ancestor| paints.get(ancestor).ok())
        else {
            continue;
        };
        let Some(handle) = made.for_colour(&material.0, colors.of(paint), &standard, &mut tinted)
        else {
            continue;
        };
        commands
            .entity(part)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((
                MeshMaterial3d(handle),
                PaintedPart {
                    paint,
                    own: material.0.clone(),
                },
            ));
    }
}

/// Paints every part again when the sides' colours change.
pub fn repaint_parts(
    colors: Res<SideColors>,
    mut parts: Query<(&PaintedPart, &mut MeshMaterial3d<SideTintedMaterial>)>,
    standard: Res<Assets<StandardMaterial>>,
    mut tinted: ResMut<Assets<SideTintedMaterial>>,
    mut made: ResMut<SideTintedMaterials>,
) {
    if !colors.is_changed() {
        return;
    }
    for (part, mut material) in &mut parts {
        if let Some(handle) =
            made.for_colour(&part.own, colors.of(part.paint), &standard, &mut tinted)
        {
            material.0 = handle;
        }
    }
}
