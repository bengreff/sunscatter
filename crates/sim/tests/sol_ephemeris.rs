//! Golden tests for the shipped Solar System ephemeris (data/ephemeris/).
//!
//! * The shipped file must be exactly what the current code generates from the
//!   committed inputs (initial states, catalog, body data): bit-for-bit prefix
//!   checks on every table, planets and moon systems. This fails if the code
//!   changes without regenerating, and fails on any platform whose floating
//!   point differs (CI: macOS + Windows).
//! * Accuracy against JPL DE440 and Horizons is a regression guard, not a
//!   requirement (D026: determinism matters, matching reality is a bonus).
//!   Bounds sit just above the measured errors (docs/technical/solar-system.md).

use sim::ephem::{ChebTable, Ephemeris, Motion, Node};
use sim::sol::{self, HorizonsReference, ReferenceFile};
use std::path::Path;
use std::sync::Arc;

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ephemeris")
}

fn shipped() -> Ephemeris {
    Ephemeris::from_bytes(&std::fs::read(dir().join("sol-2030.bin")).expect("read ephemeris")).expect("parse ephemeris")
}

/// `regen`'s table is a bit-for-bit prefix of `full`'s.
fn assert_prefix(regen: &Node, full: &Node) {
    assert_eq!(regen.name, full.name);
    assert_eq!(regen.gm.to_bits(), full.gm.to_bits(), "{}", regen.name);
    let table = |m: &Motion| match m {
        Motion::Table(t) => t.clone(),
        other => panic!("{}: {} instead of a table", regen.name, other.kind_name()),
    };
    let (x, y): (ChebTable, ChebTable) = (table(&regen.motion), table(&full.motion));
    assert_eq!(
        (x.seg_len, x.degree, x.quantum.to_bits()),
        (y.seg_len, y.degree, y.quantum.to_bits()),
        "{}",
        regen.name
    );
    assert!(!x.coeffs.is_empty() && x.coeffs.len() <= y.coeffs.len(), "{}", regen.name);
    let same = x.coeffs.iter().zip(&y.coeffs).all(|(p, q)| p.to_bits() == q.to_bits());
    assert!(same, "{}: shipped table differs from regeneration", regen.name);
}

#[test]
fn shipped_planets_match_regeneration_bit_for_bit() {
    let (initial, catalog, _) = sol::read_inputs(&dir());
    let bodies = initial.to_bodies();
    // Two years: equal to the trial span, so segment choices are identical to
    // the full run's and every table here is a prefix of the shipped one.
    let tree = sol::tree(&bodies, &catalog);
    let (regen, _) = sim::gen::generate(&bodies, &tree, &sol::gen_config(&bodies, 2.0));
    let full = shipped();
    assert!(full.generator.starts_with(&regen.generator), "{} / {}", full.generator, regen.generator);
    assert!(matches!(full.nodes()[0].motion, Motion::Root));
    for (a, b) in regen.nodes().iter().zip(full.nodes()).skip(1) {
        assert_prefix(a, b);
    }
}

/// Each moon system regenerated over a little more than its longest segment,
/// with the shipped segment lengths, under the shipped planets' tides.
#[test]
fn shipped_moon_systems_match_regeneration_bit_for_bit() {
    let (initial, catalog, systems_initial) = sol::read_inputs(&dir());
    let full = shipped();
    let n_planets = sol::tree(&initial.to_bodies(), &catalog).len();
    let planets =
        Arc::new(Ephemeris::new(full.generator.clone(), full.start, full.end, full.nodes()[..n_planets].to_vec()));
    let bodies_dir = sim::body::default_bodies_dir();
    for spec in &catalog.systems {
        let init = systems_initial.systems.iter().find(|s| s.barycenter == spec.barycenter).unwrap();
        let primary = sol::primary_physical(spec, &bodies_dir);
        let shipped_member = |name: &str| full.node(full.find(name).unwrap_or_else(|| panic!("no {name}"))).clone();
        let seg_len = |name: &str| match &shipped_member(name).motion {
            Motion::Table(t) => t.seg_len,
            _ => panic!("{name} is a table"),
        };
        let mut seg_steps = vec![0];
        seg_steps.extend(spec.members.iter().map(|m| (seg_len(&m.name) / spec.step) as usize));
        let longest = spec.members.iter().map(|m| seg_len(&m.name)).max().unwrap() as f64;
        let years = (longest + 86_400.0) / sim::time::SECONDS_PER_JULIAN_YEAR;
        let cfg = spec.config(&planets, &primary, years);
        assert!(full.generator.contains(&cfg.generator), "{}", cfg.generator);
        let (regen, _) = sim::gen::generate_with_segments(&spec.bodies(init), &spec.tree(), &cfg, &seg_steps);
        for node in &regen.nodes()[1..] {
            assert_prefix(node, &shipped_member(&node.name));
        }
        // The grafted members hang from the barycenter node.
        let bary = full.find(&spec.barycenter).unwrap();
        assert!(spec.members.iter().all(|m| full.node(full.find(&m.name).unwrap()).parent == Some(bary)));
    }
}

#[test]
fn accuracy_against_de440_is_within_recorded_bounds() {
    let eph = shipped();
    let reference: ReferenceFile =
        ron::from_str(&std::fs::read_to_string(dir().join("de440-reference.ron")).unwrap()).unwrap();
    // (a, b, years, bound in metres)
    let cases = [
        ("Moon", "Earth", 0.0, 0.1),
        ("Moon", "Earth", 1.0, 5.0e3),
        ("Moon", "Earth", 10.0, 5.0e4),
        ("Earth", "Sun", 1.0, 200.0),
        ("Earth", "Sun", 50.0, 6.0e3),
        ("Mars", "Sun", 1.0, 500.0),
        ("Mars", "Sun", 50.0, 5.0e4),
        ("Jupiter", "Sun", 10.0, 2.0e4),
        ("Neptune", "Sun", 50.0, 3.0e5),
    ];
    for (a, b, years, bound) in cases {
        let err = sol::reference_error(&eph, &reference, a, b, years);
        println!("{a} rel {b} +{years}y: {err:.3e} m (bound {bound:.1e})");
        assert!(err < bound, "{a} rel {b} at +{years}y: {err} m > {bound} m");
    }
}

/// Every Horizons reference state (2030 to 2080) within the worst error
/// measured with a margin (docs/technical/solar-system.md): the moons'
/// initial states are fitted to JPL's satellite ephemerides over the whole
/// window (ephem-tool fit), which trades the epoch's metre for no drift, so
/// what remains is model differences (Charon 110 m, Titan 8 km, Mimas
/// 390 km; Proteus, not fitted, 320 km).
#[test]
fn accuracy_against_horizons_is_within_recorded_bounds() {
    let eph = shipped();
    let refs: HorizonsReference =
        ron::from_str(&std::fs::read_to_string(dir().join("horizons-reference.ron")).unwrap()).unwrap();
    // Worst over 1-50 years, measured 2026-10-26, times 1.5 (m).
    let bound = |name: &str| {
        1.5 * match name {
            "Triton" | "Nereid" | "Mimas" | "Ceres" | "Proteus" => 5.0e5,
            "Tethys" => 1.8e5,
            "Europa" | "Titania" | "Oberon" | "Miranda" => 5.5e4,
            "Io" | "Ganymede" | "Ariel" | "Makemake" | "Haumea barycenter" => 3.6e4,
            "Dione" | "Hyperion" | "Gonggong" | "Quaoar" | "Sedna" | "Orcus barycenter" => 2.2e4,
            "Eris barycenter" | "Umbriel" | "Rhea" | "Callisto" | "Titan" => 1.2e4,
            "Enceladus" | "Iapetus" | "Phoebe" => 5.0e3,
            "Charon" => 200.0,
            other => panic!("no bound for {other}"),
        }
    };
    for s in &refs.states {
        let err = sol::relative_error(&eph, s);
        let limit = bound(&s.name);
        assert!(err < limit, "{} rel {} at +{}y: {err:.3e} m > {limit:.1e} m", s.name, s.center, s.years);
    }
    assert!(refs.states.len() > 150, "the references cover every body");
}

#[test]
fn window_covers_fifty_years_from_2030() {
    let eph = shipped();
    assert_eq!(eph.start, sol::sol_epoch());
    assert!(eph.end.seconds_since(eph.start) >= 49.9 * sim::time::SECONDS_PER_JULIAN_YEAR);
    for n in eph.nodes() {
        if let Motion::Table(t) = &n.motion {
            let covered = t.seg_len as f64 * t.n_segments() as f64;
            assert!(covered >= eph.end.seconds_since(eph.start), "{} table too short", n.name);
        }
    }
}

/// D084's first list, every body with its data (D051: a directory each).
#[test]
fn the_solar_system_has_every_planet_dwarf_planet_and_moon_over_100_km() {
    let w = sim::world::World::sol(Arc::new(shipped()));
    let names = [
        "Sun",
        "Mercury",
        "Venus",
        "Earth",
        "Moon",
        "Mars",
        "Jupiter",
        "Io",
        "Europa",
        "Ganymede",
        "Callisto",
        "Saturn",
        "Mimas",
        "Enceladus",
        "Tethys",
        "Dione",
        "Rhea",
        "Titan",
        "Hyperion",
        "Iapetus",
        "Phoebe",
        "Uranus",
        "Miranda",
        "Ariel",
        "Umbriel",
        "Titania",
        "Oberon",
        "Neptune",
        "Triton",
        "Proteus",
        "Nereid",
        "Pluto",
        "Charon",
        "Ceres",
        "Eris",
        "Dysnomia",
        "Haumea",
        "Hi'iaka",
        "Makemake",
        "Gonggong",
        "Quaoar",
        "Sedna",
        "Orcus",
        "Vanth",
    ];
    for name in names {
        let s = w.find(name).unwrap_or_else(|| panic!("{name} missing"));
        let p = s.physical.as_ref().unwrap_or_else(|| panic!("{name} has no body.ron"));
        assert!(p.radius_eq >= 100_000.0 || name == "Hyperion", "{name}: radius {}", p.radius_eq);
        assert!(s.gm > 0.0, "{name}");
    }
    let bodies = w.eph.bodies().count();
    assert_eq!(bodies, names.len(), "no unexpected bodies");
    // The file stays within its budget (D046: each file well under 50 MB).
    let size = std::fs::metadata(dir().join("sol-2030.bin")).unwrap().len();
    assert!(size < 20_000_000, "{size} bytes");
}

/// A closed moon system pulls as its stand-in (D023 clusters): the stand-in's
/// GM is its members' total, so where a cluster opens or closes the
/// gravity changes only by the system's quadrupole, under the rule's bound
/// (a hundredth of the cutoff), not by a mismatch of masses.
#[test]
fn opening_a_moon_system_changes_its_pull_by_less_than_the_rule_allows() {
    use glam::DVec3;
    let w = sim::world::World::sol(Arc::new(shipped()));
    let t = w.eph.start.add_seconds(1.0e8);
    let snap = w.snapshot(t);
    let root = w.eph.root();
    let budget = w.cutoff * sim::world::CLUSTER_ERROR_FRACTION;
    for (c, cl) in w.clusters.iter().enumerate() {
        let stand_in = &w.sources[cl.stand_in];
        let members: Vec<&sim::world::Source> =
            w.sources.iter().filter(|s| s.role == sim::world::Role::Member(c)).collect();
        let total: f64 = members.iter().map(|m| m.gm).sum();
        assert!((stand_in.gm - total).abs() <= 1e-12 * total, "{}: {} vs {total}", stand_in.name, stand_in.gm);
        // Just outside where it opens: (3 Q / (budget d^4)) = 1.
        let d_open = (3.0 * cl.quadrupole / budget).powf(0.25) * 1.01;
        let b = snap.relative_r(cl.node, root);
        for dir in [DVec3::X, DVec3::Y, DVec3::new(0.3, -0.5, 0.8).normalize()] {
            let r = b + dir * d_open;
            let pull = |gm: f64, at: DVec3| {
                let d = r - at;
                -d * (gm / (d.length_squared() * d.length()))
            };
            let point = pull(stand_in.gm, b);
            let resolved = members.iter().fold(DVec3::ZERO, |a, m| a + pull(m.gm, snap.relative_r(m.node, root)));
            let jump = (point - resolved).length();
            assert!(jump < budget, "{}: {jump:e} m/s² at {d_open:e} m (budget {budget:e})", stand_in.name);
        }
    }
}
