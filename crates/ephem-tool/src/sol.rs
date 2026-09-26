//! Solar System generation from DE440.

use anise::constants::frames::{
    EARTH_J2000, JUPITER_BARYCENTER_J2000, MARS_BARYCENTER_J2000, MERCURY_J2000, MOON_J2000, NEPTUNE_BARYCENTER_J2000,
    PLUTO_BARYCENTER_J2000, SATURN_BARYCENTER_J2000, SSB_J2000, SUN_J2000, URANUS_BARYCENTER_J2000, VENUS_J2000,
};
use anise::prelude::{Almanac, Frame};
use glam::DVec3;
use serde::Serialize;
use sim::ephem::fnv1a64;
use sim::gen::{generate, InitialBody};
use sim::sol::{sol_epoch, InitialBodyRecord, InitialConditions, ReferenceFile, ReferenceState};
use sim::time::Epoch;
use std::path::Path;

/// Bodies integrated, with DE440 GM values (km³/s², from the DE440 header).
/// Mars…Pluto are system barycenters (their moons are not modelled yet).
const BODIES: [(&str, Frame, f64); 11] = [
    ("Sun", SUN_J2000, 132_712_440_041.279_42),
    ("Mercury", MERCURY_J2000, 22_031.868_551),
    ("Venus", VENUS_J2000, 324_858.592),
    ("Earth", EARTH_J2000, 398_600.435_507),
    ("Moon", MOON_J2000, 4_902.800_118),
    ("Mars", MARS_BARYCENTER_J2000, 42_828.375_816),
    ("Jupiter", JUPITER_BARYCENTER_J2000, 126_712_764.1),
    ("Saturn", SATURN_BARYCENTER_J2000, 37_940_584.841_8),
    ("Uranus", URANUS_BARYCENTER_J2000, 5_794_556.4),
    ("Neptune", NEPTUNE_BARYCENTER_J2000, 6_836_527.100_58),
    ("Pluto", PLUTO_BARYCENTER_J2000, 975.5),
];

/// Reference dates (years after the epoch) for the golden comparison.
const REFERENCE_YEARS: [f64; 5] = [0.0, 1.0, 5.0, 10.0, 50.0];

pub fn run(bsp: &Path, years: f64, extras: &str) {
    let almanac = Almanac::default()
        .load(bsp.to_str().expect("utf-8 path"))
        .unwrap_or_else(|e| panic!("cannot load {}: {e}", bsp.display()));
    let epoch = sol_epoch();

    let initial = InitialConditions {
        epoch_tdb_seconds: epoch.whole_seconds(),
        bodies: BODIES
            .iter()
            .map(|(name, frame, gm_km3)| {
                let (r, v) = state(&almanac, *frame, epoch);
                InitialBodyRecord { name: (*name).into(), gm: gm_km3 * 1e9, r: r.to_array(), v: v.to_array() }
            })
            .collect(),
    };
    write_ron("data/ephemeris/sol-2030-initial.ron", &initial);

    let reference = ReferenceFile {
        states: REFERENCE_YEARS
            .iter()
            .flat_map(|&y| {
                let t = epoch.add_seconds(y * sim::time::SECONDS_PER_JULIAN_YEAR);
                let almanac = &almanac;
                BODIES.iter().map(move |(name, frame, _)| {
                    let (r, v) = state(almanac, *frame, t);
                    ReferenceState { name: (*name).into(), years: y, r: r.to_array(), v: v.to_array() }
                })
            })
            .collect(),
    };
    write_ron("data/ephemeris/de440-reference.ron", &reference);

    let bodies: Vec<InitialBody> = initial.to_bodies();
    let tree = sim::sol::tree(&bodies);
    let mut cfg = sim::sol::gen_config(&bodies, years);
    // Diagnostics: isolate the effect of each extra force (never written as the
    // shipped ephemeris unless "all").
    match extras {
        "all" => {}
        "none" => cfg.extras = Default::default(),
        "gr" => cfg.extras.oblate.clear(),
        "j2" => cfg.extras.relativistic_center = None,
        other => panic!("unknown --extras {other}"),
    }
    let started = std::time::Instant::now();
    let (eph, report) = generate(&bodies, &tree, &cfg);
    println!("generated in {:.1} s", started.elapsed().as_secs_f64());
    for r in &report {
        println!(
            "{:8} {:5} seg={:>9} s  table_err={:9.3e} m  rails_err={:9.3e} m",
            r.name, r.motion, r.seg_len, r.table_max_err, r.rails_max_err
        );
    }
    let bytes = eph.to_bytes();
    if extras == "all" {
        std::fs::write("data/ephemeris/sol-2030.bin", &bytes).expect("write ephemeris");
        println!("wrote data/ephemeris/sol-2030.bin: {} bytes, hash {:#018x}", bytes.len(), fnv1a64(&bytes));
    }
    compare(&eph, &reference);
}

fn state(almanac: &Almanac, frame: Frame, t: Epoch) -> (DVec3, DVec3) {
    let e = anise::prelude::Epoch::from_tdb_seconds(t.to_seconds_f64());
    let s = almanac.translate(frame, SSB_J2000, e, None).expect("translate");
    let r = DVec3::new(s.radius_km.x, s.radius_km.y, s.radius_km.z) * 1e3;
    let v = DVec3::new(s.velocity_km_s.x, s.velocity_km_s.y, s.velocity_km_s.z) * 1e3;
    (r, v)
}

fn write_ron<T: Serialize>(path: &str, value: &T) {
    let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default()).expect("serialize");
    std::fs::write(path, text).unwrap_or_else(|e| panic!("write {path}: {e}"));
    println!("wrote {path}");
}

/// Prints the error of the generated ephemeris against DE440 for a few
/// relative vectors that matter for gameplay.
fn compare(eph: &sim::ephem::Ephemeris, reference: &ReferenceFile) {
    println!("\nerror vs DE440 (position, m):");
    for pair in [("Moon", "Earth"), ("Earth", "Sun"), ("Mars", "Sun"), ("Jupiter", "Sun"), ("Neptune", "Sun")] {
        let line: Vec<String> = REFERENCE_YEARS
            .iter()
            .map(|&y| format!("+{y}y {:9.3e}", sim::sol::reference_error(eph, reference, pair.0, pair.1, y)))
            .collect();
        println!("  {:>7} rel {:<5} {}", pair.0, pair.1, line.join("  "));
    }
}
