// A side's colour put where the model has the key colour: models are exported once, their side-coloured
// surfaces painted TEAM_COLOR_KEY in the palette (tools/blender/rimefront_models/materials.py), and every
// side's units draw with the same mesh and palette, only this colour apart.

import bevy_pbr::render::{
    mesh_bindings::mesh,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

// Same as "key" in tools/blender/rimefront_models/materials.py.
const TEAM_COLOR_KEY: vec3<f32> = vec3<f32>(1.0, 0.0, 1.0);
const KEY_TOLERANCE: f32 = 0.02;

// Same layout as `SideTintSettings` in src/view/side_tint.rs.
struct SideTintSettings {
    color: vec4<f32>,
}

@if(BINDLESS) {
struct SideTintIndices {
    settings: u32,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<storage> side_tint_indices: array<SideTintIndices>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage> side_tint_array: array<SideTintSettings>;
}
@else {
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> side_tint_uniform: SideTintSettings;
}

fn side_tint(instance_index: u32) -> SideTintSettings {
@if(BINDLESS) {
    let slot = mesh[instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    return side_tint_array[side_tint_indices[slot].settings];
}
@else {
    return side_tint_uniform;
}
}

// Worn paint, rain streaks and grain, computed from where a point lies on its own part so they stay put as a
// unit moves; no texture is read. Lamps and glass stay clean.
const WEAR_PATCHES_PER_METRE: f32 = 1.7;
const WEAR_CONTRAST: f32 = 0.45;
const STREAKS_PER_METRE: vec3<f32> = vec3<f32>(7.0, 0.9, 7.0);
const STREAK_CONTRAST: f32 = 0.4;
const GRAINS_PER_METRE: f32 = 16.0;
const GRAIN_CONTRAST: f32 = 0.3;
const SMOOTHEST_SURFACE_THAT_WEARS: f32 = 0.2;
const ROUGHNESS_PER_SHADE: f32 = -0.6;

fn scattered(cell: vec3<f32>) -> f32 {
    let mixed = fract(cell * 0.3183099 + vec3<f32>(0.71, 0.113, 0.419)) * 17.0;
    return fract(mixed.x * mixed.y * mixed.z * (mixed.x + mixed.y + mixed.z));
}

fn value_noise(at: vec3<f32>) -> f32 {
    let cell = floor(at);
    let within = fract(at);
    let blend = within * within * (3.0 - 2.0 * within);
    let near = mix(
        mix(scattered(cell), scattered(cell + vec3<f32>(1.0, 0.0, 0.0)), blend.x),
        mix(scattered(cell + vec3<f32>(0.0, 1.0, 0.0)), scattered(cell + vec3<f32>(1.0, 1.0, 0.0)), blend.x),
        blend.y,
    );
    let far = mix(
        mix(scattered(cell + vec3<f32>(0.0, 0.0, 1.0)), scattered(cell + vec3<f32>(1.0, 0.0, 1.0)), blend.x),
        mix(scattered(cell + vec3<f32>(0.0, 1.0, 1.0)), scattered(cell + vec3<f32>(1.0, 1.0, 1.0)), blend.x),
        blend.y,
    );
    return mix(near, far, blend.z);
}

fn position_on_part(world_position: vec3<f32>, instance_index: u32) -> vec3<f32> {
    let world_from_local = mesh[instance_index].world_from_local;
    let packed = mesh[instance_index].local_from_world_transpose_a;
    let local_from_world_transpose = mat3x3<f32>(
        packed[0].xyz,
        vec3<f32>(packed[0].w, packed[1].xy),
        vec3<f32>(packed[1].zw, mesh[instance_index].local_from_world_transpose_b),
    );
    let origin = vec3<f32>(world_from_local[0].w, world_from_local[1].w, world_from_local[2].w);
    return (world_position - origin) * local_from_world_transpose;
}

// How much lighter or darker than its paint a point is, around zero.
fn surface_shade(on_part: vec3<f32>, steepness: f32) -> f32 {
    let wear = value_noise(on_part * WEAR_PATCHES_PER_METRE) - 0.5;
    let streaks = (value_noise(on_part * STREAKS_PER_METRE) - 0.5) * steepness;
    // Grain smaller than a pixel would only shimmer.
    let grains_per_pixel = length(fwidth(on_part)) * GRAINS_PER_METRE;
    let grain_shown = clamp(1.5 - 2.0 * grains_per_pixel, 0.0, 1.0);
    var shade = wear * WEAR_CONTRAST + streaks * STREAK_CONTRAST;
    if grain_shown > 0.0 {
        shade += (value_noise(on_part * GRAINS_PER_METRE) - 0.5) * GRAIN_CONTRAST * grain_shown;
    }
    return shade;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    let instance = in.instance_index;
@else
    let instance = 0u;
    let base = pbr_input.material.base_color;
    // The base colour arrives multiplied by the model's baked weathering, never darker than DARKEST in
    // tools/blender/rimefront_models/weathering.py; the key is looked for underneath it.
@if(VERTEX_COLORS)
    let weathering = in.color.rgb;
@else
    let weathering = vec3<f32>(1.0);
    if all(abs(base.rgb / weathering - TEAM_COLOR_KEY) < vec3<f32>(KEY_TOLERANCE)) {
        pbr_input.material.base_color = vec4<f32>(side_tint(instance).color.rgb * weathering, base.a);
    }
    let glows = any(pbr_input.material.emissive.rgb > vec3<f32>(0.0));
    let glossy = pbr_input.material.perceptual_roughness < SMOOTHEST_SURFACE_THAT_WEARS;
    let wear_shown = select(1.0, 0.0, glows || glossy);
    let steepness = 1.0 - abs(normalize(in.world_normal).y);
    let shade = surface_shade(position_on_part(in.world_position.xyz, instance), steepness) * wear_shown;
    let painted = pbr_input.material.base_color;
    pbr_input.material.base_color = vec4<f32>(painted.rgb * (1.0 + shade), painted.a);
    pbr_input.material.perceptual_roughness = clamp(
        pbr_input.material.perceptual_roughness + shade * ROUGHNESS_PER_SHADE,
        0.05,
        1.0,
    );
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
