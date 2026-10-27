// Terrain surface: StandardMaterial lighting with the body's colour map
// looked up per fragment from the body-fixed direction (no UV seams or pole
// pinching), a simple ground pattern close up, and water shading from the
// body's water mask, also looked up per fragment (so it does not change
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
}
#import sunscatter::light::{Lights, eclipse, MAX_STARS, MAX_SHINES}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::main_pass_post_lighting_processing,
    pbr_functions,
    ambient::ambient_light,
    pbr_types,
    lighting,
    shadows,
    shadow_sampling,
    mesh_types,
    mesh_view_types,
    mesh_view_bindings::{view, lights},
    mesh_view_bindings as view_bindings,
    pbr_bindings,
    mesh_functions,
    view_transformations::position_world_to_clip,
}
#ifdef ATMOSPHERE
#import bevy_pbr::atmosphere::functions::{calculate_visible_sun_ratio, clamp_to_surface}
#endif
#endif

// What changes every frame (material::TerrainFrame), in a storage buffer so
// the material is never modified per frame_data.
struct TerrainFrame {
    // Rows of the world → body-fixed rotation.
    fixed_x: vec4<f32>,
    fixed_y: vec4<f32>,
    fixed_z: vec4<f32>,
    // xyz: body centre relative to the camera (world space).
    center: vec4<f32>,
    // Lighting (D055, lighting::LightUniform): the drawn stars and their
    // light here, the bodies that can eclipse them, planetshine.
    starlight: Lights,
    // x: this body's share of the global ambient light (sky::ambient_share).
    ambient: vec4<f32>,
    // x: 1 when the zones are drawn; y, z: 1 with their warp, their pattern.
    flags: vec4<f32>,
    // Under an opaque deck: rgb the gas's extinction at the datum (m⁻¹), w
    // its scale height (m); rgb the sky's radiance (lux/sr), w 1 when on.
    fog: vec4<f32>,
    fog_light: vec4<f32>,
}

struct TerrainParams {
    // x: equatorial radius, y: polar radius, z: unused, w: rock and snow on (> 0).
    shape: vec4<f32>,
    // rgb: rock colour, w: snow line (m, < 0 = none).
    rock: vec4<f32>,
    // rgb: snow colour, w: surface roughness.
    snow: vec4<f32>,
    // rgb: open-water albedo (linear).
    ocean: vec4<f32>,
    // z: has colour map, w: has water mask (x, y unused).
    flags: vec4<u32>,
    // rgb: base colour without a map.
    base: vec4<f32>,
    // Surface zones: y the warp's first wavelength (m).
    zones: vec4<f32>,
    // rgb: shallow-water albedo (linear), w 1 when set.
    shallow: vec4<f32>,
    // Zone palette: rgb drawn colour (linear), w 1 for water zones.
    zone_color: array<vec4<f32>, 16>,
    // Zone pattern: x scale (m), y contrast.
    zone_pattern: array<vec4<f32>, 16>,
    // Cloud detail (giants): x first wavelength (m, 0 off), y warp amplitude
    // (fraction of it), z east-west stretch, w tone contrast.
    clouds: vec4<f32>,
    // Under a haze: rgb vertical optical depth, w radius / scale height (0 off);
    // rgb diffuse fraction (overhead Sun).
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
// Zone indices, read with textureLoad.
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var zone_map: texture_2d<f32>;
// Zone colours, sampled with the colour map's sampler (the same settings;
// zones are drawn only once the colour map is loaded).
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var zone_far: texture_2d<f32>;
// Ring profile (rgb albedo, a normal optical depth per bin, with mips that
// average the transmission: rings::mip_chain), read with textureLoad.
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var ring_profile: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(112) var<storage, read> frame_data: TerrainFrame;
// The value noise's lattice (noise.rs): RG8, repeating every NOISE_SIZE cells.
@group(#{MATERIAL_BIND_GROUP}) @binding(113) var noise_lattice: texture_3d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(114) var noise_sampler: sampler;

// Starlight through the rings at `p` (world, camera-relative) from a star
// at `star`: exp(−τ/|μ|) where the ray to it crosses the ring plane
// (rings::ring_transmission), the profile averaged over `foot` (m, the
// pixel's footprint): no speckles where the occultation's τ is noisy.
fn ring_transmission(p: vec3<f32>, foot: f32, star: vec3<f32>) -> f32 {
    let bins = terrain.rings.z;
    if bins <= 0.0 {
        return 1.0;
    }
    let n = normalize(frame_data.fixed_z.xyz);
    let l = normalize(star - p);
    let mu = dot(l, n);
    if abs(mu) < 1e-6 {
        return 1.0;
    }
    let rel = p - frame_data.center.xyz;
    let t = -dot(rel, n) / mu;
    if t <= 0.0 {
        return 1.0;
    }
    let r = length(rel + l * t);
    let u = (r - terrain.rings.x) / (terrain.rings.y - terrain.rings.x);
    if u < 0.0 || u > 1.0 {
        return 1.0;
    }
    let bin_m = (terrain.rings.y - terrain.rings.x) / bins;
    let level = min(i32(floor(log2(max(foot / bin_m, 1.0)))), i32(textureNumLevels(ring_profile)) - 1);
    let count = i32(textureDimensions(ring_profile, level).x);
    let x = u * f32(count) - 0.5;
    let i0 = clamp(i32(floor(x)), 0, count - 1);
    let i1 = min(i0 + 1, count - 1);
    let f = clamp(x - floor(x), 0.0, 1.0);
    let tau = mix(textureLoad(ring_profile, vec2(i0, 0), level).a, textureLoad(ring_profile, vec2(i1, 0), level).a, f);
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
    // The 8 corners by name, not in a local array (which can spill).
    let c0 = corner(i);
    let c1 = corner(i + vec3(1, 0, 0));
    let c2 = corner(i + vec3(0, 1, 0));
    let c3 = corner(i + vec3(1, 1, 0));
    let c4 = corner(i + vec3(0, 0, 1));
    let c5 = corner(i + vec3(1, 0, 1));
    let c6 = corner(i + vec3(0, 1, 1));
    let c7 = corner(i + vec3(1, 1, 1));
    let y0 = mix(mix(c0, c1, u.x), mix(c2, c3, u.x), u.y);
    let y1 = mix(mix(c4, c5, u.x), mix(c6, c7, u.x), u.y);
    return mix(y0, y1, u.z) * 2.0 - 1.0;
}

// Two lattice values in [0, 1] at integer cell `c`.
fn corner(c: vec3<i32>) -> vec2<f32> {
    let h = hash3(c);
    return vec2(f32(h & 0xffffu), f32(h >> 16u)) * (1.0 / 65535.0);
}

// The same value noise from the lattice texture (noise.rs): one trilinear
// fetch at the coordinate smoothstepped within its cell.
const NOISE_SIZE: f32 = 128.0;
fn lattice_noise2(p: vec3<f32>) -> vec2<f32> {
    let fl = floor(p);
    let f = p - fl;
    let u = f * f * (3.0 - 2.0 * f);
    return textureSampleLevel(noise_lattice, noise_sampler, (fl + u + 0.5) / NOISE_SIZE, 0.0).rg * 2.0 - 1.0;
}

// A fixed rotation, applied once more per octave (with the doubling), so
// the lattice's period never lines up between octaves.
const OCTAVE_TURN: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.00, 0.80, 0.60),
    vec3<f32>(-0.80, 0.36, -0.48),
    vec3<f32>(-0.60, -0.48, 0.64),
);

// The fBm below takes its first, coarsest octave from the hashed noise (no
// period at all) and the rest from the lattice texture; the first is out of
// the loop, so the loop holds only the texture's fetch.

// The domain warp (zone texels): 3D fBm of the body-fixed position `p`
// (m), wavelengths `lam0`, lam0/2, ..., each octave fading by footprint.
fn zone_warp(p: vec3<f32>, lam0: f32, foot: f32) -> vec2<f32> {
    var w = vec2(0.0);
    if frame_data.flags.y <= 0.0 {
        return w;
    }
    var lam = lam0;
    var amp = WARP_AMP;
    var q = p / lam0;
    let wt0 = octave_weight(lam, foot);
    if wt0 <= 0.0 || lam < WARP_MIN_M {
        return w;
    }
    w = amp * wt0 * vnoise2(q);
    for (var k = 1; k < WARP_OCTAVES; k++) {
        lam *= 0.5;
        amp *= 0.5;
        q = OCTAVE_TURN * q * 2.0;
        let wt = octave_weight(lam, foot);
        if wt <= 0.0 || lam < WARP_MIN_M {
            break;
        }
        w += amp * wt * lattice_noise2(q + f32(k) * vec3(17.31, 5.17, 11.73));
    }
    return w;
}

// A zone's pattern tone (about -1..1): fBm from `scale` (m) down, each
// octave fading by footprint, so it reads the same from any distance.
fn zone_tone(p: vec3<f32>, scale: f32, foot: f32) -> f32 {
    var t = 0.0;
    if frame_data.flags.z <= 0.0 {
        return t;
    }
    var lam = scale;
    var amp = 0.5;
    var q = p / scale;
    let offset = vec3(3.7, 41.3, 23.9);
    let wt0 = octave_weight(lam, foot);
    if wt0 <= 0.0 || lam < TONE_MIN_M {
        return t;
    }
    t = amp * wt0 * vnoise2(q + offset).x;
    for (var k = 1; k < TONE_OCTAVES; k++) {
        lam *= 0.5;
        amp *= 0.5;
        q = OCTAVE_TURN * q * 2.0;
        let wt = octave_weight(lam, foot);
        if wt <= 0.0 || lam < TONE_MIN_M {
            break;
        }
        t += amp * wt * lattice_noise2(q + offset + f32(k) * vec3(7.13, 13.7, 3.31)).x;
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

// The sea's coverage at `uv`: near, the exact bilinear coverage of a gather
// thresholded at 0.5 over one pixel (zones::coast_step); far, the filtered
// coverage. Derivatives as in `zone_surface`.
fn sea_cover(uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, dx0: vec2<f32>, dy0: vec2<f32>) -> f32 {
    if terrain.flags.w == 0u {
        return 0.0;
    }
    let ws = vec2<f32>(textureDimensions(water_map, 0));
    let wjx = dx * ws;
    let wjy = dy * ws;
    let wfar = smoothstep(0.5, 1.0, max(length(dx0 * ws), length(dy0 * ws)));
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
        far_sea = textureSampleGrad(water_map, water_sampler, uv, dx0, dy0).r;
    }
    return mix(near_sea, far_sea, wfar);
}

// Above this sea coverage the land under a pixel is not seen (it weighs
// under 0.2 % in the colour): its pattern is not computed.
const SEA_HIDES_LAND: f32 = 0.998;

// The zone surface at warped texture coordinates `uv` (derivatives dx, dy),
// body-fixed position `p` (m) and pixel footprint `foot` (m).
// `dx`, `dy`: the warped coordinates' derivatives (the edges'
// antialiasing); `dx0`, `dy0`: the unwarped ones (how many texels a pixel
// covers, and the filtered lookups), continuous where the warp stops.
// `edge`: the point is near a zone edge or the coast (zones::edge_mask);
// elsewhere the 4 texels around it hold one zone, `id` (the zone map's
// value at the point), so it is drawn without looking them up.
fn zone_surface(uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, dx0: vec2<f32>, dy0: vec2<f32>, p: vec3<f32>, foot: f32, edge: bool, id: u32) -> ZoneOut {
    var o = ZoneOut(vec3(0.0), 0.0, 0.0, 1.0, 0.0);
    // The sea first: land fully under it needs no pattern.
    o.sea = sea_cover(uv, dx, dy, dx0, dy0);
    let zs = vec2<f32>(textureDimensions(zone_map, 0));
    // Near: pixels smaller than half a zone texel; far: a texel or more.
    let far = smoothstep(0.5, 1.0, max(length(dx0 * zs), length(dy0 * zs)));
    // Fully near (a pixel under half a zone texel), the far colour is not
    // drawn: the base level (the tint below) gives the shallowness too.
    var far_c = vec4(0.0);
    if far > 0.0 {
        far_c = textureSampleGrad(zone_far, color_sampler, uv, dx0, dy0);
    }
    o.shallow = far_c.a;
    var near = vec3(0.0);
    if far < 1.0 {
        // The satellite tint, baked in the far texture: its base level over
        // the palette, both bilinear, so it varies smoothly.
        let base4 = textureSampleLevel(zone_far, color_sampler, uv, 0.0);
        let base = base4.rgb;
        if far <= 0.0 {
            o.shallow = base4.a;
        }
        var win = id;
        var run = id;
        var cov = 1.0;
        var tint = base / max(zone_col(id).rgb, vec3(1e-5));
        if edge {
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
            // The zone indices, without the edge flag (zones::EDGE_BIT).
            let ids = vec4<u32>(g * 255.0 + 0.5) & vec4(0x7fu);
            let w = gather_w(f);
            // Each zone's summed weight; the largest wins (zones::pick).
            let s = vec4(dot(w, eq4(ids, ids.x)), dot(w, eq4(ids, ids.y)), dot(w, eq4(ids, ids.z)), dot(w, eq4(ids, ids.w)));
            win = ids.x;
            var sw = s.x;
            if s.y > sw { win = ids.y; sw = s.y; }
            if s.z > sw { win = ids.z; sw = s.z; }
            if s.w > sw { win = ids.w; sw = s.w; }
            run = win;
            var sr = -1.0;
            if ids.x != win && s.x > sr { run = ids.x; sr = s.x; }
            if ids.y != win && s.y > sr { run = ids.y; sr = s.y; }
            if ids.z != win && s.z > sr { run = ids.z; sr = s.z; }
            if ids.w != win && s.w > sr { run = ids.w; sr = s.w; }
            // The edge (margin 0) antialiased over one pixel (zones::edge_coverage).
            if run != win {
                let d = eq4(ids, win) - eq4(ids, run);
                let g = vec2(dot(d, gather_dwx(f)), dot(d, gather_dwy(f)));
                cov = clamp(0.5 + (sw - sr) / max(px_gradient(g, dx * zs, dy * zs), 1e-6), 0.5, 1.0);
            }
            let pal = zone_col(ids.x).rgb * w.x + zone_col(ids.y).rgb * w.y + zone_col(ids.z).rgb * w.z + zone_col(ids.w).rgb * w.w;
            tint = base / max(pal, vec3(1e-5));
        }
        let pat = terrain.zone_pattern[min(win, 15u)];
        if o.sea < SEA_HIDES_LAND {
            o.tone = zone_tone(p, pat.x, foot);
        }
        let cw = zone_col(win);
        let cr = zone_col(run);
        near = mix(cr.rgb, cw.rgb, cov) * tint * (1.0 + pat.y * o.tone);
        o.lake = (cw.w * cov + cr.w * (1.0 - cov)) * (1.0 - far);
    }
    o.land = mix(near, far_c.rgb, far);
    return o;
}

// Rock on steep slopes and snow above the snow line (both physical). `n`
// in [0, 1] jitters the thresholds.
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

#ifndef PREPASS_PIPELINE
// The slot (frame_data.starlight.stars) of Bevy's directional light `i`: the star in its
// direction. Each slot's light shines from the camera's direction to that
// star (lighting::update_lights), and Bevy orders its lights by entity, so
// they are matched by direction, not by index.
fn slot_of_light(i: u32) -> i32 {
    let l = lights.directional_lights[i].direction_to_light.xyz;
    var best = 0;
    var best_dot = -2.0;
    for (var k = 0; k < MAX_STARS; k++) {
        let s = frame_data.starlight.stars[k];
        if s.w <= 0.0 {
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

// Bevy's shadow for directional light `i` at `p` (normal `n`, view depth
// `view_z`): `shadows::fetch_directional_shadow`'s cascade, biases and
// blend into the next cascade, with the view's filter, but without the
// soft-shadow (PCSS) path, which the stars' lights never use, and with the
// filter's taps written once: Bevy's version, inlined twice and with the
// PCSS loops, takes registers from the whole shader even where no cascade
// reaches (0.4 ms of the plunge's main pass, 2026-10-27).
fn terrain_shadow(i: u32, p: vec3<f32>, n: vec3<f32>, view_z: f32, frag_coord: vec2<f32>) -> f32 {
    let light = &lights.directional_lights[i];
    let first = shadows::get_cascade_index(i, view_z);
    if first >= (*light).num_cascades {
        return 1.0;
    }
    // The next cascade's share where the two overlap.
    var blend = 0.0;
    if first + 1u < (*light).num_cascades {
        let far = (*light).cascades[first].far_bound;
        let near_next = (1.0 - (*light).cascades_overlap_proportion) * far;
        blend = saturate((-view_z - near_next) / (far - near_next));
    }
    var shadow = 0.0;
    var weight = 1.0 - blend;
    for (var c = first; c <= first + 1u; c++) {
        if weight > 0.0 {
            let cascade = &(*light).cascades[c];
            let offset = p + (*light).shadow_normal_bias * (*cascade).texel_size * n
                + (*light).shadow_depth_bias * (*light).direction_to_light.xyz;
            let local = shadows::world_to_directional_light_local(i, c, vec4(offset, 1.0));
            var s = 1.0;
            if local.w != 0.0 {
                let layer = i32((*light).depth_texture_base_index + c);
                s = shadow_sampling::sample_shadow_map(local.xy, local.z, layer, frag_coord, (*cascade).texel_size);
            }
            shadow += weight * s;
        }
        weight = blend;
    }
    return shadow;
}

// Bevy's directional light `i` on the terrain: `lighting::directional_light`'s
// terms for one dielectric layer (Burley diffuse, GGX specular with the
// multiscatter compensation, the light's colour, and with an atmosphere its
// transmittance and the sun's visible part), written out with scalars: built
// through Bevy's `LightingInput` and `PbrInput` they took most of the
// shader's registers, and with them the zones' speed (2026-10-27: 3.8 ms of
// the plunge's 8.5 ms main pass was the lighting).
fn direct_light(
    i: u32, p: vec3<f32>, N: vec3<f32>, V: vec3<f32>, NdotV: f32, diffuse_color: vec3<f32>,
    F0: vec3<f32>, F_ab: vec2<f32>, roughness: f32,
) -> vec3<f32> {
    let light = &lights.directional_lights[i];
    let L = (*light).direction_to_light.xyz;
    let H = normalize(L + V);
    let NdotL = saturate(dot(N, L));
    let NdotH = saturate(dot(N, H));
    let LdotH = saturate(dot(L, H));
    // Fd_Burley.
    let f90 = 0.5 + 2.0 * roughness * LdotH * LdotH;
    let burley = lighting::F_Schlick(1.0, f90, NdotL) * lighting::F_Schlick(1.0, f90, NdotV) * (1.0 / PI);
    // specular(): D, V, F and the multiscatter compensation.
    let D = lighting::D_GGX(roughness, NdotH);
    let Vis = lighting::V_SmithGGXCorrelated(roughness, NdotV, NdotL);
    let F = lighting::fresnel(F0, LdotH);
    let Fr = (D * Vis) * F * (1.0 + F0 * (1.0 / (F_ab.x + F_ab.y) - 1.0));
    var color = (diffuse_color * burley + Fr) * NdotL * (*light).color.rgb;
#ifdef ATMOSPHERE
    let atmosphere = view_bindings::atmosphere;
    let P_as = (atmosphere.world_to_atmosphere * vec4(p, 1.0)).xyz;
    let P_clamped = clamp_to_surface(atmosphere, P_as);
    let r = length(P_clamped);
    let mu_light = dot(L, normalize(P_clamped));
    color *= lighting::sample_transmittance_lut(r, mu_light)
        * calculate_visible_sun_ratio(atmosphere, r, mu_light, (*light).sun_disk_angular_size);
#endif
    return color;
}

// The terrain's lighting, Bevy's StandardMaterial terms (its uniform's
// reflectance and metallic, the roughness given), evaluated once
// (2026-10-26: `apply_pbr_lighting` and then this took every light and
// shadow twice): every star at its own flux, eclipses, ring shadow and
// elevation (D055), each Bevy directional light (one per slotted star,
// carrying its flux at the camera) rescaled by its star's factor here;
// Bevy's ambient term (the sky stand-in, this body's share of it) by the
// flux-weighted mean of the factors. Then, under a haze, each star's
// diffuse light; planetshine from each reflector; and the starlight
// without a slot as ambient. `n`: the mesh's normal; `N`: the shading
// normal (the sphere's under water).
fn shade(color: vec3<f32>, perceptual_roughness: f32, p: vec3<f32>, n: vec3<f32>, N: vec3<f32>, frag_coord: vec2<f32>) -> vec3<f32> {
    let foot = length(fwidth(p));
    let up_here = normalize(p - frame_data.center.xyz);
    // Each slot's factor, by name (a dynamically indexed local array can spill).
    var f0 = vec3(0.0);
    var f1 = vec3(0.0);
    var f2 = vec3(0.0);
    var f3 = vec3(0.0);
    var sky_num = vec3(0.0);
    var sky_den = 0.0;
    var lit = vec3(0.0);
    for (var k = 0; k < MAX_STARS; k++) {
        let s = frame_data.starlight.stars[k];
        let sl = frame_data.starlight.star_light[k];
        if s.w <= 0.0 || sl.w <= 0.0 {
            continue;
        }
        let l = normalize(s.xyz - p);
        let elev = dot(up_here, l);
        // A point where this star has set gets neither its direct light
        // nor its share of the sky light: fade through twilight (6° below
        // to 3° above the local horizon), so night sides stay dark.
        let seen = eclipse(p, s, frame_data.starlight.occluders);
        var f = vec3(sl.w * seen * ring_transmission(p, foot, s.xyz) * smoothstep(-0.105, 0.052, elev));
        // Under a haze (Titan, Venus): the direct beam through the slant
        // column (spherical-shell air mass), and the light the haze
        // scatters down, Lambertian (lighting::haze_light).
        if terrain.haze.w > 0.0 {
            let mu = max(elev, 0.0);
            let rh = terrain.haze.w;
            let airmass = sqrt(rh * rh * mu * mu + 2.0 * rh + 1.0) - rh * mu;
            f *= exp(-terrain.haze.rgb * airmass);
            lit += color * terrain.haze_diffuse.rgb * sl.rgb * seen * mu / PI * view.exposure;
        }
        if k == 0 {
            f0 = f;
        } else if k == 1 {
            f1 = f;
        } else if k == 2 {
            f2 = f;
        } else {
            f3 = f;
        }
        let e_cam = lighting_luminance(sl.rgb) / sl.w;
        sky_num += f * e_cam;
        sky_den += e_cam;
    }
    // The material's terms, as `pbr_input_from_standard_material` and
    // `apply_pbr_lighting` derive them (no transmission, one layer).
    let metallic = pbr_bindings::material.metallic;
    let V = normalize(view.world_position.xyz - p);
    let NdotV = max(dot(N, V), 0.0001);
    let roughness = lighting::perceptualRoughnessToRoughness(perceptual_roughness);
    let diffuse_color = pbr_functions::calculate_diffuse_color(color, metallic, 0.0, 0.0);
    let F0 = pbr_functions::calculate_F0(color, metallic, pbr_bindings::material.reflectance);
    let F_ab = lighting::F_AB(perceptual_roughness, NdotV);
    // Bevy's directional lights, one by one, as apply_pbr_lighting adds them.
    let view_z = dot(vec4(
        view.view_from_world[0].z,
        view.view_from_world[1].z,
        view.view_from_world[2].z,
        view.view_from_world[3].z
    ), vec4(p, 1.0));
    for (var i = 0u; i < lights.n_directional_lights; i++) {
        // Every terrain chunk receives shadows (none is a NotShadowReceiver).
        var shadow = 1.0;
        if (lights.directional_lights[i].flags & mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = terrain_shadow(i, p, n, view_z, frag_coord);
        }
        let c = direct_light(i, p, N, V, NdotV, diffuse_color, F0, F_ab, roughness) * shadow * view.exposure;
        let slot = slot_of_light(i);
        lit += c * select(select(select(f3, f2, slot == 2), f1, slot == 1), f0, slot == 0);
    }
    var sky = vec3(1.0);
    if sky_den > 0.0 {
        sky = sky_num / sky_den;
    }
    // Bevy's ambient term (no environment map, no clustered lights here);
    // the daylight sky stand-in lights only the body whose air the camera is in.
    let amb = ambient_light(
        vec4(p, 1.0), N, V, NdotV, diffuse_color, F0, perceptual_roughness, vec3(frame_data.ambient.x),
    );
    lit += amb * view.exposure * sky;
    // Planetshine: a Lambert term from each reflecting body.
    for (var m = 0; m < MAX_SHINES; m++) {
        let sh = frame_data.starlight.shines[m];
        if sh.w <= 0.0 {
            continue;
        }
        let l = normalize(sh.xyz - p);
        lit += color * frame_data.starlight.shine_light[m].rgb * max(dot(n, l), 0.0) / PI * view.exposure;
    }
    // Starlight without a light slot (a fifth star), sphere-averaged.
    lit += color * frame_data.starlight.ambient.x / PI * view.exposure;
    return lit;
}

// Under an opaque deck (Venus): the gas between the camera (the origin)
// and `p` scatters the sky's light towards the camera, a deep conservative
// medium whose source is the sky's radiance (lighting::haze::
// fog_transmittance, mirrored).
fn deck_fog(lit: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    if frame_data.fog_light.w <= 0.0 {
        return lit;
    }
    let r = terrain.shape.x;
    let h = frame_data.fog.w;
    let z1 = length(frame_data.center.xyz) - r;
    let z2 = length(p - frame_data.center.xyz) - r;
    let d = length(p);
    let dz = z2 - z1;
    var mean = exp(-0.5 * (z1 + z2) / h);
    if abs(dz) > 1e-3 * h {
        mean = h * (exp(-z1 / h) - exp(-z2 / h)) / dz;
    }
    let t = exp(-frame_data.fog.rgb * d * mean);
    return lit * t + frame_data.fog_light.rgb * view.exposure * (1.0 - t);
}

fn lighting_luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3(0.2126, 0.7152, 0.0722));
}
#endif

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let rel = in.world_position.xyz - frame_data.center.xyz;
    let fixed = vec3(dot(frame_data.fixed_x.xyz, rel), dot(frame_data.fixed_y.xyz, rel), dot(frame_data.fixed_z.xyz, rel));
    let dir = normalize(fixed);
    let lat = asin(clamp(dir.z, -1.0, 1.0));
    let lon = atan2(dir.y, dir.x);
    let uv = vec2((lon + PI) / (2.0 * PI), (0.5 * PI - lat) / PI);

    let dx = vec2(wrap_d(dpdx(uv.x)), dpdx(uv.y));
    let dy = vec2(wrap_d(dpdy(uv.x)), dpdy(uv.y));
    // Screen derivatives (taken here, in uniform control flow).
    let dp1 = dpdx(in.world_position.xyz);
    let dp2 = dpdy(in.world_position.xyz);
    let duv1 = dpdx(in.uv);
    let duv2 = dpdy(in.uv);

    let height = in.color.g * 10000.0;
    let up = dir; // close enough to the ellipsoid normal for shading rules
    let roughness = terrain.snow.w;
    let slope = 1.0 - clamp(dot(normalize(in.world_normal), normalize(rel)), 0.0, 1.0);

    var color = terrain.base.rgb;
    // Sea fraction (and, with zones, lakes).
    var water = 0.0;
    let zones_on = frame_data.flags.x > 0.0;
    var water_color = terrain.ocean.rgb;
    if zones_on {
        // The pixel footprint on the surface (m), and the one warp that
        // moves both the zone and the sea lookups, so coasts and zone edges
        // are fractal, line up and keep gaining detail closer in.
        let foot = max(length(dp1), length(dp2));
        let zs = vec2<f32>(textureDimensions(zone_map, 0));
        // The warp only near an edge (the zone map's flag at this texel,
        // zones::edge_mask) and where pixels are under a zone texel:
        // elsewhere every lookup it can reach sees one zone and one side of
        // the coast (the warp took ~10 ms of a 23 ms frame from orbit).
        let n = vec2<i32>(zs);
        let c = vec2<i32>(floor(uv * zs));
        let flag = textureLoad(zone_map, vec2(((c.x % n.x) + n.x) % n.x, clamp(c.y, 0, n.y - 1)), 0).r;
        let near_edge = flag * 255.0 > 127.5;
        let pixel_texels = max(length(dx * zs), length(dy * zs));
        var uv_w = uv;
        if near_edge && pixel_texels < 1.0 {
            uv_w = uv + zone_warp(fixed, terrain.zones.y, foot) / zs;
        }
        let dxw = vec2(wrap_d(dpdx(uv_w.x)), dpdx(uv_w.y));
        let dyw = vec2(wrap_d(dpdy(uv_w.x)), dpdy(uv_w.y));
        let id = u32(flag * 255.0 + 0.5) & 0x7fu;
        let z = zone_surface(uv_w, dxw, dyw, dx, dy, fixed, foot, near_edge, id);
        color = z.land;
        water = max(z.sea, z.lake);
        // Open water, lighter over a shallow sea floor (lakes stay deep).
        if terrain.shallow.w > 0.0 {
            water_color = mix(terrain.ocean.rgb, terrain.shallow.rgb, z.shallow * (1.0 - z.lake));
        }
        // Rock on steep slopes and the snow line, on land.
        if terrain.shape.w > 0.0 {
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
        }
    }

    let dist = length(in.world_position.xyz);
    // A simple ground texture for depth perception near the ground
    // (zones::ground_* mirror the rules, D073): lighter and darker patches
    // at 8 scales, 320 m down to 2.5 m on Earth (detail units / 0.125,
    // halving), each fading out between 8 and 4 pixels a wavelength so
    // nothing shimmers, plus broad patches of cover and bare ground from the
    // two coarsest, fading in between 32 and 64 pixels a wavelength.
    // Typically ±0.3 stops, at most ~±1.
    if water < 1.0 {
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
    // The shading normal: water lies at sea level, so it is shaded with the
    // sphere's normal at this fragment, not the mesh normal. On a coarse
    // chunk the mesh normal is interpolated across triangles kilometres
    // wide, and the sharp sun glint took their shape (grey and white
    // triangles that jumped as the LOD changed while zooming).
    let n_mesh = normalize(in.world_normal);
    var n_out = n_mesh;
    if water > 0.0 {
        let sphere_n = normalize(in.world_position.xyz - frame_data.center.xyz);
        n_out = normalize(mix(n_out, sphere_n, water));
    }

    if zones_on {
        color = mix(color, water_color, water);
    } else {
        // The colour map under water is raised to at least the open-water albedo.
        color = mix(color, max(color, terrain.ocean.rgb), water);
    }

#ifdef PREPASS_PIPELINE
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.N = n_out;
    pbr_input.material.base_color = vec4(color, 1.0);
    pbr_input.material.perceptual_roughness = roughness;
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = vec4(shade(color, roughness, in.world_position.xyz, n_mesh, n_out, in.position.xy), 1.0);
    out.color = vec4(deck_fog(out.color.rgb, in.world_position.xyz), out.color.a);
    var post = pbr_types::pbr_input_new();
    post.world_position = in.world_position;
    post.frag_coord = in.position;
    out.color = main_pass_post_lighting_processing(post, out.color);
#endif
    return out;
}
