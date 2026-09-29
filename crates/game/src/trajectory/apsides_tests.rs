use super::*;
use crate::state::{Prediction, SimState};
use sim::kepler::Elements;
use sim::vessel::Vessel;
use std::f64::consts::TAU;

const R_EARTH: f64 = 6_378_137.0;

fn t0() -> Epoch {
    sim::sol::sol_epoch()
}

/// Scans a radial distance profile `d(t)` (with its derivative) sampled
/// every `dt` over `[a, b]`, about one body (or `body_at(t)`), refining on
/// the profile itself.
fn scan(d: impl Fn(f64) -> (f64, f64) + Copy, a: f64, b: f64, dt: f64, body_at: impl Fn(f64) -> NodeId) -> Vec<Apsis> {
    let mut s = Scanner::default();
    let n = ((b - a) / dt).ceil() as usize;
    for k in 0..=n {
        let t = (a + k as f64 * dt).min(b);
        let (r, v) = d(t);
        let refine = |ta: Epoch, tb: Epoch| {
            let g = |x: f64| Some(d(x).0 * d(x).1);
            let (xa, xb) = (ta.seconds_since(t0()), tb.seconds_since(t0()));
            let x = refine_root(g, xa, xb, g(xa)?, g(xb)?, 1e-6);
            Some((t0().add_seconds(x), d(x).0))
        };
        s.push(t0().add_seconds(t), body_at(t), DVec3::X * r, DVec3::X * v, refine);
    }
    s.apsides(|_| R_EARTH)
}

const EARTH: NodeId = NodeId(3);
const MOON: NodeId = NodeId(4);

/// An orbit-like radial profile: a(1 − e cos nt) plus a wobble of
/// amplitude `w` at `k` times the orbital frequency.
fn orbit(a: f64, e: f64, w: f64, k: f64) -> impl Fn(f64) -> (f64, f64) + Copy {
    let n = TAU / 5_550.0;
    move |t: f64| {
        let d = a * (1.0 - e * (n * t).cos()) + w * (k * n * t).cos();
        let dd = a * e * n * (n * t).sin() - w * k * n * (k * n * t).sin();
        (d, dd)
    }
}

#[test]
fn apsides_of_profiles() {
    let period = 5_550.0;
    let a = R_EARTH + 400_000.0;
    let q = period / 4.0;
    struct Case {
        name: &'static str,
        apo: usize,
        peri: usize,
    }
    let cases: Vec<(Case, Vec<Apsis>)> = vec![
        // An eccentric orbit over three revolutions from P/4: Ap at P/2,
        // 3P/2, 5P/2; Pe at P, 2P, 3P.
        (
            Case { name: "eccentric", apo: 3, peri: 3 },
            scan(orbit(a, 0.05, 0.0, 1.0), q, 3.0 * period + q, 60.0, |_| EARTH),
        ),
        // The same with a fast small wobble (0.6 km, 120 per revolution):
        // its extra extrema near each apsis are dropped, the real Ap/Pe kept.
        (
            Case { name: "eccentric with wobble", apo: 3, peri: 3 },
            scan(orbit(a, 0.01, 600.0, 120.0), q, 3.0 * period + q, 2.0, |_| EARTH),
        ),
        // A near-circular orbit (Ap − Pe ≈ 0.7 km) wobbling twice per
        // revolution by J2's size at 45° (0.8 km): no apsis at all.
        (
            Case { name: "near-circular", apo: 0, peri: 0 },
            scan(orbit(a, 0.00005, 800.0, 2.0), q, 3.0 * period + q, 10.0, |_| EARTH),
        ),
        // A hyperbolic flyby from before periapsis: one Pe, no Ap.
        (
            Case { name: "escape", apo: 0, peri: 1 },
            scan(
                |t: f64| {
                    let (p, v) = (a, 11_000.0);
                    let d = (p * p + (v * t).powi(2)).sqrt();
                    (d, v * v * t / d)
                },
                -3_000.0,
                30_000.0,
                30.0,
                |_| EARTH,
            ),
        ),
        // Falling to the surface: no periapsis.
        (Case { name: "descent", apo: 0, peri: 0 }, scan(|t| (a - 50.0 * t, -50.0), 0.0, 8_000.0, 10.0, |_| EARTH)),
    ];
    for (case, got) in cases {
        let apo = got.iter().filter(|x| x.is_apo).count();
        let peri = got.len() - apo;
        assert_eq!((apo, peri), (case.apo, case.peri), "{}: {got:?}", case.name);
        assert!(got.windows(2).all(|w| w[0].is_apo != w[1].is_apo), "{}: alternate", case.name);
    }
}

#[test]
fn a_refined_apsis_is_at_the_true_extremum() {
    let a = R_EARTH + 400_000.0;
    let got = scan(orbit(a, 0.05, 0.0, 1.0), 0.0, 5_550.0, 300.0, |_| EARTH);
    let ap = got.iter().find(|x| x.is_apo).expect("an apoapsis");
    assert!((ap.t.seconds_since(t0()) - 2_775.0).abs() < 1e-3, "{ap:?}");
    assert!((ap.distance - a * 1.05).abs() < 1e-3);
    assert!((ap.altitude - (a * 1.05 - R_EARTH)).abs() < 1e-3);
}

#[test]
fn a_change_of_dominant_body_splits_the_runs() {
    // The minimum at t = 0 falls exactly on the switch: it is neither
    // run's extremum.
    let got =
        scan(|t: f64| (1e7 + 100.0 * t * t, 200.0 * t), -100.0, 100.0, 1.0, |t| if t < 0.0 { EARTH } else { MOON });
    assert!(got.is_empty(), "{got:?}");
    let got = scan(|t: f64| (1e7 + 100.0 * t * t, 200.0 * t), -100.0, 100.0, 1.0, |_| EARTH);
    assert_eq!(got.len(), 1);
}

#[test]
fn next_after_skips_the_past() {
    let at = |s: f64, is_apo| Apsis { is_apo, t: t0().add_seconds(s), body: EARTH, distance: 0.0, altitude: 0.0 };
    let v =
        VesselApsides { list: vec![at(-10.0, true), at(5.0, false), at(20.0, true), at(40.0, false)], impact: None };
    assert_eq!(v.times_to_next(t0()), (Some(20.0), Some(5.0)));
    assert_eq!(v.times_to_next(t0().add_seconds(30.0)), (None, Some(10.0)));
}

#[test]
fn refine_root_finds_a_crossing() {
    let g = |x: f64| Some(x * x * x - 2.0);
    let x = refine_root(g, 0.0, 3.0, -2.0, 25.0, 1e-12);
    assert!((x - 2f64.cbrt()).abs() < 1e-9, "{x}");
}

/// Apsides of a real coast agree with the conic in low orbit (the orbit is
/// nearly Keplerian over one revolution) and are cached.
#[test]
fn a_real_low_orbit_has_one_ap_and_pe_per_revolution() {
    let mut sim = SimState::new();
    sim.spawn_test_ships(1);
    let i = sim.fleet.len() - 1;
    let world = sim.world.clone();
    let until = sim.clock.add_seconds(3.0 * 5_600.0);
    sim.fleet[i].extend_coast(&world, until, 200_000);
    let pred = Prediction::default();
    let got = Apsides::compute(&sim, &pred, i);
    let earth = sim.world.find("Earth").unwrap().node;
    let (apo, peri): (Vec<&Apsis>, Vec<&Apsis>) = got.list.iter().partition(|a| a.is_apo);
    assert!((2..=4).contains(&apo.len()) && (2..=4).contains(&peri.len()), "{got:?}");
    // Osculating e = 0.001 at a = 6778 km, but under J2 the orbit is nearly
    // frozen: the distance swings ≈3 km once per revolution (Ap ≈ 393 km,
    // Pe ≈ 390 km above the equatorial radius) — the conic's 407/393 km is
    // not what happens.
    for a in &apo {
        assert!(a.body == earth && (a.altitude - 393_200.0).abs() < 2_000.0, "{a:?}");
    }
    for a in &peri {
        assert!((a.altitude - 390_200.0).abs() < 2_000.0, "{a:?}");
    }
    assert!(got.impact.is_none());
    // The cache matches a fresh computation, and extends incrementally.
    let mut cache = Apsides::default();
    cache.refresh(&sim, &pred, None);
    assert_eq!(cache.get(sim.fleet[i].id()), Some(&got));
    let until = sim.clock.add_seconds(5.0 * 5_600.0);
    sim.fleet[i].extend_coast(&world, until, 200_000);
    cache.refresh(&sim, &pred, None);
    assert_eq!(cache.get(sim.fleet[i].id()), Some(&Apsides::compute(&sim, &pred, i)));
    assert!(cache.get(sim.fleet[i].id()).unwrap().list.len() > got.list.len());
}

/// A suborbital coast: an apoapsis, then the surface; no periapsis.
#[test]
fn a_suborbital_coast_ends_in_an_impact_not_a_periapsis() {
    let mut sim = SimState::new();
    let earth = sim.world.find("Earth").unwrap().clone();
    let el = Elements { a: 6_000_000.0, e: 0.1, i: 0.5, raan: 0.0, argp: 0.0, mean_anomaly: 3.0 };
    let (r, v) = el.to_state(earth.gm);
    let id = sim.vessel_ids.allocate();
    let vessel = Vessel::coasting(&sim.world, id, sim.clock, earth.node, r, v, sim::craft::test_craft());
    sim.fleet.push(vessel);
    let i = sim.fleet.len() - 1;
    let world = sim.world.clone();
    let until = sim.clock.add_seconds(20_000.0);
    sim.fleet[i].extend_coast(&world, until, 200_000);
    let got = Apsides::compute(&sim, &Prediction::default(), i);
    let impact = got.impact.expect("an impact");
    assert_eq!(impact.body, earth.node);
    assert!(got.list.iter().all(|a| a.is_apo && a.t.seconds_since(impact.t) < 0.0), "{got:?}");
}
