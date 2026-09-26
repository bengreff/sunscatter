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
}

impl Orbit {
    /// Period (s) if bound.
    pub fn period(&self) -> Option<f64> {
        (self.elements.e < 1.0).then(|| self.elements.period(self.mu))
    }
}

/// Osculating orbit of the state (`r`, `v` relative to `anchor`) about `body`.
pub fn orbit_about(world: &World, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3, body: NodeId) -> Option<Orbit> {
    let mu = world.source(body)?.gm;
    let k = world.snapshot(t).relative(body, anchor);
    Some(Orbit { mu, elements: Elements::from_state(r - k.r, v - k.v, mu) })
}

/// A vessel's osculating orbit about its nearest body.
pub fn vessel_orbit(world: &World, t: Epoch, vessel: &Vessel) -> Option<Orbit> {
    let (anchor, r, v) = vessel.state(world);
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
