// A side's colour put where the model has the key colour: models are exported once, their side-coloured
// surfaces painted TEAM_COLOR_KEY in the palette (tools/blender/irontide_models/materials.py), and every
// side's units draw with the same mesh and palette, only this colour apart.

import bevy_pbr::render::{
    mesh_bindings::mesh,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

// Same as "key" in tools/blender/irontide_models/materials.py.
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

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let base = pbr_input.material.base_color;
    if all(abs(base.rgb - TEAM_COLOR_KEY) < vec3<f32>(KEY_TOLERANCE)) {
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
        let tint = side_tint(in.instance_index);
@else
        let tint = side_tint(0u);
        pbr_input.material.base_color = vec4<f32>(tint.color.rgb, base.a);
    }
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
