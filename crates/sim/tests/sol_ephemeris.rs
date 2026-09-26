//! Golden tests for the shipped Solar System ephemeris (data/ephemeris/).
//!
//! * The shipped file must be exactly what the current code generates from the
//!   committed initial conditions (bit-for-bit prefix check on every table).
//!   This fails if the code changes without regenerating, and fails on any
//!   platform whose floating point differs (CI: macOS + Windows).
//! * Accuracy against JPL DE440 is a regression guard, not a requirement
//!   (D026: determinism matters, matching reality is a bonus). Bounds sit just
//!   above the measured errors recorded in the prototype plan.

use sim::ephem::{Ephemeris, Motion};
use sim::sol::{self, InitialConditions, ReferenceFile};

fn data(path: &str) -> String {
    format!("{}/../../data/ephemeris/{path}", env!("CARGO_MANIFEST_DIR"))
}

fn shipped() -> Ephemeris {
    Ephemeris::from_bytes(&std::fs::read(data("sol-2030.bin")).expect("read ephemeris")).expect("parse ephemeris")
}

#[test]
fn shipped_ephemeris_matches_regeneration_bit_for_bit() {
    let initial: InitialConditions =
        ron::from_str(&std::fs::read_to_string(data("sol-2030-initial.ron")).unwrap()).unwrap();
    let bodies = initial.to_bodies();
    // Two years: equal to the trial span, so segment choices are identical to
    // the full 50-year run and every table here is a prefix of the shipped one.
    let (regen, _) = sim::gen::generate(&bodies, &sol::tree(&bodies), &sol::gen_config(&bodies, 2.0));
    let full = shipped();
    assert_eq!(regen.generator, full.generator);
    for (a, b) in regen.nodes().iter().zip(full.nodes()) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.gm.to_bits(), b.gm.to_bits(), "{}", a.name);
        match (&a.motion, &b.motion) {
            (Motion::Root, Motion::Root) => {}
            (Motion::Table(x), Motion::Table(y)) => {
                assert_eq!((x.seg_len, x.degree), (y.seg_len, y.degree), "{}", a.name);
                assert!(!x.coeffs.is_empty());
                let prefix = &y.coeffs[..x.coeffs.len()];
                let same = x.coeffs.iter().zip(prefix).all(|(p, q)| p.to_bits() == q.to_bits());
                assert!(same, "{}: shipped table differs from regeneration", a.name);
            }
            (x, y) => panic!("{}: motion kinds differ ({} vs {})", a.name, x.kind_name(), y.kind_name()),
        }
    }
}

#[test]
fn accuracy_against_de440_is_within_recorded_bounds() {
    let eph = shipped();
    let reference: ReferenceFile =
        ron::from_str(&std::fs::read_to_string(data("de440-reference.ron")).unwrap()).unwrap();
    // (a, b, years, bound in metres)
    let cases = [
        ("Moon", "Earth", 0.0, 1e-3),
        ("Moon", "Earth", 1.0, 5.0e3),
        ("Moon", "Earth", 10.0, 5.0e4),
        ("Earth", "Sun", 1.0, 200.0),
        ("Earth", "Sun", 50.0, 1.0e4),
        ("Mars", "Sun", 1.0, 500.0),
        ("Jupiter", "Sun", 10.0, 2.0e4),
    ];
    for (a, b, years, bound) in cases {
        let err = sol::reference_error(&eph, &reference, a, b, years);
        println!("{a} rel {b} +{years}y: {err:.3e} m (bound {bound:.1e})");
        assert!(err < bound, "{a} rel {b} at +{years}y: {err} m > {bound} m");
    }
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
