// Planetary rings (rings.rs): each fragment of the disc is a slab of the
// profile's normal optical depth τ. Coverage 1 − exp(−τ/μ_v); brightness
// from the single-scattering slab law with isotropic particles of the
// region's albedo ϖ:
//   lit face:   I/F = ϖ/4 · μ_s/(μ_s + μ_v) · (1 − exp(−τ (1/μ_s + 1/μ_v)))
//   unlit face: I/F = ϖ/4 · μ_s/(μ_s − μ_v) · (exp(−τ/μ_s) − exp(−τ/μ_v))
// lit by the star's illuminance at the rings, in the planet's shadow dark.
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

struct RingParams {
    // xyz: the planet's centre relative to the camera; w: its equatorial radius.
    center: vec4<f32>,
    // xyz: the ring plane's normal.
    normal: vec4<f32>,
    // xyz: the star relative to the camera; w: its illuminance at the rings (lux).
    star: vec4<f32>,
    // x inner, y outer radius (m), z bins.
    radii: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ring: RingParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var profile: texture_2d<f32>;

const PI: f32 = 3.14159265;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xyz;
    let n = normalize(ring.normal.xyz);
    let d = p - ring.center.xyz;
    let r = length(d - n * dot(d, n));
    let bins = ring.radii.z;
    let x = (r - ring.radii.x) / (ring.radii.y - ring.radii.x) * bins - 0.5;
    if x < -0.5 || x > bins - 0.5 {
        discard;
    }
    let i0 = clamp(i32(floor(x)), 0, i32(bins) - 1);
    let i1 = min(i0 + 1, i32(bins) - 1);
    let f = clamp(x - floor(x), 0.0, 1.0);
    let s = mix(textureLoad(profile, vec2(i0, 0), 0), textureLoad(profile, vec2(i1, 0), 0), f);
    let tau = s.a;
    let v = normalize(-p);
    let l = normalize(ring.star.xyz - p);
    let mu_v = max(abs(dot(v, n)), 1e-3);
    let mu_s = max(abs(dot(l, n)), 1e-3);
    let alpha = 1.0 - exp(-tau / mu_v);
    if alpha < 1e-4 {
        discard;
    }
    var i_f: f32;
    if dot(v, n) * dot(l, n) > 0.0 {
        i_f = 0.25 * mu_s / (mu_s + mu_v) * (1.0 - exp(-tau * (1.0 / mu_s + 1.0 / mu_v)));
    } else if abs(mu_s - mu_v) < 1e-3 {
        i_f = 0.25 * tau / mu_v * exp(-tau / mu_v);
    } else {
        i_f = 0.25 * mu_s / (mu_s - mu_v) * (exp(-tau / mu_s) - exp(-tau / mu_v));
    }
    // The planet's shadow: the ray to the star meets its sphere.
    let oc = p - ring.center.xyz;
    let b = dot(oc, l);
    let c = dot(oc, oc) - ring.center.w * ring.center.w;
    let shadow = select(1.0, 0.0, b < 0.0 && b * b - c > 0.0);
    let radiance = s.rgb * i_f * ring.star.w / PI * shadow;
    return vec4(radiance * view.exposure / alpha, alpha);
}
