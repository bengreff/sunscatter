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
        extras: Default::default(),
        free_segments: false,
        quantize: false,
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
/// The moon system test's output: external tides, J2–J6, free segment
/// lengths, quantized and packed tables (CI: macOS and Windows).
const HIERARCHICAL_GOLDEN: u64 = 0x73af5485187aba10;

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

/// The hierarchical pipeline (motion-model §1, D084): the planet–moon pair
/// as one point mass in the star's integration, then the pair alone in its
/// barycenter's frame under the star's tide from that ephemeris, grafted
/// under the barycenter node. It must match one integration of all three
/// bodies: the moon about the planet to the fits' tolerance; about the star
/// to the one approximation, the star's pull on the pair's quadrupole,
/// which the point-mass barycenter misses (about 1.5 GM☉ μ r²/d⁴: 5e-10
/// m/s² for this Earth–Moon-like pair at 1 AU, ~100 m in 0.3 years, which
/// is why Earth and the Moon stay in the planetary integration; 2e-13 m/s²
/// for Jupiter's moons, 2e-14 for Saturn's, as DE440 also treats those
/// systems as point masses). The quantized, free-length tables must stay
/// within tolerance and read back bit for bit.
#[test]
fn a_moon_system_in_its_barycenter_frame_matches_one_full_integration() {
    use sim::gen::{External, Extras};
    use sim::sol::systems::graft;
    use std::sync::Arc;
    let (bodies, full_tree) = system();
    let full_tree: Vec<NodeSpec> = full_tree.into_iter().filter(|n| n.name != "Far").collect();
    let bodies: Vec<InitialBody> = bodies.into_iter().filter(|b| b.name != "Far").collect();
    // Indices after dropping "Far": star 0, planet 1, moon 2.
    let full_tree: Vec<NodeSpec> = full_tree
        .into_iter()
        .map(|mut n| {
            n.members = n.members.iter().map(|&m| if m > 1 { m - 1 } else { m }).collect();
            n.parent = n.parent.map(|p| if p > 2 { p - 1 } else { p });
            n
        })
        .collect();
    let mut cfg = config(0.3);
    (cfg.free_segments, cfg.quantize) = (true, true);
    // A strongly oblate planet: the zonal terms are part of what the golden
    // hash pins down.
    let oblate = |body| sim::gen::Oblate {
        body,
        j2: 1.0e-3,
        j4: -1.0e-4,
        j6: 1.0e-5,
        radius: 6.4e6,
        rotation: sim::body::Rotation {
            ra: 0.3,
            dec: 1.2,
            ra_rate: 1e-12,
            dec_rate: 0.0,
            w0: 0.0,
            w_rate: 7e-5,
            librations: sim::body::Rotation::NO_LIBRATIONS,
            n_librations: 0,
        },
    };
    let mut full_cfg = cfg.clone();
    full_cfg.extras.oblate = vec![oblate(1)];
    let (full, _) = generate(&bodies, &full_tree, &full_cfg);

    // The star and the pair's barycenter as one mass.
    let f = bodies[2].gm / (bodies[1].gm + bodies[2].gm);
    let pair = |x: DVec3, y: DVec3| x * (1.0 - f) + y * f;
    let pmb = InitialBody {
        name: "PMB".into(),
        gm: bodies[1].gm + bodies[2].gm,
        r: pair(bodies[1].r, bodies[2].r),
        v: pair(bodies[1].v, bodies[2].v),
    };
    let outer = vec![bodies[0].clone(), pmb.clone()];
    let node = |name: &str, kind, parent, members: Vec<usize>, tol| NodeSpec {
        name: name.into(),
        kind,
        parent,
        members,
        tolerance: tol,
    };
    let outer_tree = vec![
        node("SSB", NodeKind::Barycenter, None, vec![0, 1], 0.0),
        node("Star", NodeKind::Body, Some(0), vec![0], 1.0),
        node("PMB", NodeKind::Barycenter, Some(1), vec![1], 1.0),
    ];
    let (planets, _) = generate(&outer, &outer_tree, &cfg);
    let planets = Arc::new(planets);
    let inner: Vec<InitialBody> =
        bodies[1..].iter().map(|b| InitialBody { r: b.r - pmb.r, v: b.v - pmb.v, ..b.clone() }).collect();
    let inner_tree = vec![
        node("PMB", NodeKind::Barycenter, None, vec![0, 1], 0.0),
        node("Planet", NodeKind::Body, Some(0), vec![0], 1.0),
        node("Moon", NodeKind::Body, Some(0), vec![1], 1.0),
    ];
    let star = planets.find("Star").unwrap();
    let mut inner_cfg = cfg.clone();
    inner_cfg.extras = Extras {
        external: Some(External {
            eph: planets.clone(),
            origin: planets.find("PMB").unwrap(),
            perturbers: vec![(star, planets.node(star).gm)],
            relativistic: None,
        }),
        oblate: vec![oblate(0)],
        ..Extras::default()
    };
    let (system, report) = generate(&inner, &inner_tree, &inner_cfg);
    for r in &report {
        println!("{:7} seg={:>8}s err={:.3e} m", r.name, r.seg_len, r.table_max_err);
        assert!(r.table_max_err < 1.0, "{} fit error {}", r.name, r.table_max_err);
    }
    let members = system.nodes()[1..].to_vec();
    let hier = graft(&planets, "PMB", members, "test system");
    assert_eq!(hier.nodes().len(), 5);
    let back = Ephemeris::from_bytes(&hier.to_bytes()).unwrap();
    assert_eq!(back.to_bytes(), hier.to_bytes());
    let id = |e: &Ephemeris, n: &str| e.find(n).unwrap();
    let (mut worst, mut worst_star): (f64, f64) = (0.0, 0.0);
    for k in 0..60 {
        let t = full.start.add_seconds(1_000.0 + k as f64 * 150_000.0);
        let ours = back.relative(id(&back, "Moon"), id(&back, "Planet"), t);
        let theirs = full.relative(id(&full, "Moon"), id(&full, "Planet"), t);
        worst = worst.max((ours.r - theirs.r).length());
        let sun_dist = back.relative(id(&back, "Moon"), id(&back, "Star"), t).r;
        let sun_full = full.relative(id(&full, "Moon"), id(&full, "Star"), t).r;
        worst_star = worst_star.max((sun_dist - sun_full).length());
    }
    println!("hierarchical vs full: moon about the planet {worst:.3} m, about the star {worst_star:.1} m");
    assert!(worst < 3.0, "{worst} m");
    assert!(worst_star < 300.0, "{worst_star} m");
    let hash = fnv1a64(&hier.to_bytes());
    println!("hierarchical golden hash: {hash:#018x}");
    assert_eq!(hash, HIERARCHICAL_GOLDEN, "generation output changed (or differs on this platform)");
}
