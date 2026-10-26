// An opaque cloud deck's top (clouds.rs): a Lambert sphere of the deck's
// albedo, lit by every drawn star at its illuminance at the body, each
// dimmed where another body hides it, plus planetshine.
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}
#import sunscatter::light::{Lights, eclipse, MAX_STARS, MAX_SHINES}

struct DeckParams {
    // xyz: the body's centre relative to the camera.
    center: vec4<f32>,
    // rgb: Lambert albedo.
    albedo: vec4<f32>,
    // The body's light (lighting::LightUniform).
    starlight: Lights,
    // From below the deck's top: rgb the sky's radiance (lux/sr), w 1.
    inside: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> deck: DeckParams;

const PI: f32 = 3.14159265;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // From below, the shell is the sky: the diffuse light under the deck.
    if deck.inside.w > 0.0 {
        return vec4(deck.inside.rgb * view.exposure, 1.0);
    }
    let p = in.world_position.xyz;
    let n = normalize(p - deck.center.xyz);
    var e = vec3(0.0);
    for (var k = 0; k < MAX_STARS; k++) {
        let star = deck.starlight.stars[k];
        if star.w <= 0.0 {
            continue;
        }
        let l = normalize(star.xyz - p);
        e += deck.starlight.star_light[k].rgb * max(dot(n, l), 0.0) * eclipse(p, star, deck.starlight.occluders);
    }
    for (var m = 0; m < MAX_SHINES; m++) {
        let sh = deck.starlight.shines[m];
        if sh.w > 0.0 {
            e += deck.starlight.shine_light[m].rgb * max(dot(n, normalize(sh.xyz - p)), 0.0);
        }
    }
    e += vec3(deck.starlight.ambient.x);
    return vec4(deck.albedo.rgb / PI * e * view.exposure, 1.0);
}
