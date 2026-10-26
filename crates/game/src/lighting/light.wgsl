// The lighting rules the shaders share (game::lighting, D055): eclipses of
// any star by any of a body's occluders. Terrain, rings and cloud decks
// import it, so there is one eclipse rule on the GPU, mirroring
// `sim::light::eclipse_factor`.
#define_import_path sunscatter::light

const PI: f32 = 3.14159265;

// Lights and occluders per material (lighting::MAX_STARS, MAX_OCCLUDERS).
const MAX_STARS: i32 = 4;
const MAX_OCCLUDERS: i32 = 4;
const MAX_SHINES: i32 = 2;

// lighting::LightUniform: one body's light.
struct Lights {
    // Per light slot: the star's centre (camera-relative), radius (w = 0: none).
    stars: array<vec4<f32>, 4>,
    // rgb: the star's light at the body's centre (lux); w: its flux there
    // over its flux at the camera (the slot's directional light's).
    star_light: array<vec4<f32>, 4>,
    // Bodies that can eclipse the stars here: centre, radius (w = 0: none).
    occluders: array<vec4<f32>, 4>,
    // Reflecting bodies: centre, w 1 (0: none); rgb their planetshine (lux).
    shines: array<vec4<f32>, 2>,
    shine_light: array<vec4<f32>, 2>,
    // x: starlight without a light slot, sphere-averaged (lux).
    ambient: vec4<f32>,
}

// Area of overlap of two discs (radii r1, r2, centres d apart); mirrors
// `sim::light::disc_overlap` (tested there).
fn disc_overlap(r1: f32, r2: f32, d: f32) -> f32 {
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= abs(r1 - r2) {
        let r = min(r1, r2);
        return PI * r * r;
    }
    let a1 = acos(clamp((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1), -1.0, 1.0));
    let a2 = acos(clamp((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2), -1.0, 1.0));
    return r1 * r1 * (a1 - sin(a1) * cos(a1)) + r2 * r2 * (a2 - sin(a2) * cos(a2));
}

// Fraction of the star's disc (`star`: centre, radius) visible from `p`
// past `occluders` (centre, radius; w = 0 unused): their hidden areas add
// up. The separation is atan2(|a × b|, a · b): acos loses ~3e-4 rad in f32
// near zero, a third of the Sun's radius seen from Jupiter.
fn eclipse(p: vec3<f32>, star: vec4<f32>, occluders: array<vec4<f32>, 4>) -> f32 {
    let to_star = star.xyz - p;
    let ds = length(to_star);
    if star.w <= 0.0 || ds <= star.w {
        return 1.0;
    }
    let s_ang = asin(clamp(star.w / ds, 0.0, 1.0));
    var hidden = 0.0;
    for (var i = 0; i < MAX_OCCLUDERS; i++) {
        let o = occluders[i];
        let to_c = o.xyz - p;
        let dc = length(to_c);
        if o.w <= 0.0 || dc <= o.w || dc >= ds || dot(to_c, to_star) <= 0.0 {
            continue;
        }
        let o_ang = asin(clamp(o.w / dc, 0.0, 1.0));
        let a = to_c / dc;
        let b = to_star / ds;
        let sep = atan2(length(cross(a, b)), dot(a, b));
        hidden += disc_overlap(s_ang, o_ang, sep);
    }
    return clamp(1.0 - hidden / (PI * s_ang * s_ang), 0.0, 1.0);
}
