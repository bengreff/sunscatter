enable dual_source_blending;

// Sunscatter's copy of Bevy 0.19.1's `bevy_pbr/atmosphere/render_sky.wgsl`
// (MIT/Apache-2.0), installed over the embedded original by `sky::haze`.
// Changes, for geometry pixels only (the sky is unchanged):
// - beyond the aerial-view LUT's range the ray is raymarched instead of
//   clamping the in-scatter to the LUT's last slice (with the transmittance
//   still taken over the whole distance, far ground turned dark behind a
//   hard edge: the owner's "dark horizon");
// - the haze strength setting scales the aerial perspective: in-scatter by
//   HAZE and the optical depth by HAZE (as if the air were HAZE times as
//   dense along the view ray; 1 = physical).
// sunscatter haze override
#import bevy_pbr::atmosphere::{
    bindings::{view, settings},
    functions::{
        sample_transmittance_lut, sample_transmittance_lut_segment,
        sample_sky_view_lut, direction_world_to_atmosphere,
        uv_to_ray_direction, uv_to_ndc, sample_aerial_view_lut,
        sample_sun_radiance, ndc_to_camera_dist, raymarch_atmosphere, 
        get_view_position, max_atmosphere_distance
    },
};

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#ifdef MULTISAMPLED
@group(0) @binding(13) var depth_texture: texture_depth_multisampled_2d;
#else
@group(0) @binding(13) var depth_texture: texture_depth_2d;
#endif

const HAZE: f32 = {{HAZE}};

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
        inscattering = sample_aerial_view_lut(in.uv, t);
        transmittance = sample_transmittance_lut_segment(r, mu, t);
        if should_raymarch || t > settings.aerial_view_lut_max_distance {
            let result = raymarch_atmosphere(world_pos, ray_dir_ws, t, max(max_samples, 16u), in.uv, false);
            inscattering = result.inscattering;
            transmittance = result.transmittance;
        }
        inscattering *= HAZE;
        transmittance = pow(transmittance, vec3(HAZE));
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
