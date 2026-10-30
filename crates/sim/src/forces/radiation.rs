//! Starlight pressure at a point (`docs/technical/limbo.md` §1): for each
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
use crate::world::{Role, World};
use glam::DVec3;

/// Occluders kept inline before spilling to the heap.
const INLINE: usize = 8;

/// Calls `f(pressure, dir)` for each star that is at least partly visible
/// from `r` (relative to `anchor`): the radiation pressure (Pa, for a
/// perfectly absorbing surface facing the star) and the unit direction the
/// light travels (inertial axes).
///
/// A moon system (`World::clusters`) whose sphere (its barycenter, its
/// extent plus its largest member's radius) cannot reach the star's shadow
/// cone is passed over as a whole, so its bodies are never evaluated (the
/// snapshot evaluates moons lazily); every other body is tested on its own.
/// Both tests only drop bodies that hide nothing: the result is the one rule
/// with every body an occluder ([`crate::light::sunlight`]).
pub fn starlight(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, mut f: impl FnMut(f64, DVec3)) {
    // The largest member radius of each moon system (at most 64 systems:
    // `World::clusters`).
    let mut member_r = [0.0f64; 64];
    for s in &world.sources {
        if let (Role::Member(c), Some(b)) = (s.role, &s.physical) {
            member_r[c] = member_r[c].max(b.radius_eq);
        }
    }
    for (k, star) in world.sources.iter().enumerate() {
        let Some(p) = star.physical.as_ref().filter(|p| p.luminosity > 0.0) else { continue };
        let at_star = snap.relative_r(star.node, anchor);
        let to_star = at_star - r;
        let ds = to_star.length();
        if ds <= p.radius_eq {
            continue;
        }
        // Every point between the vessel and the star's disc is within `ds`
        // of the star, so a body (or moon system) that keeps farther than
        // `ds` plus its size from the star (`Source::faint_min_distance`,
        // `Cluster::star_distance`, measured from `World::star`) hides
        // nothing and is passed over without evaluating its position.
        let keeps_away = |q: Option<f64>, size: f64| {
            world.star == Some(k) && q.is_some_and(|q| q - ds > size * (1.0 + 1e-6) + 1e-6 * ds)
        };
        let u = to_star / ds;
        let mut inline = [(DVec3::ZERO, 0.0); INLINE];
        let mut n = 0;
        let mut spill: Vec<(DVec3, f64)> = Vec::new();
        // Per moon system: decided (bit) and may shade (bit).
        let (mut decided, mut shades) = (0u64, 0u64);
        for s in &world.sources {
            if s.node == star.node {
                continue;
            }
            let Some(b) = &s.physical else { continue };
            if let Role::Member(c) = s.role {
                if decided & (1 << c) == 0 {
                    decided |= 1 << c;
                    let cl = &world.clusters[c];
                    if !keeps_away(Some(cl.star_distance), cl.extent + member_r[c]) {
                        let to_c = snap.relative_r(cl.node, anchor) - r;
                        if system_may_shade(to_c, cl.extent, member_r[c], u, ds, p.radius_eq) {
                            shades |= 1 << c;
                        }
                    }
                }
                if shades & (1 << c) == 0 {
                    continue;
                }
            } else if keeps_away(s.faint_min_distance, b.radius_eq) {
                continue;
            }
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

/// Whether any body of a moon system can hide part of the star: the system
/// at `to_c` (its barycenter, from the point), every member within
/// `extent` of it and of radius at most `member_r`. False only where no
/// such body passes [`may_occlude`]: all behind the point, all beyond the
/// star, or the barycenter farther from the line of sight than the extent
/// plus the largest radius plus the star's apparent radius at the farthest
/// member's distance (margin 1e-6).
fn system_may_shade(to_c: DVec3, extent: f64, member_r: f64, u: DVec3, ds: f64, star_r: f64) -> bool {
    let along = to_c.dot(u);
    let dc = to_c.length();
    if along + extent + member_r <= 0.0 || dc - extent - member_r >= ds {
        return false;
    }
    let perp = (to_c - u * along).length();
    perp <= (extent + member_r + star_r * (dc + extent) / ds) * (1.0 + 1e-6)
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

    /// The one rule far from Earth too: in the shadows of planets and
    /// dwarf planets inside closed moon systems (Uranus, Neptune, Pluto and
    /// Eris seen from beyond their tidal reach) and across their penumbrae,
    /// the filtered sunlight is `light::sunlight`'s (every body an occluder).
    #[test]
    fn far_shadows_are_the_one_rules() {
        let w = world();
        let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
        let snap = w.snapshot(t);
        let sun = w.find("Sun").unwrap().node;
        let (mut dark, mut partial) = (0, 0);
        for (name, dist) in
            [("Uranus", 5.0e10), ("Uranus", 9.0e10), ("Neptune", 1.3e11), ("Eris", 1.5e10), ("Pluto", 9.0e9)]
        {
            let body = w.find(name).unwrap();
            let at = snap.relative_r(body.node, sun);
            let side = at.normalize().any_orthonormal_vector();
            let radius = body.physical.as_ref().unwrap().radius_eq;
            for k in -20..=20 {
                // Across the shadow, behind the body, out to twice its radius.
                let r = at + at.normalize() * dist + side * (radius * 0.1 * k as f64);
                let full = crate::light::sunlight(&w, &snap, sun, r);
                let got = sunlight(&w, &snap, sun, r);
                assert!((got - full).length() <= 1e-12 * full.length().max(1e-300), "{name} {k}: {got} vs {full}");
                let unshaded =
                    crate::light::star_flux(w.find("Sun").unwrap().physical.as_ref().unwrap().luminosity, (r).length());
                dark += usize::from(full.length() < 0.5 * unshaded);
                partial += usize::from(full.length() < 0.999 * unshaded);
            }
        }
        assert!(dark > 0 && partial > dark, "some points in shadow ({dark}) and penumbra ({partial})");
    }
}
