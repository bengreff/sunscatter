//! The Solar System: generation inputs (tree, config) shared by `ephem-tool`
//! and the tests, plus loading of the generated ephemeris. The bodies beyond
//! the planets (dwarf planets, moon systems) are in [`systems`].

pub mod systems;

use crate::ephem::{Ephemeris, NodeKind};
use crate::gen::{Extras, GenConfig, InitialBody, NodeReport, NodeSpec, Oblate};
use crate::time::{Epoch, SECONDS_PER_JULIAN_YEAR};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
pub use systems::{Catalog, SystemsInitial};

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
/// relative motion closest to a conic). A planet with a moon system in the
/// catalog is its system's barycenter node (`"Jupiter barycenter"`, the
/// planet itself being a member of the system); the catalog's dwarf
/// planets follow the planets.
pub fn tree(bodies: &[InitialBody], catalog: &Catalog) -> Vec<NodeSpec> {
    let idx = |name: &str| bodies.iter().position(|b| b.name == name).unwrap_or_else(|| panic!("missing {name}"));
    let all: Vec<usize> = (0..bodies.len()).collect();
    let body = |name: &str, parent: usize, tol: f64| {
        let barycenter = format!("{name} barycenter");
        let system = catalog.systems.iter().any(|s| s.barycenter == barycenter);
        NodeSpec {
            name: if system { barycenter } else { name.into() },
            kind: if system { NodeKind::Barycenter } else { NodeKind::Body },
            parent: Some(parent),
            members: vec![idx(name)],
            tolerance: tol,
        }
    };
    let mut tree = vec![
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
    ];
    for d in &catalog.dwarfs {
        let parent = match d.parent.as_str() {
            "SSB" => 0,
            "Sun" => 1,
            other => panic!("{}: parent must be Sun or SSB, not {other}", d.name),
        };
        tree.push(NodeSpec {
            name: d.name.clone(),
            kind: if d.is_barycenter() { NodeKind::Barycenter } else { NodeKind::Body },
            parent: Some(parent),
            members: vec![idx(&d.name)],
            tolerance: d.tolerance,
        });
    }
    tree
}

/// Assumption set: Newtonian point masses for all bodies (the planets'
/// systems and the dwarf planets' as one mass each), the Sun's 1PN field, and
/// Earth's J2 about its IAU precessing pole.
pub fn gen_config(bodies: &[InitialBody], years: f64) -> GenConfig {
    let start = sol_epoch();
    let idx = |name: &str| bodies.iter().position(|b| b.name == name).expect(name);
    let earth = crate::body::earth();
    GenConfig {
        generator: format!(
            "sol-nbody-v4 yoshida8 h=3600s degree=14 +sun1pn +earthJ2(iau pole), {} bodies",
            bodies.len()
        ),
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
                j4: 0.0,
                j6: 0.0,
                radius: earth.radius_eq,
                rotation: earth.rotation,
            }],
            external: None,
        },
        // Power-of-two segments and raw coefficients: the planets are the
        // anchors (Earth, the Moon) whose tables must stay smooth. A fit has
        // velocity jumps at segment boundaries in proportion to its error,
        // and free lengths fit closer to the tolerance: a year anchored at the
        // Sun then drifted 4.8 km from the same coast anchored at the
        // barycenter, against 19 m with these settings.
        free_segments: false,
        quantize: false,
    }
}

/// The whole Solar System over `years` from the epoch: the planetary
/// integration (planets, the Moon, dwarf planets), then each moon system of
/// the catalog in its barycenter's frame, grafted under it. The primaries'
/// physical data (J2, radius, pole) come from `bodies_dir/<name>/body.ron`.
/// Returns the ephemeris and the fit report of every node.
pub fn generate_sol(
    initial: &InitialConditions,
    catalog: &Catalog,
    systems: &SystemsInitial,
    years: f64,
    bodies_dir: &Path,
) -> (Ephemeris, Vec<NodeReport>) {
    let cfg = gen_config(&initial.to_bodies(), years);
    generate_sol_with(&cfg, initial, catalog, systems, years, bodies_dir)
}

/// [`generate_sol`] with the planetary integration's settings given.
pub fn generate_sol_with(
    cfg: &GenConfig,
    initial: &InitialConditions,
    catalog: &Catalog,
    systems: &SystemsInitial,
    years: f64,
    bodies_dir: &Path,
) -> (Ephemeris, Vec<NodeReport>) {
    let bodies = initial.to_bodies();
    let (planets, mut report) = crate::gen::generate(&bodies, &tree(&bodies, catalog), cfg);
    let planets = Arc::new(planets);
    let mut eph = (*planets).clone();
    for spec in &catalog.systems {
        let init = systems.systems.iter().find(|s| s.barycenter == spec.barycenter).expect("system initial states");
        let primary = primary_physical(spec, bodies_dir);
        let (members, r, label) = systems::generate_system(spec, init, &planets, &primary, years);
        eph = systems::graft(&eph, &spec.barycenter, members, &label);
        report.extend(r);
    }
    (eph, report)
}

/// A moon system's primary's physical data from its `body.ron`.
pub fn primary_physical(spec: &systems::SystemSpec, bodies_dir: &Path) -> crate::body::BodyPhysical {
    let name = &spec.members[0].name;
    let dir = bodies_dir.join(crate::body::data::body_dir_name(name));
    crate::body::load_body(&dir).unwrap_or_else(|e| panic!("{e}")).physical
}

/// Reads the catalog, the planets' and the moon systems' initial states
/// from `dir` (`data/ephemeris`).
pub fn read_inputs(dir: &Path) -> (InitialConditions, Catalog, SystemsInitial) {
    (read_ron(dir, "sol-2030-initial.ron"), read_ron(dir, CATALOG_FILE), read_ron(dir, SYSTEMS_INITIAL_FILE))
}

/// File names in `data/ephemeris` of the catalog and the moon systems' states.
pub const CATALOG_FILE: &str = "sol-systems.ron";
pub const SYSTEMS_INITIAL_FILE: &str = "sol-2030-systems-initial.ron";

fn read_ron<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> T {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Position error (m) of `a` relative to `b` in `eph` against DE440. A
/// planet whose system is modelled is compared by its barycenter (DE440's
/// planets beyond Mars are their systems' barycenters).
pub fn reference_error(eph: &Ephemeris, reference: &ReferenceFile, a: &str, b: &str, years: f64) -> f64 {
    let find = |name: &str| {
        reference.states.iter().find(|s| s.name == name && s.years == years).unwrap_or_else(|| panic!("no ref {name}"))
    };
    let theirs = DVec3::from_array(find(a).r) - DVec3::from_array(find(b).r);
    let t = sol_epoch().add_seconds(years * SECONDS_PER_JULIAN_YEAR);
    let node = |name: &str| eph.find(&format!("{name} barycenter")).or_else(|| eph.find(name)).expect(name);
    let ours = eph.relative(node(a), node(b), t).r;
    (ours - theirs).length()
}

/// Reference states of bodies relative to a centre (JPL Horizons), for
/// golden tests of the moons and dwarf planets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HorizonsReference {
    pub states: Vec<RelativeState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelativeState {
    /// Node names in our ephemeris.
    pub name: String,
    pub center: String,
    pub years: f64,
    pub r: [f64; 3],
    pub v: [f64; 3],
}

/// Position error (m) of `state` (`name` relative to `center`) in `eph`.
pub fn relative_error(eph: &Ephemeris, state: &RelativeState) -> f64 {
    let t = sol_epoch().add_seconds(state.years * SECONDS_PER_JULIAN_YEAR);
    let find = |name: &str| eph.find(name).unwrap_or_else(|| panic!("no node {name}"));
    (eph.relative(find(&state.name), find(&state.center), t).r - DVec3::from_array(state.r)).length()
}

/// Path of the Horizons reference states, relative to the workspace root.
pub const HORIZONS_REFERENCE_PATH: &str = "data/ephemeris/horizons-reference.ron";

/// Path of the generated Solar System ephemeris, relative to the workspace root.
pub const EPHEMERIS_PATH: &str = "data/ephemeris/sol-2030.bin";
