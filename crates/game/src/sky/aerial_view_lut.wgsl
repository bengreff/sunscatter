// Sunscatter's copy of Bevy 0.19.1's `bevy_pbr/atmosphere/aerial_view_lut.wgsl`
// (MIT/Apache-2.0), installed over the embedded original by `sky::haze`.
// Inside the atmosphere it is Bevy's table, unchanged. From outside it
// (raymarched, `render_sky.wgsl`) the table holds, for each direction on
// the screen, the light the air scatters towards the camera between the
// top of the atmosphere and the point where the ray comes down to each of
// the slices' altitudes (`space_slice_altitude`): the ground's aerial
// perspective from space at any terrain height, looked up per pixel
// instead of raymarched per pixel (4 ms of a 21 ms frame at 3024×1964 from
// 185 km, 2026-10-27).
// sunscatter aerial view override
#import bevy_pbr::{
    atmosphere::{
        bindings::{settings, atmosphere},
        functions::{
            sample_density_lut, sample_local_inscattering, uv_to_ray_direction, get_view_position,
            MIDPOINT_RATIO, MIN_EXTINCTION, ABSORPTION_DENSITY, SCATTERING_DENSITY,
        },
    }
}


@group(0) @binding(13) var aerial_view_lut_out: texture_storage_3d<rgba16float, write>;

// Raymarch steps between two slices of the table from space.
const SPACE_STEPS_PER_SLICE: u32 = 2u;

// The altitude of slice `k` of `n` (k = 0 the atmosphere's ground, n − 1
// its top), quadratic so the slices crowd where the air is dense; the
// render pass inverts it (`render_sky.wgsl`: `space_slice_coord`).
fn space_slice_altitude(k: f32, n: f32, thickness: f32) -> f32 {
    let u = k / (n - 1.0);
    return thickness * u * u;
}

// The distance along a ray from `pos` (outside the sphere, `b` = pos · dir)
// to where it first comes down to radius `radius`; its closest approach if
// it never does.
fn descent_to(pos_r2: f32, b: f32, radius: f32) -> f32 {
    let disc = b * b - (pos_r2 - radius * radius);
    if disc < 0.0 {
        return -b;
    }
    return -b - sqrt(disc);
}

// The table from space: each slice the in-scatter (log) from the top of the
// atmosphere down to the slice's altitude along this texel's ray.
fn from_space(idx: vec2<u32>, pos: vec3<f32>, ray_dir: vec3<f32>) {
    let n = settings.aerial_view_lut_size.z;
    let thickness = atmosphere.outer_radius - atmosphere.inner_radius;
    let pos_r2 = dot(pos, pos);
    let b = dot(pos, ray_dir);
    var total = vec3(0.0);
    var throughput = vec3(1.0);
    // The top: where the ray enters (nothing scattered yet).
    var t_prev = descent_to(pos_r2, b, atmosphere.outer_radius);
    let entered = b * b - (pos_r2 - atmosphere.outer_radius * atmosphere.outer_radius) >= 0.0 && b < 0.0;
    textureStore(aerial_view_lut_out, vec3(idx, n - 1u), vec4(log(vec3(1e-6)), 0.0));
    for (var k = i32(n) - 2; k >= 0; k--) {
        let h = space_slice_altitude(f32(k), f32(n), thickness);
        let t_next = descent_to(pos_r2, b, atmosphere.inner_radius + h);
        let span = max(t_next - t_prev, 0.0);
        if entered && span > 0.0 {
            let dt = span / f32(SPACE_STEPS_PER_SLICE);
            for (var s = 0u; s < SPACE_STEPS_PER_SLICE; s++) {
                let sample_pos = pos + ray_dir * (t_prev + (f32(s) + MIDPOINT_RATIO) * dt);
                let local_r = length(sample_pos);
                let absorption = sample_density_lut(local_r, ABSORPTION_DENSITY);
                let scattering = sample_density_lut(local_r, SCATTERING_DENSITY);
                let extinction = absorption + scattering;
                let sample_transmittance = exp(-extinction * dt);
                let inscattering = sample_local_inscattering(scattering, ray_dir, sample_pos);
                let s_int = (inscattering - inscattering * sample_transmittance) / max(extinction, MIN_EXTINCTION);
                total += throughput * s_int;
                throughput *= sample_transmittance;
            }
        }
        t_prev = max(t_prev, t_next);
        textureStore(aerial_view_lut_out, vec3(idx, u32(k)), vec4(log(max(total, vec3(1e-6))), 0.0));
    }
}

@compute
@workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) idx: vec3<u32>) {
    if any(idx.xy >= settings.aerial_view_lut_size.xy) { return; }

    let uv = (vec2<f32>(idx.xy) + 0.5) / vec2<f32>(settings.aerial_view_lut_size.xy);
    let ray_dir = uv_to_ray_direction(uv);
    let world_pos = get_view_position();

    let r = length(world_pos);
    if settings.rendering_method == 1u && r > atmosphere.outer_radius {
        from_space(idx.xy, world_pos, ray_dir);
        return;
    }
    let t_max = settings.aerial_view_lut_max_distance;

    var prev_t = 0.0;
    var total_inscattering = vec3(0.0);
    var throughput = vec3(1.0);

    for (var slice_i: u32 = 0; slice_i < settings.aerial_view_lut_size.z; slice_i++) {
        for (var step_i: u32 = 0; step_i < settings.aerial_view_lut_samples; step_i++) {
            let t_i = t_max * (f32(slice_i) + ((f32(step_i) + MIDPOINT_RATIO) / f32(settings.aerial_view_lut_samples))) / f32(settings.aerial_view_lut_size.z);
            let dt = (t_i - prev_t);
            prev_t = t_i;

            let sample_pos = world_pos + ray_dir * t_i;
            let local_r = length(sample_pos);

            let absorption = sample_density_lut(local_r, ABSORPTION_DENSITY);
            let scattering = sample_density_lut(local_r, SCATTERING_DENSITY);
            let extinction = absorption + scattering;

            let sample_optical_depth = extinction * dt;
            let sample_transmittance = exp(-sample_optical_depth);

            // evaluate one segment of the integral
            var inscattering = sample_local_inscattering(scattering, ray_dir, sample_pos);

            // Analytical integration of the single scattering term in the radiance transfer equation
            let s_int = (inscattering - inscattering * sample_transmittance) / max(extinction, MIN_EXTINCTION);
            total_inscattering += throughput * s_int;

            throughput *= sample_transmittance;
            if all(throughput < vec3(0.001)) {
                break;
            }
        }

        // Store in log space to allow linear interpolation of exponential values between slices
        let log_inscattering = log(max(total_inscattering, vec3(1e-6)));
        textureStore(aerial_view_lut_out, vec3(vec2<u32>(idx.xy), slice_i), vec4(log_inscattering, 0.0));
    }
}
