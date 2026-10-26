//! Starlight pressure at a point (`docs/features/limbo.md` §1): for each
//! star, the radiation pressure P = L/(4πd²c) times the visible fraction of
//! its disc, and the direction the light travels. The eclipse is
//! [`crate::light::eclipse_factor`] (the one rule); occluders that cannot
//! overlap the star's disc are dropped first by a conservative test
//! without trigonometry, which cannot change its result (a dropped
//! occluder adds exactly zero), so the cost per evaluation is a few flops
//! per body.

use crate::ephem::Snapshot;
use crate::frame::NodeId;
use crate::light::{eclipse_factor, star_flux};
use crate::relativity::C;
use crate::world::World;
use glam::DVec3;

/// Occluders kept inline before spilling to the heap.
const INLINE: usize = 8;

/// Calls `f(pressure, dir)` for each star that is at least partly visible
/// from `r` (relative to `anchor`): the radiation pressure (Pa, for a
/// perfectly absorbing surface facing the star) and the unit direction the
/// light travels (inertial axes).
pub fn starlight(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, mut f: impl FnMut(f64, DVec3)) {
    for star in &world.sources {
        let Some(p) = star.physical.as_ref().filter(|p| p.luminosity > 0.0) else { continue };
        let at_star = snap.relative_r(star.node, anchor);
        let to_star = at_star - r;
        let ds = to_star.length();
        if ds <= p.radius_eq {
            continue;
        }
        let u = to_star / ds;
        let mut inline = [(DVec3::ZERO, 0.0); INLINE];
        let mut n = 0;
        let mut spill: Vec<(DVec3, f64)> = Vec::new();
        for s in &world.sources {
            if s.node == star.node {
                continue;
            }
            let Some(b) = &s.physical else { continue };
            let c = snap.relative_r(s.node, anchor);
            if !may_occlude(c - r, b.radius_eq, u, ds, p.radius_eq) {
                continue;
            }
            if n < INLINE {
                inline[n] = (c, b.radius_eq);
                n += 1;
            } else {
                spill.push((c, b.radius_eq));
            }
        }
        let seen = if n == 0 {
            1.0
        } else if spill.is_empty() {
            eclipse_factor(r, at_star, p.radius_eq, &inline[..n])
        } else {
            let mut all = inline[..n].to_vec();
            all.extend(spill);
            eclipse_factor(r, at_star, p.radius_eq, &all)
        };
        if seen > 0.0 {
            f(star_flux(p.luminosity, ds) * seen / C, -u);
        }
    }
}

/// Sunlight at `r` (relative to `anchor`): the flux vector of every star
/// (W/m², along the light, inertial), eclipses included; the same as
/// [`crate::light::sunlight`] to rounding, without its allocations and
/// trigonometry for bodies that cannot shade.
pub fn sunlight(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> DVec3 {
    let mut flux = DVec3::ZERO;
    starlight(world, snap, anchor, r, |p, dir| flux += dir * (p * C));
    flux
}

/// Whether a sphere of radius `radius` at `to_c` (from the point) can hide
/// part of a star of radius `star_r` at distance `ds` in unit direction
/// `u`. False only where [`eclipse_factor`] finds no overlap: the body is
/// behind the point, beyond the star, contains the point, or its centre
/// lies farther from the line of sight than its radius plus the star's
/// apparent radius at its distance (then the angular separation exceeds
/// the sum of the two apparent radii, since sin(a + b) ≤ sin a + sin b),
/// with a margin of 1e-6 against rounding.
fn may_occlude(to_c: DVec3, radius: f64, u: DVec3, ds: f64, star_r: f64) -> bool {
    let along = to_c.dot(u);
    if along <= 0.0 {
        return false;
    }
    let dc2 = to_c.length_squared();
    if dc2 <= radius * radius || dc2 >= ds * ds {
        return false;
    }
    let perp2 = (to_c - u * along).length_squared();
    let reach = (radius + star_r * dc2.sqrt() / ds) * (1.0 + 1e-6);
    perp2 <= reach * reach
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ephem::Ephemeris;
    use crate::sol;
    use std::sync::Arc;

    fn world() -> World {
        let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
        World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
    }

    /// The filtered eclipse equals the unfiltered one (every body an
    /// occluder) bit for bit, in and out of shadow and through the
    /// penumbra; the pressure is `light::sunlight`'s flux over c.
    #[test]
    fn filtering_occluders_changes_nothing() {
        let w = world();
        let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
        let snap = w.snapshot(t);
        let earth = w.find("Earth").unwrap().node;
        let sun = w.find("Sun").unwrap();
        let sun_r = sun.physical.as_ref().unwrap().radius_eq;
        let at_sun = snap.relative_r(sun.node, earth);
        let to_sun = at_sun.normalize();
        let side = to_sun.any_orthonormal_vector();
        let all: Vec<(DVec3, f64)> = (w.sources.iter())
            .filter(|s| s.node != sun.node)
            .filter_map(|s| s.physical.as_ref().map(|b| (snap.relative_r(s.node, earth), b.radius_eq)))
            .collect();
        let re = 6_378_137.0;
        let mut penumbra = 0;
        for k in 0..400 {
            // Points sweeping across Earth's shadow at 400 km, and far out.
            let a = -1.2 + 2.4 * k as f64 / 400.0;
            for dist in [re + 400_000.0, 3.0e7, 3.8e8] {
                let r = (-to_sun * (1.0 - a * a).abs().sqrt() + side * a).normalize() * dist;
                let u = (at_sun - r).normalize();
                let ds = (at_sun - r).length();
                let kept: Vec<(DVec3, f64)> =
                    all.iter().copied().filter(|&(c, rad)| may_occlude(c - r, rad, u, ds, sun_r)).collect();
                let full = eclipse_factor(r, at_sun, sun_r, &all);
                assert_eq!(eclipse_factor(r, at_sun, sun_r, &kept), full, "{a} at {dist}");
                penumbra += usize::from(full > 0.0 && full < 1.0);
                let reference = crate::light::sunlight(&w, &snap, earth, r);
                let mut got = DVec3::ZERO;
                starlight(&w, &snap, earth, r, |p, dir| got += dir * (p * C));
                assert!((got - reference).length() <= 1e-12 * reference.length(), "{a} at {dist}");
            }
        }
        assert!(penumbra > 0, "some points in the penumbra");
    }
}
