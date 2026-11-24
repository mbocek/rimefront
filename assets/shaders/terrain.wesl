import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};
import bevy_pbr::render::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing};
import bevy_pbr::render::mesh_view_bindings::globals;

struct TerrainSettings {
    grass_tint: vec4<f32>,
    layer_tile_sizes: vec4<f32>,
    map_area: vec4<f32>,
    pad_tile_size: f32,
    normal_strength: f32,
    macro_variation: f32,
    water_level: f32,
    snow_cover: f32,
    wet_ground: f32,
    cloud_cover: f32,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> terrain: TerrainSettings;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var albedo_layers: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var albedo_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var normal_layers: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var normal_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(105) var fog_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(106) var fog_sampler: sampler;

const LAYER_COUNT: u32 = 5u;
const ANTI_TILE_ROTATION: mat2x2<f32> = mat2x2<f32>(0.8, -0.6, 0.6, 0.8);
const ANTI_TILE_SCALE: f32 = 0.37;
const HEIGHT_BLEND_SHARPNESS: f32 = 0.25;
// The fog texture stores perceived brightness; lighting is linear, so it is raised to the display gamma.
const FOG_GAMMA: f32 = 2.2;
const CLOUD_DRIFT: vec2<f32> = vec2<f32>(1.6, 0.9);
const CLOUD_SCALE: f32 = 0.011;
const CLOUD_SHADOW_DEPTH: f32 = 0.32;
const FOAM_COLOR: vec3<f32> = vec3<f32>(0.82, 0.86, 0.85);
const FOAM_BAND: f32 = 0.09;
// Same as SNOW in src/season.rs.
const SNOW_COLOR: vec3<f32> = vec3<f32>(0.82, 0.86, 0.92);

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let blend = local * local * (3.0 - 2.0 * local);
    let bottom = mix(hash(cell), hash(cell + vec2<f32>(1.0, 0.0)), blend.x);
    let top = mix(hash(cell + vec2<f32>(0.0, 1.0)), hash(cell + vec2<f32>(1.0, 1.0)), blend.x);
    return mix(bottom, top, blend.y);
}

fn tile_size(layer: u32) -> f32 {
    if layer == 4u {
        return terrain.pad_tile_size;
    }
    return terrain.layer_tile_sizes[layer];
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let world_xz = in.world_position.xz;

    var splat = vec4<f32>(0.0);
    @if(VERTEX_COLORS)
    {
        splat = clamp(in.color, vec4<f32>(0.0), vec4<f32>(1.0));
    }
    var weights = array<f32, 5>(
        max(1.0 - (splat.x + splat.y + splat.z + splat.w), 0.0),
        splat.x,
        splat.y,
        splat.z,
        splat.w,
    );

    let tiling_breakup = smoothstep(0.25, 0.75, value_noise(world_xz * 0.045));
    var albedo_roughness: array<vec4<f32>, 5>;
    var normals: array<vec3<f32>, 5>;
    var blend_heights: array<f32, 5>;
    var highest = 0.0;
    // Explicit gradients let layers with no weight here skip their texture reads entirely.
    let world_dx = dpdx(world_xz);
    let world_dy = dpdy(world_xz);
    for (var layer = 0u; layer < LAYER_COUNT; layer += 1u) {
        blend_heights[layer] = -1.0;
        if weights[layer] <= 0.001 {
            continue;
        }
        let tile = tile_size(layer);
        let uv = world_xz / tile;
        let uv_dx = world_dx / tile;
        let uv_dy = world_dy / tile;
        let detail = textureSampleGrad(albedo_layers, albedo_sampler, uv, layer, uv_dx, uv_dy);
        let broad_uv = ANTI_TILE_ROTATION * uv * ANTI_TILE_SCALE + vec2<f32>(0.37, 0.71);
        let broad = textureSampleGrad(
            albedo_layers,
            albedo_sampler,
            broad_uv,
            layer,
            ANTI_TILE_ROTATION * uv_dx * ANTI_TILE_SCALE,
            ANTI_TILE_ROTATION * uv_dy * ANTI_TILE_SCALE,
        );
        albedo_roughness[layer] = mix(detail, broad, tiling_breakup * 0.5);
        if layer == 0u {
            albedo_roughness[layer] = vec4<f32>(albedo_roughness[layer].rgb * terrain.grass_tint.rgb, albedo_roughness[layer].a);
        }
        normals[layer] = textureSampleGrad(normal_layers, normal_sampler, uv, layer, uv_dx, uv_dy).xyz * 2.0 - 1.0;

        let surface_height = dot(albedo_roughness[layer].rgb, vec3<f32>(0.3, 0.5, 0.2));
        blend_heights[layer] = weights[layer] + surface_height * 0.6;
        highest = max(highest, blend_heights[layer]);
    }

    var total = 0.0;
    var albedo = vec3<f32>(0.0);
    var roughness = 0.0;
    var normal_ts = vec3<f32>(0.0);
    for (var layer = 0u; layer < LAYER_COUNT; layer += 1u) {
        let weight = max(blend_heights[layer] - (highest - HEIGHT_BLEND_SHARPNESS), 0.0);
        albedo += albedo_roughness[layer].rgb * weight;
        roughness += albedo_roughness[layer].a * weight;
        normal_ts += normals[layer] * weight;
        total += weight;
    }
    albedo /= total;
    roughness /= total;
    normal_ts = normalize(vec3<f32>(normal_ts.xy / total * terrain.normal_strength, max(normal_ts.z / total, 0.2)));

    let broad_shading = 1.0 + (value_noise(world_xz * 0.018) - 0.5) * 2.0 * terrain.macro_variation;
    albedo *= broad_shading;

    if terrain.snow_cover > 0.0 {
        let drift = value_noise(world_xz * 0.3) * 0.6 + value_noise(world_xz * 1.9) * 0.4;
        let flat_ground = smoothstep(0.7, 0.95, pbr_input.world_normal.y);
        let above_water = smoothstep(terrain.water_level + 0.15, terrain.water_level + 0.7, in.world_position.y);
        let snow = flat_ground * above_water * smoothstep(0.55, 0.3, drift - terrain.snow_cover * 0.45);
        albedo = mix(albedo, SNOW_COLOR, snow);
        roughness = mix(roughness, 0.6, snow);
    }

    let wetness = 1.0 - smoothstep(terrain.water_level, terrain.water_level + 0.5, in.world_position.y);
    albedo *= mix(1.0, 0.55, wetness);
    roughness = mix(roughness, 0.25, wetness);

    // Rain soaks low and flat ground first, so puddles show before the slopes darken. The branch skips the
    // noise on dry ground, which is most of the time and covers the whole screen.
    if terrain.wet_ground > 0.0 {
        let pooling = value_noise(world_xz * 0.45) * 0.5 + smoothstep(0.85, 1.0, pbr_input.world_normal.y) * 0.5;
        let soaked = terrain.wet_ground * smoothstep(0.15, 0.6, pooling + terrain.wet_ground * 0.4);
        albedo *= mix(1.0, 0.62, soaked);
        roughness = mix(roughness, 0.22, soaked);
    }

    let shoreline = 1.0 - smoothstep(0.0, FOAM_BAND, abs(in.world_position.y - (terrain.water_level + 0.03)));
    let foam_ripple = value_noise(world_xz * 2.3 + vec2<f32>(globals.time * 0.6, globals.time * 0.35));
    let foam = shoreline * smoothstep(0.35, 0.75, foam_ripple);
    albedo = mix(albedo, FOAM_COLOR, foam * 0.7);
    roughness = mix(roughness, 0.6, foam);

    let cloud_uv = (world_xz + CLOUD_DRIFT * globals.time) * CLOUD_SCALE;
    let cloud = value_noise(cloud_uv) * 0.65 + value_noise(cloud_uv * 2.7 + vec2<f32>(5.2, 1.3)) * 0.35;
    albedo *= 1.0 - CLOUD_SHADOW_DEPTH * (1.0 - terrain.cloud_cover) * smoothstep(0.52, 0.72, cloud);

    let geometry_normal = pbr_input.world_normal;
    let tangent = normalize(vec3<f32>(1.0, 0.0, 0.0) - geometry_normal * geometry_normal.x);
    let bitangent = cross(geometry_normal, tangent);
    pbr_input.N = normalize(tangent * normal_ts.x + bitangent * normal_ts.y + geometry_normal * normal_ts.z);

    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = clamp(roughness, 0.08, 1.0);
    pbr_input.material.metallic = 0.0;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    let map_uv = (world_xz - terrain.map_area.xy) / terrain.map_area.zw;
    // The sampler clamps to the edge, so ground beyond the map takes the fog of the nearest edge.
    let fog = textureSampleLevel(fog_texture, fog_sampler, map_uv, 0.0).r;
    out.color = vec4<f32>(out.color.rgb * pow(fog, FOG_GAMMA), out.color.a);
    return out;
}
