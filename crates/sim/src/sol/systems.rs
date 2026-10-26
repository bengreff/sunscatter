//! The Solar System beyond the planets (D084): the catalog of dwarf planets
//! and moon systems (`data/ephemeris/sol-systems.ron`), and the generation
//! of each moon system.
//!
//! * **Dwarf planets** (or, for those with moons, their system barycenters)
//!   are integrated with the planets as point masses ([`super::tree`]):
//!   they perturb the planets, so a vessel's cut-source rule stays exact.
//! * **Moon systems** are integrated once each, in the frame of their
//!   barycenter node from the planetary ephemeris, under the tides of the
//!   planets and the Sun taken from it, the primary's zonal field (J2 from
//!   its `body.ron`, J4 and J6 from the catalog) and optionally its 1PN
//!   field. The planetary integration carries each system as a point mass
//!   of its total GM, which is how the system's barycenter moves (the
//!   hierarchical approximation of JPL's satellite ephemerides; recorded in
//!   the generator string). Every member, primary included, is a child of
//!   the barycenter node: positions never compound two fits.
//!
//! Generation is a pure function of the catalog, the initial states and the
//! planetary ephemeris, so regenerating gives the same bits (golden tests).

use super::InitialBodyRecord;
use crate::body::BodyPhysical;
use crate::ephem::{Ephemeris, Node, NodeKind};
use crate::frame::NodeId;
use crate::gen::{generate, External, Extras, GenConfig, InitialBody, NodeReport, NodeSpec, Oblate};
use crate::time::SECONDS_PER_JULIAN_YEAR;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// What the Solar System has beyond the planets.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Catalog {
    pub dwarfs: Vec<DwarfSpec>,
    pub systems: Vec<SystemSpec>,
}

/// A dwarf planet in the planetary integration.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DwarfSpec {
    /// Node name: the body, or `"<Body> barycenter"` for a system with moons.
    pub name: String,
    /// JPL Horizons command for its state (the system barycenter's for one
    /// with moons).
    pub horizons: String,
    /// GM (km³/s²): the whole system's for a barycenter.
    pub gm: f64,
    /// Parent node: `"Sun"` inside Jupiter's orbit, `"SSB"` outside (the
    /// planets' convention, closest to a conic).
    pub parent: String,
    /// Fit tolerance (m).
    pub tolerance: f64,
}

impl DwarfSpec {
    pub fn is_barycenter(&self) -> bool {
        self.name.ends_with(" barycenter")
    }
}

/// A moon system: its barycenter node and members.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SystemSpec {
    /// The barycenter node in the planetary ephemeris.
    pub barycenter: String,
    /// Horizons centre of the members' states (the barycenter, e.g. `"500@5"`).
    pub horizons_center: String,
    /// Integration step (whole seconds; a few hundred per orbit of the
    /// fastest member).
    pub step: i64,
    /// Chebyshev degree of the members' tables.
    pub degree: usize,
    /// Span used to choose segment lengths (days): several orbits of every
    /// member, and a whole one of the slowest. The golden test regenerates it.
    pub trial_days: f64,
    /// Planetary nodes whose tides act on the members.
    pub perturbers: Vec<String>,
    /// The primary's first post-Newtonian field acts on the moons.
    #[serde(default)]
    pub relativistic: bool,
    /// The primary's J4 and J6, referenced to its `radius_eq` (J2, the
    /// radius and the pole come from its `body.ron`).
    #[serde(default)]
    pub j4: f64,
    #[serde(default)]
    pub j6: f64,
    /// The members, primary first.
    pub members: Vec<MemberSpec>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct MemberSpec {
    pub name: String,
    /// Horizons command for its state relative to the system's centre.
    pub horizons: String,
    /// GM (km³/s²).
    pub gm: f64,
    /// Fit tolerance (m).
    pub tolerance: f64,
    /// Julian date (TDB) to take the Horizons state at when the solution
    /// ends before the epoch (the TNO satellite solutions end at
    /// 2030-01-01); it is carried to the epoch on the two-body conic about
    /// the primary.
    #[serde(default)]
    pub state_jd: Option<f64>,
}

/// The moon systems' initial states: per system, each member relative to
/// the barycenter (ICRF, SI units), at the epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemsInitial {
    pub epoch_tdb_seconds: i64,
    pub systems: Vec<SystemInitial>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemInitial {
    pub barycenter: String,
    pub members: Vec<InitialBodyRecord>,
}

impl SystemSpec {
    /// The members as integration bodies (GM in SI) from their initial states.
    pub fn bodies(&self, initial: &SystemInitial) -> Vec<InitialBody> {
        self.members
            .iter()
            .map(|m| {
                let rec = initial
                    .members
                    .iter()
                    .find(|r| r.name == m.name)
                    .unwrap_or_else(|| panic!("no state for {}", m.name));
                InitialBody {
                    name: m.name.clone(),
                    gm: m.gm * 1e9,
                    r: DVec3::from_array(rec.r),
                    v: DVec3::from_array(rec.v),
                }
            })
            .collect()
    }

    /// The tree: the barycenter (the frame's origin) and every member under it.
    pub fn tree(&self) -> Vec<NodeSpec> {
        let mut tree = vec![NodeSpec {
            name: self.barycenter.clone(),
            kind: NodeKind::Barycenter,
            parent: None,
            members: (0..self.members.len()).collect(),
            tolerance: 0.0,
        }];
        tree.extend(self.members.iter().enumerate().map(|(i, m)| NodeSpec {
            name: m.name.clone(),
            kind: NodeKind::Body,
            parent: Some(0),
            members: vec![i],
            tolerance: m.tolerance,
        }));
        tree
    }

    /// Generation settings over `years` from the planetary ephemeris'
    /// start, with `planets` supplying the perturbers and `primary` the
    /// primary's physical data (J2, radius, pole).
    pub fn config(&self, planets: &Arc<Ephemeris>, primary: &BodyPhysical, years: f64) -> GenConfig {
        let find = |name: &str| planets.find(name).unwrap_or_else(|| panic!("{name} is not a planetary node"));
        let perturbers: Vec<(NodeId, f64)> =
            self.perturbers.iter().map(|p| (find(p), planets.node(find(p)).gm)).collect();
        let start = planets.start;
        let oblate = (primary.j2 != 0.0).then_some(Oblate {
            body: 0,
            j2: primary.j2,
            j4: self.j4,
            j6: self.j6,
            radius: primary.radius_eq,
            rotation: primary.rotation,
        });
        GenConfig {
            generator: self.generator_label(primary),
            start,
            end: start.add_seconds((years * SECONDS_PER_JULIAN_YEAR).round()),
            step: self.step,
            degree: self.degree,
            trial_steps: (self.trial_days * 86_400.0 / self.step as f64).round() as usize,
            rails_stride: 24,
            extras: Extras {
                relativistic_center: self.relativistic.then_some(0),
                oblate: oblate.into_iter().collect(),
                external: Some(External {
                    eph: planets.clone(),
                    origin: find(&self.barycenter),
                    perturbers,
                    relativistic: None,
                }),
            },
            free_segments: true,
            quantize: true,
        }
    }

    /// The recorded assumption set of this system's generation.
    fn generator_label(&self, primary: &BodyPhysical) -> String {
        let zonal = match (primary.j2 != 0.0, self.j4 != 0.0 || self.j6 != 0.0) {
            (false, _) => "",
            (true, false) => " +J2",
            (true, true) => " +J2J4J6",
        };
        let pn = if self.relativistic { " +1PN" } else { "" };
        format!(
            "{}: yoshida8 h={}s degree={}{zonal}(iau pole){pn} +tides[{}]",
            self.barycenter,
            self.step,
            self.degree,
            self.perturbers.join(",")
        )
    }
}

/// Generates one moon system: its member nodes (parents index into the
/// returned list, the barycenter being `None`) and the fit report.
pub fn generate_system(
    spec: &SystemSpec,
    initial: &SystemInitial,
    planets: &Arc<Ephemeris>,
    primary: &BodyPhysical,
    years: f64,
) -> (Vec<Node>, Vec<NodeReport>, String) {
    let bodies = spec.bodies(initial);
    let cfg = spec.config(planets, primary, years);
    let (eph, report) = generate(&bodies, &spec.tree(), &cfg);
    let nodes = eph.nodes()[1..].iter().map(|n| Node { parent: None, ..n.clone() }).collect();
    (nodes, report, cfg.generator)
}

/// `base` with `members` appended under its node `origin` (every member's
/// parent is the origin). The generator string records `label`.
pub fn graft(base: &Ephemeris, origin: &str, members: Vec<Node>, label: &str) -> Ephemeris {
    let at = base.find(origin).unwrap_or_else(|| panic!("no node {origin}"));
    let mut nodes = base.nodes().to_vec();
    nodes.extend(members.into_iter().map(|n| Node { parent: Some(at), ..n }));
    Ephemeris::new(format!("{}; {label}", base.generator), base.start, base.end, nodes)
}
