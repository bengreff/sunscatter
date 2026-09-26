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
    let k = eph.relative(node, p, t);
    let mu = eph.node(node).gm + eph.node(p).gm;
    Some((p, Orbit { mu, elements: Elements::from_state(k.r, k.v, mu), distance: k.r.length() }))
}

/// A vessel's osculating orbit about its nearest body.
pub fn vessel_orbit(world: &World, t: Epoch, vessel: &Vessel) -> Option<Orbit> {
    let (anchor, r, v) = vessel.state_at(world, t);
    let body = nearest_body(world, t, anchor, r)?;
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
}
