use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// A standard material whose vertices the GPU moves: plants lean in the wind and crystal shards hover;
/// scenery also varies its colour a little per instance.
/// Doing it in the shaders keeps the CPU out of it and lets identical meshes keep drawing together; changing
/// a material asset instead makes Bevy rewrite the draw data of everything using it, every frame.
pub type MovingMaterial = ExtendedMaterial<StandardMaterial, Motion>;

const GUST_PERIOD_SECS: f32 = 9.0;
const SHARD_BOB: f32 = 0.08;
const SHARD_LOOP_SECS: f32 = 4.0;
const SCENERY_TINT_VARIATION: f32 = 0.14;
const CORE_PULSE_SPEED: f32 = 1.6;

/// Bindless like the standard material, so meshes with different materials still draw in one batch.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[data(100, MotionSettings, binding_array(101))]
#[bindless(index_table(range(100..101), binding(102)))]
pub struct Motion {
    pub settings: MotionSettings,
}

impl From<&Motion> for MotionSettings {
    fn from(motion: &Motion) -> Self {
        motion.settings
    }
}

pub use uniforms::MotionSettings;

mod uniforms {
    // The ShaderType derive emits per-field `check` helpers that rustc reports as unused.
    #![allow(dead_code)]
    use bevy::prelude::*;
    use bevy::render::render_resource::ShaderType;

    /// Same layout as `MotionSettings` in assets/shaders/motion_common.wesl.
    #[derive(Clone, Copy, Debug, Reflect, ShaderType)]
    pub struct MotionSettings {
        /// Radians a plant leans at its tip in a full gust; zero keeps it still.
        pub sway_lean: f32,
        pub gust_period_secs: f32,
        pub hover_bob: f32,
        /// Zero leaves shard data in the mesh unused.
        pub hover_loop_secs: f32,
        /// How far each instance's colour strays from the model's, so a forest is not one tree copied.
        pub tint_variation: f32,
        /// How fast the emission swells and fades, in radians per second; zero glows steadily.
        pub glow_pulse_speed: f32,
    }
}

impl Motion {
    /// Scenery standing in the wind; `lean` zero for anything rigid.
    pub fn scenery(lean: f32) -> Self {
        Self {
            settings: MotionSettings {
                sway_lean: lean,
                gust_period_secs: GUST_PERIOD_SECS,
                hover_bob: 0.0,
                hover_loop_secs: 0.0,
                tint_variation: SCENERY_TINT_VARIATION,
                glow_pulse_speed: 0.0,
            },
        }
    }

    /// Crystal parts: floating shards hover, and the glowing cores pulse.
    pub fn hovering(pulsing: bool) -> Self {
        Self {
            settings: MotionSettings {
                sway_lean: 0.0,
                gust_period_secs: GUST_PERIOD_SECS,
                hover_bob: SHARD_BOB,
                hover_loop_secs: SHARD_LOOP_SECS,
                tint_variation: 0.0,
                glow_pulse_speed: if pulsing { CORE_PULSE_SPEED } else { 0.0 },
            },
        }
    }
}

impl MaterialExtension for Motion {
    fn vertex_shader() -> ShaderRef {
        "shaders/motion.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/motion.wesl".into()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/motion_prepass.wesl".into()
    }
}
