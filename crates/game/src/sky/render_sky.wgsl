enable dual_source_blending;

// Sunscatter's copy of Bevy 0.19.1's `bevy_pbr/atmosphere/render_sky.wgsl`
// (MIT/Apache-2.0), installed over the embedded original by `sky::haze`.
// Changes, for geometry pixels only (the sky is unchanged):
// - beyond the aerial-view LUT's range the ray is raymarched instead of
//   clamping the in-scatter to the LUT's last slice (with the transmittance
//   still taken over the whole distance, far ground turned dark behind a
//   hard edge: the owner's "dark horizon");
// - from outside the atmosphere, the ground's aerial perspective is
//   raymarched in SPACE_SAMPLES steps growing geometrically upwards from
//   the ground, where the air is (`raymarch_from_space`), instead of 24
//   even steps over the whole segment, most of them in near vacuum (14 ms
//   of a 38 ms frame at 3024×1964 from 185 km, 2026-10-26); and the LUTs
//   are not sampled where the raymarch replaces them;
// - from outside, that in-scatter is looked up in the table from space our
//   `aerial_view_lut.wgsl` fills (screen direction × altitude of the
//   ground hit, `ground_from_space`), raymarched per pixel only near the
//   limb, where the table's texels around a pixel look past the hit;
// - from outside, geometry the ray meets before the air (a craft in orbit)
//   takes no aerial perspective: the table assumed a hit in the air and,
//   for a ray rising from the camera, read the light back down through the
//   planet (craft drawn dark red above a line across the view whenever
//   the camera looked up at them more than ~15°, 2026-10-31).
// sunscatter sky override
#import bevy_pbr::atmosphere::{
    bindings::{view, settings, atmosphere, aerial_view_lut, atmosphere_lut_sampler},
    functions::{
        sample_transmittance_lut, sample_transmittance_lut_segment,
        sample_sky_view_lut, direction_world_to_atmosphere,
        uv_to_ray_direction, uv_to_ndc, sample_aerial_view_lut,
        sample_sun_radiance, ndc_to_camera_dist, raymarch_atmosphere, 
        get_view_position, max_atmosphere_distance, get_raymarch_segment,
        sample_density_lut, sample_local_inscattering, RaymarchResult,
        ABSORPTION_DENSITY, SCATTERING_DENSITY, MIN_EXTINCTION
    },
};

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#ifdef MULTISAMPLED
@group(0) @binding(13) var depth_texture: texture_depth_multisampled_2d;
#else
@group(0) @binding(13) var depth_texture: texture_depth_2d;
#endif

// Steps of the raymarch from space, each SPACE_RATIO times the one below
// it: from 100 km straight down, 1.4 km at the ground to 38 km at the top.
const SPACE_SAMPLES: u32 = 8u;
const SPACE_RATIO: f32 = 1.6;

// The in-scatter and transmittance from a camera outside the atmosphere
// along `ray_dir` to the ground `t_ground` away (the density only grows
// along such a ray: it is on its way down when it meets the ground).
fn raymarch_from_space(pos: vec3<f32>, ray_dir: vec3<f32>, t_ground: f32) -> RaymarchResult {
    var result: RaymarchResult;
    result.inscattering = vec3(0.0);
    result.transmittance = vec3(1.0);
    let r = length(pos);
    let segment = get_raymarch_segment(r, dot(ray_dir, pos / r));
    let t_end = min(segment.end, t_ground);
    let total = t_end - segment.start;
    if total <= 0.0 {
        return result;
    }
    let n = f32(SPACE_SAMPLES);
    var dt = total * (SPACE_RATIO - 1.0) / (pow(SPACE_RATIO, n) - 1.0) * pow(SPACE_RATIO, n - 1.0);
    var t = segment.start;
    for (var i = 0u; i < SPACE_SAMPLES; i++) {
        let sample_pos = pos + ray_dir * (t + 0.5 * dt);
        let local_r = length(sample_pos);
        let absorption = sample_density_lut(local_r, ABSORPTION_DENSITY);
        let scattering = sample_density_lut(local_r, SCATTERING_DENSITY);
        let extinction = absorption + scattering;
        let sample_transmittance = exp(-extinction * dt);
        let inscattering = sample_local_inscattering(scattering, ray_dir, sample_pos);
        let s_int = (inscattering - inscattering * sample_transmittance) / max(extinction, MIN_EXTINCTION);
        result.inscattering += result.transmittance * s_int;
        result.transmittance *= sample_transmittance;
        t += dt;
        dt /= SPACE_RATIO;
    }
    return result;
}

// The table from space (`aerial_view_lut.wgsl`): the texture coordinate of
// altitude `h` (m) along its slices, the inverse of its quadratic
// `space_slice_altitude`.
fn space_slice_coord(h: f32, n: f32, thickness: f32) -> f32 {
    let u = sqrt(clamp(h / thickness, 0.0, 1.0));
    return (u * (n - 1.0) + 0.5) / n;
}

// The ground's aerial perspective from outside the atmosphere at distance
// `t` along this pixel's ray: the in-scatter from the table (the slices
// around the hit's altitude, the texels around the pixel), the
// transmittance from the transmittance table (the ray back from the ground
// leaves the atmosphere without meeting it again). Raymarched instead where
// a texel around the pixel looks past the slice below the hit (near the
// limb): its ray's distance from the centre (`b`) can differ from the
// pixel's by the texel's angle times sqrt(r² − b²).
fn ground_from_space(pos: vec3<f32>, ray_dir: vec3<f32>, t: f32, uv: vec2<f32>) -> RaymarchResult {
    var result: RaymarchResult;
    result.inscattering = vec3(0.0);
    result.transmittance = vec3(1.0);
    // Geometry the ray meets before the air (a craft above it), or a ray
    // that never enters the air: nothing between (`sky::haze::air_before`,
    // mirrored for its tests).
    let r = length(pos);
    let segment = get_raymarch_segment(r, dot(ray_dir, pos / r));
    if min(segment.end, t) <= segment.start {
        return result;
    }
    let thickness = atmosphere.outer_radius - atmosphere.inner_radius;
    let n = f32(settings.aerial_view_lut_size.z);
    let hit = pos + ray_dir * t;
    let r_hit = clamp(length(hit), atmosphere.inner_radius, atmosphere.outer_radius);
    let w = space_slice_coord(r_hit - atmosphere.inner_radius, n, thickness);
    let u_lo = floor(w * n - 0.5) / (n - 1.0);
    let r_lo = atmosphere.inner_radius + thickness * u_lo * u_lo;
    let size = vec2<f32>(settings.aerial_view_lut_size.xy);
    let texel = vec2(2.0 / (size.x * view.clip_from_view[0][0]), 2.0 / (size.y * view.clip_from_view[1][1]));
    let pos_r2 = dot(pos, pos);
    let b = length(cross(pos, ray_dir));
    let margin = 1.5 * sqrt(max(pos_r2 - b * b, 0.0)) * length(texel);
    if b + margin > r_lo {
        return raymarch_from_space(pos, ray_dir, t);
    }
    let lut = textureSampleLevel(aerial_view_lut, atmosphere_lut_sampler, vec3(uv, w), 0.0).rgb;
    result.inscattering = exp(lut);
    result.transmittance = sample_transmittance_lut(r_hit, dot(-ray_dir, hit) / length(hit));
    return result;
}

struct RenderSkyOutput {
#ifdef DUAL_SOURCE_BLENDING
    @location(0) @blend_src(0) inscattering: vec4<f32>,
    @location(0) @blend_src(1) transmittance: vec4<f32>,
#else
    @location(0) inscattering: vec4<f32>,
#endif
}

@fragment
fn main(in: FullscreenVertexOutput) -> RenderSkyOutput {
    let depth = textureLoad(depth_texture, vec2<i32>(in.position.xy), 0);

    let ray_dir_ws = uv_to_ray_direction(in.uv);
    let world_pos = get_view_position();
    let r = length(world_pos);
    let up = normalize(world_pos);
    let mu = dot(ray_dir_ws, up);
    let max_samples = settings.sky_max_samples;
    let should_raymarch = settings.rendering_method == 1u;

    var transmittance: vec3<f32>;
    var inscattering: vec3<f32>;

    let sun_radiance = sample_sun_radiance(ray_dir_ws);

    if depth == 0.0 {
        let ray_dir_as = direction_world_to_atmosphere(ray_dir_ws);
        transmittance = sample_transmittance_lut(r, mu);
        inscattering = sample_sky_view_lut(r, ray_dir_as);
        if should_raymarch {
            let t_max = max_atmosphere_distance(r, mu);
            let result = raymarch_atmosphere(world_pos, ray_dir_ws, t_max, max_samples, in.uv, true);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        }
        inscattering += sun_radiance * transmittance;
    } else {
        let t = ndc_to_camera_dist(vec3(uv_to_ndc(in.uv), depth));
        if should_raymarch && r > atmosphere.outer_radius {
            let result = ground_from_space(world_pos, ray_dir_ws, t, in.uv);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        } else if should_raymarch || t > settings.aerial_view_lut_max_distance {
            let result = raymarch_atmosphere(world_pos, ray_dir_ws, t, max(max_samples, 16u), in.uv, false);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        } else {
            inscattering = sample_aerial_view_lut(in.uv, t);
            transmittance = sample_transmittance_lut_segment(r, mu, t);
        }
    }

    // exposure compensation
    inscattering *= view.exposure;
    
#ifdef DUAL_SOURCE_BLENDING
    return RenderSkyOutput(vec4(inscattering, 0.0), vec4(transmittance, 1.0));
#else
    let mean_transmittance = (transmittance.r + transmittance.g + transmittance.b) / 3.0;
    return RenderSkyOutput(vec4(inscattering, mean_transmittance));
#endif
    
}
