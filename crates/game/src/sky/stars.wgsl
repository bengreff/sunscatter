// Stars: one screen-aligned quad per star at a huge distance in its
// catalogue direction, sized in pixels, with a Gaussian profile. Drawn
// additively; the atmosphere pass later adds sky light and dims them.

#import bevy_pbr::mesh_view_bindings::view

struct StarParams {
    // Overall brightness (daylight fades it).
    intensity: f32,
    // Pixel size multiplier.
    px_scale: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: StarParams;

struct Vertex {
    // Unit direction (world axes = inertial).
    @location(0) position: vec3<f32>,
    // Quad corner in [-1, 1]².
    @location(2) uv: vec2<f32>,
    // rgb: colour × brightness, a: radius in pixels.
    @location(5) color: vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
}

@vertex
fn vertex(v: Vertex) -> VertexOut {
    let world = view.world_position + v.position * 1.0e11;
    var clip = view.clip_from_world * vec4(world, 1.0);
    let radius = v.color.a * params.px_scale;
    clip.x += v.uv.x * radius * 2.0 / view.viewport.z * clip.w;
    clip.y += v.uv.y * radius * 2.0 / view.viewport.w * clip.w;
    var out: VertexOut;
    out.clip = clip;
    out.uv = v.uv;
    out.color = v.color.rgb;
    return out;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let a = exp(-dot(in.uv, in.uv) * 5.0);
    return vec4(in.color * a * params.intensity, 0.0);
}
