// Ships (ship_light.rs): Bevy's standard material, each craft lit as one
// at its own position (D110): each Bevy directional light (one per slotted
// star, carrying its flux at the camera) rescaled by the share the craft
// receives (its flux here, its eclipses) and filtered by the air on the
// craft's line to the star (this shader is built without Bevy's
// per-fragment atmosphere: ShipExt::specialize), planetshine from the
// patches of each reflector the craft sees (lighting::rules::
// shine_patches), the sky
// light of the craft's own air, and the starlight without a light slot.
// The brightest star's shadow comes from the craft's own cascade of the
// craft shadow light (lighting::shadows: fitted to the craft, so it does
// not change with the camera), never from the camera-fitted cascades.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    pbr_functions,
    pbr_types,
    lighting,
    shadows,
    mesh_types,
    mesh_view_types,
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
const SHINE_PATCHES: i32 = 32;

// ship_light::ShipLight.
struct ShipLight {
    // Per slot: xyz the direction to its star, w the share of its light here.
    stars: array<vec4<f32>, 4>,
    // Per slot: rgb the air's transmittance on the craft's line to the star.
    star_air: array<vec4<f32>, 4>,
    // Planetshine: per reflector, its patches (xyz the direction scaled by
    // the irradiance a face turned to it takes).
    shine: array<vec4<f32>, 64>,
    // Per reflector: rgb its light's colour; w 0 when there is none.
    shine_tint: array<vec4<f32>, 2>,
    // x: starlight without a slot (lux, sphere-averaged); y: the sky's
    // radiance around the craft; z: the craft's cascade of the craft shadow
    // light (-1: none).
    ambient: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<storage, read> ship: ShipLight;

// The planetshine irradiance on a surface of normal n: each patch of each
// reflector by the cosine law (lighting::rules::ShinePatches::irradiance,
// mirrored).
fn shine_irradiance(n: vec3<f32>) -> vec3<f32> {
    var e = vec3(0.0);
    for (var m = 0; m < MAX_SHINES; m++) {
        let tint = ship.shine_tint[m];
        if tint.w <= 0.0 {
            continue;
        }
        var sum = 0.0;
        for (var p = 0; p < SHINE_PATCHES; p++) {
            sum += max(dot(n, ship.shine[m * SHINE_PATCHES + p].xyz), 0.0);
        }
        e += tint.rgb * sum;
    }
    return e;
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

// The craft shadow light: the one that gives no light but casts shadows.
fn is_craft_shadow_light(i: u32) -> bool {
    let l = lights.directional_lights[i];
    return (l.flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u
        && dot(l.color.rgb, l.color.rgb) == 0.0;
}

// The brightest star's shadow on this fragment, from the craft's cascade.
fn craft_shadow(in: pbr_types::PbrInput) -> f32 {
    let cascade = i32(ship.ambient.z);
    if cascade < 0 || (in.flags & mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT) == 0u {
        return 1.0;
    }
    for (var j = 0u; j < lights.n_directional_lights; j++) {
        if is_craft_shadow_light(j) {
            return shadows::sample_directional_cascade(j, u32(cascade), in.world_position, in.world_normal, in.frag_coord.xy);
        }
    }
    return 1.0;
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
    var lit = vec3(0.0);
    let shadow0 = craft_shadow(in);
    for (var i = 0u; i < lights.n_directional_lights; i++) {
        if is_craft_shadow_light(i) {
            continue;
        }
        let slot = slot_of_light(i);
        let shadow = select(1.0, shadow0, slot == 0);
        lit += lighting::directional_light(i, &input, true) * shadow * ship.stars[slot].w * ship.star_air[slot].rgb;
    }
    // Planetshine, diffuse.
    lit += input.diffuse_color * shine_irradiance(in.N) / PI;
    // The sky around the craft (a uniform radiance) and the starlight
    // without a light slot, sphere-averaged.
    lit += input.diffuse_color * (ship.ambient.y + ship.ambient.x / PI);
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
