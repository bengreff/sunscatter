//! Gravity source cutoff (D023): cut sources pull the vessel as they pull its
//! anchor, sources are added when they start to matter (flybys), and the
//! anchor still does not change the trajectory (rule 1).

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{CoastStart, Segment};
use sim::world::World;
use std::sync::Arc;

const YEAR: f64 = 365.25 * 86_400.0;

fn world(cutoff: f64) -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let mut w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    w.cutoff = cutoff;
    w
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

/// Integrates a whole segment of `span` seconds.
fn coast(w: &World, anchor: NodeId, r: DVec3, v: DVec3, span: f64, fixed_anchor: bool) -> Segment {
    let start = CoastStart { anchor, r, v, drag: None, contact_height: 0.0, horizon: span, fixed_anchor };
    let mut seg = Segment::new(w, t0(), start);
    while !seg.finished() {
        seg.extend(w, 1000);
    }
    seg
}

/// A heliocentric orbit at 1.2 AU, half a radian ahead of Earth (Sun-relative).
fn heliocentric(w: &World) -> (NodeId, DVec3, DVec3) {
    let sun = w.find("Sun").unwrap().node;
    let e = w.eph.relative(w.find("Earth").unwrap().node, sun, t0());
    let n = e.r.cross(e.v).normalize();
    let rot = |x: DVec3| x * 0.5f64.cos() + n.cross(x) * 0.5f64.sin();
    (sun, rot(e.r) * 1.2, rot(e.v) / 1.2f64.sqrt())
}

fn index(w: &World, name: &str) -> usize {
    w.sources.iter().position(|s| s.name == name).unwrap()
}

/// Position relative to `about` at the end of a segment.
fn end_position(w: &World, seg: &Segment, span: f64, about: NodeId) -> DVec3 {
    let (anchor, r, _) = seg.eval(span).unwrap();
    r + w.eph.relative(anchor, about, t0().add_seconds(span)).r
}

/// Pluto is cut (its tidal pull is ~1e-15 m/s²). Dropping its pull on the
/// vessel while the anchor's ephemeris acceleration still contains it used to
/// leave a fictitious force: 33 m of drift in a year against the full model.
#[test]
fn a_cut_source_pulls_the_vessel_like_its_anchor() {
    let (w, full) = (world(1e-12), world(0.0));
    let (sun, r, v) = heliocentric(&w);
    let cut = coast(&w, sun, r, v, YEAR, true);
    assert!(!cut.active_sources().0.contains(&index(&w, "Pluto")));
    let reference = coast(&full, sun, r, v, YEAR, true);
    let dr = (end_position(&w, &cut, YEAR, sun) - end_position(&w, &reference, YEAR, sun)).length();
    println!("cut vs full model after a year: {dr:.3} m");
    assert!(dr < 2.0, "{dr} m");
}

/// With a source near the cutoff (Neptune: cut at the start, added during the
/// year), anchoring at the Sun or at the barycenter gives the same trajectory
/// to within the barycentric anchor's precision (~20 m at 1.8e11 m offsets,
/// the same as with every source simulated).
#[test]
fn anchor_choice_does_not_change_a_year_with_a_source_near_the_cutoff() {
    let w = world(2.5e-11);
    let neptune = index(&w, "Neptune");
    let (sun, r, v) = heliocentric(&w);
    let root = w.eph.root();
    let shift = w.eph.relative(sun, root, t0());
    let about_sun = coast(&w, sun, r, v, YEAR, true);
    let about_ssb = coast(&w, root, r + shift.r, v + shift.v, YEAR, true);
    for seg in [&about_sun, &about_ssb] {
        assert!(seg.active_sources().0.contains(&neptune), "Neptune is added during the year");
        let (a, r0, _) = seg.eval(0.0).unwrap();
        let start = sim::forces::ActiveSources::select(&w, &w.snapshot(t0()), a, r0);
        assert!(!start.0.contains(&neptune), "Neptune is cut at the start");
    }
    let dr = (end_position(&w, &about_sun, YEAR, sun) - end_position(&w, &about_ssb, YEAR, sun)).length();
    println!("Sun- vs barycenter-anchored after a year: {dr:.3} m");
    assert!(dr < 40.0, "{dr} m");
}

/// A Pluto flyby: Pluto is cut at the start (with a raised cutoff, so the
/// approach is short) and must be picked up in time to deflect the vessel.
#[test]
fn a_flyby_picks_up_a_source_cut_at_the_start() {
    let (w, full) = (world(1e-9), world(0.0));
    let pluto = w.find("Pluto").unwrap().node;
    let root = w.eph.root();
    let p = w.eph.relative(pluto, root, t0());
    // 5e10 m out, 10 km/s towards Pluto with a 10,000 km miss distance.
    let dir = p.v.normalize();
    let side = dir.cross(DVec3::Z).normalize();
    let (r, v) = (p.r - dir * 5e10 + side * 1e7, p.v + dir * 1e4);
    let span = 120.0 * 86_400.0;
    let seg = coast(&w, root, r, v, span, false);
    let (a, r0, _) = seg.eval(0.0).unwrap();
    let i = index(&w, "Pluto");
    let first = sim::forces::ActiveSources::select(&w, &w.snapshot(t0()), a, r0);
    assert!(!first.0.contains(&i), "Pluto is cut at the start");
    assert!(seg.active_sources().0.contains(&i), "the flyby picks Pluto up");
    // The source set is stored state: extending in small chunks is identical.
    let start = CoastStart { anchor: root, r, v, drag: None, contact_height: 0.0, horizon: span, fixed_anchor: false };
    let mut chunked = Segment::new(&w, t0(), start);
    while !chunked.finished() {
        chunked.extend(&w, 7);
    }
    assert_eq!(chunked, seg);
    let reference = coast(&full, root, r, v, span, false);
    let mut no_pluto = world(1e-9);
    no_pluto.sources[i].gm = 0.0;
    let missed = coast(&no_pluto, root, r, v, span, false);
    let end = end_position(&w, &seg, span, root);
    let end_full = end_position(&w, &reference, span, root);
    let err = (end - end_full).length();
    let deflection = (end_full - end_position(&w, &missed, span, root)).length();
    println!("flyby: error {err:.0} m against a {deflection:.0} m deflection");
    assert!(err < 1e-3 * deflection, "error {err} m, deflection {deflection} m");
}
