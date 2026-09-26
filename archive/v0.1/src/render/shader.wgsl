// Camera uniform for view transformation
struct CameraUniform {
    position: vec2<f32>,      // Camera center in world space (unused in shader)
    zoom: f32,                // Zoom level
    aspect_ratio: f32,        // Window aspect ratio
    rotation: f32,            // Camera rotation in radians
    fine_offset_x: f32,       // Ship offset X from SOI body center (applied here for precision)
    fine_offset_y: f32,       // Ship offset Y from SOI body center
    _pad0: f32,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var body_texture: texture_2d_array<f32>;
@group(1) @binding(1)
var body_sampler: sampler;

@group(2) @binding(0)
var sprite_texture: texture_2d<f32>;
@group(2) @binding(1)
var sprite_sampler: sampler;

// Vertex shader

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform camera-relative position to clip space:
    // Positions are relative to SOI body center (computed in CPU for f64 precision).
    // The fine_offset (ship's offset from body center) is subtracted here in f32
    // to avoid adding a small value to a galaxy-scale f64 on the CPU.
    let pos = in.position - vec2<f32>(camera.fine_offset_x, camera.fine_offset_y);
    let cos_r = cos(camera.rotation);
    let sin_r = sin(camera.rotation);
    let rotated = vec2<f32>(
        pos.x * cos_r - pos.y * sin_r,
        pos.x * sin_r + pos.y * cos_r,
    );
    let view_pos = rotated * camera.zoom;
    let corrected_x = view_pos.x / camera.aspect_ratio;

    out.clip_position = vec4<f32>(corrected_x, view_pos.y, 0.0, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}

// Fragment shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sprite: uv.x >= 2.0 flags sprite atlas sampling
    if in.uv.x >= 2.0 {
        let sprite_uv = in.uv - vec2<f32>(2.0, 0.0);
        let tex_color = textureSample(sprite_texture, sprite_sampler, sprite_uv);
        if tex_color.a < 0.01 { discard; }
        return tex_color * in.color;
    }

    // Textured body: uv is non-zero, layer index stored in color.a
    if in.uv.x + in.uv.y > 0.0 {
        let layer = i32(in.color.a + 0.5);
        return textureSample(body_texture, body_sampler, in.uv, layer);
    }

    // Atmosphere vertices use negative alpha as a flag.
    // Alpha encodes -(1 + t) where t is 0 at surface, 1 at edge.
    // The GPU linearly interpolates alpha from -1 (surface) to -2 (edge).
    if in.color.a < 0.0 {
        let t = -in.color.a - 1.0;
        let brightness = exp(-6.0 * t);
        return vec4<f32>(in.color.r * brightness, in.color.g * brightness, in.color.b * brightness, 1.0);
    }
    return in.color;
}
