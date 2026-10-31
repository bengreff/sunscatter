//! Installs our copies of Bevy's atmosphere shaders over the embedded
//! originals, and again if Bevy (re)loads an original: the sky compositing
//! (`render_sky.wgsl`) and the aerial-view table (`aerial_view_lut.wgsl`,
//! which from outside the atmosphere holds the table from space the
//! compositing reads; see their headers).

use bevy::prelude::*;
use bevy::shader::{Shader, Source};

/// (Bevy's embedded path, our source, the marker that tells ours apart).
const OVERRIDES: [(&str, &str, &str); 2] = [
    ("embedded://bevy_pbr/atmosphere/render_sky.wgsl", include_str!("render_sky.wgsl"), "sunscatter sky override"),
    (
        "embedded://bevy_pbr/atmosphere/aerial_view_lut.wgsl",
        include_str!("aerial_view_lut.wgsl"),
        "sunscatter aerial view override",
    ),
];

/// The stretch of a ray in the air before the geometry it hits, from a
/// camera outside the air (`render_sky.wgsl`'s `ground_from_space`, which
/// mirrors this): `r` the camera's distance from the body's centre, `mu`
/// the cosine of the ray's angle from the camera's vertical, `t` the
/// geometry's distance along it, the air between radii `inner` (the ground)
/// and `outer`. `None` when the ray meets the geometry before the air (a
/// craft above the air seen from orbit) or never enters the air: nothing
/// between, no aerial perspective. The air's entry and exit are Bevy's
/// `get_raymarch_segment` from outside.
#[cfg_attr(not(test), allow(dead_code))]
pub fn air_before(r: f64, mu: f64, t: f64, inner: f64, outer: f64) -> Option<(f64, f64)> {
    // Bevy's `ray_sphere_intersect`: the distances to a sphere along the
    // ray, none when it misses.
    let hits = |radius: f64| {
        let disc = r * r * (mu * mu - 1.0) + radius * radius;
        (disc >= 0.0).then(|| (-r * mu - disc.sqrt(), -r * mu + disc.sqrt()))
    };
    let (start, exit) = hits(outer).filter(|h| h.0 >= 0.0)?;
    // Bevy's `ray_intersects_ground`: the ray goes down and meets the
    // ground sphere.
    let end = match hits(inner) {
        Some((ground, _)) if mu < 0.0 => ground,
        _ => exit,
    };
    let end = end.min(t);
    (end > start).then_some((start, end))
}

/// Keeps our atmosphere shaders installed.
pub fn install(assets: Res<AssetServer>, mut shaders: ResMut<Assets<Shader>>, mut handles: Local<Vec<Handle<Shader>>>) {
    if handles.is_empty() {
        *handles = OVERRIDES.iter().map(|(path, _, _)| assets.load(*path)).collect();
    }
    for (handle, (path, source, marker)) in handles.iter().zip(OVERRIDES) {
        let Some(current) = shaders.get(handle) else { continue };
        if matches!(&current.source, Source::Wgsl(s) if s.contains(marker)) {
            continue;
        }
        let mut shader = Shader::from_wgsl(source, path);
        shader.shader_defs.clone_from(&current.shader_defs);
        let _ = shaders.insert(handle, shader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_shaders_carry_their_markers_and_no_placeholders() {
        for (path, source, marker) in OVERRIDES {
            assert!(source.contains(marker), "{path}");
            assert!(!source.contains("{{"), "{path}");
        }
    }

    #[test]
    fn geometry_before_the_air_takes_no_aerial_perspective() {
        // Earth's ground and the top of its air (m), the camera in orbit.
        let (inner, outer) = (6.371e6, 6.471e6);
        let r = inner + 343e3;
        let deg = |e: f64| (90.0 - e).to_radians().cos();
        // The ground straight below: 343 km away, its air from 243 km.
        let below = air_before(r, -1.0, 343e3, inner, outer).unwrap();
        // (case, elevation of the ray (deg), the geometry's distance (m), in the air)
        let cases = [
            // DOCS-3's shot: the second stage 40 m away, the camera below
            // it looking up (the table read the light back down through the
            // planet and drew the craft dark red above a line).
            ("a craft above, looking up 20°", 20.0, 40.0, false),
            ("a craft above, looking up 60°", 60.0, 40.0, false),
            ("a craft beside, level", 0.0, 40.0, false),
            ("a craft below, looking down 30°", -30.0, 40.0, false),
            ("the ground below", -90.0, 343e3, true),
            ("the ground at a slant", -40.0, 600e3, true),
            ("a booster in the air, seen from orbit", -60.0, 330e3, true),
            ("a ray over the limb, missing the air", -10.0, 1e7, false),
            ("a craft beyond the limb, the ray through the air", -18.0, 4e6, true),
        ];
        for (case, e, t, want) in cases {
            let got = air_before(r, deg(e), t, inner, outer);
            assert_eq!(got.is_some(), want, "{case}: {got:?}");
            if let Some((start, end)) = got {
                assert!(start >= 0.0 && start < end && end <= t, "{case}: {got:?}");
            }
        }
        assert!((below.0 - 243e3).abs() < 1.0 && (below.1 - 343e3).abs() < 1.0, "{below:?}");
    }
}
