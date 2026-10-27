// The main pass's part of the HDR texture, upscaled to all of it before
// tonemapping (render_scale.rs): Catmull-Rom from 9 bilinear taps (the
// usual "sharp bicubic" of temporal upscalers), clamped to the rendered
// part so nothing outside it bleeds in, and to positive light.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Params {
    // The rendered part's size over the texture's.
    ratio: vec2<f32>,
    // The texture's texel size (1 / its size).
    texel: vec2<f32>,
}

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

fn tap(uv: vec2<f32>) -> vec3<f32> {
    let lo = 0.5 * params.texel;
    let hi = params.ratio - 0.5 * params.texel;
    return textureSampleLevel(src, src_sampler, clamp(uv, lo, hi), 0.0).rgb;
}

@fragment
fn fs_main(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = 1.0 / params.texel;
    let pos = in.uv * params.ratio * size;
    let centre = floor(pos - 0.5) + 0.5;
    let f = pos - centre;
    // Catmull-Rom weights, the middle two taps folded into one bilinear.
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let o12 = w2 / w12;
    let t0 = (centre - 1.0) * params.texel;
    let t3 = (centre + 2.0) * params.texel;
    let t12 = (centre + o12) * params.texel;
    var c = vec3(0.0);
    c += tap(vec2(t0.x, t0.y)) * w0.x * w0.y;
    c += tap(vec2(t12.x, t0.y)) * w12.x * w0.y;
    c += tap(vec2(t3.x, t0.y)) * w3.x * w0.y;
    c += tap(vec2(t0.x, t12.y)) * w0.x * w12.y;
    c += tap(vec2(t12.x, t12.y)) * w12.x * w12.y;
    c += tap(vec2(t3.x, t12.y)) * w3.x * w12.y;
    c += tap(vec2(t0.x, t3.y)) * w0.x * w3.y;
    c += tap(vec2(t12.x, t3.y)) * w12.x * w3.y;
    c += tap(vec2(t3.x, t3.y)) * w3.x * w3.y;
    return vec4(max(c, vec3(0.0)), 1.0);
}
