//! The Solar System: generation inputs (tree, config) shared by `ephem-tool`
//! and the tests, plus loading of the generated ephemeris.

use crate::ephem::{Ephemeris, NodeKind};
use crate::gen::{Extras, GenConfig, InitialBody, NodeSpec, Oblate};
use crate::time::{Epoch, SECONDS_PER_JULIAN_YEAR};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Game start epoch: 2030-01-01T00:00:00 TDB (D027).
pub fn sol_epoch() -> Epoch {
    Epoch::from_calendar(2030, 1, 1, 0, 0, 0.0)
}

/// Initial conditions extracted from DE440 (SSB-centred ICRF, SI units).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InitialConditions {
    pub epoch_tdb_seconds: i64,
    pub bodies: Vec<InitialBodyRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InitialBodyRecord {
    pub name: String,
    pub gm: f64,
    pub r: [f64; 3],
    pub v: [f64; 3],
}

impl InitialConditions {
    pub fn to_bodies(&self) -> Vec<InitialBody> {
        self.bodies
            .iter()
            .map(|b| InitialBody {
                name: b.name.clone(),
                gm: b.gm,
                r: DVec3::from_array(b.r),
                v: DVec3::from_array(b.v),
            })
            .collect()
    }
}

/// DE440 states at reference dates (SSB-centred), for golden tests.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceFile {
    pub states: Vec<ReferenceState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceState {
    pub name: String,
    pub years: f64,
    pub r: [f64; 3],
    pub v: [f64; 3],
}

/// The node tree. Inner planets and the Earth–Moon barycenter orbit the Sun;
/// outer planets orbit the system barycenter (Jacobi-like, which keeps their
/// relative motion closest to a conic).
pub fn tree(bodies: &[InitialBody]) -> Vec<NodeSpec> {
    let idx = |name: &str| bodies.iter().position(|b| b.name == name).unwrap_or_else(|| panic!("missing {name}"));
    let all: Vec<usize> = (0..bodies.len()).collect();
    let body = |name: &str, parent: usize, tol: f64| NodeSpec {
        name: name.into(),
        kind: NodeKind::Body,
        parent: Some(parent),
        members: vec![idx(name)],
        tolerance: tol,
    };
    vec![
        NodeSpec { name: "SSB".into(), kind: NodeKind::Barycenter, parent: None, members: all, tolerance: 0.0 },
        body("Sun", 0, 100.0),
        body("Mercury", 1, 1_000.0),
        body("Venus", 1, 1_000.0),
        NodeSpec {
            name: "EMB".into(),
            kind: NodeKind::Barycenter,
            parent: Some(1),
            members: vec![idx("Earth"), idx("Moon")],
            tolerance: 10.0,
        },
        body("Earth", 4, 1.0),
        body("Moon", 4, 1.0),
        body("Mars", 1, 1_000.0),
        body("Jupiter", 0, 1_000.0),
        body("Saturn", 0, 1_000.0),
        body("Uranus", 0, 1_000.0),
        body("Neptune", 0, 1_000.0),
        body("Pluto", 0, 1_000.0),
    ]
}

/// Assumption set: Newtonian point masses for all bodies, the Sun's 1PN field,
/// and Earth's J2 about its IAU precessing pole.
pub fn gen_config(bodies: &[InitialBody], years: f64) -> GenConfig {
    let start = sol_epoch();
    let idx = |name: &str| bodies.iter().position(|b| b.name == name).expect(name);
    let earth = crate::body::earth();
    GenConfig {
        generator: "sol-nbody-v3 yoshida8 h=3600s degree=14 +sun1pn +earthJ2(iau pole)".into(),
        start,
        end: start.add_seconds((years * SECONDS_PER_JULIAN_YEAR).round()),
        step: 3600,
        degree: 14,
        trial_steps: 2 * 8766,
        rails_stride: 24,
        extras: Extras {
            relativistic_center: Some(idx("Sun")),
            oblate: vec![Oblate {
                body: idx("Earth"),
                j2: earth.j2,
                radius: earth.radius_eq,
                rotation: earth.rotation,
            }],
        },
    }
}

/// Position error (m) of `a` relative to `b` in `eph` against DE440.
pub fn reference_error(eph: &Ephemeris, reference: &ReferenceFile, a: &str, b: &str, years: f64) -> f64 {
    let find = |name: &str| {
        reference.states.iter().find(|s| s.name == name && s.years == years).unwrap_or_else(|| panic!("no ref {name}"))
    };
    let theirs = DVec3::from_array(find(a).r) - DVec3::from_array(find(b).r);
    let t = sol_epoch().add_seconds(years * SECONDS_PER_JULIAN_YEAR);
    let ours = eph.relative(eph.find(a).expect(a), eph.find(b).expect(b), t).r;
    (ours - theirs).length()
}

/// Path of the generated Solar System ephemeris, relative to the workspace root.
pub const EPHEMERIS_PATH: &str = "data/ephemeris/sol-2030.bin";
