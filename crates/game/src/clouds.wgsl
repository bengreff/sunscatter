// An opaque cloud deck's top (clouds.rs): a Lambert sphere of the deck's
// albedo, lit by the star's illuminance at the body.
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

struct DeckParams {
    // xyz: the body's centre relative to the camera.
    center: vec4<f32>,
    // xyz: the star relative to the camera; w: its illuminance at the body (lux).
    star: vec4<f32>,
    // rgb: Lambert albedo.
    albedo: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> deck: DeckParams;

const PI: f32 = 3.14159265;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xyz;
    let n = normalize(p - deck.center.xyz);
    let l = normalize(deck.star.xyz - p);
    let e = deck.star.w * max(dot(n, l), 0.0);
    return vec4(deck.albedo.rgb / PI * e * view.exposure, 1.0);
}
