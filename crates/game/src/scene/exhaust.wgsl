// Engine exhaust (scene/exhaust.rs): a bounded raymarch through one plume,
// the same rules as the Rust side (boundary, temperature, extinction): soot
// that absorbs, glows at its Planck colour and scatters the starlight at the
// craft (isotropic). Premultiplied: rgb the radiance, a the soot's opacity.
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

struct PlumeParams {
    // xyz: the exit's centre (camera-relative); w: the exit radius.
    origin: vec4<f32>,
    // xyz: the exhaust's direction; w: the drawn length.
    axis: vec4<f32>,
    // x slope, y spread, z cell length, w drawn width.
    shape: vec4<f32>,
    // x exit temperature, y cooling exponent, z shock, w afterburning.
    heat: vec4<f32>,
    // x extinction at the exit, y profile exponent, z core length, w albedo.
    soot: vec4<f32>,
    // rgb: starlight at the craft (lux); w: the afterburning temperature.
    light: vec4<f32>,
    // rgb: the blackbody scale per channel (nits); w: the cells the shocks fade over.
    planck: vec4<f32>,
    // x: the potential core's length (the axis keeps the exit state).
    core: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> plume: PlumeParams;

const PI: f32 = 3.14159265;
// Samples on each side of the closest approach (16 in all).
const HALF: i32 = 8;
// C2/λ (K) at 610, 550 and 465 nm.
const PLANCK_A: vec3<f32> = vec3(23586.5, 26159.6, 30941.4);

fn boundary(x: f32) -> f32 {
    let k = plume.shape.x;
    let d = plume.shape.y;
    let den = d + x * k;
    let guard = select(1e-9, -1e-9, den < 0.0) * max(plume.origin.w, 1e-3);
    return plume.origin.w + x * k * d / (den + guard);
}

// r₀/R on the axis: 1 in the potential core, then as the boundary widens.
fn spread_ratio(x: f32) -> f32 {
    return plume.origin.w / boundary(max(x - plume.core.x, 0.0));
}

// The soot's share across the jet at `rho` from the axis, boundary `r`
// (`exhaust::profile`): even in a core from the exit's radius at the lip
// to the axis at the potential core's end, Simons' outside it.
fn profile(x: f32, rho: f32, r: f32) -> f32 {
    let core = min(plume.origin.w * clamp(1.0 - x / max(plume.core.x, 1e-6), 0.0, 1.0), r);
    if rho <= core {
        return 1.0;
    }
    let s = min((rho - core) / max(r - core, 1e-6), 1.0);
    return pow(cos(0.5 * PI * s), plume.soot.y);
}

fn temperature(x: f32) -> f32 {
    let cell = plume.shape.z;
    let iso = plume.heat.x * pow(spread_ratio(x), plume.heat.y);
    let cells = 0.5 * (1.0 - cos(2.0 * PI * x / cell)) * exp(-x / (plume.planck.w * cell));
    let shocked = iso * (1.0 + plume.heat.z * cells);
    return shocked + (plume.light.w - shocked) * plume.heat.w * (1.0 - exp(-x / cell));
}

fn planck(t: f32) -> vec3<f32> {
    return plume.planck.rgb / (exp(min(PLANCK_A / max(t, 1.0), vec3(80.0))) - 1.0);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let cam = view.world_position;
    let to = in.world_position.xyz - cam;
    let t_face = length(to);
    let d = to / t_face;
    let a = plume.axis.xyz;
    let len = plume.axis.w;
    let w = plume.shape.w;
    // The ray against the bounding cylinder (radius w, x in [0, len]).
    let p = cam - plume.origin.xyz;
    let x0 = dot(p, a);
    let dx = dot(d, a);
    let q = p - x0 * a;
    let qd = d - dx * a;
    let qa = max(dot(qd, qd), 1e-12);
    let qb = dot(q, qd);
    let qc = dot(q, q) - w * w;
    let disc = qb * qb - qa * qc;
    if disc <= 0.0 {
        discard;
    }
    let s = sqrt(disc);
    var t0 = (-qb - s) / qa;
    var t1 = (-qb + s) / qa;
    let inv = select(1.0 / dx, 1e30, abs(dx) < 1e-9);
    let ta = (0.0 - x0) * inv;
    let tb = (len - x0) * inv;
    t0 = max(t0, min(ta, tb));
    t1 = min(t1, max(ta, tb));
    let inside = t0 <= 0.0;
    // Front faces march from the entry; back faces only from inside.
    if !front && !inside {
        discard;
    }
    t0 = max(t0, 0.0);
    if t1 <= t0 {
        discard;
    }
    // Samples crowd quadratically toward the ray's closest approach to the
    // exit, where the soot is densest (∝ (r₀/R)²), on each side of it, front
    // to back: a 150 m chord through a vacuum fan still samples its 1 m core.
    let tc = clamp(-dot(p, d), t0, t1);
    var trans = 1.0;
    var radiance = vec3(0.0);
    let albedo = plume.soot.w;
    let scatter = plume.light.rgb / (4.0 * PI);
    let n = f32(HALF);
    for (var j = 0; j < 2 * HALF; j++) {
        // Near side from its far end in, then the far side outwards.
        let i = select(j - HALF, HALF - 1 - j, j < HALF);
        let side = select(t1 - tc, tc - t0, j < HALF);
        let sign = select(1.0, -1.0, j < HALF);
        let fi = f32(i);
        let t = tc + sign * side * ((fi + 0.5) / n) * ((fi + 0.5) / n);
        let ds = side * (2.0 * fi + 1.0) / (n * n);
        let pt = p + d * t;
        let x = dot(pt, a);
        if x < 0.0 || x > len {
            continue;
        }
        let r = boundary(x);
        let rho = length(pt - x * a);
        if rho >= r {
            continue;
        }
        let ratio = spread_ratio(x);
        let kappa = plume.soot.x * ratio * ratio * exp(-x / plume.soot.z) * profile(x, rho, r);
        let absorbed = 1.0 - exp(-kappa * ds);
        let source = (1.0 - albedo) * planck(temperature(x)) + albedo * scatter;
        radiance += trans * absorbed * source;
        trans *= 1.0 - absorbed;
    }
    let alpha = 1.0 - trans;
    if alpha < 1e-5 && dot(radiance, vec3(1.0)) * view.exposure < 1e-4 {
        discard;
    }
    return vec4(radiance * view.exposure, alpha);
}
