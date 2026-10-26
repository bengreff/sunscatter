//! The inputs of a coast thermal step ([`super::aerothermal::coast_step`]):
//! sunlight and thin air in body axes, averaged over the turn of the
//! craft's attitude against them where the attitude is known
//! (`super::pointing`): once round a short orbit for a craft held in its
//! orbital frame (or holding still while the air turns), over all
//! orientations for a fast tumbler. Coast temperatures are orbit averages
//! (the eclipse swing is not shown); averaging the attitude's turn the
//! same way keeps the inputs steady, so the lattice skips like a still
//! craft's.

use super::aerothermal::{coast_sunlight, LATTICE, ORBIT_AVERAGE_PERIOD};
use crate::aero;
use crate::ephem::Snapshot;
use crate::frame::NodeId;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// Samples of a coast step's average over an orbit ([`coast_suns`]).
const ORBIT_SAMPLES: usize = 24;

/// The sunlight a coast step takes, in body axes: one vector, or samples
/// whose heating is averaged; `key` stands for it in the skip test (the
/// lattice skips while the key holds within 1 %).
#[derive(Clone, Debug, PartialEq)]
pub struct CoastSun {
    pub suns: Vec<DVec3>,
    pub key: DVec3,
    /// Thin air ([`coast_air`]): samples of the direction it travels
    /// relative to the craft (body axes, unit) and its heat flux (W/m²,
    /// facing it), averaged like the sunlight; empty in vacuum.
    pub air: Vec<(DVec3, f64)>,
}

impl CoastSun {
    pub fn one(sun_body: DVec3) -> Self {
        CoastSun { suns: vec![sun_body], key: sun_body, air: Vec::new() }
    }

    /// The air's part of the skip test: the mean flux and the size of the
    /// mean of direction × flux (its anisotropy in body axes).
    pub(super) fn air_key(&self) -> DVec3 {
        if self.air.is_empty() {
            return DVec3::ZERO;
        }
        let n = self.air.len() as f64;
        let flux = self.air.iter().map(|a| a.1).sum::<f64>() / n;
        let along = self.air.iter().map(|&(d, f)| d * f).sum::<DVec3>() / n;
        DVec3::new(flux, along.length(), 0.0)
    }
}

/// The free-molecular heat flux of the air at `(r, v)` relative to
/// `anchor` (W/m² facing it, and the direction it travels, inertial):
/// ½ρv³ (full accommodation: every molecule gives up its kinetic energy),
/// bridged by Wilmoth's weight in the Knudsen number for `length` (towards
/// the far smaller continuum flux).
fn air_flux(world: &World, snap: &Snapshot, anchor: NodeId, (r, v): (DVec3, DVec3), length: f64) -> (DVec3, f64) {
    let mut out = (DVec3::ZERO, 0.0);
    super::limbo::each_air(world, snap, anchor, r, v, |air| {
        let speed = air.v_rel.length();
        let flux = 0.5 * air.rho * speed * speed * speed * aero::bridge(air.mean_free_path / length);
        if out.1 == 0.0 && speed > 0.0 {
            out = (-air.v_rel / speed, flux);
        }
    });
    out
}

/// The thin air's heating of a coast above the live boundary at `t`, for a
/// vessel at `(r, v)` relative to `anchor` with attitude `att`, its segment
/// held by `pointing` (if known), the design's Knudsen length `length`:
/// in low orbit 60 W/m² at 200 km, up to the live boundary's 1 kW/m²
/// ([`super::limbo::LiveBoundary`]); nothing under 0.1 W/m² (above
/// ~600 km). Above 1 W/m² on a short bound orbit with a known attitude, it
/// is averaged over the osculating orbit like the sunlight (fixed phases,
/// the pointing's attitude at each); a fast tumbler takes it isotropically.
pub fn coast_air(
    world: &World,
    t: Epoch,
    (anchor, r, v): (NodeId, DVec3, DVec3),
    pointing: Option<&super::pointing::Pointing>,
    att: &super::Attitude,
    length: f64,
) -> Vec<(DVec3, f64)> {
    use super::pointing::Pointing;
    let snap = world.snapshot(t);
    let (dir, flux) = air_flux(world, &snap, anchor, (r, v), length);
    if flux <= 0.1 {
        return Vec::new();
    }
    match pointing {
        Some(Pointing::Free { .. }) if att.omega.length() * LATTICE > 2.0 * crate::math::PI => {
            return icosahedron().into_iter().map(|d| (d, flux)).collect();
        }
        Some(p) if flux > 1.0 => {
            let body = world.source(anchor);
            let el = body.map(|b| crate::kepler::Elements::from_state(r, v, b.gm));
            if let (Some(b), Some(el)) = (body, el) {
                if el.e < 1.0 && el.period(b.gm) <= ORBIT_AVERAGE_PERIOD {
                    // Half the sunlight's samples: the air has no shadow edge.
                    let n = ORBIT_SAMPLES / 2;
                    let samples: Option<Vec<_>> = (0..n)
                        .map(|k| {
                            let m = 2.0 * crate::math::PI * (k as f64 + 0.5) / n as f64 - el.argp;
                            let (rk, vk) = crate::kepler::Elements { mean_anomaly: m, ..el }.to_state(b.gm);
                            let q = p.attitude(world, &snap, anchor, rk, vk)?.q;
                            let (d, f) = air_flux(world, &snap, anchor, (rk, vk), length);
                            Some((q.inverse() * d, f))
                        })
                        .collect();
                    if let Some(s) = samples {
                        return s;
                    }
                }
            }
        }
        _ => {}
    }
    vec![(att.q.inverse() * dir, flux)]
}

/// The 12 directions of an icosahedron (an isotropic average).
fn icosahedron() -> Vec<DVec3> {
    let phi = 0.5 * (1.0 + 5.0f64.sqrt());
    let mut out = Vec::with_capacity(12);
    for (a, b) in [(1.0, phi), (1.0, -phi), (-1.0, phi), (-1.0, -phi)] {
        for d in [DVec3::new(0.0, a, b), DVec3::new(a, b, 0.0), DVec3::new(b, 0.0, a)] {
            out.push(d.normalize());
        }
    }
    out
}

/// The sunlight of a coast step at `t` in body axes, for a vessel at
/// `(r, v)` relative to `anchor` with attitude `att`, its segment held by
/// `pointing` (`super::pointing`, if the attitude is known):
/// * held in an orbital frame on a bound orbit of at most
///   [`ORBIT_AVERAGE_PERIOD`] clear of its body (a prograde hold in low
///   orbit): averaged over one revolution of the osculating orbit, which
///   turns the craft once against the Sun: [`ORBIT_SAMPLES`] points at
///   fixed mean anomalies, each in the pointing's attitude there, dark in
///   the body's cylindrical shadow (as `light::orbit_sunlit_fraction`); the
///   key is their mean;
/// * tumbling torque-free faster than once per [`LATTICE`]: isotropic, the
///   coast sunlight's flux from the 12 directions of an icosahedron (a
///   tumble's average over orientations); the key is the flux;
/// * otherwise the coast sunlight ([`coast_sunlight`]) in the attitude now.
///
/// Coast temperatures are orbit averages already (the eclipse swing is not
/// shown); averaging the attitude's turn the same way keeps a held or
/// tumbling craft's input steady, so its lattice skips like a still one's.
pub fn coast_suns(
    world: &World,
    t: Epoch,
    (anchor, r, v): (NodeId, DVec3, DVec3),
    pointing: Option<&super::pointing::Pointing>,
    att: &super::Attitude,
) -> CoastSun {
    use super::pointing::Pointing;
    match pointing {
        Some(p @ Pointing::Track { .. }) => {
            if let Some(suns) = orbit_suns(world, t, (anchor, r, v), p) {
                let key = suns.iter().sum::<DVec3>() / suns.len() as f64;
                return CoastSun { suns, key, air: Vec::new() };
            }
        }
        Some(Pointing::Free { .. }) if att.omega.length() * LATTICE > 2.0 * crate::math::PI => {
            let flux = coast_sunlight(world, t, anchor, r, v).length();
            let suns = icosahedron().into_iter().map(|d| d * flux).collect();
            return CoastSun { suns, key: DVec3::new(flux, 0.0, 0.0), air: Vec::new() };
        }
        _ => {}
    }
    CoastSun::one(att.q.inverse() * coast_sunlight(world, t, anchor, r, v))
}

/// The orbit-averaged samples of [`coast_suns`] for a frame-held pointing,
/// if the orbit qualifies.
fn orbit_suns(
    world: &World,
    t: Epoch,
    (anchor, r, v): (NodeId, DVec3, DVec3),
    p: &super::pointing::Pointing,
) -> Option<Vec<DVec3>> {
    let body = world.source(anchor)?;
    let phys = body.physical.as_ref().filter(|b| b.luminosity == 0.0)?;
    let el = crate::kepler::Elements::from_state(r, v, body.gm);
    if !(el.e < 1.0 && el.a * (1.0 - el.e) > phys.radius_eq && el.period(body.gm) <= ORBIT_AVERAGE_PERIOD) {
        return None;
    }
    let snap = world.snapshot(t);
    let stars: Vec<(DVec3, f64)> = (world.sources.iter())
        .filter_map(|s| s.physical.as_ref().filter(|x| x.luminosity > 0.0).map(|x| (s, x.luminosity)))
        .map(|(s, l)| {
            let at_star = snap.relative_r(s.node, anchor);
            let d = (r - at_star).length();
            (at_star.normalize(), crate::light::star_flux(l, d))
        })
        .collect();
    let n = ORBIT_SAMPLES;
    let mut suns = Vec::with_capacity(n);
    for k in 0..n {
        // Fixed phases from the ascending node, not the vessel's (nor the
        // perigee's, ill-defined on a near-circular orbit): the samples do
        // not slide round the orbit from one lattice point to the next.
        let m = 2.0 * crate::math::PI * (k as f64 + 0.5) / n as f64 - el.argp;
        let (rk, vk) = crate::kepler::Elements { mean_anomaly: m, ..el }.to_state(body.gm);
        let q = p.attitude(world, &snap, anchor, rk, vk)?.q;
        let mut flux = DVec3::ZERO;
        for &(to_star, s) in &stars {
            let along = rk.dot(to_star);
            if along >= 0.0 || (rk - to_star * along).length_squared() >= phys.radius_eq * phys.radius_eq {
                flux -= to_star * s;
            }
        }
        suns.push(q.inverse() * flux);
    }
    Some(suns)
}
