//! The simulated world: the ephemeris plus, for each gravity source, the
//! physical data the ship dynamics need (J2, surface, atmosphere, anchoring).
//! The physical data comes from `data/bodies/<body>/body.ron` ([`crate::body::data`]).
//!
//! **Clusters** (D023, motion-model §4): a moon system far from a vessel
//! pulls it as one point mass at its barycenter, with the system's GM (as
//! the planetary integration moved that barycenter). Each barycenter node
//! below the root is a cluster with a stand-in source; its members count
//! only while it is open, the stand-in only while it is closed. A cluster
//! opens where the point mass's error, the system's quadrupole field
//! `~3 Q / d⁴`, could reach a hundredth of the cutoff ([`World::open_clusters`]).
//! The rule depends on the vessel's position only, never on its anchor
//! (rule 1), so it is a force model, not a reference body.

use crate::body::data::{body_dir_name, default_bodies_dir, load_body, BODY_FILE};
use crate::body::{BodyPhysical, DataError};
use crate::ephem::{Ephemeris, NodeKind, Snapshot};
use crate::frame::NodeId;
use crate::time::Epoch;
use glam::DVec3;
use std::path::Path;
use std::sync::Arc;

/// Distances (m) at which a body becomes / stops being the preferred anchor.
/// Pure precision policy: any value is *correct* (invariance tests), these
/// just keep offsets small near each body.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnchorZone {
    pub enter: f64,
    pub exit: f64,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub node: NodeId,
    pub name: String,
    pub gm: f64,
    /// Physical body data (surface, J2, rotation, atmosphere), if modelled.
    pub physical: Option<BodyPhysical>,
    /// The source's 1PN field acts on vessels (the Sun).
    pub relativistic: bool,
    pub anchor_zone: Option<AnchorZone>,
    /// What the source is in the cluster tree.
    pub role: Role,
}

/// A source's part in [`Cluster`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A body outside every moon system: always counts.
    Body,
    /// A body of moon system (cluster) `.0`: counts while the cluster is open.
    Member(usize),
    /// The point mass standing for cluster `.0` (its barycenter, no physical
    /// data): counts while the cluster is closed.
    StandIn(usize),
}

/// A moon system seen as one point mass from afar (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    /// Its barycenter node.
    pub node: NodeId,
    /// Index of its stand-in source.
    pub stand_in: usize,
    /// Its quadrupole strength `Q = Σ GM_m r_m²` over the members, with each
    /// `r_m` the apoapsis of its orbit about the barycenter at the start of
    /// the ephemeris (m⁵/s²).
    pub quadrupole: f64,
}

/// Which clusters are open for one evaluation (bit `c` for cluster `c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenClusters(pub u64);

impl OpenClusters {
    pub fn contains(self, c: usize) -> bool {
        self.0 & (1 << c) != 0
    }
}

/// A cluster opens where its quadrupole error could reach this fraction of
/// the cutoff.
pub const CLUSTER_ERROR_FRACTION: f64 = 0.01;

#[derive(Clone, Debug)]
pub struct World {
    pub eph: Arc<Ephemeris>,
    pub sources: Vec<Source>,
    /// Gravity sources whose tidal acceleration at the vessel relative to its
    /// anchor is below this (m/s²) are cut: they pull the vessel as they pull
    /// the anchor (D023, `forces::ActiveSources`). The neglected acceleration
    /// is below this per source, so at most `cutoff·t²/2` of drift per source
    /// (~500 m over a year at 1e-12; typically far less, since tidal terms
    /// mostly oscillate).
    pub cutoff: f64,
    /// Sources with anchor zones, smallest zone first (anchor policy order).
    pub anchor_order: Vec<usize>,
    /// Moon systems that count as one mass from afar (at most 64).
    pub clusters: Vec<Cluster>,
}

impl World {
    /// The Solar System world from the bodies in this source tree's
    /// `data/bodies` (panics if that data is missing or invalid).
    pub fn sol(eph: Arc<Ephemeris>) -> Self {
        Self::from_dir(eph, &default_bodies_dir()).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Builds the world from body data in `dir`: every body node of the
    /// ephemeris named `N` takes its physical data from `dir/<n>/body.ron`
    /// (lower-case name) if that file exists, and is a point mass otherwise.
    pub fn from_dir(eph: Arc<Ephemeris>, dir: &Path) -> Result<Self, DataError> {
        let mut sources = Vec::new();
        for id in eph.bodies() {
            let n = eph.node(id);
            let body_dir = dir.join(body_dir_name(&n.name));
            let def = if body_dir.join(BODY_FILE).exists() {
                let mut def = load_body(&body_dir)?;
                def.load_terrain()?;
                Some(def)
            } else {
                None
            };
            if let Some(d) = &def {
                if d.physical.name != n.name {
                    let message = format!("name {:?} does not match ephemeris node {:?}", d.physical.name, n.name);
                    return Err(DataError { path: body_dir.join(BODY_FILE), message });
                }
            }
            let (physical, relativistic, anchor_zone) =
                def.map_or((None, false, None), |d| (Some(d.physical), d.relativistic, d.anchor_zone));
            let role = cluster_of(&eph, id).map_or(Role::Body, Role::Member);
            sources.push(Source {
                node: id,
                name: n.name.clone(),
                gm: n.gm,
                physical,
                relativistic,
                anchor_zone,
                role,
            });
        }
        let clusters = cluster_nodes(&eph);
        assert!(clusters.len() <= 64, "at most 64 moon systems");
        let mut cluster_list = Vec::with_capacity(clusters.len());
        for (c, &node) in clusters.iter().enumerate() {
            cluster_list.push(Cluster { node, stand_in: sources.len(), quadrupole: quadrupole(&eph, node) });
            let n = eph.node(node);
            sources.push(Source {
                node,
                name: n.name.clone(),
                gm: n.gm,
                physical: None,
                relativistic: false,
                anchor_zone: None,
                role: Role::StandIn(c),
            });
        }
        debug_assert!(eph.nodes().iter().any(|n| n.kind == NodeKind::Barycenter));
        let mut anchor_order: Vec<usize> = (0..sources.len()).filter(|&i| sources[i].anchor_zone.is_some()).collect();
        anchor_order.sort_by(|&a, &b| {
            let zone = |i: usize| sources[i].anchor_zone.map_or(f64::INFINITY, |z| z.enter);
            zone(a).total_cmp(&zone(b))
        });
        Ok(World { eph, sources, cutoff: 1e-12, anchor_order, clusters: cluster_list })
    }

    /// The clusters open for a vessel at `r` (relative to `anchor`): those
    /// whose quadrupole error `3 Q / d⁴` at the vessel's distance `d` from
    /// the barycenter is at least [`CLUSTER_ERROR_FRACTION`] of the cutoff
    /// (all of them with a zero cutoff, the full model).
    pub fn open_clusters(&self, snap: &Snapshot, anchor: NodeId, r: DVec3) -> OpenClusters {
        let mut open = 0u64;
        for (c, cl) in self.clusters.iter().enumerate() {
            let d2 = (r - snap.relative_r(cl.node, anchor)).length_squared();
            if d2 * d2 * (self.cutoff * CLUSTER_ERROR_FRACTION) <= 3.0 * cl.quadrupole {
                open |= 1 << c;
            }
        }
        OpenClusters(open)
    }

    /// Whether source `i` takes part in the gravity with these clusters open.
    pub fn counts(&self, i: usize, open: OpenClusters) -> bool {
        match self.sources[i].role {
            Role::Body => true,
            Role::Member(c) => open.contains(c),
            Role::StandIn(c) => !open.contains(c),
        }
    }

    pub fn source(&self, node: NodeId) -> Option<&Source> {
        self.sources.iter().find(|s| s.node == node)
    }

    pub fn find(&self, name: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.name == name)
    }

    /// The end of the ephemeris window: nothing is simulated past it (coasts
    /// end with `EndKind::EphemerisEnd`, vessels stop, the game clock stops).
    pub fn end(&self) -> Epoch {
        self.eph.end
    }

    pub fn snapshot(&self, t: Epoch) -> Snapshot<'_> {
        self.eph.snapshot(t)
    }

    /// Sources with a solid surface (candidates for landing and impact).
    pub fn surfaces(&self) -> impl Iterator<Item = &Source> {
        self.sources.iter().filter(|s| s.physical.as_ref().is_some_and(|p| p.solid))
    }
}

/// Barycenter nodes below the root (moon systems), in node order.
fn cluster_nodes(eph: &Ephemeris) -> Vec<NodeId> {
    (1..eph.nodes().len())
        .map(|i| NodeId(i as u16))
        .filter(|&id| eph.node(id).kind == NodeKind::Barycenter && eph.node(id).parent.is_some())
        .collect()
}

/// The innermost cluster containing node `id`, if any.
fn cluster_of(eph: &Ephemeris, id: NodeId) -> Option<usize> {
    let clusters = cluster_nodes(eph);
    let mut up = eph.node(id).parent;
    while let Some(p) = up {
        if let Some(c) = clusters.iter().position(|&n| n == p) {
            return Some(c);
        }
        up = eph.node(p).parent;
    }
    None
}

/// `Σ GM_m r_m²` over the bodies under barycenter `node`, `r_m` the
/// apoapsis distance of each one's osculating orbit about it at the start
/// (its current distance if unbound).
fn quadrupole(eph: &Ephemeris, node: NodeId) -> f64 {
    let mu = eph.node(node).gm;
    let t = eph.start;
    eph.bodies()
        .filter(|&b| cluster_nodes(eph).contains(&node) && is_under(eph, b, node))
        .map(|b| {
            let k = eph.relative(b, node, t);
            let el = crate::kepler::Elements::from_state(k.r, k.v, mu);
            let reach = if el.e < 1.0 { el.a * (1.0 + el.e) } else { k.r.length() };
            eph.node(b).gm * reach.max(k.r.length()) * reach.max(k.r.length())
        })
        .sum()
}

fn is_under(eph: &Ephemeris, id: NodeId, ancestor: NodeId) -> bool {
    let mut up = eph.node(id).parent;
    while let Some(p) = up {
        if p == ancestor {
            return true;
        }
        up = eph.node(p).parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sol::EPHEMERIS_PATH;

    fn eph() -> Arc<Ephemeris> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(EPHEMERIS_PATH);
        Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap())
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sunscatter-world-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mars")).unwrap();
        dir
    }

    const ROCK: &str = r#"(physical: (name: "NAME", radius_eq: 3396190.0, radius_polar: 3376200.0,
        rotation: (ra_deg: 317.68, dec_deg: 52.89, w0_deg: 176.63, w_rate_deg_per_day: 350.89), solid: true))"#;

    #[test]
    fn adding_a_body_needs_only_data() {
        let dir = temp_dir("mars");
        std::fs::write(dir.join("mars/body.ron"), ROCK.replace("NAME", "Mars")).unwrap();
        let w = World::from_dir(eph(), &dir).unwrap();
        let mars = w.find("Mars").unwrap().physical.as_ref().unwrap();
        assert_eq!(mars.radius_polar, 3_376_200.0);
        assert!(w.find("Earth").unwrap().physical.is_none(), "no Earth data in this directory");
        assert_eq!(w.surfaces().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Mars"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_name_that_does_not_match_the_ephemeris_is_an_error() {
        let dir = temp_dir("badname");
        std::fs::write(dir.join("mars/body.ron"), ROCK.replace("NAME", "Marz")).unwrap();
        let e = World::from_dir(eph(), &dir).unwrap_err();
        assert!(e.message.contains("Marz"), "{e}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_far_moon_system_pulls_as_one_mass_and_opens_near() {
        use crate::forces::{ActiveSources, ForceContext};
        let mut w = World::sol(eph());
        // Every barycenter below the root is a cluster with a stand-in source
        // after the bodies; its members belong to it.
        let emb = w.eph.find("EMB").unwrap();
        let c = w.clusters.iter().position(|c| c.node == emb).expect("EMB is a cluster");
        let stand_in = w.clusters[c].stand_in;
        assert_eq!(w.sources[stand_in].role, Role::StandIn(c));
        assert!(w.sources[stand_in].physical.is_none());
        assert_eq!(w.find("Earth").unwrap().role, Role::Member(c));
        assert_eq!(w.find("Sun").unwrap().role, Role::Body);
        // Q ~ GM_moon r² (the Moon's apogee about the barycenter).
        let q = w.clusters[c].quadrupole;
        assert!((6.0e29..9.0e29).contains(&q), "{q:e}");
        let t = w.eph.start.add_seconds(3.0e6);
        let sun = w.find("Sun").unwrap().node;
        let i_earth = w.sources.iter().position(|s| s.name == "Earth").unwrap();
        let (near_earth, near_jupiter) = {
            let snap = w.snapshot(t);
            let at = |name: &str, offset: DVec3| snap.relative_r(w.find(name).unwrap().node, sun) + offset;
            let near_earth = at("Earth", DVec3::new(7.0e6, 0.0, 0.0));
            let near_jupiter = at("Jupiter", DVec3::new(0.0, 7.0e8, 0.0));
            let open = w.open_clusters(&snap, sun, near_earth);
            assert!(open.contains(c) && w.counts(i_earth, open) && !w.counts(stand_in, open));
            let closed = w.open_clusters(&snap, sun, near_jupiter);
            assert!(!closed.contains(c) && !w.counts(i_earth, closed) && w.counts(stand_in, closed));
            (near_earth, near_jupiter)
        };
        // Near Jupiter the point mass is the members' pull to far below the
        // cutoff; with a zero cutoff (the full model) every cluster is open.
        let accel = |w: &World, r: DVec3| {
            let active = ActiveSources((0..w.sources.len()).collect());
            let ctx = ForceContext { world: w, anchor: sun, active: &active, drag: None, thrust: DVec3::ZERO };
            ctx.accel_with(&w.snapshot(t), r, DVec3::ZERO)
        };
        let clustered = accel(&w, near_jupiter);
        w.cutoff = 0.0;
        assert!(w.open_clusters(&w.snapshot(t), sun, near_jupiter).contains(c));
        let full = accel(&w, near_jupiter);
        let err = (clustered - full).length();
        assert!(err > 0.0 && err < 1e-15, "{err:e}");
        // Near Earth the cluster is open either way: identical arithmetic.
        let full_near = accel(&w, near_earth);
        w.cutoff = 1e-12;
        assert_eq!(accel(&w, near_earth), full_near);
    }

    #[test]
    fn sol_world_has_the_shipped_bodies() {
        let w = World::sol(eph());
        let solid: Vec<&str> = w.surfaces().map(|s| s.name.as_str()).collect();
        assert_eq!(solid, ["Earth", "Moon"]);
        assert!(w.find("Sun").unwrap().relativistic);
        let order: Vec<&str> = w.anchor_order.iter().map(|&i| w.sources[i].name.as_str()).collect();
        assert_eq!(order, ["Moon", "Earth"]);
    }
}
