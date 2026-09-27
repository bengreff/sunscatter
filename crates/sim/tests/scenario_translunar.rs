//! Executable scenario: a trans-lunar coast from low Earth orbit reaches the
//! Moon's vicinity, and the stored segment re-anchors to the Moon on the way
//! (a pure precision change) and back out again.

use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::vessel::{CoastStart, Segment};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

#[test]
fn trans_lunar_coast_reaches_the_moon_and_reanchors() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let moon = w.find("Moon").unwrap();
    let (mu, e_id, m_id) = (earth.gm, earth.node, moon.node);
    let t0 = sol::sol_epoch().add_seconds(10.0 * 86_400.0);

    // Put a 300 km circular parking orbit in the Moon's orbital plane.
    let mk = w.eph.relative(m_id, e_id, t0);
    let normal = mk.r.cross(mk.v).normalize();
    let a = 6_678_137.0;
    let apogee = 3.9e8;
    // Vis-viva: speed at perigee for an ellipse reaching `apogee`.
    let v_tli = (mu * (2.0 / a - 2.0 / (a + apogee))).sqrt();

    // Search departure phases; keep the closest lunar approach.
    let mut best = (f64::INFINITY, 0.0, None);
    for k in 0..72 {
        let phase = k as f64 / 72.0 * std::f64::consts::TAU;
        let (s, c) = (phase.sin(), phase.cos());
        let x = mk.r.normalize();
        let y = normal.cross(x);
        let r = (x * c + y * s) * a;
        let v_dir = (y * c - x * s).normalize();
        let mut seg = Segment::new(
            &w,
            t0,
            CoastStart {
                anchor: e_id,
                r,
                v: v_dir * v_tli,
                drag: None,
                contact_height: 0.0,
                horizon: 6.0 * 86_400.0,
                fixed_anchor: false,
                proper_time: 0.0,
            },
        );
        while !seg.finished() {
            seg.extend(&w, 512);
        }
        let closest = seg
            .samples
            .iter()
            .map(|s| {
                let t = t0.add_seconds(s.s.t);
                (s.s.r - w.eph.relative(m_id, s.anchor, t).r).length()
            })
            .fold(f64::INFINITY, f64::min);
        if closest < best.0 {
            best = (closest, phase, Some(seg));
        }
    }
    let (closest, phase, seg) = best;
    let seg = seg.unwrap();
    println!("closest lunar approach {:.0} km (departure phase {:.1}°)", closest / 1e3, phase.to_degrees());
    assert!(closest < 30_000e3, "missed the Moon: {closest} m");
    let anchors: Vec<_> = seg.samples.iter().map(|s| s.anchor).collect();
    assert!(anchors.contains(&m_id), "never re-anchored to the Moon");
    // Elements about the Moon at closest approach are hyperbolic (a flyby).
    let near = seg
        .samples
        .iter()
        .filter(|s| s.anchor == m_id)
        .min_by(|a, b| a.s.r.length().total_cmp(&b.s.r.length()))
        .unwrap();
    let el = Elements::from_state(near.s.r, near.s.v, moon.gm);
    println!("lunar flyby: e = {:.3}, periapsis {:.0} km", el.e, el.periapsis() / 1e3);
    assert!(el.e > 1.0 || el.periapsis() > 0.0);
}
