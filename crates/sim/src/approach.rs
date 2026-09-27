//! Closest approaches between two objects' stored paths (realism-1 §6c).
//!
//! Each path is a function of time giving `(anchor, r, v)` (a vessel's
//! trajectory, `Trajectory::eval`, or a body through the ephemeris). The
//! relative position is formed through the frame tree at each time (rule 3),
//! sampled on a uniform grid, and every local minimum of the distance is
//! refined by golden-section search on the dense output. Nothing is
//! integrated: this reads what is stored (rule 4).

use crate::frame::NodeId;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// One closest approach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Approach {
    pub t: Epoch,
    /// Distance (m).
    pub distance: f64,
    /// Relative speed (m/s).
    pub speed: f64,
}

/// Iterations of the golden-section refinement (each shrinks the bracket by
/// 0.618; 60 reach f64 resolution of any sampling interval).
const REFINE: usize = 60;

/// The relative state of `b` with respect to `a` at `t`, or `None` where
/// either path is not stored.
fn relative<A, B>(world: &World, a: &A, b: &B, t: Epoch) -> Option<(DVec3, DVec3)>
where
    A: Fn(Epoch) -> Option<(NodeId, DVec3, DVec3)>,
    B: Fn(Epoch) -> Option<(NodeId, DVec3, DVec3)>,
{
    let (na, ra, va) = a(t)?;
    let (nb, rb, vb) = b(t)?;
    let k = world.snapshot(t).relative(nb, na);
    Some((k.r + rb - ra, k.v + vb - va))
}

/// Closest approaches of `b` to `a` over `[t0, t1]`, sampled `samples`
/// times (≥ 3), nearest first. Minima at the span's ends are included only
/// if the distance is still falling there (they may continue outside).
pub fn closest_approaches<A, B>(world: &World, a: A, b: B, t0: Epoch, t1: Epoch, samples: usize) -> Vec<Approach>
where
    A: Fn(Epoch) -> Option<(NodeId, DVec3, DVec3)>,
    B: Fn(Epoch) -> Option<(NodeId, DVec3, DVec3)>,
{
    let span = t1.seconds_since(t0);
    let n = samples.max(3);
    let dist = |s: f64| relative(world, &a, &b, t0.add_seconds(s)).map(|(r, _)| r.length());
    let grid: Vec<(f64, f64)> = (0..n)
        .filter_map(|k| {
            let s = span * k as f64 / (n - 1) as f64;
            dist(s).map(|d| (s, d))
        })
        .collect();
    let mut out = Vec::new();
    for k in 0..grid.len() {
        let d = grid[k].1;
        let left = k.checked_sub(1).map(|j| grid[j].1);
        let right = grid.get(k + 1).map(|g| g.1);
        let is_min = left.is_none_or(|l| d <= l) && right.is_none_or(|r| d < r);
        if !is_min || (left.is_none() && right.is_none()) {
            continue;
        }
        // Bracket between the neighbouring samples and refine.
        let lo = k.checked_sub(1).map_or(grid[k].0, |j| grid[j].0);
        let hi = grid.get(k + 1).map_or(grid[k].0, |g| g.0);
        let s = golden_min(&|s| dist(s).unwrap_or(f64::INFINITY), lo, hi);
        let t = t0.add_seconds(s);
        if let Some((r, v)) = relative(world, &a, &b, t) {
            out.push(Approach { t, distance: r.length(), speed: v.length() });
        }
    }
    out.sort_by(|x, y| x.distance.total_cmp(&y.distance));
    out
}

/// Golden-section search for the minimum of `f` on `[lo, hi]`.
fn golden_min(f: &impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
    const PHI: f64 = 0.618_033_988_749_894_8;
    let mut x1 = hi - PHI * (hi - lo);
    let mut x2 = lo + PHI * (hi - lo);
    let (mut f1, mut f2) = (f(x1), f(x2));
    for _ in 0..REFINE {
        if f1 < f2 {
            hi = x2;
            (x2, f2) = (x1, f1);
            x1 = hi - PHI * (hi - lo);
            f1 = f(x1);
        } else {
            lo = x1;
            (x1, f1) = (x2, f2);
            x2 = lo + PHI * (hi - lo);
            f2 = f(x2);
        }
    }
    if f1 < f2 {
        x1
    } else {
        x2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math;

    fn world() -> World {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(crate::sol::EPHEMERIS_PATH);
        let eph = crate::ephem::Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap();
        World::sol(std::sync::Arc::new(eph))
    }

    /// A circular orbit about the anchor at radius `r` (m), angular rate
    /// `w` (rad/s), phase `p0`.
    fn circle(anchor: NodeId, t0: Epoch, r: f64, w: f64, p0: f64) -> impl Fn(Epoch) -> Option<(NodeId, DVec3, DVec3)> {
        move |t| {
            let a = p0 + w * t.seconds_since(t0);
            let (s, c) = (math::sin(a), math::cos(a));
            Some((anchor, DVec3::new(c, s, 0.0) * r, DVec3::new(-s, c, 0.0) * (r * w)))
        }
    }

    #[test]
    fn two_coplanar_circles_meet_where_the_phases_line_up() {
        let w = world();
        let earth = w.find("Earth").unwrap().node;
        let t0 = crate::sol::sol_epoch();
        // Inner orbit 6,700 km, outer 6,800 km, outer 0.5 rad ahead and
        // slower: they align (minimum distance 100 km) when the phase
        // difference closes.
        let mu = w.find("Earth").unwrap().gm;
        let (r1, r2) = (6.7e6, 6.8e6);
        let (w1, w2) = ((mu / (r1 * r1 * r1)).sqrt(), (mu / (r2 * r2 * r2)).sqrt());
        let t_meet = 0.5 / (w1 - w2);
        let a = circle(earth, t0, r1, w1, 0.0);
        let b = circle(earth, t0, r2, w2, 0.5);
        let found = closest_approaches(&w, a, b, t0, t0.add_seconds(1.5 * t_meet), 400);
        let first = found.first().expect("an approach");
        assert!((first.distance - 1.0e5).abs() < 1.0, "{}", first.distance);
        assert!((first.t.seconds_since(t0) - t_meet).abs() < 0.5, "{} vs {t_meet}", first.t.seconds_since(t0));
        assert!((first.speed - (r2 * w2 - r1 * w1).abs()).abs() < 1e-3);
    }

    #[test]
    fn golden_section_finds_a_parabola_minimum() {
        let x = golden_min(&|x: f64| (x - 1.234).powi(2) + 5.0, 0.0, 3.0);
        // A minimum is only located to ~sqrt(epsilon) of the value: 5 + dx² is flat below that.
        assert!((x - 1.234).abs() < 1e-6);
    }
}
