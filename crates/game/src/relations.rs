//! How objects relate to bodies, for display and controls: which body an
//! object is nearest to, a body's display primary, and osculating elements
//! about a body. This module is the game's one owner of these rules; every
//! HUD readout, camera choice, map line and tracking-station row uses it.
//! Physics never does (rule 1: no reference bodies in the motion).

use glam::DVec3;
use sim::ephem::{Ephemeris, NodeKind};
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::time::Epoch;
use sim::vessel::Vessel;
use sim::world::World;

/// The body with a solid surface whose surface is closest to `r` (relative
/// to `anchor`, at `t`), measured as distance to its centre minus its
/// equatorial radius.
pub fn nearest_body(world: &World, t: Epoch, anchor: NodeId, r: DVec3) -> Option<NodeId> {
    let snap = world.snapshot(t);
    world
        .surfaces()
        .map(|s| {
            let radius = s.physical.as_ref().map_or(0.0, |p| p.radius_eq);
            (s.node, (r - snap.relative_r(s.node, anchor)).length() - radius)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(n, _)| n)
}

/// An object's osculating orbit about a body.
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    /// Gravitational parameter used (the body's; the object's mass is neglected).
    pub mu: f64,
    pub elements: Elements,
    /// Current distance from the body's centre (m).
    pub distance: f64,
}

impl Orbit {
    /// Period (s) if bound.
    pub fn period(&self) -> Option<f64> {
        (self.elements.e < 1.0).then(|| self.elements.period(self.mu))
    }

    /// Size of the orbit for display: the semi-major axis if bound, else
    /// the current distance.
    pub fn radius(&self) -> f64 {
        if self.elements.e < 1.0 {
            self.elements.a
        } else {
            self.distance
        }
    }
}

/// Osculating orbit of the state (`r`, `v` relative to `anchor`) about `body`.
pub fn orbit_about(world: &World, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3, body: NodeId) -> Option<Orbit> {
    let mu = world.source(body)?.gm;
    let k = world.snapshot(t).relative(body, anchor);
    let (r, v) = (r - k.r, v - k.v);
    Some(Orbit { mu, elements: Elements::from_state(r, v, mu), distance: r.length() })
}

/// A body's osculating orbit about its display [`primary`] (two-body: the
/// sum of both GMs), or `None` for a body with no primary.
pub fn body_orbit(eph: &Ephemeris, t: Epoch, node: NodeId) -> Option<(NodeId, Orbit)> {
    let p = primary(eph, node)?;
    Some((p, body_orbit_about(eph, t, node, p)))
}

/// A body's two-body osculating orbit about another body `about`.
pub fn body_orbit_about(eph: &Ephemeris, t: Epoch, node: NodeId, about: NodeId) -> Orbit {
    let k = eph.relative(node, about, t);
    let mu = eph.node(node).gm + eph.node(about).gm;
    Orbit { mu, elements: Elements::from_state(k.r, k.v, mu), distance: k.r.length() }
}

/// A vessel's osculating orbit about its dominant body (D056).
pub fn vessel_orbit(world: &World, dom: &Dominance, t: Epoch, vessel: &Vessel) -> Option<Orbit> {
    let (anchor, r, v) = vessel.state_at(world, t);
    let body = dom.of(&world.eph, t, anchor, r, None, None);
    orbit_about(world, t, anchor, r, v, body)
}

/// The body a node orbits for display: the dominant body of its parent
/// barycenter, or (for that dominant body) the barycenter's own primary.
/// Earth → Sun, Moon → Earth, Jupiter → Sun, Sun → none.
pub fn primary(eph: &Ephemeris, node: NodeId) -> Option<NodeId> {
    let parent = eph.node(node).parent?;
    if eph.node(parent).kind == NodeKind::Body {
        return Some(parent);
    }
    let dominant = eph
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, n)| n.parent == Some(parent) && n.kind == NodeKind::Body)
        .max_by(|a, b| a.1.gm.total_cmp(&b.1.gm))
        .map(|(i, _)| NodeId(i as u16))?;
    if dominant != node {
        Some(dominant)
    } else {
        primary(eph, parent)
    }
}

/// A body's Laplace sphere of influence about its display primary.
#[derive(Clone, Copy, Debug)]
struct Sphere {
    body: NodeId,
    primary: NodeId,
    /// Laplace radius `d (m_B / m_P)^(2/5)` (m).
    radius: f64,
}

/// Which body dominates an object's motion, for display (D056): tidal
/// dominance, not raw pull.
///
/// Body B dominates object o when o is inside B's Laplace sphere of
/// influence, `|r_oB| < d_BP (μ_B / μ_P)^(2/5)`, with P = B's display
/// [`primary`] and d_BP their distance. That radius is where the ratio of
/// B's pull on o to the perturbation (tidal pull) from P equals the ratio of
/// P's pull to the perturbation from B, so inside it B's pull relative to
/// its perturbation is the larger. Spheres nest (the Moon's inside Earth's
/// inside the Sun's, which is unbounded), and the dominant body is the
/// innermost sphere containing o. So: Earth for the Moon and for LEO, the
/// Moon in low lunar orbit, the Sun for Earth and for deep space.
///
/// The radii are taken once at construction time (they change slowly).
/// Display only: physics never uses it (rule 1).
#[derive(Clone, Debug)]
pub struct Dominance {
    root: NodeId,
    spheres: Vec<Sphere>,
    /// Twice the largest distance of the root's satellites: beyond this an
    /// object has left the region of interest (escape).
    region: f64,
}

impl Dominance {
    pub fn new(eph: &Ephemeris, t: Epoch) -> Self {
        let mut root = None;
        let mut spheres = Vec::new();
        for body in eph.bodies() {
            match primary(eph, body) {
                None => root = root.or(Some(body)),
                Some(p) => {
                    let d = eph.relative(body, p, t).r.length();
                    let radius = d * (eph.node(body).gm / eph.node(p).gm).powf(0.4);
                    spheres.push(Sphere { body, primary: p, radius });
                }
            }
        }
        let root = root.unwrap_or(eph.root());
        let region = 2.0
            * spheres
                .iter()
                .filter(|s| s.primary == root)
                .map(|s| eph.relative(s.body, root, t).r.length())
                .fold(0.0, f64::max);
        Dominance { root, spheres, region }
    }

    /// The body with no primary (the Sun).
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Radius of the region of interest about the root (m).
    pub fn region(&self) -> f64 {
        self.region
    }

    /// Laplace sphere-of-influence radius of `body` (infinite for the root).
    pub fn sphere_radius(&self, body: NodeId) -> f64 {
        self.spheres.iter().find(|s| s.body == body).map_or(f64::INFINITY, |s| s.radius)
    }

    /// The dominant body for position `r` (relative to `anchor`, at `t`).
    /// `exclude` is the object itself when it is a body. `hint` (the
    /// previous answer along a path) makes the common case cheap: while o
    /// stays inside the hint's sphere, only the hint's satellites are checked.
    pub fn of(
        &self,
        eph: &Ephemeris,
        t: Epoch,
        anchor: NodeId,
        r: DVec3,
        exclude: Option<NodeId>,
        hint: Option<NodeId>,
    ) -> NodeId {
        let dist = |b: NodeId| (r - eph.relative(b, anchor, t).r).length();
        let mut current = match hint {
            Some(h) if h != self.root && Some(h) != exclude && dist(h) < self.sphere_radius(h) => h,
            _ => self.root,
        };
        // Descend while o is inside one of the current body's satellites'
        // spheres (siblings' spheres do not overlap; take the deepest).
        loop {
            let inside = self
                .spheres
                .iter()
                .filter(|s| s.primary == current && Some(s.body) != exclude)
                .map(|s| (s.body, dist(s.body) / s.radius))
                .filter(|&(_, q)| q < 1.0)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match inside {
                Some((b, _)) => current = b,
                None => return current,
            }
        }
    }

    /// The dominant body of body `node` at `t` (never itself; `None` for the root).
    pub fn of_body(&self, eph: &Ephemeris, t: Epoch, node: NodeId) -> Option<NodeId> {
        (node != self.root).then(|| self.of(eph, t, node, DVec3::ZERO, Some(node), None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::kepler::Elements;
    use std::sync::Arc;

    fn world() -> World {
        let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sim::sol::EPHEMERIS_PATH);
        let eph = sim::ephem::Ephemeris::from_bytes(&std::fs::read(path).expect("ephemeris")).expect("valid");
        World::sol(Arc::new(eph))
    }

    #[test]
    fn primaries_follow_the_body_tree() {
        let w = world();
        let id = |n: &str| w.eph.find(n).expect(n);
        assert_eq!(primary(&w.eph, id("Moon")), Some(id("Earth")));
        assert_eq!(primary(&w.eph, id("Earth")), Some(id("Sun")));
        assert_eq!(primary(&w.eph, id("Jupiter")), Some(id("Sun")));
        assert_eq!(primary(&w.eph, id("Sun")), None);
    }

    #[test]
    fn body_orbits_about_their_primaries() {
        let w = world();
        let t = sim::sol::sol_epoch();
        let id = |n: &str| w.eph.find(n).expect(n);
        let (p, moon) = body_orbit(&w.eph, t, id("Moon")).expect("Moon orbit");
        assert_eq!(p, id("Earth"));
        assert!((moon.radius() - 3.844e8).abs() < 0.02 * 3.844e8, "{}", moon.radius());
        let (p, earth) = body_orbit(&w.eph, t, id("Earth")).expect("Earth orbit");
        assert_eq!(p, id("Sun"));
        assert!((earth.radius() - 1.496e11).abs() < 0.02 * 1.496e11);
        assert!(body_orbit(&w.eph, t, id("Sun")).is_none());
    }

    #[test]
    fn orbit_radius_is_the_distance_when_unbound() {
        let el = Elements { a: -1.0e7, e: 1.5, i: 0.0, raan: 0.0, argp: 0.0, mean_anomaly: 0.3 };
        let o = Orbit { mu: 3.986e14, elements: el, distance: 2.0e7 };
        assert_eq!(o.radius(), 2.0e7);
        let bound = Orbit { elements: Elements { a: 7.0e6, e: 0.1, ..el }, ..o };
        assert_eq!(bound.radius(), 7.0e6);
    }

    #[test]
    fn nearest_body_and_orbit_in_low_earth_orbit_and_near_the_moon() {
        let w = world();
        let t = sim::sol::sol_epoch();
        let earth = w.find("Earth").expect("Earth").clone();
        let moon = w.find("Moon").expect("Moon").node;
        let el = Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 0.3, argp: 0.0, mean_anomaly: 1.0 };
        let (r, v) = el.to_state(earth.gm);
        assert_eq!(nearest_body(&w, t, earth.node, r), Some(earth.node));
        let o = orbit_about(&w, t, earth.node, r, v, earth.node).expect("orbit");
        assert!((o.elements.a - el.a).abs() < 1e-3 && o.period().is_some());
        // 100 km above the Moon's surface, expressed relative to Earth.
        let near_moon = w.snapshot(t).relative_r(moon, earth.node) + DVec3::X * 1_837_400.0;
        assert_eq!(nearest_body(&w, t, earth.node, near_moon), Some(moon));
    }

    #[test]
    fn dominant_body_is_tidal_not_raw_pull() {
        let w = world();
        let t = sim::sol::sol_epoch();
        let eph = &w.eph;
        let id = |n: &str| eph.find(n).expect(n);
        let (sun, earth, moon) = (id("Sun"), id("Earth"), id("Moon"));
        let d = Dominance::new(eph, t);
        assert_eq!(d.root(), sun);
        // Laplace radii: Earth ~0.92e9 m, Moon ~6.6e7 m.
        assert!((d.sphere_radius(earth) - 9.2e8).abs() < 0.05e9, "{}", d.sphere_radius(earth));
        assert!((d.sphere_radius(moon) - 6.6e7).abs() < 0.4e7, "{}", d.sphere_radius(moon));
        // Bodies: the Sun pulls the Moon harder, but Earth dominates it.
        assert_eq!(d.of_body(eph, t, moon), Some(earth));
        assert_eq!(d.of_body(eph, t, earth), Some(sun));
        assert_eq!(d.of_body(eph, t, id("Jupiter")), Some(sun));
        assert_eq!(d.of_body(eph, t, sun), None);
        let earth_moon = eph.relative(moon, earth, t).r;
        let up = earth_moon.cross(DVec3::Z).normalize();
        // Positions relative to Earth, and the expected dominant body.
        let cases = [
            (DVec3::X * 6.778e6, earth),                         // LEO
            (DVec3::Y * 4.2164e7, earth),                        // GEO
            (earth_moon + up * 1.837e6, moon),                   // low lunar orbit
            (earth_moon + earth_moon.normalize() * 6.0e7, moon), // far side, inside the Moon's sphere
            (earth_moon * 0.5, earth),                           // halfway to the Moon
            (-earth_moon.normalize() * 3.0e9, sun),              // 3 million km out
            (up * 5.0e10, sun),                                  // deep space
        ];
        for (r, want) in cases {
            for hint in [None, Some(earth), Some(moon), Some(sun)] {
                assert_eq!(d.of(eph, t, earth, r, None, hint), want, "r = {r:?}, hint {hint:?}");
            }
        }
        // The same from another anchor (display frames change nothing).
        assert_eq!(d.of(eph, t, moon, up * 1.837e6, None, None), moon);
        assert!(d.region() > 1.0e13, "{}", d.region());
    }
}
