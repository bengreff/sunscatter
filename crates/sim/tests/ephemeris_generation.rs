//! Generation pipeline on a synthetic star / planet / planet+moon system:
//! classifier choices, fit accuracy, serialization, and determinism.

use glam::DVec3;
use sim::ephem::{fnv1a64, Ephemeris, NodeKind};
use sim::gen::{generate, GenConfig, InitialBody, NodeReport, NodeSpec};
use sim::kepler::Elements;
use sim::time::Epoch;

const GM_STAR: f64 = 1.327_124_400_18e20;
const GM_EARTH: f64 = 3.986_004_418e14;
const GM_MOON: f64 = 4.902_800_1e12;

fn system() -> (Vec<InitialBody>, Vec<NodeSpec>) {
    let circ =
        |a: f64, mu: f64, i: f64| Elements { a, e: 0.02, i, raan: 0.3, argp: 1.0, mean_anomaly: 0.5 }.to_state(mu);
    // Lonely planet far out (nearly Keplerian around the star).
    let (rp, vp) = circ(4.5e12, GM_STAR, 0.02);
    // Planet + moon pair.
    let (remb, vemb) = circ(1.496e11, GM_STAR, 0.0);
    let (rm, vm) = circ(3.844e8, GM_EARTH + GM_MOON, 0.09);
    let f = GM_MOON / (GM_EARTH + GM_MOON);
    let bodies = vec![
        InitialBody { name: "Star".into(), gm: GM_STAR, r: DVec3::ZERO, v: DVec3::ZERO },
        InitialBody { name: "Far".into(), gm: 1.0e10, r: rp, v: vp },
        InitialBody { name: "Planet".into(), gm: GM_EARTH, r: remb - rm * f, v: vemb - vm * f },
        InitialBody { name: "Moon".into(), gm: GM_MOON, r: remb + rm * (1.0 - f), v: vemb + vm * (1.0 - f) },
    ];
    let node = |name: &str, kind, parent, members: Vec<usize>, tol| NodeSpec {
        name: name.into(),
        kind,
        parent,
        members,
        tolerance: tol,
    };
    let tree = vec![
        node("SSB", NodeKind::Barycenter, None, vec![0, 1, 2, 3], 0.0),
        node("Star", NodeKind::Body, Some(0), vec![0], 1.0),
        // Outer body: parent is the barycenter of everything inside its orbit.
        node("Far", NodeKind::Body, Some(0), vec![1], 1000.0),
        node("PMB", NodeKind::Barycenter, Some(1), vec![2, 3], 1000.0),
        node("Planet", NodeKind::Body, Some(3), vec![2], 1.0),
        node("Moon", NodeKind::Body, Some(3), vec![3], 1.0),
    ];
    (bodies, tree)
}

fn config(years: f64) -> GenConfig {
    let start = Epoch::from_calendar(2030, 1, 1, 0, 0, 0.0);
    GenConfig {
        generator: "test-v1".into(),
        start,
        end: start.add_seconds((years * 365.25 * 86_400.0).round()),
        step: 3600,
        degree: 14,
        trial_steps: 24 * 200,
        rails_stride: 24,
    }
}

fn find<'a>(report: &'a [NodeReport], name: &str) -> &'a NodeReport {
    report.iter().find(|r| r.name == name).unwrap()
}

#[test]
fn classifier_picks_rails_for_keplerian_and_table_for_perturbed() {
    let (bodies, tree) = system();
    let (_, report) = generate(&bodies, &tree, &config(1.0));
    for r in &report {
        println!(
            "{:8} {:5} seg={:>8}s table_err={:.3e} rails_err={:.3e}",
            r.name, r.motion, r.seg_len, r.table_max_err, r.rails_max_err
        );
    }
    assert_eq!(find(&report, "Far").motion, "rails");
    assert_eq!(find(&report, "Moon").motion, "table");
    for r in &report {
        let tol = tree.iter().find(|n| n.name == r.name).unwrap().tolerance;
        let chosen_err = if r.motion == "rails" { r.rails_max_err } else { r.table_max_err };
        assert!(chosen_err <= tol, "{} error {} > tolerance {}", r.name, chosen_err, tol);
    }
}

#[test]
fn serialization_round_trips_exactly() {
    let (bodies, tree) = system();
    let (eph, _) = generate(&bodies, &tree, &config(0.3));
    let bytes = eph.to_bytes();
    let back = Ephemeris::from_bytes(&bytes).unwrap();
    assert_eq!(back, eph);
    assert_eq!(back.to_bytes(), bytes);
}

#[test]
fn generation_is_deterministic_and_matches_golden_hash() {
    let (bodies, tree) = system();
    let a = generate(&bodies, &tree, &config(0.3)).0.to_bytes();
    let b = generate(&bodies, &tree, &config(0.3)).0.to_bytes();
    assert_eq!(a, b);
    // Cross-platform determinism: CI runs this on macOS (ARM64) and Windows
    // (x86-64). If this changes intentionally, update the constant.
    let hash = fnv1a64(&a);
    println!("golden hash: {hash:#018x}");
    assert_eq!(hash, GOLDEN_HASH, "generation output changed (or differs on this platform)");
}

const GOLDEN_HASH: u64 = 0xda43c59162cf1402;

#[test]
fn table_and_integration_agree_between_samples() {
    // The Moon relative to the planet–moon barycenter must be continuous and
    // smooth: compare the table's velocity against a centred difference.
    let (bodies, tree) = system();
    let (eph, _) = generate(&bodies, &tree, &config(0.3));
    let moon = eph.find("Moon").unwrap();
    let pmb = eph.find("PMB").unwrap();
    for k in 0..200 {
        let t = eph.start.add_seconds(1234.5 + k as f64 * 40_000.0);
        let dt = 1.0;
        let k0 = eph.relative(moon, pmb, t);
        let rp = eph.relative(moon, pmb, t.add_seconds(dt)).r;
        let rm = eph.relative(moon, pmb, t.add_seconds(-dt)).r;
        let fd = (rp - rm) / (2.0 * dt);
        assert!((fd - k0.v).length() < 1e-4, "velocity mismatch {} at step {k}", (fd - k0.v).length());
    }
}
