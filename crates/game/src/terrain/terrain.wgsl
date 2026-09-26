// Terrain surface: StandardMaterial lighting with the body's colour map
// looked up per fragment from the body-fixed direction (no UV seams or pole
// pinching), a close-range procedural detail layer, and water shading from
// the body's water mask, also looked up per fragment (so it does not change
// with the LOD level; CPU mirror and tests in water.rs).

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::view,
    mesh_functions,
    view_transformations::position_world_to_clip,
}
#endif

struct TerrainParams {
    // Rows of the world → body-fixed rotation.
    fixed_x: vec4<f32>,
    fixed_y: vec4<f32>,
    fixed_z: vec4<f32>,
    // xyz: body centre relative to the camera (world space).
    center: vec4<f32>,
    // x: equatorial radius, y: polar radius, z: detail fade distance (m), w: detail strength.
    shape: vec4<f32>,
    // rgb: rock colour, w: snow line (m, < 0 = none).
    rock: vec4<f32>,
    // rgb: snow colour, w: surface roughness.
    snow: vec4<f32>,
    // rgb: water tint, w: water roughness.
    ocean: vec4<f32>,
    // x: detail on, y: glint on, z: has colour map, w: has water mask.
    flags: vec4<u32>,
    // rgb: base colour without a map.
    base: vec4<f32>,
    // Lighting (D055): the star (centre, radius), the body that can eclipse
    // it, planetshine (reflector centre, lux here), and rgb shine colour +
    // w this body's sunlight relative to the shared light's.
    star: vec4<f32>,
    occluder: vec4<f32>,
    shine: vec4<f32>,
    light: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> terrain: TerrainParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var color_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var color_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var water_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var water_sampler: sampler;

const PI: f32 = 3.14159265;

// Area of overlap of two discs (radii r1, r2, centres d apart); mirrors
// `lighting::disc_overlap` (tested there).
fn disc_overlap(r1: f32, r2: f32, d: f32) -> f32 {
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= abs(r1 - r2) {
        let r = min(r1, r2);
        return PI * r * r;
    }
    let a1 = acos(clamp((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1), -1.0, 1.0));
    let a2 = acos(clamp((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2), -1.0, 1.0));
    return r1 * r1 * (a1 - sin(a1) * cos(a1)) + r2 * r2 * (a2 - sin(a2) * cos(a2));
}

// Fraction of the star's disc visible from `p` past the occluder; mirrors
// `lighting::eclipse_factor`.
fn eclipse(p: vec3<f32>) -> f32 {
    let r = terrain.occluder.w;
    if r <= 0.0 {
        return 1.0;
    }
    let to_star = terrain.star.xyz - p;
    let ds = length(to_star);
    let to_c = terrain.occluder.xyz - p;
    let dc = length(to_c);
    if dc <= r || dc >= ds || dot(to_c, to_star) <= 0.0 {
        return 1.0;
    }
    let s_ang = asin(clamp(terrain.star.w / ds, 0.0, 1.0));
    let o_ang = asin(clamp(r / dc, 0.0, 1.0));
    let sep = acos(clamp(dot(to_c / dc, to_star / ds), -1.0, 1.0));
    return clamp(1.0 - disc_overlap(s_ang, o_ang, sep) / (PI * s_ang * s_ang), 0.0, 1.0);
}

fn hash2(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

// Value noise with lattice period `period` (so it tiles every `period` units).
fn noise(p: vec2<f32>, period: f32) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2((i + vec2(0.0, 0.0)) % period);
    let b = hash2((i + vec2(1.0, 0.0)) % period);
    let c = hash2((i + vec2(0.0, 1.0)) % period);
    let d = hash2((i + vec2(1.0, 1.0)) % period);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// `octaves` octaves, normalised to [0, 1]; coordinates wrap at 256, so each
// octave's period is 256·2^k.
fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var k = 0; k < octaves; k++) {
        sum += amp * noise(p * freq, 256.0 * freq);
        amp *= 0.5;
        freq *= 2.0;
    }
    return sum / (1.0 - amp * 2.0 + 1e-6);
}

// Wraps a derivative of the longitude coordinate across the seam.
fn wrap_d(d: f32) -> f32 {
    return d - round(d);
}

#ifndef PREPASS_PIPELINE
struct TerrainVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    // Offset to the parent level's surface (geomorphing).
    @location(8) morph: vec3<f32>,
}

// Bevy's mesh vertex shader plus geomorphing: each vertex moves towards
// the parent level's surface by the chunk's morph factor (its MeshTag /
// 1000), so a level change happens when the shapes already match.
@vertex
fn vertex(vertex: TerrainVertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let t = f32(mesh_functions::get_tag(vertex.instance_index)) / 1000.0;
    let local = vertex.position + vertex.morph * t;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}
#endif

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let rel = in.world_position.xyz - terrain.center.xyz;
    let fixed = vec3(dot(terrain.fixed_x.xyz, rel), dot(terrain.fixed_y.xyz, rel), dot(terrain.fixed_z.xyz, rel));
    let dir = normalize(fixed);
    let lat = asin(clamp(dir.z, -1.0, 1.0));
    let lon = atan2(dir.y, dir.x);
    let uv = vec2((lon + PI) / (2.0 * PI), (0.5 * PI - lat) / PI);

    let dx = vec2(wrap_d(dpdx(uv.x)), dpdx(uv.y));
    let dy = vec2(wrap_d(dpdy(uv.x)), dpdy(uv.y));
    var color = terrain.base.rgb;
    if terrain.flags.z != 0u {
        color = textureSampleGrad(color_map, color_sampler, uv, dx, dy).rgb;
    }
    // Sea fraction; the baked coverage is sharpened into a clean coastline.
    var water = 0.0;
    if terrain.flags.w != 0u {
        water = smoothstep(0.3, 0.7, textureSampleGrad(water_map, water_sampler, uv, dx, dy).r);
    }

    let height = in.color.g * 10000.0;
    let up = dir; // close enough to the ellipsoid normal for shading rules
    var roughness = terrain.snow.w;

    // Close-range detail: brightness variation, rock on slopes, snow.
    if terrain.flags.x != 0u && water < 1.0 {
        let dist = length(in.world_position.xyz);
        let fade = clamp(1.0 - dist / terrain.shape.z, 0.0, 1.0);
        if fade > 0.0 {
            let n = fbm(in.uv, 4);
            let fine = fbm(in.uv * 16.0, 3);
            // The finest octaves (tens of cm) only right around the camera.
            let near = clamp(1.0 - dist / (terrain.shape.z * 0.05), 0.0, 1.0);
            var micro = 0.5;
            if near > 0.0 {
                micro = mix(0.5, fbm(in.uv * 128.0, 2), near);
            }
            let variation =
                1.0 + terrain.shape.w * ((n - 0.5) * 2.0 + (fine - 0.5) * 1.6 + (micro - 0.5) * 1.2);
            // Patches of slightly different hue (drier / lusher ground).
            let hue = fbm(in.uv + vec2(17.0, 3.0), 2);
            color = mix(color, color * vec3(1.15, 1.05, 0.8), smoothstep(0.4, 0.7, hue) * terrain.shape.w);
            let slope = 1.0 - clamp(dot(normalize(in.world_normal), normalize(rel)), 0.0, 1.0);
            let rockiness = smoothstep(0.08, 0.25, slope + (n - 0.5) * 0.1);
            var detailed = mix(color, terrain.rock.rgb, rockiness * 0.8) * variation;
            if terrain.rock.w > 0.0 {
                let line = terrain.rock.w * pow(max(cos(lat), 0.0), 1.5);
                let snow = smoothstep(line - 400.0, line + 400.0, height + (n - 0.5) * 600.0) * (1.0 - rockiness * 0.7);
                detailed = mix(detailed, terrain.snow.rgb, snow);
            }
            color = mix(color, detailed, fade * (1.0 - water));
        }
    }

    color = mix(color, color * terrain.ocean.rgb, water);
    if terrain.flags.y != 0u {
        roughness = mix(roughness, terrain.ocean.w, water);
    }

    pbr_input.material.base_color = vec4(color, 1.0);
    pbr_input.material.perceptual_roughness = roughness;
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Sunlight at this body's own distance, dimmed where another body
    // hides the star (D055: never the flux at the camera).
    let sun = terrain.light.w * eclipse(in.world_position.xyz);
    var lit = out.color.rgb * sun;
    // Planetshine: a Lambert term from the reflecting body.
    if terrain.shine.w > 0.0 {
        let l = normalize(terrain.shine.xyz - in.world_position.xyz);
        let n = normalize(in.world_normal);
        lit += color * terrain.light.rgb * terrain.shine.w * max(dot(n, l), 0.0) / PI * view.exposure;
    }
    out.color = vec4(lit, out.color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
