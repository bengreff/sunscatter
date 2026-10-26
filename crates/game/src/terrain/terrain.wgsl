// Terrain surface: StandardMaterial lighting with the body's colour map
// looked up per fragment from the body-fixed direction (no UV seams or pole
// pinching), a close-range procedural detail layer (or, with it off, a
// simple patch pattern), and water shading from
// the body's water mask, also looked up per fragment (so it does not change
// with the LOD level; CPU mirror and tests in water.rs).
//
// With surface zones (D073, crisp-surfaces.md; CPU mirrors and tests in
// zones.rs) the colour map is replaced: one domain warp moves the lookups
// of the zone map and the water mask, the coast is the bilinear sea
// fraction thresholded at 0.5 over one pixel, zone edges are picked from
// the 4 zone texels around the point and antialiased against the runner-up, each zone
// has a procedural pattern, and a mipmapped zone-colour texture takes over
// once a pixel covers about a zone texel.

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
    // Surface zones: x 1 when drawn, y the warp's first wavelength (m).
    zones: vec4<f32>,
    // rgb: shallow-water albedo (linear), w 1 when set.
    shallow: vec4<f32>,
    // Zone palette: rgb drawn colour (linear), w 1 for water zones.
    zone_color: array<vec4<f32>, 16>,
    // Zone pattern: x scale (m), y contrast.
    zone_pattern: array<vec4<f32>, 16>,
    // x: this body's share of the global ambient light (sky::ambient_share).
    ambient: vec4<f32>,
    // Cloud detail (giants): x first wavelength (m, 0 off), y warp amplitude
    // (fraction of it), z east-west stretch, w tone contrast.
    clouds: vec4<f32>,
    // Under a haze: rgb vertical optical depth, w radius / scale height (0 off);
    // rgb diffuse fraction (overhead Sun), w the Sun's lux at the camera.
    haze: vec4<f32>,
    haze_diffuse: vec4<f32>,
    // Rings: x inner, y outer radius (m), z the profile's bins (0: none).
    rings: vec4<f32>,
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
// Zone indices, read with textureLoad (no sampler: the fragment stage's 16
// samplers are all taken at High and Ultra).
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var zone_map: texture_2d<f32>;
// Zone colours, sampled with the colour map's sampler (the same settings;
// zones are drawn only once the colour map is loaded).
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var zone_far: texture_2d<f32>;
// Ring profile (rgb albedo, a normal optical depth per bin), read with textureLoad.
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var ring_profile: texture_2d<f32>;

// Sunlight through the rings at `p` (world, camera-relative): exp(−τ/|μ|)
// where the ray to the star crosses the ring plane (rings::ring_transmission).
fn ring_transmission(p: vec3<f32>) -> f32 {
    let bins = terrain.rings.z;
    if bins <= 0.0 {
        return 1.0;
    }
    let n = normalize(terrain.fixed_z.xyz);
    let l = normalize(terrain.star.xyz - p);
    let mu = dot(l, n);
    if abs(mu) < 1e-6 {
        return 1.0;
    }
    let rel = p - terrain.center.xyz;
    let t = -dot(rel, n) / mu;
    if t <= 0.0 {
        return 1.0;
    }
    let r = length(rel + l * t);
    let x = (r - terrain.rings.x) / (terrain.rings.y - terrain.rings.x) * bins - 0.5;
    if x < -0.5 || x > bins - 0.5 {
        return 1.0;
    }
    let i0 = clamp(i32(floor(x)), 0, i32(bins) - 1);
    let i1 = min(i0 + 1, i32(bins) - 1);
    let f = clamp(x - floor(x), 0.0, 1.0);
    let tau = mix(textureLoad(ring_profile, vec2(i0, 0), 0).a, textureLoad(ring_profile, vec2(i1, 0), 0).a, f);
    return exp(-tau / abs(mu));
}

const PI: f32 = 3.14159265;
// Distance (m) over which the simple ground pattern fades out.
const SIMPLE_GROUND_M: f32 = 20000.0;
// The simple ground pattern (zones::GROUND_*): octaves, and the broad
// patches' contrast (stops).
const GROUND_OCTAVES: i32 = 8;
const GROUND_PATCH_STOPS: f32 = 0.35;

// A ground-pattern octave of wavelength `period` (detail units) at a pixel
// footprint `foot` (the same units): full at 8 pixels a wavelength, gone
// at 4 (zones::ground_octave_weight).
fn ground_octave_weight(period: f32, foot: f32) -> f32 {
    return clamp(period / (4.0 * max(foot, 1e-9)) - 1.0, 0.0, 1.0);
}

// Area of overlap of two discs (radii r1, r2, centres d apart); mirrors
// `sim::light::disc_overlap` (tested there).
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
// `sim::light::eclipse_factor`.
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

// ---- Surface zones (zones.rs mirrors the pure rules) ----

// The warp's first octave amplitude (zone texels; zones::WARP_AMP_TEXELS);
// amplitude and wavelength halve per octave.
const WARP_AMP: f32 = 0.6;
const WARP_OCTAVES: i32 = 10;
// No warp or pattern octave is finer than these (m): the body-fixed
// position is good to ~0.5 m in f32, and the simple ground pattern takes
// over close up.
const WARP_MIN_M: f32 = 100.0;
const TONE_OCTAVES: i32 = 6;
const TONE_MIN_M: f32 = 40.0;

// An octave is fully drawn at a wavelength of 3 pixel footprints or more
// and gone at 2 (zones::octave_weight).
fn octave_weight(lam: f32, foot: f32) -> f32 {
    return clamp(lam / max(foot, 1e-6) - 2.0, 0.0, 1.0);
}

// Integer lattice hash (exact for the lattice coordinates used, |c| < 2^24).
fn hash3(c: vec3<i32>) -> u32 {
    var h = (bitcast<u32>(c.x) * 0x8da6b343u) ^ (bitcast<u32>(c.y) * 0xd8163841u) ^ (bitcast<u32>(c.z) * 0xcb1ab31fu);
    h = (h ^ (h >> 15u)) * 0x2c1b3c6du;
    h = (h ^ (h >> 12u)) * 0x297a2d39u;
    return h ^ (h >> 15u);
}

// Two independent 3D value noises in [-1, 1] (lattice spacing 1).
fn vnoise2(p: vec3<f32>) -> vec2<f32> {
    let fl = floor(p);
    let i = vec3<i32>(fl);
    let f = p - fl;
    let u = f * f * (3.0 - 2.0 * f);
    var c: array<vec2<f32>, 8>;
    for (var k = 0; k < 8; k++) {
        let h = hash3(i + vec3<i32>(k & 1, (k >> 1) & 1, (k >> 2) & 1));
        c[k] = vec2(f32(h & 0xffffu), f32(h >> 16u)) * (1.0 / 65535.0);
    }
    let y0 = mix(mix(c[0], c[1], u.x), mix(c[2], c[3], u.x), u.y);
    let y1 = mix(mix(c[4], c[5], u.x), mix(c[6], c[7], u.x), u.y);
    return mix(y0, y1, u.z) * 2.0 - 1.0;
}

// The domain warp (zone texels): 3D fBm of the body-fixed position `p`
// (m), wavelengths `lam0`, lam0/2, ..., each octave fading by footprint.
fn zone_warp(p: vec3<f32>, lam0: f32, foot: f32) -> vec2<f32> {
    var w = vec2(0.0);
    var lam = lam0;
    var amp = WARP_AMP;
    for (var k = 0; k < WARP_OCTAVES; k++) {
        let wt = octave_weight(lam, foot);
        if wt <= 0.0 || lam < WARP_MIN_M {
            break;
        }
        w += amp * wt * vnoise2(p / lam + f32(k) * vec3(17.31, 5.17, 11.73));
        lam *= 0.5;
        amp *= 0.5;
    }
    return w;
}

// A zone's pattern tone (about -1..1): fBm from `scale` (m) down, each
// octave fading by footprint, so it reads the same from any distance.
fn zone_tone(p: vec3<f32>, scale: f32, foot: f32) -> f32 {
    var t = 0.0;
    var lam = scale;
    var amp = 0.5;
    for (var k = 0; k < TONE_OCTAVES; k++) {
        let wt = octave_weight(lam, foot);
        if wt <= 0.0 || lam < TONE_MIN_M {
            break;
        }
        t += amp * wt * vnoise2(p / lam + vec3(3.7, 41.3, 23.9) + f32(k) * vec3(7.13, 13.7, 3.31)).x;
        lam *= 0.5;
        amp *= 0.5;
    }
    return t;
}

// Bilinear weights of a gather's 4 texels ((i0,j1), (i1,j1), (i1,j0),
// (i0,j0)) at fractions f, and their derivatives along f.x and f.y
// (zones::gather_weights).
fn gather_w(f: vec2<f32>) -> vec4<f32> {
    return vec4((1.0 - f.x) * f.y, f.x * f.y, f.x * (1.0 - f.y), (1.0 - f.x) * (1.0 - f.y));
}
fn gather_dwx(f: vec2<f32>) -> vec4<f32> {
    return vec4(-f.y, f.y, 1.0 - f.y, f.y - 1.0);
}
fn gather_dwy(f: vec2<f32>) -> vec4<f32> {
    return vec4(1.0 - f.x, f.x, -f.x, f.x - 1.0);
}

// Change per pixel of a function with gradient g along the texel
// fractions; jx, jy: texel coordinates per pixel along screen x, y
// (zones::px_gradient).
fn px_gradient(g: vec2<f32>, jx: vec2<f32>, jy: vec2<f32>) -> f32 {
    return length(vec2(dot(g, jx), dot(g, jy)));
}

fn eq4(ids: vec4<u32>, z: u32) -> vec4<f32> {
    return select(vec4(0.0), vec4(1.0), ids == vec4<u32>(z));
}

fn zone_col(i: u32) -> vec4<f32> {
    return terrain.zone_color[min(i, 15u)];
}

struct ZoneOut {
    // Land colour (and, far away, lakes in their water colour).
    land: vec3<f32>,
    // Sea coverage (water mask) and water-zone coverage (lakes, near).
    sea: f32,
    lake: f32,
    // Shallowness of the sea floor (1 at the shore).
    shallow: f32,
    // The winning zone's pattern tone.
    tone: f32,
}

// The zone surface at warped texture coordinates `uv` (derivatives dx, dy),
// body-fixed position `p` (m) and pixel footprint `foot` (m).
fn zone_surface(uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, p: vec3<f32>, foot: f32) -> ZoneOut {
    var o = ZoneOut(vec3(0.0), 0.0, 0.0, 1.0, 0.0);
    let zs = vec2<f32>(textureDimensions(zone_map, 0));
    let jx = dx * zs;
    let jy = dy * zs;
    // Near: pixels smaller than half a zone texel; far: a texel or more.
    let far = smoothstep(0.5, 1.0, max(length(jx), length(jy)));
    let far_c = textureSampleGrad(zone_far, color_sampler, uv, dx, dy);
    o.shallow = far_c.a;
    var near = vec3(0.0);
    if far < 1.0 {
        let t = uv * zs - 0.5;
        let i = floor(t);
        let f = t - i;
        // The 4 texels around the point, in gather order (longitude wraps,
        // latitude clamps; zones::texel_block).
        let n = vec2<i32>(textureDimensions(zone_map, 0));
        let x0 = ((i32(i.x) % n.x) + n.x) % n.x;
        let x1 = (x0 + 1) % n.x;
        let y0 = clamp(i32(i.y), 0, n.y - 1);
        let y1 = clamp(i32(i.y) + 1, 0, n.y - 1);
        let g = vec4(
            textureLoad(zone_map, vec2(x0, y1), 0).r,
            textureLoad(zone_map, vec2(x1, y1), 0).r,
            textureLoad(zone_map, vec2(x1, y0), 0).r,
            textureLoad(zone_map, vec2(x0, y0), 0).r,
        );
        let ids = vec4<u32>(g * 255.0 + 0.5);
        let w = gather_w(f);
        // Each zone's summed weight; the largest wins (zones::pick).
        let s = vec4(dot(w, eq4(ids, ids.x)), dot(w, eq4(ids, ids.y)), dot(w, eq4(ids, ids.z)), dot(w, eq4(ids, ids.w)));
        var win = ids.x;
        var sw = s.x;
        if s.y > sw { win = ids.y; sw = s.y; }
        if s.z > sw { win = ids.z; sw = s.z; }
        if s.w > sw { win = ids.w; sw = s.w; }
        var run = win;
        var sr = -1.0;
        if ids.x != win && s.x > sr { run = ids.x; sr = s.x; }
        if ids.y != win && s.y > sr { run = ids.y; sr = s.y; }
        if ids.z != win && s.z > sr { run = ids.z; sr = s.z; }
        if ids.w != win && s.w > sr { run = ids.w; sr = s.w; }
        // The edge (margin 0) antialiased over one pixel (zones::edge_coverage).
        var cov = 1.0;
        if run != win {
            let d = eq4(ids, win) - eq4(ids, run);
            let g = vec2(dot(d, gather_dwx(f)), dot(d, gather_dwy(f)));
            cov = clamp(0.5 + (sw - sr) / max(px_gradient(g, jx, jy), 1e-6), 0.5, 1.0);
        }
        // The satellite tint, baked in the far texture: its base level over
        // the palette, both bilinear, so it varies smoothly.
        let base = textureSampleLevel(zone_far, color_sampler, uv, 0.0).rgb;
        let pal = zone_col(ids.x).rgb * w.x + zone_col(ids.y).rgb * w.y + zone_col(ids.z).rgb * w.z + zone_col(ids.w).rgb * w.w;
        let tint = base / max(pal, vec3(1e-5));
        let pat = terrain.zone_pattern[min(win, 15u)];
        o.tone = zone_tone(p, pat.x, foot);
        let cw = zone_col(win);
        let cr = zone_col(run);
        near = mix(cr.rgb, cw.rgb, cov) * tint * (1.0 + pat.y * o.tone);
        o.lake = (cw.w * cov + cr.w * (1.0 - cov)) * (1.0 - far);
    }
    o.land = mix(near, far_c.rgb, far);
    // The sea: near, the exact bilinear coverage of a gather thresholded at
    // 0.5 over one pixel (zones::coast_step); far, the filtered coverage.
    if terrain.flags.w != 0u {
        let ws = vec2<f32>(textureDimensions(water_map, 0));
        let wjx = dx * ws;
        let wjy = dy * ws;
        let wfar = smoothstep(0.5, 1.0, max(length(wjx), length(wjy)));
        var near_sea = 0.0;
        if wfar < 1.0 {
            let t = uv * ws - 0.5;
            let i = floor(t);
            let f = t - i;
            let g = textureGather(0, water_map, water_sampler, (i + 1.0) / ws);
            let raw = dot(g, gather_w(f));
            let grad = px_gradient(vec2(dot(g, gather_dwx(f)), dot(g, gather_dwy(f))), wjx, wjy);
            near_sea = clamp(0.5 + (raw - 0.5) / max(grad, 1e-6), 0.0, 1.0);
        }
        var far_sea = 0.0;
        if wfar > 0.0 {
            far_sea = textureSampleGrad(water_map, water_sampler, uv, dx, dy).r;
        }
        o.sea = mix(near_sea, far_sea, wfar);
    }
    return o;
}

// Rock on steep slopes and snow above the snow line (both physical;
// shared by the detail layer and the zones). `n` in [0, 1] jitters the
// thresholds.
fn rock_cover(slope: f32, n: f32) -> f32 {
    return smoothstep(0.08, 0.25, slope + (n - 0.5) * 0.1);
}

fn snow_cover(height: f32, lat: f32, n: f32, rock: f32) -> f32 {
    if terrain.rock.w <= 0.0 {
        return 0.0;
    }
    let line = terrain.rock.w * pow(max(cos(lat), 0.0), 1.5);
    return smoothstep(line - 400.0, line + 400.0, height + (n - 0.5) * 600.0) * (1.0 - rock * 0.7);
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
    // The daylight sky stand-in lights only the body whose air the camera is in.
    pbr_input.diffuse_occlusion *= terrain.ambient.x;

    let rel = in.world_position.xyz - terrain.center.xyz;
    let fixed = vec3(dot(terrain.fixed_x.xyz, rel), dot(terrain.fixed_y.xyz, rel), dot(terrain.fixed_z.xyz, rel));
    let dir = normalize(fixed);
    let lat = asin(clamp(dir.z, -1.0, 1.0));
    let lon = atan2(dir.y, dir.x);
    let uv = vec2((lon + PI) / (2.0 * PI), (0.5 * PI - lat) / PI);

    let dx = vec2(wrap_d(dpdx(uv.x)), dpdx(uv.y));
    let dy = vec2(wrap_d(dpdy(uv.x)), dpdy(uv.y));
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
    let slope = 1.0 - clamp(dot(normalize(in.world_normal), normalize(rel)), 0.0, 1.0);

    var color = terrain.base.rgb;
    // Sea fraction (and, with zones, lakes).
    var water = 0.0;
    var coast = 0.0;
    let zones_on = terrain.zones.x > 0.0;
    var water_color = terrain.ocean.rgb;
    if zones_on {
        // The pixel footprint on the surface (m), and the one warp that
        // moves both the zone and the sea lookups, so coasts and zone edges
        // are fractal, line up and keep gaining detail closer in.
        let foot = max(length(dp1), length(dp2));
        let zs = vec2<f32>(textureDimensions(zone_map, 0));
        let uv_w = uv + zone_warp(fixed, terrain.zones.y, foot) / zs;
        let dxw = vec2(wrap_d(dpdx(uv_w.x)), dpdx(uv_w.y));
        let dyw = vec2(wrap_d(dpdy(uv_w.x)), dpdy(uv_w.y));
        let z = zone_surface(uv_w, dxw, dyw, fixed, foot);
        color = z.land;
        water = max(z.sea, z.lake);
        // Open water, lighter over a shallow sea floor (lakes stay deep).
        if terrain.shallow.w > 0.0 {
            water_color = mix(terrain.ocean.rgb, terrain.shallow.rgb, z.shallow * (1.0 - z.lake));
        }
        // Rock on steep slopes and the snow line, on land (the detail
        // layer has its own).
        if terrain.flags.x == 0u && terrain.shape.w > 0.0 {
            let n = z.tone * 0.5 + 0.5;
            let rock = rock_cover(slope, n) * (1.0 - water);
            let snow = snow_cover(height, lat, n, rock) * (1.0 - water);
            color = mix(color, terrain.rock.rgb, rock * 0.6);
            color = mix(color, terrain.snow.rgb, snow);
        }
    } else {
        if terrain.flags.z != 0u {
            var uv_c = uv;
            var tone = 0.0;
            if terrain.clouds.x > 0.0 {
                // Cloud tops (D073): the map looked up through a warp that
                // is stretched east-west like zonal flow, plus a fine tone;
                // every octave fades by footprint, so band edges stay crisp
                // and turbulent at any distance.
                let foot = max(length(dp1), length(dp2));
                let east = vec3(-dir.y, dir.x, 0.0) / max(length(dir.xy), 1e-4);
                let q = fixed + east * dot(fixed, east) * (1.0 / terrain.clouds.z - 1.0);
                let w = zone_warp(q, terrain.clouds.x, foot) * (terrain.clouds.x * terrain.clouds.y / WARP_AMP);
                let r = terrain.shape.x;
                uv_c = uv + vec2(w.x / (2.0 * PI * r * max(cos(lat), 0.05)), -w.y / (PI * r));
                tone = zone_tone(q, terrain.clouds.x * 0.5, foot) * terrain.clouds.w;
            }
            color = textureSampleGrad(color_map, color_sampler, uv_c, dx, dy).rgb * (1.0 + tone);
        }
        // The baked coverage sharpened into a coastline.
        if terrain.flags.w != 0u {
            let raw = textureSampleGrad(water_map, water_sampler, uv, dx, dy).r;
            water = smoothstep(0.3, 0.7, raw);
            // A beach band on the land side of the coastline.
            coast = smoothstep(0.02, 0.3, raw) * (1.0 - water);
        }
    }

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
        let rockiness = rock_cover(slope, n);
        let snow = max(snow_cover(height, lat, n, rockiness), smoothstep(0.45, 0.6, lum));
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
    // Without the detail layer (the Minimal tier, D073): a simple ground
    // texture for depth perception near the ground (zones::ground_*
    // mirror the rules). Lighter and darker patches at 8 scales, 320 m down
    // to 2.5 m on Earth (detail units / 0.125, halving), each fading out
    // between 8 and 4 pixels a wavelength so nothing shimmers, plus broad
    // patches of cover and bare ground from the two coarsest, fading in
    // between 32 and 64 pixels a wavelength. Typically ±0.3 stops, at most
    // ~±1.
    if terrain.flags.x == 0u && water < 1.0 {
        let near = clamp(1.0 - dist / SIMPLE_GROUND_M, 0.0, 1.0);
        if near > 0.0 {
            let foot = max(length(duv1), length(duv2));
            var fine = 0.0;
            var coarse = 0.0;
            var freq = 0.125;
            var amp = 0.5;
            for (var k = 0; k < GROUND_OCTAVES; k++) {
                let w = ground_octave_weight(1.0 / freq, foot);
                if w <= 0.0 {
                    break;
                }
                let o = f32(k) * vec2(7.31, 3.17);
                let n = (noise(in.uv * freq + o, 256.0 * freq) - 0.5) * 2.0;
                fine += amp * w * n;
                if k < 2 {
                    coarse += amp * w * n;
                }
                freq *= 2.0;
                amp *= 0.8;
            }
            let pw = clamp(4.0 / foot / 32.0 - 1.0, 0.0, 1.0);
            let patches = (smoothstep(-0.15, 0.15, coarse) * 2.0 - 1.0) * pw;
            color *= exp2((fine + GROUND_PATCH_STOPS * patches) * near * (1.0 - water));
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

    if zones_on {
        color = mix(color, water_color, water);
    } else {
        // The colour map under water is raised to at least the open-water albedo.
        color = mix(color, max(color, terrain.ocean.rgb), water);
    }
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
    let sun = terrain.light.w * eclipse(in.world_position.xyz) * ring_transmission(in.world_position.xyz);
    // The sky light (Bevy's environment map) is the sky seen from the
    // camera; a point where the Sun has set has no such sky. Fade all
    // non-direct light out through twilight (Sun 6° below to 3° above the
    // local horizon), so night sides stay dark (direct light is already
    // zero there).
    let up_here = normalize(in.world_position.xyz - terrain.center.xyz);
    let sun_elev = dot(up_here, normalize(terrain.star.xyz - in.world_position.xyz));
    let day = smoothstep(-0.105, 0.052, sun_elev);
    var lit = out.color.rgb * sun * day;
    // Under a haze (Titan, Venus): the direct beam through the slant
    // column (spherical-shell air mass), and the light the haze scatters
    // down, Lambertian (lighting::haze_light).
    if terrain.haze.w > 0.0 {
        let mu = max(sun_elev, 0.0);
        let rh = terrain.haze.w;
        let airmass = sqrt(rh * rh * mu * mu + 2.0 * rh + 1.0) - rh * mu;
        lit *= exp(-terrain.haze.rgb * airmass);
        let e = terrain.haze_diffuse.w * terrain.light.w * eclipse(in.world_position.xyz);
        lit += color * terrain.haze_diffuse.rgb * e * mu / PI * view.exposure;
    }
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
