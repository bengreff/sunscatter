//! Fitting a moon system's initial states to JPL Horizons over the window.
//!
//! The states Horizons gives at the epoch carry its force model (higher
//! harmonics, tides, its own constants); integrated with ours they drift
//! along their orbits (a mean-motion offset of 1e-7 to 3e-6). Each moon's
//! mean motion and phase are corrected so that our integration follows
//! Horizons' along-track over observation epochs spanning the table window:
//! the along-track residual at each epoch `δs_k` (projected on our
//! velocity) is fitted with `c0 + c1 t` by least squares; the moon is moved
//! along its orbit by `c0 / a` and its semi-major axis scaled by
//! `1 − (2/3) c1 / (a n)` (so its mean motion gains `c1 / a`), keeping its
//! other elements, and the primary is placed so the barycenter stays at the
//! origin. A few rounds of this (each one integration of the system)
//! converge; the moons are corrected together (Jacobi-style), which also
//! handles resonances. Only the drift is fitted; radial and cross-track
//! residuals are reported.

use crate::horizons::{Query, EPOCH_JD};
use glam::DVec3;
use sim::body::BodyPhysical;
use sim::ephem::Ephemeris;
use sim::gen::InitialBody;
use sim::kepler::Elements;
use sim::sol::systems::{integrate_samples, SystemSpec};
use std::sync::Arc;

/// Observation epochs: every 913 days (2.5 years) from the epoch, 21 of
/// them (to 49.99 years), whole days so every system's step divides them.
pub fn fit_days() -> Vec<i64> {
    (0..21).map(|k| k * 913).collect()
}

/// Member `i`'s Horizons states (relative to the system's centre) at the
/// observation epochs.
pub fn fit_query(spec: &SystemSpec, i: usize) -> Query {
    let jds = fit_days().iter().map(|&d| EPOCH_JD + d as f64).collect();
    Query { command: spec.members[i].horizons.clone(), center: spec.horizons_center.clone(), jds }
}

/// Directory of JPL satellite SPK kernels (gitignored), the fallback when
/// Horizons' answers are not cached: the same satellite ephemerides
/// (JUP365, SAT441, NEP097/NEP105, PLU060) that Horizons serves.
pub const KERNEL_DIR: &str = "data/external/kernels";

/// Each moon's position relative to the primary at the observation
/// epochs (`None` for the primary and for moons without data): from the
/// Horizons cache if present, else from the kernels in [`KERNEL_DIR`]. None
/// if there is nothing for the primary.
pub fn observations(spec: &SystemSpec) -> Option<Vec<Option<Vec<DVec3>>>> {
    let n = spec.members.len();
    if (0..n).all(|i| fit_query(spec, i).file().exists()) {
        let all: Vec<Vec<(DVec3, DVec3)>> = (0..n).map(|i| fit_query(spec, i).states()).collect();
        let rel = |m: &Vec<(DVec3, DVec3)>| m.iter().zip(&all[0]).map(|(x, p)| x.0 - p.0).collect();
        return Some(all.iter().enumerate().map(|(i, m)| (i > 0).then(|| rel(m))).collect());
    }
    let mut almanac = anise::prelude::Almanac::default();
    let mut loaded = 0;
    for entry in std::fs::read_dir(KERNEL_DIR).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "bsp") {
            almanac = almanac.load(path.to_str()?).ok()?;
            loaded += 1;
        }
    }
    if loaded == 0 {
        return None;
    }
    let ids: Vec<i32> = spec.members.iter().map(|m| m.horizons.parse().ok()).collect::<Option<_>>()?;
    let primary = anise::prelude::Frame::from_ephem_j2000(ids[0]);
    let moon = |id: i32| -> Option<Vec<DVec3>> {
        fit_days()
            .iter()
            .map(|&d| {
                let jd = EPOCH_JD + d as f64;
                let e = anise::prelude::Epoch::from_tdb_seconds((jd - 2_451_545.0) * 86_400.0);
                let s = almanac.translate(anise::prelude::Frame::from_ephem_j2000(id), primary, e, None).ok()?;
                Some(DVec3::new(s.radius_km.x, s.radius_km.y, s.radius_km.z) * 1e3)
            })
            .collect()
    };
    let out: Vec<Option<Vec<DVec3>>> =
        ids.iter().enumerate().map(|(i, &id)| if i == 0 { None } else { moon(id) }).collect();
    if out.iter().all(Option::is_none) {
        return None;
    }
    println!("{}: observations from the SPK kernels in {KERNEL_DIR}", spec.barycenter);
    Some(out)
}

/// Per moon: its name and the worst position error about the primary over
/// the observation epochs, before and after the fit (m).
pub struct FitLine {
    pub name: String,
    pub before: f64,
    pub after: f64,
}

/// Errors of each moon about the primary at the observation epochs, and
/// the along-track fit `(c0, c1)` of each.
fn residuals(
    spec: &SystemSpec,
    bodies: &[InitialBody],
    planets: &Arc<Ephemeris>,
    primary: &BodyPhysical,
    obs: &[Option<Vec<DVec3>>],
) -> (Vec<f64>, Vec<(f64, f64)>) {
    let days = fit_days();
    let samples = integrate_samples(spec, bodies, planets, primary, &days);
    let mut worst = vec![0.0; bodies.len()];
    let mut fits = vec![(0.0, 0.0); bodies.len()];
    for m in 1..bodies.len() {
        let Some(obs_m) = &obs[m] else { continue };
        let (mut st, mut ss, mut stt, mut sts, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (k, &d) in days.iter().enumerate() {
            let ours = samples[k][m].0 - samples[k][0].0;
            let vel = samples[k][m].1 - samples[k][0].1;
            let res = obs_m[k] - ours;
            worst[m] = f64::max(worst[m], res.length());
            let ds = res.dot(vel.normalize());
            let t = d as f64 * 86_400.0;
            (st, ss, stt, sts, n) = (st + t, ss + ds, stt + t * t, sts + t * ds, n + 1.0);
        }
        let c1 = (n * sts - st * ss) / (n * stt - st * st);
        let c0 = (ss - c1 * st) / n;
        fits[m] = (c0, c1);
    }
    (worst, fits)
}

/// Moves each moon along its orbit and changes its mean motion by its
/// fit, then re-centres the members on the barycenter.
fn correct(bodies: &mut [InitialBody], fits: &[(f64, f64)]) {
    let (rp, vp) = (bodies[0].r, bodies[0].v);
    let gm_p = bodies[0].gm;
    let mut rel: Vec<(DVec3, DVec3)> = Vec::new();
    for (b, &(c0, c1)) in bodies.iter().zip(fits).skip(1) {
        let mu = gm_p + b.gm;
        let mut el = Elements::from_state(b.r - rp, b.v - vp, mu);
        let n = el.mean_motion(mu);
        el.mean_anomaly = sim::math::wrap_tau(el.mean_anomaly + c0 / el.a);
        el.a *= 1.0 - (2.0 / 3.0) * c1 / (el.a * n);
        rel.push(el.to_state(mu));
    }
    let total: f64 = bodies.iter().map(|b| b.gm).sum();
    let (sr, sv) = rel
        .iter()
        .zip(&bodies[1..])
        .fold((DVec3::ZERO, DVec3::ZERO), |(sr, sv), ((r, v), b)| (sr + *r * (b.gm / total), sv + *v * (b.gm / total)));
    let (r0, v0) = (-sr, -sv);
    bodies[0].r = r0;
    bodies[0].v = v0;
    for (b, (r, v)) in bodies[1..].iter_mut().zip(rel) {
        b.r = r0 + r;
        b.v = v0 + v;
    }
}

/// Fits `bodies` (barycentric, at the epoch) to the Horizons states `obs`
/// (each moon about the primary at the observation epochs; a moon without
/// them is left as it is) in `rounds` rounds, keeping the best. Returns the
/// per-moon report (worst errors of 0 for moons without observations).
pub fn fit_system(
    spec: &SystemSpec,
    bodies: &mut Vec<InitialBody>,
    planets: &Arc<Ephemeris>,
    primary: &BodyPhysical,
    obs: &[Option<Vec<DVec3>>],
    rounds: usize,
) -> Vec<FitLine> {
    let (first, mut fits) = residuals(spec, bodies, planets, primary, obs);
    let score = |w: &[f64]| w.iter().map(|e| e * e).sum::<f64>();
    let (mut best, mut best_bodies, mut best_score) = (first.clone(), bodies.clone(), score(&first));
    for round in 0..rounds {
        correct(bodies, &fits);
        let (worst, next) = residuals(spec, bodies, planets, primary, obs);
        println!(
            "  {} round {}: worst {:.3e} m",
            spec.barycenter,
            round + 1,
            worst.iter().copied().fold(0.0, f64::max)
        );
        let s = score(&worst);
        if s < best_score {
            (best, best_bodies, best_score) = (worst, bodies.clone(), s);
        }
        fits = next;
    }
    *bodies = best_bodies;
    spec.members
        .iter()
        .enumerate()
        .skip(1)
        .map(|(m, s)| FitLine { name: s.name.clone(), before: first[m], after: best[m] })
        .collect()
}
