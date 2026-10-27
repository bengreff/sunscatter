//! Starlight under a hazy atmosphere (Titan's haze, Venus's clouds): the
//! direct beam and the diffuse light reaching the ground, and the share
//! reaching the camera for the exposure (D087). From each body's
//! atmosphere data, for any body with `raymarch_inside`.

/// Sunlight under a hazy atmosphere (Titan's haze, Venus's clouds): the
/// vertical extinction optical depth per channel (the direct beam is
/// exp(−τ m(μ₀)) with the spherical-shell air mass m), the diffuse part
/// reaching the ground for an overhead Sun (a fraction of the light at the
/// top), and the radius over the extinction-weighted scale height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HazeLight {
    pub tau: [f32; 3],
    pub diffuse: [f32; 3],
    pub r_over_h: f32,
}

/// [`HazeLight`] from an atmosphere's data (one owner of the numbers: the
/// same coefficients the sky is drawn with). The diffuse part is a
/// diffusion estimate, not a radiative-transfer solution: the total
/// transmission of a slab, 1 / (1 + ¾ (1 − g) τ_s) for conservative
/// scattering (Eddington), damped by exp(−k τ) with the diffusion exponent
/// k = √(3 (1 − ω)(1 − ω g)) for absorption; minus the direct beam.
pub fn haze_light(def: &crate::body_visual::AtmosphereDef, radius: f64) -> HazeLight {
    haze_above(def, radius, 0.0)
}

/// The fraction of a profile's vertical optical depth above `h` (m).
fn share_above(profile: crate::body_visual::ProfileDef, h: f64) -> f64 {
    use crate::body_visual::ProfileDef;
    match profile {
        ProfileDef::Exponential { scale_height_m } => (-h.max(0.0) / f64::from(scale_height_m)).exp(),
        ProfileDef::Tent { center_m, width_m } => {
            let (c, w) = (f64::from(center_m), f64::from(width_m) / 2.0);
            let x = ((h - (c - w)) / w).clamp(0.0, 2.0);
            if x < 1.0 {
                1.0 - 0.5 * x * x
            } else {
                0.5 * (2.0 - x) * (2.0 - x)
            }
        }
    }
}

/// [`haze_light`] for the column above height `above_m`. Where the data
/// give the measured diffuse transmission at the ground (`ground_diffuse`),
/// the estimate is scaled to it at the ground, the factor fading upwards
/// with the share of the column's optical depth below the height (all of
/// it at the ground, none above the air): the measured value where it was
/// measured, the estimate's height profile elsewhere.
pub fn haze_above(def: &crate::body_visual::AtmosphereDef, radius: f64, above_m: f64) -> HazeLight {
    let mut out = estimate(def, radius, above_m);
    if let Some(measured) = def.ground_diffuse {
        let ground = estimate(def, radius, 0.0);
        for c in 0..3 {
            let (est0, tau0) = (f64::from(ground.diffuse[c]), f64::from(ground.tau[c]));
            if est0 <= 0.0 || tau0 <= 0.0 {
                continue;
            }
            let below = 1.0 - f64::from(out.tau[c]) / tau0;
            let factor = (f64::from(measured[c]) / est0).powf(1.0 - below.clamp(0.0, 1.0));
            let direct = (-f64::from(out.tau[c])).exp();
            out.diffuse[c] = (f64::from(out.diffuse[c]) * factor).clamp(0.0, 1.0 - direct) as f32;
        }
    }
    out
}

/// The diffusion estimate of [`haze_above`], uncalibrated.
fn estimate(def: &crate::body_visual::AtmosphereDef, radius: f64, above_m: f64) -> HazeLight {
    use crate::body_visual::ProfileDef;
    // Per channel: scattering and absorption optical depths, the
    // scattering-weighted asymmetry sum, and the extinction-weighted height.
    let (mut ts, mut ta, mut gs, mut th) = ([0.0f64; 3], [0.0f64; 3], [0.0f64; 3], [0.0f64; 3]);
    let mut add = |s: [f32; 3], a: [f32; 3], g: [f32; 3], profile: ProfileDef| {
        let (depth, height) = match profile {
            ProfileDef::Exponential { scale_height_m } => (f64::from(scale_height_m), f64::from(scale_height_m)),
            ProfileDef::Tent { center_m, width_m } => (f64::from(width_m) / 2.0, f64::from(center_m)),
        };
        let depth = depth * share_above(profile, above_m);
        for c in 0..3 {
            let (sc, ac) = (f64::from(s[c]) * depth, f64::from(a[c]) * depth);
            ts[c] += sc;
            ta[c] += ac;
            gs[c] += f64::from(g[c]) * sc;
            th[c] += (sc + ac) * height;
        }
    };
    let rayleigh = ProfileDef::Exponential { scale_height_m: def.rayleigh_scale_height_m };
    add(def.rayleigh_scattering, [0.0; 3], [0.0; 3], rayleigh);
    let mie = ProfileDef::Exponential { scale_height_m: def.mie_scale_height_m };
    add(def.mie_scattering, def.mie_absorption, def.mie_asymmetry_rgb.unwrap_or([def.mie_asymmetry; 3]), mie);
    if let Some(o) = &def.ozone {
        add([0.0; 3], o.absorption, [0.0; 3], ProfileDef::Tent { center_m: o.center_m, width_m: o.width_m });
    }
    for l in &def.layers {
        add(l.scattering, l.absorption, l.asymmetry, l.profile);
    }
    let mut out = HazeLight { tau: [0.0; 3], diffuse: [0.0; 3], r_over_h: 0.0 };
    let (mut tau_sum, mut h_sum) = (0.0, 0.0);
    for c in 0..3 {
        let tau = ts[c] + ta[c];
        let (omega, g) = if tau > 0.0 { (ts[c] / tau, gs[c] / ts[c].max(1e-30)) } else { (1.0, 0.0) };
        let k = (3.0 * (1.0 - omega) * (1.0 - omega * g)).max(0.0).sqrt();
        let total = (-k * tau).exp() / (1.0 + 0.75 * (1.0 - g) * ts[c]);
        out.tau[c] = tau as f32;
        out.diffuse[c] = (total - (-tau).exp()).clamp(0.0, 1.0) as f32;
        tau_sum += tau;
        h_sum += th[c];
    }
    out.r_over_h = if h_sum > 0.0 { (radius * tau_sum / h_sum) as f32 } else { 0.0 };
    out
}

/// Transmittance through a gas whose extinction is `sigma0` (m⁻¹) at the
/// datum, falling off exponentially with scale height `scale_h`, along a
/// straight path of length `d` between heights `z1` and `z2`: the optical
/// depth is σ₀ d times the mean of exp(−z/H) over the path (the shader's
/// fog under a deck mirrors it).
#[cfg_attr(not(test), allow(dead_code))] // the CPU mirror of terrain.wgsl's `deck_fog`
pub fn fog_transmittance(sigma0: f64, scale_h: f64, z1: f64, z2: f64, d: f64) -> f64 {
    if sigma0 <= 0.0 || d <= 0.0 || scale_h <= 0.0 {
        return 1.0;
    }
    let dz = z2 - z1;
    let mean = if dz.abs() > 1e-3 * scale_h {
        scale_h * ((-z1 / scale_h).exp() - (-z2 / scale_h).exp()) / dz
    } else {
        (-0.5 * (z1 + z2) / scale_h).exp()
    };
    (-sigma0 * d * mean).exp()
}

/// The sky's radiance under an opaque deck (lux/sr per channel): the
/// diffuse light reaching the camera's height spread evenly over the sky
/// (L = F/π, the deep, conservative medium's mean intensity), from each
/// star: `(tint × lux at the top, cosine of its zenith angle)`; a star
/// below the horizon gives none.
pub fn under_deck_radiance(diffuse: [f32; 3], stars: &[([f64; 3], f64)]) -> [f64; 3] {
    let mut l = [0.0; 3];
    for (light, mu) in stars {
        for c in 0..3 {
            l[c] += light[c] * mu.max(0.0) * f64::from(diffuse[c]) / std::f64::consts::PI;
        }
    }
    l
}

/// The radiance the gas under a deck scatters towards the camera (lux/sr):
/// its mean intensity J, the sky's radiance `sky` from above and the
/// ground's reflection of it (albedo `ground`) from below, each over half
/// the sphere: J = L (1 + A) / 2. Looking down through the gas, the ground
/// fades to J; the sky overhead is L.
pub fn fog_source(sky: [f64; 3], ground: [f32; 3]) -> [f64; 3] {
    [0, 1, 2].map(|c| sky[c] * (1.0 + f64::from(ground[c])) / 2.0)
}

/// The share of the starlight that reaches a camera under a haze, for the
/// exposure (D087: the flux at the camera): the luminance-weighted (Rec.
/// 709) direct and diffuse transmission of the column above it, for an
/// overhead Sun (so a night side stays dark: only the haze, not the Sun's
/// height, sets it).
pub fn haze_exposure_share(h: &HazeLight) -> f64 {
    let w = [0.2126, 0.7152, 0.0722];
    (0..3).map(|c| w[c] * ((-f64::from(h.tau[c])).exp() + f64::from(h.diffuse[c]))).sum()
}

/// The spherical-shell air mass towards zenith cosine `mu` (radius over
/// scale height `r_over_h`; 1 overhead, ~38 at the horizon for Earth's
/// 8 km): `terrain.wgsl`'s `star_lighting` mirrors it.
pub fn air_mass(r_over_h: f64, mu: f64) -> f64 {
    let mu = mu.clamp(-1.0, 1.0);
    (r_over_h * r_over_h * mu * mu + 2.0 * r_over_h + 1.0).sqrt() - r_over_h * mu
}

/// The direct beam's transmittance per channel from a camera `above_m` over
/// a body's mean radius towards a star at zenith cosine `mu`: the vertical
/// optical depth above it ([`haze_above`], every layer of the atmosphere's
/// data) times the air mass. A star below the horizon is the eclipse
/// rule's to hide.
pub fn direct_transmittance(def: &crate::body_visual::AtmosphereDef, radius: f64, above_m: f64, mu: f64) -> [f64; 3] {
    let h = haze_above(def, radius, above_m);
    let m = air_mass(f64::from(h.r_over_h), mu);
    h.tau.map(|t| (-f64::from(t) * m).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_direct_beam_through_the_air() {
        let earth = crate::body_visual::load("Earth").and_then(|d| d.atmosphere).expect("Earth");
        let r = 6.371e6;
        // (case, height, zenith cosine, green transmittance range)
        let cases = [
            ("noon at sea level: ~0.85", 0.0, 1.0, (0.75, 0.95)),
            ("the Sun 5° up: much less", 0.0, 0.087, (0.05, 0.5)),
            ("at 185 km: none lost", 185_000.0, 0.0, (0.999, 1.0)),
        ];
        for (case, h, mu, (lo, hi)) in cases {
            let t = direct_transmittance(&earth, r, h, mu);
            assert!(t[1] >= lo && t[1] <= hi, "{case}: {t:?}");
        }
        // Red gets through best near the horizon (the sunset's colour).
        let low = direct_transmittance(&earth, r, 0.0, 0.05);
        assert!(low[0] > low[1] && low[1] > low[2], "{low:?}");
        // The air mass: 1 overhead, ~38 at the horizon for R/H = 800.
        assert!((air_mass(800.0, 1.0) - 1.0).abs() < 1e-3);
        assert!((air_mass(800.0, 0.0) - 40.0).abs() < 1.0);
    }

    #[test]
    fn haze_light_from_the_atmosphere_data() {
        use crate::body_visual::AtmosphereDef;
        let load = |name: &str| crate::body_visual::load(name).and_then(|d| d.atmosphere).expect(name);
        let clear = AtmosphereDef { mie_scattering: [0.0; 3], mie_absorption: [0.0; 3], ..load("Titan") };
        let none = AtmosphereDef { rayleigh_scattering: [0.0; 3], layers: vec![], ..clear.clone() };
        // (case, atmosphere, vertical optical depth range, diffuse range at 550 nm)
        let cases = [
            ("no air: nothing scattered", none, (0.0, 1e-9), (0.0, 1e-9)),
            ("Titan's N2 alone (Rayleigh, conservative)", clear, (1.0, 1.3), (0.15, 0.35)),
            ("Titan with its haze: a few per cent arrive diffusely", load("Titan"), (4.0, 5.5), (0.02, 0.12)),
            ("Venus: the measured 3.4 % (Venera 11/12)", load("Venus"), (35.0, 60.0), (0.0339, 0.0341)),
        ];
        for (case, def, tau, diffuse) in cases {
            let h = haze_light(&def, 2.5e6);
            let (t, d) = (f64::from(h.tau[1]), f64::from(h.diffuse[1]));
            assert!(t >= tau.0 && t <= tau.1, "{case}: tau {t}");
            assert!(d >= diffuse.0 && d <= diffuse.1, "{case}: diffuse {d}");
        }
        // Venus's measured light is red: blue a quarter of red; the estimate
        // alone (without the measurement) gives under 1 %.
        let venus = haze_light(&load("Venus"), 6.05e6);
        assert!((venus.diffuse[0] - 0.049).abs() < 1e-6 && (venus.diffuse[2] - 0.013).abs() < 1e-6);
        let raw = haze_light(&AtmosphereDef { ground_diffuse: None, ..load("Venus") }, 6.05e6);
        assert!(raw.diffuse[1] < 0.01, "{:?}", raw.diffuse);
        // Blue is the most extinguished under Titan's haze, and the least transmitted.
        let titan = haze_light(&load("Titan"), 2.575e6);
        assert!(titan.tau[2] > titan.tau[1] && titan.tau[1] > titan.tau[0]);
        assert!(titan.diffuse[2] < titan.diffuse[0]);
        assert!(titan.r_over_h > 10.0 && titan.r_over_h < 200.0, "{}", titan.r_over_h);
    }

    #[test]
    fn fog_through_an_exponential_gas() {
        // (case, sigma0, H, z1, z2, d, expected transmittance)
        let e = std::f64::consts::E;
        let cases = [
            ("no gas", 0.0, 9e3, 0.0, 0.0, 5e3, 1.0),
            ("level at the datum: exp(-sigma d)", 1e-3, 9e3, 0.0, 0.0, 2e3, (-2.0f64).exp()),
            ("level one scale height up: e times thinner", 1e-3, 9e3, 9e3, 9e3, 2e3, (-2.0 / e).exp()),
            ("zero length", 1e-3, 9e3, 0.0, 0.0, 0.0, 1.0),
        ];
        for (case, s0, h, z1, z2, d, want) in cases {
            let t = fog_transmittance(s0, h, z1, z2, d);
            assert!((t - want).abs() < 1e-9, "{case}: {t}");
        }
        // A slant path equals the mean of its level parts (fine sum), and is
        // symmetric in its ends; nearly level paths are continuous.
        let (s0, h, d) = (1e-3, 9e3, 6e3);
        let n = 2000;
        let tau: f64 =
            (0..n).map(|i| s0 * d / f64::from(n) * (-(2e3 * (f64::from(i) + 0.5) / f64::from(n)) / h).exp()).sum();
        assert!((fog_transmittance(s0, h, 0.0, 2e3, d) - (-tau).exp()).abs() < 1e-6);
        assert_eq!(fog_transmittance(s0, h, 0.0, 2e3, d), fog_transmittance(s0, h, 2e3, 0.0, d));
        let near = fog_transmittance(s0, h, 1e3, 1e3 + 1e-3, d);
        assert!((near / fog_transmittance(s0, h, 1e3, 1e3, d) - 1.0).abs() < 1e-6);
        assert!(fog_transmittance(s0, h, -50.0, 1e3, d).is_finite());
    }

    #[test]
    fn the_sky_under_a_deck_is_the_diffuse_light_over_pi() {
        let diffuse = [0.049, 0.034, 0.013];
        // The Sun overhead at Venus (2601 W/m2 ~ 242,000 lux): L = F / pi.
        let sun = ([242_000.0; 3], 1.0);
        let l = under_deck_radiance(diffuse, &[sun]);
        assert!((l[1] / (242_000.0 * 0.034 / std::f64::consts::PI) - 1.0).abs() < 1e-6);
        assert!(l[0] > l[1] && l[1] > l[2], "reddish: {l:?}");
        // At 60 deg, half; below the horizon, none; two stars add.
        let half = under_deck_radiance(diffuse, &[([242_000.0; 3], 0.5)]);
        assert!((half[1] - 0.5 * l[1]).abs() < 1e-9);
        assert_eq!(under_deck_radiance(diffuse, &[([242_000.0; 3], -0.3)]), [0.0; 3]);
        let two = under_deck_radiance(diffuse, &[sun, ([1000.0, 500.0, 200.0], 1.0)]);
        assert!(two[0] > l[0] && two.iter().all(|c| c.is_finite()));
        assert_eq!(under_deck_radiance(diffuse, &[]), [0.0; 3]);
        // The gas's source: half the sky's radiance over a black ground, all
        // of it over a white one.
        assert_eq!(fog_source([2.0, 1.0, 0.5], [0.0; 3]), [1.0, 0.5, 0.25]);
        assert_eq!(fog_source([2.0, 1.0, 0.5], [1.0; 3]), [2.0, 1.0, 0.5]);
    }

    #[test]
    fn the_exposure_follows_the_light_under_a_haze() {
        let load = |name: &str| crate::body_visual::load(name).and_then(|d| d.atmosphere).expect(name);
        let venus = load("Venus");
        let share = |h: f64| haze_exposure_share(&haze_above(&venus, 6.05e6, h));
        // (case, height above the mean radius, expected share of the light at the top)
        let cases = [
            ("on Venus's ground: about 3.5 %, ~5 stops down", 0.0, (0.03, 0.04)),
            ("at the deck's top (tau ~ 1 of cloud): most of it", 74_000.0, (0.4, 0.9)),
            ("above the atmosphere: all of it", 2.0e5, (0.999, 1.0)),
        ];
        for (case, h, (lo, hi)) in cases {
            let s = share(h);
            assert!(s >= lo && s <= hi, "{case}: {s}");
        }
        // The light grows monotonically upwards.
        let mut last = 0.0;
        for i in 0..=40 {
            let s = share(f64::from(i) * 3000.0);
            assert!(s >= last, "at {} km", i * 3);
            last = s;
        }
        // A tent profile's share above a height: all below it, half at its centre, none above.
        use crate::body_visual::ProfileDef::Tent;
        let tent = Tent { center_m: 57_000.0, width_m: 46_000.0 };
        for (h, want) in
            [(0.0, 1.0), (34_000.0, 1.0), (57_000.0, 0.5), (74_000.0, 0.5 * (6.0f64 / 23.0).powi(2)), (90_000.0, 0.0)]
        {
            assert!((share_above(tent, h) - want).abs() < 1e-9, "{h}");
        }
    }
}
