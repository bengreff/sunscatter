//! Solar System generation: planets from DE440, dwarf planets and moon
//! systems from JPL Horizons (`data/ephemeris/sol-systems.ron`).

use crate::horizons::{Query, EPOCH_JD, REFERENCE_YEARS};
use anise::constants::frames::{
    EARTH_J2000, JUPITER_BARYCENTER_J2000, MARS_BARYCENTER_J2000, MERCURY_J2000, MOON_J2000, NEPTUNE_BARYCENTER_J2000,
    PLUTO_BARYCENTER_J2000, SATURN_BARYCENTER_J2000, SSB_J2000, SUN_J2000, URANUS_BARYCENTER_J2000, VENUS_J2000,
};
use anise::prelude::{Almanac, Frame};
use glam::DVec3;
use serde::Serialize;
use sim::ephem::{fnv1a64, Ephemeris, Motion};
use sim::kepler::Elements;
use sim::sol::systems::{SystemInitial, SystemSpec};
use sim::sol::{
    sol_epoch, Catalog, HorizonsReference, InitialBodyRecord, InitialConditions, ReferenceFile, ReferenceState,
    RelativeState, SystemsInitial,
};
use sim::time::Epoch;
use std::path::Path;

/// Bodies integrated, with DE440 GM values (km³/s², from the DE440 header).
/// Mars…Pluto are system barycenters (moon systems are integrated after).
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

/// Reference dates (years after the epoch) for the DE440 comparison.
const DE440_YEARS: [f64; 5] = [0.0, 1.0, 5.0, 10.0, 50.0];

pub fn catalog() -> Catalog {
    let path = Path::new("data/ephemeris").join(sim::sol::CATALOG_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A dwarf planet's state at the epoch (SSB-centred, like DE440's).
fn dwarf_query(horizons: &str) -> Query {
    Query::at(horizons, "500@0", EPOCH_JD)
}

/// Its reference states, heliocentric: adding the dwarf planets' masses
/// moves the system barycenter (~15 km from DE440's), so states about the
/// SSB would differ by that, not by any error.
fn dwarf_reference_query(horizons: &str) -> Query {
    Query::at_reference_dates(horizons, "500@10")
}

fn member_query(sys: &SystemSpec, i: usize) -> Query {
    let m = &sys.members[i];
    match m.state_jd {
        Some(jd) => Query::at(&m.horizons, &sys.horizons_center, jd),
        None => Query::at_reference_dates(&m.horizons, &sys.horizons_center),
    }
}

/// Downloads every Horizons state the catalog needs (cached).
pub fn fetch() {
    let cat = catalog();
    for d in &cat.dwarfs {
        dwarf_query(&d.horizons).fetch();
        dwarf_reference_query(&d.horizons).fetch();
    }
    for s in &cat.systems {
        for i in 0..s.members.len() {
            member_query(s, i).fetch();
        }
    }
}

pub struct Options<'a> {
    pub bsp: &'a Path,
    pub years: f64,
    pub extras: &'a str,
    pub out: &'a Path,
    pub bodies: &'a Path,
}

pub fn run(opt: &Options) {
    let almanac = Almanac::default()
        .load(opt.bsp.to_str().expect("utf-8 path"))
        .unwrap_or_else(|e| panic!("cannot load {}: {e}", opt.bsp.display()));
    let epoch = sol_epoch();
    let cat = catalog();
    std::fs::create_dir_all(opt.out).expect("create the output directory");

    let mut initial = InitialConditions {
        epoch_tdb_seconds: epoch.whole_seconds(),
        bodies: BODIES
            .iter()
            .map(|(name, frame, gm_km3)| {
                let (r, v) = state(&almanac, *frame, epoch);
                InitialBodyRecord { name: (*name).into(), gm: gm_km3 * 1e9, r: r.to_array(), v: v.to_array() }
            })
            .collect(),
    };
    let mut horizons = HorizonsReference { states: Vec::new() };
    for d in &cat.dwarfs {
        let states = dwarf_query(&d.horizons).states();
        let (r, v) = states[0];
        initial.bodies.push(InitialBodyRecord {
            name: d.name.clone(),
            gm: d.gm * 1e9,
            r: r.to_array(),
            v: v.to_array(),
        });
        for (&years, (r, v)) in REFERENCE_YEARS.iter().zip(&dwarf_reference_query(&d.horizons).states()) {
            let (r, v) = (r.to_array(), v.to_array());
            horizons.states.push(RelativeState { name: d.name.clone(), center: "Sun".into(), years, r, v });
        }
    }
    write_ron(opt.out, "sol-2030-initial.ron", &initial);

    let reference = ReferenceFile {
        states: DE440_YEARS
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
    write_ron(opt.out, "de440-reference.ron", &reference);

    let systems = SystemsInitial {
        epoch_tdb_seconds: epoch.whole_seconds(),
        systems: cat.systems.iter().map(|s| system_initial(s, &mut horizons)).collect(),
    };
    write_ron(opt.out, sim::sol::SYSTEMS_INITIAL_FILE, &systems);
    write_ron(opt.out, "horizons-reference.ron", &horizons);

    let bodies = initial.to_bodies();
    let mut cfg = sim::sol::gen_config(&bodies, opt.years);
    // DEV: the new data's settings, until gen_config ships them.
    cfg.generator = format!(
        "sol-nbody-v4 yoshida8 h=3600s degree=14 +sun1pn +earthJ2(iau pole), {} bodies, free segments, quantized",
        bodies.len()
    );
    (cfg.free_segments, cfg.quantize) = (true, true);
    // Diagnostics: isolate the effect of each extra force (never written as
    // the shipped ephemeris unless "all").
    match opt.extras {
        "all" => {}
        "none" => cfg.extras = Default::default(),
        "gr" => cfg.extras.oblate.clear(),
        "j2" => cfg.extras.relativistic_center = None,
        other => panic!("unknown --extras {other}"),
    }
    let started = std::time::Instant::now();
    let (eph, report) = sim::sol::generate_sol_with(&cfg, &initial, &cat, &systems, opt.years, opt.bodies);
    println!("generated in {:.1} s", started.elapsed().as_secs_f64());
    for r in &report {
        let tol = node_tolerance(&cat, &r.name);
        let flag = if r.motion == "table" && r.table_max_err > tol { "  OVER TOLERANCE" } else { "" };
        println!(
            "{:18} {:5} seg={:>9} s ({:7.2} d)  table_err={:9.3e} m  rails_err={:9.3e} m  {:>9} B{flag}",
            r.name,
            r.motion,
            r.seg_len,
            r.seg_len as f64 / 86_400.0,
            r.table_max_err,
            r.rails_max_err,
            node_bytes(&eph, &r.name)
        );
    }
    let bytes = eph.to_bytes();
    let path = opt.out.join("sol-2030.bin");
    if opt.extras == "all" {
        std::fs::write(&path, &bytes).expect("write ephemeris");
        println!("wrote {}: {} bytes, hash {:#018x}", path.display(), bytes.len(), fnv1a64(&bytes));
    }
    compare(&eph, &reference);
    compare_horizons(&eph, &horizons);
}

/// A system's members at the epoch, relative to its barycenter, and their
/// Horizons reference states (each moon also about the primary).
fn system_initial(s: &SystemSpec, refs: &mut HorizonsReference) -> SystemInitial {
    let states: Vec<Vec<(DVec3, DVec3)>> = (0..s.members.len()).map(|i| member_query(s, i).states()).collect();
    let mut at_epoch: Vec<(DVec3, DVec3)> = states.iter().map(|v| v[0]).collect();
    if let Some(jd) = s.members[0].state_jd {
        assert!(s.members.iter().all(|m| m.state_jd == Some(jd)), "{}: one state date for all", s.barycenter);
        at_epoch = carry_to_epoch(s, &at_epoch, (EPOCH_JD - jd) * 86_400.0);
    } else {
        for (i, m) in s.members.iter().enumerate() {
            for (k, &years) in REFERENCE_YEARS.iter().enumerate() {
                let (r, v) = states[i][k];
                let (r, v) = (r.to_array(), v.to_array());
                refs.states.push(RelativeState { name: m.name.clone(), center: s.barycenter.clone(), years, r, v });
                if i > 0 {
                    let (rp, vp) = states[0][k];
                    let (r, v) = ((states[i][k].0 - rp).to_array(), (states[i][k].1 - vp).to_array());
                    refs.states.push(RelativeState {
                        name: m.name.clone(),
                        center: s.members[0].name.clone(),
                        years,
                        r,
                        v,
                    });
                }
            }
        }
    }
    SystemInitial {
        barycenter: s.barycenter.clone(),
        members: s
            .members
            .iter()
            .zip(&at_epoch)
            .map(|(m, (r, v))| InitialBodyRecord {
                name: m.name.clone(),
                gm: m.gm * 1e9,
                r: r.to_array(),
                v: v.to_array(),
            })
            .collect(),
    }
}

/// Barycentric states carried `dt` seconds on two-body conics: each
/// secondary about the primary (μ of the pair), the primary placed so the
/// barycenter stays at the origin.
fn carry_to_epoch(s: &SystemSpec, states: &[(DVec3, DVec3)], dt: f64) -> Vec<(DVec3, DVec3)> {
    let gm: Vec<f64> = s.members.iter().map(|m| m.gm * 1e9).collect();
    let total: f64 = gm.iter().sum();
    let rel: Vec<(DVec3, DVec3)> = states[1..]
        .iter()
        .zip(&gm[1..])
        .map(|((r, v), g)| {
            let mu = gm[0] + g;
            let mut el = Elements::from_state(*r - states[0].0, *v - states[0].1, mu);
            el.mean_anomaly = sim::math::wrap_tau(el.mean_anomaly + el.mean_motion(mu) * dt);
            el.to_state(mu)
        })
        .collect();
    let primary = rel
        .iter()
        .zip(&gm[1..])
        .fold((DVec3::ZERO, DVec3::ZERO), |acc, ((r, v), g)| (acc.0 - *r * (g / total), acc.1 - *v * (g / total)));
    std::iter::once(primary).chain(rel.iter().map(|(r, v)| (*r + primary.0, *v + primary.1))).collect()
}

fn node_tolerance(cat: &Catalog, name: &str) -> f64 {
    let member = cat.systems.iter().flat_map(|s| &s.members).find(|m| m.name == name).map(|m| m.tolerance);
    let dwarf = cat.dwarfs.iter().find(|d| d.name == name).map(|d| d.tolerance);
    member.or(dwarf).unwrap_or(match name {
        "Earth" | "Moon" => 1.0,
        "EMB" => 10.0,
        "Sun" => 100.0,
        _ => 1_000.0,
    })
}

/// Bytes a node's motion takes in the file (approximate for packed tables:
/// the serialized size of a one-node ephemeris).
fn node_bytes(eph: &Ephemeris, name: &str) -> usize {
    let Some(id) = eph.find(name) else { return 0 };
    let node = eph.node(id).clone();
    let one = Ephemeris::new(String::new(), eph.start, eph.end, vec![sim::ephem::Node { parent: None, ..node }]);
    match &eph.node(id).motion {
        Motion::Table(_) | Motion::Rails(_) => one.to_bytes().len(),
        _ => 0,
    }
}

fn state(almanac: &Almanac, frame: Frame, t: Epoch) -> (DVec3, DVec3) {
    let e = anise::prelude::Epoch::from_tdb_seconds(t.to_seconds_f64());
    let s = almanac.translate(frame, SSB_J2000, e, None).expect("translate");
    let r = DVec3::new(s.radius_km.x, s.radius_km.y, s.radius_km.z) * 1e3;
    let v = DVec3::new(s.velocity_km_s.x, s.velocity_km_s.y, s.velocity_km_s.z) * 1e3;
    (r, v)
}

fn write_ron<T: Serialize>(dir: &Path, name: &str, value: &T) {
    let path = dir.join(name);
    let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default()).expect("serialize");
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    println!("wrote {}", path.display());
}

/// Prints the error of the generated ephemeris against DE440 for a few
/// relative vectors that matter for gameplay.
fn compare(eph: &Ephemeris, reference: &ReferenceFile) {
    println!("\nerror vs DE440 (position, m):");
    for pair in [("Moon", "Earth"), ("Earth", "Sun"), ("Mars", "Sun"), ("Jupiter", "Sun"), ("Neptune", "Sun")] {
        let line: Vec<String> = DE440_YEARS
            .iter()
            .map(|&y| format!("+{y}y {:9.3e}", sim::sol::reference_error(eph, reference, pair.0, pair.1, y)))
            .collect();
        println!("  {:>7} rel {:<5} {}", pair.0, pair.1, line.join("  "));
    }
}

/// Prints the error against Horizons of every reference state, by body.
fn compare_horizons(eph: &Ephemeris, refs: &HorizonsReference) {
    println!("\nerror vs JPL Horizons (position, m):");
    let mut keys: Vec<(String, String)> = Vec::new();
    for s in &refs.states {
        let k = (s.name.clone(), s.center.clone());
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    for (name, center) in keys {
        let line: Vec<String> = refs
            .states
            .iter()
            .filter(|s| s.name == name && s.center == center)
            .map(|s| format!("+{}y {:9.3e}", s.years, sim::sol::relative_error(eph, s)))
            .collect();
        println!("  {name:>10} rel {center:<18} {}", line.join("  "));
    }
}
