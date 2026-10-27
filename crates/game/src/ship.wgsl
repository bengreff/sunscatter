// Ships (ship_light.rs): Bevy's standard material, lit like the terrain
// (D055): each Bevy directional light (one per slotted star, carrying its
// flux at the camera) rescaled by the share the ship receives (its flux
// here, its eclipses), Bevy's ambient term, planetshine from each
// reflector as a lit disc (right in the near field: a ship in low orbit is
// lit by the Earth below it on every side that sees it), and the
// starlight without a light slot.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    pbr_functions,
    pbr_types,
    lighting,
    shadows,
    mesh_types,
    mesh_view_types,
    ambient::ambient_light,
    mesh_view_bindings::{view, lights},
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::{prepass_io::{VertexOutput, FragmentOutput}, pbr_deferred_functions::deferred_output}
#else
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
#endif

const PI: f32 = 3.14159265;
const MAX_STARS: i32 = 4;
const MAX_SHINES: i32 = 2;

// ship_light::ShipLight.
struct ShipLight {
    // Per slot: xyz the direction to its star, w the share of its light here.
    stars: array<vec4<f32>, 4>,
    // Per reflector: xyz the direction to its lit disc, w sin² of its angular radius.
    shine_dir: array<vec4<f32>, 2>,
    // rgb its mean radiance (lux/sr); w 0 when there is none.
    shine_radiance: array<vec4<f32>, 2>,
    // x: starlight without a slot (lux, sphere-averaged).
    ambient: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<storage, read> ship: ShipLight;

// Irradiance per unit radiance from a uniformly bright disc of angular
// radius asin(√sin2) at an angle of cosine cos_n from the normal, the part
// below the horizon cut off (lighting::rules::disc_irradiance, mirrored).
fn disc_irradiance(cos_n: f32, sin2: f32) -> f32 {
    let c = clamp(cos_n, -1.0, 1.0);
    if sin2 <= 0.0 {
        return 0.0;
    }
    if c * c >= sin2 {
        return PI * sin2 * max(c, 0.0);
    }
    let sin_n = sqrt(max(1.0 - c * c, 1e-12));
    let x = sqrt(max(1.0 / min(sin2, 1.0) - 1.0, 0.0));
    let y = clamp(-x * c / sin_n, -1.0, 1.0);
    let s = sin_n * sqrt(1.0 - y * y);
    return max((c * acos(y) - x * s) * sin2 + atan(s / max(x, 1e-12)), 0.0);
}

#ifndef PREPASS_PIPELINE
// The slot of Bevy's directional light `i`: the star in its direction.
fn slot_of_light(i: u32) -> i32 {
    let l = lights.directional_lights[i].direction_to_light.xyz;
    var best = 0;
    var best_dot = -2.0;
    for (var k = 0; k < MAX_STARS; k++) {
        let s = ship.stars[k];
        if dot(s.xyz, s.xyz) <= 0.0 {
            continue;
        }
        let d = dot(l, normalize(s.xyz));
        if d > best_dot {
            best_dot = d;
            best = k;
        }
    }
    return best;
}

fn shade(in: pbr_types::PbrInput) -> vec3<f32> {
    let base = in.material.base_color;
    let metallic = in.material.metallic;
    let perceptual_roughness = in.material.perceptual_roughness;
    let NdotV = max(dot(in.N, in.V), 0.0001);
    var input: lighting::LightingInput;
    input.layers[lighting::LAYER_BASE].NdotV = NdotV;
    input.layers[lighting::LAYER_BASE].N = in.N;
    input.layers[lighting::LAYER_BASE].R = reflect(-in.V, in.N);
    input.layers[lighting::LAYER_BASE].perceptual_roughness = perceptual_roughness;
    input.layers[lighting::LAYER_BASE].roughness = lighting::perceptualRoughnessToRoughness(perceptual_roughness);
    input.P = in.world_position.xyz;
    input.V = in.V;
    input.diffuse_color = pbr_functions::calculate_diffuse_color(base.rgb, metallic, in.material.specular_transmission, in.material.diffuse_transmission);
    input.metallic = metallic;
    input.F0_dielectric = pbr_functions::calculate_F0_dielectric(in.material.reflectance);
    input.F0_metallic = base.rgb;
    input.F_ab = lighting::F_AB(perceptual_roughness, NdotV);
    let view_z = dot(vec4(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), in.world_position);
    var lit = vec3(0.0);
    for (var i = 0u; i < lights.n_directional_lights; i++) {
        var shadow = 1.0;
        if (in.flags & mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u
                && (lights.directional_lights[i].flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = shadows::fetch_directional_shadow(i, in.world_position, in.world_normal, view_z, in.frag_coord.xy);
        }
        lit += lighting::directional_light(i, &input, true) * shadow * ship.stars[slot_of_light(i)].w;
    }
    let f0 = mix(input.F0_dielectric, input.F0_metallic, metallic);
    lit += ambient_light(in.world_position, in.N, in.V, NdotV, input.diffuse_color, f0, perceptual_roughness, in.diffuse_occlusion);
    // Planetshine: each reflector's lit disc, diffuse.
    for (var m = 0; m < MAX_SHINES; m++) {
        let r = ship.shine_radiance[m];
        if r.w <= 0.0 {
            continue;
        }
        let d = ship.shine_dir[m];
        lit += input.diffuse_color * r.rgb * disc_irradiance(dot(in.N, d.xyz), d.w) / PI;
    }
    // Starlight without a light slot, sphere-averaged.
    lit += input.diffuse_color * ship.ambient.x / PI;
    return lit * view.exposure;
}
#endif

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = vec4(shade(pbr_input), pbr_input.material.base_color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
