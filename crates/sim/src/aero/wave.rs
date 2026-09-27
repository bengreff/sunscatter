//! Transonic and supersonic wave drag from the area distribution along the
//! flow (D070; FAR's approach).
//!
//! * **A(x) along the flow** (baked per grid direction, [`area_along`]):
//!   the cross-section area at stations along **d**, from the surface
//!   triangles by the divergence theorem (A(x) = −Σ A_t·(n_t·d) upstream
//!   of x), except that faces turned downstream more steeply than 45° (a
//!   base, the back of a blunt body) do not close it: the separated wake
//!   keeps their area. Behind the craft the wake runs on at constant area.
//! * **Wave drag** ([`wave_drag_area`]): von Kármán's slender-body integral
//!   D/q = −(1/2π)∬A″(x₁)A″(x₂)·ln|x₁−x₂| dx₁dx₂ (Ashley & Landahl,
//!   *Aerodynamics of Wings and Bodies*, §9.3) in its Fourier form, the
//!   series of A′ truncated at 16 terms (which smooths A(x) as FAR smooths
//!   its sections). A Sears–Haack body gives 9π·A_max²/(2L²).
//! * **Mach dependence** ([`mach_factor`]): none below M 0.8, rising
//!   smoothly to full at M 1 (drag divergence), full above (the linear
//!   theory's wave drag of a slender body does not depend on Mach), faded
//!   into Newtonian with the rest of the subsonic model by M 5. Capped at
//!   [`WAVE_CAP`] × the projected area: slender-body theory diverges for
//!   blunt shapes, whose drag rise is bounded (Apollo's C_D rises ≈ 0.5
//!   from M 0.8 to 1.2; a sphere's about as much).

use crate::math;
use glam::DVec3;

/// Stations along the flow.
pub const STATIONS: usize = 64;
/// Largest wave-drag coefficient on the projected area.
pub const WAVE_CAP: f64 = 0.5;
/// Faces turned downstream beyond this (n·d above) are separated.
const SEPARATED: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Cross-section areas along `d` at STATIONS + 1 boundaries over `lo..hi`
/// (positions along `d`), from triangles (centroid, unit normal, area).
pub fn area_along(tris: &[(DVec3, DVec3, f64)], d: DVec3, lo: f64, hi: f64) -> Vec<f64> {
    let dx = ((hi - lo) / STATIONS as f64).max(1e-9);
    let mut bins = vec![0.0; STATIONS];
    for &(c, n, a) in tris {
        let nd = n.dot(d);
        if nd > SEPARATED {
            continue;
        }
        let k = ((c.dot(d) - lo) / dx).floor().clamp(0.0, (STATIONS - 1) as f64) as usize;
        bins[k] -= a * nd;
    }
    let mut area = Vec::with_capacity(STATIONS + 1);
    let mut sum = 0.0;
    area.push(0.0);
    for b in bins {
        sum += b;
        area.push(sum.max(0.0));
    }
    area
}

/// Terms of the sine series of A′ (the rest is discretisation noise,
/// amplified by n: a Sears–Haack body is exact to 1 % with 16).
const TERMS: usize = 16;
/// Quadrature points in θ per station.
const SAMPLES_PER_STATION: usize = 4;

/// Wave drag per unit q (m²) of the area distribution `area` (station
/// boundaries `dx` apart; the wake continues at the last value): with
/// x = (l/2)(1 − cos θ) and A′(x) = Σ aₙ·sin nθ, D/q = (π/4)·Σ n·aₙ²
/// (the von Kármán integral in Fourier form; Ashley & Landahl §9.3). A′
/// between the stations' midpoints is linear; aₙ by the midpoint rule in θ
/// (sin nθ by recurrence).
pub fn wave_drag_area(area: &[f64], dx: f64) -> f64 {
    let n = area.len().saturating_sub(1);
    if n == 0 || dx <= 0.0 {
        return 0.0;
    }
    let slope: Vec<f64> = area.windows(2).map(|w| (w[1] - w[0]) / dx).collect();
    let l = dx * n as f64;
    let samples = SAMPLES_PER_STATION * n;
    let mut a = [0.0; TERMS + 1];
    for j in 0..samples {
        let theta = (j as f64 + 0.5) * math::PI / samples as f64;
        let (sin, cos) = (math::sin(theta), math::cos(theta));
        let u = 0.5 * l * (1.0 - cos) / dx - 0.5;
        let s = if u <= 0.0 {
            slope[0]
        } else if u >= (n - 1) as f64 {
            slope[n - 1]
        } else {
            let k = u.floor() as usize;
            let f = u - k as f64;
            slope[k] * (1.0 - f) + slope[k + 1] * f
        };
        // sin nθ: sin(n+1)θ = 2cosθ·sin nθ − sin(n−1)θ.
        let (mut prev, mut cur) = (0.0, sin);
        for an in a.iter_mut().skip(1) {
            *an += s * cur;
            let next = 2.0 * cos * cur - prev;
            (prev, cur) = (cur, next);
        }
    }
    let scale = 2.0 / samples as f64;
    math::PI / 4.0 * (1..=TERMS).map(|k| k as f64 * (a[k] * scale) * (a[k] * scale)).sum::<f64>()
}

/// Share of the wave drag at Mach `mach`: 0 below 0.8, smoothstep to 1 at
/// M 1, 1 above.
pub fn mach_factor(mach: f64) -> f64 {
    let x = ((mach - 0.8) / 0.2).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
