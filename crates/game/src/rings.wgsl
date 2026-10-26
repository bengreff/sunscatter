// Planetary rings (rings.rs): each fragment of the disc is a slab of the
// profile's normal optical depth τ. Coverage 1 − exp(−τ/μ_v); brightness
// from the single-scattering slab law with isotropic particles of the
// region's albedo ϖ:
//   lit face:   I/F = ϖ/4 · μ_s/(μ_s + μ_v) · (1 − exp(−τ (1/μ_s + 1/μ_v)))
//   unlit face: I/F = ϖ/4 · μ_s/(μ_s − μ_v) · (exp(−τ/μ_s) − exp(−τ/μ_v))
// summed over every drawn star at its illuminance at the rings, each
// dimmed where the planet or a moon hides it (sunscatter::light::eclipse,
// with the planet's penumbra).
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}
#import sunscatter::light::{Lights, eclipse, MAX_STARS}

struct RingParams {
    // xyz: the planet's centre relative to the camera; w: its equatorial radius.
    center: vec4<f32>,
    // xyz: the ring plane's normal.
    normal: vec4<f32>,
    // x inner, y outer radius (m), z bins.
    radii: vec4<f32>,
    // The planet's light (lighting::LightUniform), the planet first among the occluders.
    starlight: Lights,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ring: RingParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var profile: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var profile_sampler: sampler;

const PI: f32 = 3.14159265;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xyz;
    let n = normalize(ring.normal.xyz);
    let d = p - ring.center.xyz;
    let r = length(d - n * dot(d, n));
    // The profile averaged over the pixel's radial footprint (its mips).
    let bin_m = (ring.radii.y - ring.radii.x) / ring.radii.z;
    let lod = log2(max(fwidth(r) / bin_m, 1.0));
    let u = (r - ring.radii.x) / (ring.radii.y - ring.radii.x);
    if u < 0.0 || u > 1.0 {
        discard;
    }
    let s = textureSampleLevel(profile, profile_sampler, vec2(u, 0.5), lod);
    let tau = s.a;
    let v = normalize(-p);
    let mu_v = max(abs(dot(v, n)), 1e-3);
    let alpha = 1.0 - exp(-tau / mu_v);
    if alpha < 1e-4 {
        discard;
    }
    var radiance = vec3(0.0);
    for (var k = 0; k < MAX_STARS; k++) {
        let star = ring.starlight.stars[k];
        if star.w <= 0.0 {
            continue;
        }
        let l = normalize(star.xyz - p);
        let mu_s = max(abs(dot(l, n)), 1e-3);
        var i_f: f32;
        if dot(v, n) * dot(l, n) > 0.0 {
            i_f = 0.25 * mu_s / (mu_s + mu_v) * (1.0 - exp(-tau * (1.0 / mu_s + 1.0 / mu_v)));
        } else if abs(mu_s - mu_v) < 1e-3 {
            i_f = 0.25 * tau / mu_v * exp(-tau / mu_v);
        } else {
            i_f = 0.25 * mu_s / (mu_s - mu_v) * (exp(-tau / mu_s) - exp(-tau / mu_v));
        }
        radiance += s.rgb * i_f * ring.starlight.star_light[k].rgb / PI * eclipse(p, star, ring.starlight.occluders);
    }
    return vec4(radiance * view.exposure / alpha, alpha);
}
