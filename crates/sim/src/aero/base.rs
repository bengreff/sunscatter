//! Base drag against Mach number (D070): the suction on the craft's bases
//! (faces turned downstream more steeply than 45°, separated: [`DirSums::base`]),
//! power off.
//!
//! * **Base pressure** −C_pb(M) of a blunt-based body of revolution with a
//!   turbulent boundary layer: subsonic ≈ 0.12 (Hoerner's 0.029/√C_D,forebody
//!   for typical forebodies, as printed in NASA H-2287, eq. 14); transonic
//!   from Peck's free-flight cylindrical body (Katz & Stoney, NACA RM
//!   L51F29, fig. 6a: 0.125 at M 0.8 to a peak of 0.22 just above M 1);
//!   supersonic Love's compilation (NACA TN 3819, fig. 18: 0.176 at M 1.5,
//!   0.147 at 2, 0.097 at 3, 0.048 at 5, 0.021 at 8; Chapman's curve within
//!   0.005), beyond M 8 the vacuum limit's 1/M² scaled to join it. Read from
//!   the figures (±0.005).
//! * **In the force model:** the craft's subsonic drag coefficient Cd₀
//!   already holds its base drag (at the subsonic −C_pb), and the
//!   pressure model fades from it to Newtonian between M 1.2 and 5
//!   (Newtonian has no base suction). [`correction`] adds what is missing
//!   so the base carries −C_pb(M)·q·A_base at every Mach number: none
//!   subsonic, the transonic rise, the base drag that Newtonian leaves out
//!   above M 5. Fins, a short afterbody or a jet change C_pb (TN 3819 figs.
//!   19, 28): not modelled.
//!
//! [`DirSums::base`]: super::DirSums::base

/// −C_pb against M.
const BASE_PRESSURE: [(f64, f64); 18] = [
    (0.0, 0.12),
    (0.6, 0.12),
    (0.8, 0.125),
    (0.9, 0.135),
    (0.95, 0.165),
    (1.0, 0.21),
    (1.02, 0.22),
    (1.1, 0.21),
    (1.5, 0.176),
    (2.0, 0.147),
    (2.5, 0.118),
    (3.0, 0.097),
    (3.5, 0.081),
    (4.0, 0.067),
    (5.0, 0.048),
    (6.0, 0.036),
    (7.0, 0.027),
    (8.0, 0.021),
];

/// The subsonic base pressure the craft's Cd₀ holds.
pub const SUBSONIC: f64 = 0.12;

/// −C_pb at Mach `mach`.
pub fn pressure(mach: f64) -> f64 {
    let t = &BASE_PRESSURE;
    let m = mach.max(0.0);
    let last = t[t.len() - 1];
    if m >= last.0 {
        return last.1 * (last.0 / m) * (last.0 / m);
    }
    let k = t.partition_point(|e| e.0 <= m);
    let (a, b) = (t[k - 1], t[k]);
    a.1 + (b.1 - a.1) * (m - a.0) / (b.0 - a.0)
}

/// The share of the subsonic (Cd₀) model left in the pressure model at
/// Mach `mach`: 1 to M 1.2, falling linearly to 0 at M 5.
pub fn subsonic_share(mach: f64) -> f64 {
    ((super::HYPERSONIC_MACH - mach) / (super::HYPERSONIC_MACH - 1.2)).clamp(0.0, 1.0)
}

/// The base-pressure coefficient to add to the pressure model at Mach
/// `mach` (on the base's projected area): −C_pb(M) less what Cd₀ already
/// carries there.
pub fn correction(mach: f64) -> f64 {
    pressure(mach) - SUBSONIC * subsonic_share(mach)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_pressure_follows_peck_and_love() {
        for (m, want) in
            [(0.5, 0.12), (0.8, 0.125), (1.02, 0.22), (2.0, 0.147), (3.0, 0.097), (5.0, 0.048), (8.0, 0.021)]
        {
            assert!((pressure(m) - want).abs() < 1e-12, "M {m}");
        }
        // Beyond M 8 toward the vacuum limit 2/(γM²) (0.0223 at M 8).
        assert!((pressure(16.0) - 0.021 / 4.0).abs() < 1e-12);
        assert!(pressure(8.0) < 2.0 / (1.4 * 64.0));
        // Continuous, and nothing added where Cd₀ holds the base drag.
        for k in 0..400 {
            let m = k as f64 * 0.025;
            assert!((pressure(m + 1e-9) - pressure(m)).abs() < 1e-6, "M {m}");
        }
        assert_eq!(correction(0.3), 0.0);
        assert!((correction(1.0) - 0.09).abs() < 1e-12);
        assert!((correction(5.0) - 0.048).abs() < 1e-12);
    }
}
