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
//   are not sampled where the raymarch replaces them.
// sunscatter sky override
#import bevy_pbr::atmosphere::{
    bindings::{view, settings, atmosphere},
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
            let result = raymarch_from_space(world_pos, ray_dir_ws, t);
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
