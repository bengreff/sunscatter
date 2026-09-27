use super::*;
use sim::kepler::Elements;
use sim::vessel::{Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;

const A: NodeId = NodeId(1);
const B: NodeId = NodeId(2);
const ROOT: NodeId = NodeId(0);

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sim::sol::EPHEMERIS_PATH);
    let eph = Ephemeris::from_bytes(&std::fs::read(path).expect("ephemeris")).expect("valid");
    World::sol(Arc::new(eph))
}

/// Where the line ends along `points`, or `None` if they run out first.
fn line_end(points: impl IntoIterator<Item = PathPoint>, limits: Limits, root: NodeId) -> Option<LineEnd> {
    let mut rule = LineRule::new(limits, root);
    points.into_iter().find_map(|p| rule.push(p))
}

fn limits(revolutions: Option<f64>) -> Limits {
    Limits { revolutions, t_cap: 1.0e9, escape_radius: 1.0e12 }
}

/// A circle of radius 1 about `about`, starting at angle `phase` (rad), one
/// point per `step` rad, `n` points, time = angle swept (so ω = 1 rad/s).
fn circle(about: NodeId, t0: f64, phase: f64, step: f64, n: usize, sign: f64) -> Vec<PathPoint> {
    (0..n)
        .map(|k| {
            let th = phase + sign * step * k as f64;
            let (s, c) = th.sin_cos();
            PathPoint {
                t: t0 + step * k as f64,
                dominant: about,
                r: DVec3::new(c, s, 0.0),
                v: sign * DVec3::new(-s, c, 0.0),
            }
        })
        .collect()
}

#[test]
fn line_ends_after_the_requested_revolutions() {
    let step = 0.07;
    let n = 400; // 28 rad: over four revolutions.
    for (revs, sign) in [(1.0, 1.0), (2.0, 1.0), (1.0, -1.0), (0.5, 1.0)] {
        let pts = circle(A, 0.0, 0.3, step, n, sign);
        let end = line_end(pts, limits(Some(revs)), ROOT).expect("ends");
        assert_eq!(end.reason, EndReason::Revolutions);
        assert!((end.t - revs * TAU).abs() < 1e-9, "revs {revs} sign {sign}: {}", end.t);
    }
    // No revolution limit and nothing else: the points run out.
    assert_eq!(line_end(circle(A, 0.0, 0.0, step, n, 1.0), limits(None), ROOT), None);
}

#[test]
fn a_change_of_dominant_body_restarts_the_count() {
    // Half a turn about A, then about B: the line ends one full turn after the switch.
    let first = circle(A, 0.0, 0.0, 0.05, 63, 1.0); // ~3.1 rad
    let t_switch = 63.0 * 0.05;
    let second = circle(B, t_switch, 1.0, 0.05, 200, 1.0);
    let end = line_end(first.into_iter().chain(second), limits(Some(1.0)), ROOT).expect("ends");
    assert_eq!(end.reason, EndReason::Revolutions);
    assert!((end.t - (t_switch + TAU)).abs() < 1e-9, "{}", end.t);
}

#[test]
fn caps_end_the_line() {
    // Time cap inside the first revolution.
    let lim = Limits { t_cap: 2.0, ..limits(Some(1.0)) };
    let end = line_end(circle(A, 0.0, 0.0, 0.07, 200, 1.0), lim, ROOT).expect("ends");
    assert_eq!(end, LineEnd { t: 2.0, reason: EndReason::TimeCap });
    // Escape: a radial path about the root beyond the region of interest.
    let lim = Limits { escape_radius: 10.0, ..limits(Some(1.0)) };
    let out = (0..50).map(|k| PathPoint { t: k as f64, dominant: ROOT, r: DVec3::X * (1.0 + k as f64), v: DVec3::X });
    assert_eq!(line_end(out, lim, ROOT), Some(LineEnd { t: 10.0, reason: EndReason::Escape }));
    // The same distance about a non-root body is not an escape.
    let about_a = (0..50).map(|k| PathPoint { t: k as f64, dominant: A, r: DVec3::X * (1.0 + k as f64), v: DVec3::X });
    assert_eq!(line_end(about_a, lim, ROOT), None);
}

/// A vessel coasting from (`r`, `v`) about `anchor`, integrated `span` s ahead.
fn coast(w: &World, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3, span: f64) -> Vessel {
    let mut ship = Vessel::coasting(w, VesselId(1), t, anchor, r, v, sim::craft::test_craft());
    ship.extend_coast(w, t.add_seconds(span), 2_000_000);
    ship
}

#[test]
fn circular_low_orbits_end_at_one_revolution() {
    let w = world();
    let t = sim::sol::sol_epoch();
    // Low Earth orbit and low lunar orbit (the Moon dominates there).
    for (name, a) in [("Earth", 6_778_137.0), ("Moon", 1_837_400.0)] {
        let body = w.find(name).expect(name).clone();
        let el = Elements { a, e: 0.0, i: 0.9, raan: 0.3, argp: 0.0, mean_anomaly: 1.0 };
        let (r, v) = el.to_state(body.gm);
        let period = el.period(body.gm);
        let ship = coast(&w, t, body.node, r, v, 1.3 * period);
        let seg = ship.segment().expect("coasting");
        let dom = Dominance::new(&w.eph, t);
        let pts: Vec<_> = segment_points(&w.eph, &dom, seg, 0.0, 100_000).collect();
        assert!(pts.iter().all(|p| p.dominant == body.node), "{name}");
        let end = line_end(pts, limits(Some(1.0)), dom.root()).expect("ends");
        assert_eq!(end.reason, EndReason::Revolutions);
        // The end angle, measured on the trajectory itself: 360° ± 0.5°.
        let (anchor, r_end, _) = seg.eval(end.t).expect("computed");
        let r_end = r_end + w.eph.relative(anchor, body.node, t.add_seconds(end.t)).r;
        let n = r.cross(v).normalize();
        let mut angle = n.dot(r.cross(r_end)).atan2(r.dot(r_end)).to_degrees();
        if angle < 180.0 {
            angle += 360.0; // just past a full turn (or short of it, if negative)
        }
        assert!((angle - 360.0).abs() < 0.5, "{name}: end angle {angle}°");
        assert!((end.t - period).abs() < 0.01 * period, "{name}: {} vs {period}", end.t);
    }
}

#[test]
fn a_moon_flyby_changes_dominant_body_and_restarts() {
    let w = world();
    let t = sim::sol::sol_epoch();
    let (earth, moon) = (w.find("Earth").expect("Earth").clone(), w.find("Moon").expect("Moon").clone());
    // Inbound hyperbola about the Moon (v∞ 1 km/s, periapsis ~2,000 km),
    // starting ~80,000 km out: outside the Moon's sphere, dominated by Earth.
    let a = -moon.gm / 1.0e6;
    let el = Elements { a, e: 1.0 - 2.0e6 / a, i: 0.2, raan: 0.0, argp: 0.0, mean_anomaly: -14.0 };
    let (r, v) = el.to_state(moon.gm);
    let k = w.eph.relative(moon.node, earth.node, t);
    let span = 40.0 * 86_400.0;
    let ship = coast(&w, t, earth.node, r + k.r, v + k.v, span);
    let seg = ship.segment().expect("segment");
    let dom = Dominance::new(&w.eph, t);
    let lim = Limits { t_cap: span, ..limits(Some(1.0)) };
    let mut rule = LineRule::new(lim, dom.root());
    let mut seen = Vec::new();
    let mut end = None;
    for p in segment_points(&w.eph, &dom, seg, 0.0, 1_000_000) {
        if seen.last() != Some(&p.dominant) {
            seen.push(p.dominant);
        }
        end = rule.push(p);
        if end.is_some() {
            break;
        }
    }
    assert_eq!(seen[..2], [earth.node, moon.node], "dominant bodies along the path: {seen:?}");
    let end = end.or_else(|| seg.finished().then(|| LineEnd { t: seg.computed_until(), reason: EndReason::TimeCap }));
    let end = end.expect("line ends within the computed span");
    match end.reason {
        // A full turn about the last dominant body, counted from the switch
        // (the count includes the whole last step; the end is interpolated in it).
        EndReason::Revolutions => assert!(rule.swept >= TAU && rule.swept < TAU + 0.3, "{}", rule.swept),
        EndReason::TimeCap | EndReason::Escape => {}
    }
    // It never ends inside the first (Earth-dominated) approach.
    assert!(end.t > 0.5 * 86_400.0, "{end:?}");
}

#[test]
fn the_moons_line_is_one_lunar_orbit_about_earth() {
    let w = world();
    let t = sim::sol::sol_epoch();
    let eph = &w.eph;
    let (earth, moon) = (eph.find("Earth").unwrap(), eph.find("Moon").unwrap());
    let dom = Dominance::new(eph, t);
    let about = dom.of_body(eph, t, moon).expect("dominant");
    assert_eq!(about, earth);
    let period = crate::relations::body_orbit_about(eph, t, moon, about).period().expect("bound");
    let end =
        line_end(body_points(eph, moon, about, t, period / 256.0, 400), limits(Some(1.0)), dom.root()).expect("ends");
    assert_eq!(end.reason, EndReason::Revolutions);
    // One sidereal month, 27.32 d (the osculating period varies a little).
    let days = end.t / 86_400.0;
    assert!((days - 27.32).abs() < 0.6, "{days} d");
}

#[test]
fn prediction_and_body_lines_follow_the_rule() {
    use crate::trajectory::{body_line, extend_to_line_end, settings::OrbitSettings, vessel_line_end};
    let w = world();
    let t = sim::sol::sol_epoch();
    let earth = w.find("Earth").expect("Earth").clone();
    // A prediction is integrated just past one revolution, not further.
    let el = Elements { a: 7.0e6, e: 0.05, i: 0.4, raan: 0.0, argp: 0.0, mean_anomaly: 0.0 };
    let (r, v) = el.to_state(earth.gm);
    let period = el.period(earth.gm);
    let ship = Vessel::coasting(&w, VesselId(1), t, earth.node, r, v, sim::craft::test_craft());
    let mut seg = ship.coast_from_now(&w);
    let settings = OrbitSettings::default();
    extend_to_line_end(&w, &mut seg, &settings, 40_000);
    let computed = seg.computed_until();
    assert!(computed >= period && computed < 1.5 * period, "{computed} vs {period}");
    let (end, done) = vessel_line_end(&w, &seg, 0.0, &settings);
    assert!(done && (end - period).abs() < 0.01 * period, "{end} vs {period}");
    // The Moon's drawn line closes on itself after one revolution about Earth.
    let (eph, moon) = (&w.eph, w.eph.find("Moon").unwrap());
    let dom = Dominance::new(eph, t);
    let lim = settings.body_limits(dom.region()).unwrap();
    let (orbit, pts) = body_line(eph, &dom, moon, earth.node, t, lim, 256).expect("bound");
    let (first, last) = (pts[0], *pts.last().unwrap());
    assert!((first.angle_between(last)).to_degrees() < 0.5, "{} points", pts.len());
    assert!(pts.len() > 200 && pts.len() < 300 && orbit.period().is_some(), "{}", pts.len());
}
