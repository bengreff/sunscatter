// Terrain surface: StandardMaterial lighting with the body's colour map
// looked up per fragment from the body-fixed direction (no UV seams or pole
// pinching), a close-range procedural detail layer, and water shading from
// the body's water mask, also looked up per fragment (so it does not change
// with the LOD level; CPU mirror and tests in water.rs).

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_view_bindings::globals,
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
    // rgb: open-water albedo (linear), w: water roughness.
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
    // Ground textures: x > 0 loaded; y, z fine and coarse tile (detail uv).
    ground: vec4<f32>,
    // Mean linear colour of each ground layer.
    ground_means: array<vec4<f32>, 5>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> terrain: TerrainParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var color_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var color_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var water_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var water_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var ground_color: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var ground_color_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var ground_normal: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var ground_normal_sampler: sampler;

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

// Ground layers, in `ground::LAYERS` order.
const GRASS: i32 = 0;
const SOIL: i32 = 1;
const SAND: i32 = 2;
const ROCK: i32 = 3;
const SNOW: i32 = 4;

// Wave height gradient (d/du, d/dv) at detail-uv `p` and time `t`: a few
// sines whose wave vectors are whole multiples of 2π/256, so they repeat
// exactly with the detail uv's 256-unit wrap (no seam).
fn wave_slope(p: vec2<f32>, t: f32) -> vec2<f32> {
    let base = 6.2831853 / 256.0;
    var g = vec2(0.0);
    let ks = array<vec2<f32>, 4>(vec2(1024.0, 310.0), vec2(-700.0, 1300.0), vec2(2300.0, -900.0), vec2(400.0, 3100.0));
    let amps = array<f32, 4>(0.0009, 0.0007, 0.0004, 0.0003);
    let speeds = array<f32, 4>(0.9, 1.1, 1.6, 2.0);
    for (var i = 0; i < 4; i++) {
        let k = ks[i] * base;
        let ph = dot(k, p) + speeds[i] * t;
        g += amps[i] * k * cos(ph);
    }
    return g;
}

// Perturbs normal `n` by tangent-space normal `tn` (x along +u, y along +v),
// with the frame built from screen derivatives of position and uv.
fn perturb(n: vec3<f32>, dp1: vec3<f32>, dp2: vec3<f32>, duv1: vec2<f32>, duv2: vec2<f32>, tn: vec3<f32>) -> vec3<f32> {
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    let t = dp2perp * duv1.x + dp1perp * duv2.x;
    let b = dp2perp * duv1.y + dp1perp * duv2.y;
    let m = max(dot(t, t), dot(b, b));
    if m <= 0.0 {
        return n;
    }
    let s = inverseSqrt(m);
    return normalize(t * s * tn.x + b * s * tn.y + n * tn.z);
}

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
    var coast = 0.0;
    if terrain.flags.w != 0u {
        let raw = textureSampleGrad(water_map, water_sampler, uv, dx, dy).r;
        water = smoothstep(0.3, 0.7, raw);
        // A beach band on the land side of the coastline.
        coast = smoothstep(0.02, 0.3, raw) * (1.0 - water);
    }
    // Screen derivatives for the ground textures and normal perturbation
    // (taken here, in uniform control flow).
    let fine_uv = in.uv / terrain.ground.y;
    let coarse_uv = in.uv / terrain.ground.z;
    let fdx = dpdx(fine_uv);
    let fdy = dpdy(fine_uv);
    let cdx = dpdx(coarse_uv);
    let cdy = dpdy(coarse_uv);
    let dp1 = dpdx(in.world_position.xyz);
    let dp2 = dpdy(in.world_position.xyz);
    let duv1 = dpdx(in.uv);
    let duv2 = dpdy(in.uv);

    let height = in.color.g * 10000.0;
    let up = dir; // close enough to the ellipsoid normal for shading rules
    var roughness = terrain.snow.w;

    // Close range: ground textures chosen per fragment from the colour map
    // (green: grass, bright and dry: sand), the coast (beach), slope (rock)
    // and snow line; each used relative to its mean colour, so the satellite
    // colour stays and only the detail comes from the texture.
    let dist = length(in.world_position.xyz);
    let fade = select(0.0, clamp(1.0 - dist / terrain.shape.z, 0.0, 1.0), terrain.flags.x != 0u);
    var n_out = normalize(pbr_input.N);
    if fade > 0.0 && water < 1.0 {
        let n = fbm(in.uv, 3);
        let lum = dot(color, vec3(0.2126, 0.7152, 0.0722));
        let slope = 1.0 - clamp(dot(normalize(in.world_normal), normalize(rel)), 0.0, 1.0);
        let rockiness = smoothstep(0.08, 0.25, slope + (n - 0.5) * 0.1);
        var snow = 0.0;
        if terrain.rock.w > 0.0 {
            let line = terrain.rock.w * pow(max(cos(lat), 0.0), 1.5);
            snow = smoothstep(line - 400.0, line + 400.0, height + (n - 0.5) * 600.0) * (1.0 - rockiness * 0.7);
        }
        snow = max(snow, smoothstep(0.45, 0.6, lum));
        var w = array<f32, 5>(0.0, 0.0, 0.0, 0.0, 0.0);
        let green = clamp((color.g - max(color.r, color.b)) / (lum + 0.01) * 3.0, 0.0, 1.0);
        let dry = clamp((color.r - color.b) / (lum + 0.01), 0.0, 1.0) * smoothstep(0.06, 0.18, lum);
        w[ROCK] = rockiness;
        w[SNOW] = snow * (1.0 - rockiness);
        let rest = max(1.0 - w[ROCK] - w[SNOW], 0.0);
        w[SAND] = rest * max(coast, dry * (1.0 - green));
        // Patches at ~80 m and ~10 m: clearings of bare soil in grassland,
        // so a single 5 km colour-map pixel is not one uniform field.
        let cover = fbm(in.uv * 0.5 + vec2(31.0, 7.0), 3) * 0.7 + fbm(in.uv * 4.0 + vec2(3.0, 11.0), 2) * 0.3;
        let clearing = smoothstep(0.52, 0.62, cover) * 0.8;
        w[GRASS] = (rest - w[SAND]) * green * (1.0 - clearing);
        w[SOIL] = max(rest - w[SAND] - w[GRASS], 0.0);
        var rel_col = vec3(0.0);
        var tn = vec3(0.0);
        for (var i = 0; i < 5; i++) {
            if w[i] > 0.001 {
                let f = textureSampleGrad(ground_color, ground_color_sampler, fine_uv, i, fdx, fdy).rgb;
                let c = textureSampleGrad(ground_color, ground_color_sampler, coarse_uv, i, cdx, cdy).rgb;
                let mean = max(terrain.ground_means[i].rgb, vec3(0.01));
                rel_col += w[i] * (f / mean) * mix(vec3(1.0), c / mean, 0.5);
                let nf = textureSampleGrad(ground_normal, ground_normal_sampler, fine_uv, i, fdx, fdy).xyz * 2.0 - 1.0;
                tn += w[i] * nf;
            }
        }
        if terrain.ground.x > 0.0 {
            // Snow and rock take their own colour; the others keep the map's.
            let base = mix(color, terrain.rock.rgb, w[ROCK] * 0.6);
            let base2 = mix(base, terrain.snow.rgb, w[SNOW]);
            // Lighter and darker patches (uneven cover, moisture).
            let tone = 1.0 + (fbm(in.uv * 1.5 + vec2(5.0, 19.0), 3) - 0.5) * 0.7;
            color = mix(color, base2 * rel_col * tone, fade * (1.0 - water));
            let tn_n = normalize(vec3(tn.xy * fade * (1.0 - water), max(tn.z, 0.1)));
            n_out = perturb(n_out, dp1, dp2, duv1, duv2, tn_n);
        }
    }
    // Water lies at sea level: shade it with the sphere's normal at this
    // fragment, not the mesh normal. On a coarse chunk the mesh normal is
    // interpolated across triangles kilometres wide, and the sharp sun glint
    // took their shape (grey and white triangles that jumped as the LOD
    // changed while zooming).
    if water > 0.0 {
        let sphere_n = normalize(in.world_position.xyz - terrain.center.xyz);
        n_out = normalize(mix(n_out, sphere_n, water));
    }
    // Waves on water, near the camera.
    if water > 0.0 && fade > 0.0 {
        let g = wave_slope(in.uv, globals.time) * 3.0 * fade * water;
        n_out = perturb(n_out, dp1, dp2, duv1, duv2, normalize(vec3(-g.x, -g.y, 1.0)));
    }
    pbr_input.N = n_out;

    color = mix(color, max(color, terrain.ocean.rgb), water);
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
    // The sky light (Bevy's environment map) is the sky seen from the
    // camera; a point where the Sun has set has no such sky. Fade all
    // non-direct light out through twilight (Sun 6° below to 3° above the
    // local horizon), so night sides stay dark (direct light is already
    // zero there).
    let up_here = normalize(in.world_position.xyz - terrain.center.xyz);
    let sun_elev = dot(up_here, normalize(terrain.star.xyz - in.world_position.xyz));
    let day = smoothstep(-0.105, 0.052, sun_elev);
    var lit = out.color.rgb * sun * day;
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
