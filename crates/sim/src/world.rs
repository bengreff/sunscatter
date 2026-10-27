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
    /// For a faint source outside every moon system (a dwarf planet, a far
    /// system's stand-in: under `ephem::FAINT_GM_FRACTION` of the system's
    /// GM): the least distance from the star it keeps over the ephemeris
    /// window (m). See [`World::counts_in`].
    pub faint_min_distance: Option<f64>,
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
    /// The largest of those apoapsis distances (m): every member stays
    /// within this of the barycenter.
    pub extent: f64,
    /// One past the last member's source index, when the members' sources
    /// are contiguous (the gravity loop then skips a closed system at once).
    pub members_end: Option<usize>,
    /// The least distance of its barycenter from the star over the window
    /// (m; 0 without a star): a vessel `d` from the star is at least
    /// `q − d` from it, which closes far systems without evaluating them.
    pub star_distance: f64,
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

/// A faint source is left out where its pull on both the vessel and the
/// anchor is surely under this fraction of the cutoff ([`World::counts_in`]).
pub const FAINT_ERROR_FRACTION: f64 = 0.01;

/// What one gravity evaluation at a point includes ([`World::gravity_view`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GravityView {
    pub open: OpenClusters,
    /// The larger of the vessel's and the anchor's distances from the star (m).
    pub star_reach: f64,
}

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
    /// The star (the first source with a luminosity) faint sources are
    /// measured from.
    pub star: Option<usize>,
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
                faint_min_distance: None,
            });
        }
        let clusters = cluster_nodes(&eph);
        assert!(clusters.len() <= 64, "at most 64 moon systems");
        let mut cluster_list = Vec::with_capacity(clusters.len());
        for (c, &node) in clusters.iter().enumerate() {
            let (quadrupole, extent) = quadrupole(&eph, node);
            let members: Vec<usize> = (0..sources.len()).filter(|&i| sources[i].role == Role::Member(c)).collect();
            let contiguous = members.windows(2).all(|w| w[1] == w[0] + 1);
            let members_end = members.last().filter(|_| contiguous).map(|&m| m + 1);
            let star_distance = 0.0; // set below, once the star is known
            cluster_list.push(Cluster {
                node,
                stand_in: sources.len(),
                quadrupole,
                extent,
                members_end,
                star_distance,
            });
            let n = eph.node(node);
            // Its members' total GM, so opening or closing the system
            // changes the pull only by its quadrupole (the barycenter node's
            // GM, which moved it in the planetary integration, can differ from
            // the satellite solutions' by a few 1e-8: Jupiter's by 2.5 km³/s²;
            // at Earth that is ~4e-15 m/s² of a cut pull).
            let gm = sources.iter().filter(|s| s.role == Role::Member(c)).map(|s| s.gm).sum::<f64>();
            let gm = if gm > 0.0 { gm } else { n.gm };
            sources.push(Source {
                node,
                name: n.name.clone(),
                gm,
                physical: None,
                relativistic: false,
                anchor_zone: None,
                role: Role::StandIn(c),
                faint_min_distance: None,
            });
        }
        let star = sources.iter().position(|s| s.physical.as_ref().is_some_and(|p| p.luminosity > 0.0));
        if let Some(star) = star {
            let (star_node, total) = (sources[star].node, eph.nodes()[0].gm);
            for cl in &mut cluster_list {
                cl.star_distance = min_distance(&eph, cl.node, star_node);
            }
            for s in sources.iter_mut().filter(|s| !matches!(s.role, Role::Member(_))) {
                if s.gm < crate::ephem::FAINT_GM_FRACTION * total {
                    s.faint_min_distance = match s.role {
                        Role::StandIn(c) => Some(cluster_list[c].star_distance),
                        _ => Some(min_distance(&eph, s.node, star_node)),
                    };
                }
            }
        }
        debug_assert!(eph.nodes().iter().any(|n| n.kind == NodeKind::Barycenter));
        let mut anchor_order: Vec<usize> = (0..sources.len()).filter(|&i| sources[i].anchor_zone.is_some()).collect();
        anchor_order.sort_by(|&a, &b| {
            let zone = |i: usize| sources[i].anchor_zone.map_or(f64::INFINITY, |z| z.enter);
            zone(a).total_cmp(&zone(b))
        });
        Ok(World { eph, sources, cutoff: 1e-12, anchor_order, clusters: cluster_list, star })
    }

    /// What a gravity evaluation for a vessel at `r` (relative to `anchor`)
    /// includes: the open clusters, and how far from the star the vessel and
    /// the anchor are (for the faint sources).
    pub fn gravity_view(&self, snap: &Snapshot, anchor: NodeId, r: DVec3) -> GravityView {
        let Some(s) = self.star else {
            return GravityView { open: self.open_clusters_from(snap, anchor, r, 0.0), star_reach: f64::INFINITY };
        };
        let at_star = snap.relative_r(self.sources[s].node, anchor);
        let d_star = (r - at_star).length();
        GravityView { open: self.open_clusters_from(snap, anchor, r, d_star), star_reach: d_star.max(at_star.length()) }
    }

    /// The indices of the sources that take part in the gravity
    /// ([`Self::counts_in`]), in order; a closed moon system is skipped at
    /// once.
    pub fn counted<'a>(&'a self, view: &'a GravityView) -> impl Iterator<Item = usize> + 'a {
        let mut i = 0;
        std::iter::from_fn(move || {
            while i < self.sources.len() {
                let k = i;
                i += 1;
                if let Role::Member(c) = self.sources[k].role {
                    if !view.open.contains(c) {
                        if let Some(end) = self.clusters[c].members_end {
                            i = end;
                        }
                        continue;
                    }
                }
                if self.counts_in(k, view) {
                    return Some(k);
                }
            }
            None
        })
    }

    /// Whether source `i` could have a tidal acceleration of at least the
    /// cutoff at the vessel (`r`, relative to `anchor`): false only where a
    /// bound proves it cannot, so the gravity's active set
    /// (`forces::ActiveSources`) is the one the exact test over every
    /// source would give, without evaluating far moons: a moon of a closed
    /// system (`2 GM ρ / d³`, with `ρ = |r|` and `d` the least distance
    /// from the anchor-vessel segment to the system), or a faint source left
    /// out of the gravity ([`Self::counts_in`]; its pull is far below the
    /// cutoff, so it would be cut anyway).
    pub fn may_be_active(&self, snap: &Snapshot, anchor: NodeId, r: DVec3, i: usize, view: &GravityView) -> bool {
        let s = &self.sources[i];
        match s.role {
            Role::Member(c) if !view.open.contains(c) => {
                self.cluster_may_matter(snap, anchor, r, view, &self.clusters[c], s.gm)
            }
            Role::Member(_) => true,
            _ => s.faint_min_distance.is_none() || self.counts_in(i, view),
        }
    }

    /// The indices, in order, of the sources [`Self::may_be_active`] does
    /// not rule out, a closed moon system skipped at once where the bound
    /// with its whole GM rules out every member (never the anchor's own).
    pub fn may_be_active_all<'a>(
        &'a self,
        snap: &'a Snapshot,
        anchor: NodeId,
        r: DVec3,
        view: &'a GravityView,
    ) -> impl Iterator<Item = usize> + 'a {
        let anchor_cluster = self.source(anchor).and_then(|s| match s.role {
            Role::Member(c) => Some(c),
            _ => None,
        });
        let mut i = 0;
        std::iter::from_fn(move || {
            while i < self.sources.len() {
                let k = i;
                i += 1;
                if let Role::Member(c) = self.sources[k].role {
                    let cl = &self.clusters[c];
                    let first = cl.members_end.is_some_and(|_| k == 0 || self.sources[k - 1].role != Role::Member(c));
                    if first && !view.open.contains(c) && anchor_cluster != Some(c) {
                        let total = self.sources[cl.stand_in].gm;
                        if !self.cluster_may_matter(snap, anchor, r, view, cl, total) {
                            i = cl.members_end.expect("contiguous");
                            continue;
                        }
                    }
                }
                if self.sources[k].node == anchor || self.may_be_active(snap, anchor, r, k, view) {
                    return Some(k);
                }
            }
            None
        })
    }

    /// The bound of [`Self::may_be_active`] for a body of GM `gm` in closed
    /// system `cl`: false where its tidal acceleration surely stays under
    /// the cutoff.
    fn cluster_may_matter(
        &self,
        snap: &Snapshot,
        anchor: NodeId,
        r: DVec3,
        view: &GravityView,
        cl: &Cluster,
        gm: f64,
    ) -> bool {
        let rho = r.length();
        let far = cl.star_distance - view.star_reach;
        let d_ends = if self.star.is_some() && far > cl.extent + rho {
            far
        } else {
            let b = snap.relative_r(cl.node, anchor);
            (r - b).length().max(b.length())
        };
        let d = d_ends - rho - cl.extent;
        d <= 0.0 || 2.0 * gm * rho >= self.cutoff * d * d * d
    }

    /// Whether source `i` takes part in the gravity: [`Self::counts`], and
    /// a faint source only where its pull on the vessel or the anchor could
    /// reach [`FAINT_ERROR_FRACTION`] of the cutoff (`GM / (q − R)²`, with
    /// `q` its least distance from the star and `R` the farther of the two
    /// from it). Left out, its pull on the anchor (in the anchor's
    /// ephemeris) is not cancelled: an error under that fraction of the
    /// cutoff, within D023's budget, and decided without evaluating it.
    pub fn counts_in(&self, i: usize, view: &GravityView) -> bool {
        if !self.counts(i, view.open) {
            return false;
        }
        let s = &self.sources[i];
        s.faint_min_distance.is_none_or(|q| {
            let gap = q - view.star_reach;
            gap <= 0.0 || s.gm >= FAINT_ERROR_FRACTION * self.cutoff * gap * gap
        })
    }

    /// The clusters open for a vessel at `r` (relative to `anchor`): those
    /// whose quadrupole error `3 Q / d⁴` at the vessel's distance `d` from
    /// the barycenter is at least [`CLUSTER_ERROR_FRACTION`] of the cutoff
    /// (all of them with a zero cutoff, the full model).
    pub fn open_clusters(&self, snap: &Snapshot, anchor: NodeId, r: DVec3) -> OpenClusters {
        self.open_clusters_from(snap, anchor, r, self.star_distance(snap, anchor, r))
    }

    /// The vessel's distance from the star (0 without one: no bound), for
    /// [`Self::may_reach`].
    pub fn star_distance(&self, snap: &Snapshot, anchor: NodeId, r: DVec3) -> f64 {
        self.star.map_or(0.0, |s| (r - snap.relative_r(self.sources[s].node, anchor)).length())
    }

    /// [`Self::open_clusters`] with the vessel's distance from the star:
    /// a system's barycenter keeps at least `q` from the star
    /// ([`Cluster::star_distance`]), so the vessel is at least `q − d_star`
    /// from it, which closes it without evaluating it (the exact test
    /// would close it too).
    fn open_clusters_from(&self, snap: &Snapshot, anchor: NodeId, r: DVec3, d_star: f64) -> OpenClusters {
        let mut open = 0u64;
        let threshold = self.cutoff * CLUSTER_ERROR_FRACTION;
        for (c, cl) in self.clusters.iter().enumerate() {
            let gap = cl.star_distance - d_star;
            if gap > 0.0 && gap * gap * gap * gap * threshold > 3.0 * cl.quadrupole {
                continue;
            }
            let d2 = (r - snap.relative_r(cl.node, anchor)).length_squared();
            if d2 * d2 * threshold <= 3.0 * cl.quadrupole {
                open |= 1 << c;
            }
        }
        OpenClusters(open)
    }

    /// Whether source `src` could be within `reach` of a vessel at `r`
    /// (relative to `anchor`, `d_star` from the star: [`Self::star_distance`]):
    /// false for a member of a moon system whose barycenter is farther than
    /// its extent plus `reach`, or a body or system that keeps farther from
    /// the star than `d_star + reach` (+ its extent). A cheap first test for
    /// per-step searches over bodies (contact, nearest surface, air), which
    /// then never evaluate the tables of far moons and dwarf planets.
    pub fn may_reach(&self, snap: &Snapshot, anchor: NodeId, r: DVec3, d_star: f64, src: &Source, reach: f64) -> bool {
        // A body or system keeps `q` from the star: the vessel is at least
        // `q − d_star` from it.
        let too_far = |q: Option<f64>, margin: f64| q.is_some_and(|q| q - d_star > margin);
        match src.role {
            Role::Member(c) => {
                let cl = &self.clusters[c];
                !too_far(self.star.is_some().then_some(cl.star_distance), cl.extent + reach)
                    && (r - snap.relative_r(cl.node, anchor)).length() <= cl.extent + reach
            }
            _ => !too_far(src.faint_min_distance, reach),
        }
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

/// The least distance of `node` from `star` over the ephemeris window,
/// sampled every 5 days, less the largest distance it moves between
/// samples (so no pass between samples comes closer).
fn min_distance(eph: &Ephemeris, node: NodeId, star: NodeId) -> f64 {
    const STEP: f64 = 5.0 * 86_400.0;
    let span = eph.end.seconds_since(eph.start);
    let n = (span / STEP).ceil() as usize;
    let mut least = f64::INFINITY;
    let mut largest_move: f64 = 0.0;
    let mut last: Option<DVec3> = None;
    for k in 0..=n {
        let t = eph.start.add_seconds((k as f64 * STEP).min(span));
        let p = eph.relative_r(node, star, t);
        least = least.min(p.length());
        if let Some(q) = last {
            largest_move = largest_move.max((p - q).length());
        }
        last = Some(p);
    }
    (least - largest_move).max(0.0)
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
/// (its current distance if unbound, or if larger), and the largest `r_m`.
fn quadrupole(eph: &Ephemeris, node: NodeId) -> (f64, f64) {
    let mu = eph.node(node).gm;
    let t = eph.start;
    let (mut q, mut extent) = (0.0, 0.0f64);
    for b in eph.bodies().filter(|&b| is_under(eph, b, node)) {
        let k = eph.relative(b, node, t);
        let el = crate::kepler::Elements::from_state(k.r, k.v, mu);
        let reach = if el.e < 1.0 { el.a * (1.0 + el.e) } else { k.r.length() };
        let r = reach.max(k.r.length());
        q += eph.node(b).gm * r * r;
        extent = extent.max(r);
    }
    (q, extent)
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
        use crate::forces::{field_degree, Active, ActiveSources, ForceContext};
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
        assert!(
            (3.3e8..4.2e8).contains(&w.clusters[c].extent),
            "about the Moon's distance: {:e}",
            w.clusters[c].extent
        );
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
            // A search near Jupiter can skip Earth and the Moon, never the Sun.
            let d = |r: DVec3| w.star_distance(&snap, sun, r);
            assert!(!w.may_reach(&snap, sun, near_jupiter, d(near_jupiter), &w.sources[i_earth], 1e9));
            assert!(w.may_reach(&snap, sun, near_earth, d(near_earth), &w.sources[i_earth], 1e5));
            assert!(w.may_reach(&snap, sun, near_jupiter, d(near_jupiter), w.find("Sun").unwrap(), 0.0));
            (near_earth, near_jupiter)
        };
        // Near Jupiter the clustered model is the full one (zero cutoff: every
        // cluster open, every source in, every field at its data's degree)
        // to within the rules' budgets: each closed system's quadrupole, each
        // faint body left out and each body's neglected field degrees under a
        // hundredth of the cutoff.
        let accel = |w: &World, r: DVec3| {
            let snap = w.snapshot(t);
            let active = ActiveSources(
                (0..w.sources.len()).map(|i| Active { source: i, degree: field_degree(w, &snap, sun, r, i) }).collect(),
            );
            let ctx = ForceContext { world: w, anchor: sun, active: &active, drag: None, thrust: DVec3::ZERO };
            ctx.accel_with(&snap, r, DVec3::ZERO)
        };
        let clustered = accel(&w, near_jupiter);
        w.cutoff = 0.0;
        assert!(w.open_clusters(&w.snapshot(t), sun, near_jupiter).contains(c));
        let full = accel(&w, near_jupiter);
        let err = (clustered - full).length();
        let budget = 1e-12 * FAINT_ERROR_FRACTION * (w.sources.len() as f64);
        assert!(err > 0.0 && err < budget, "{err:e} > {budget:e}");
        // Near Earth the Earth-Moon system is open either way; what differs
        // is the far systems' quadrupoles and the neglected field degrees,
        // each under its rule's bound.
        let full_near = accel(&w, near_earth);
        w.cutoff = 1e-12;
        let fields = w.sources.iter().filter(|s| s.physical.as_ref().is_some_and(|p| p.gravity.is_some())).count();
        let budget = 1e-12 * (CLUSTER_ERROR_FRACTION + FAINT_ERROR_FRACTION * fields as f64);
        let err = (accel(&w, near_earth) - full_near).length();
        assert!(err < budget, "{err:e} > {budget:e}");
    }

    #[test]
    fn sol_world_has_the_shipped_bodies() {
        let w = World::sol(eph());
        let solid: Vec<&str> = w.surfaces().map(|s| s.name.as_str()).collect();
        for name in ["Mercury", "Earth", "Moon", "Mars", "Io", "Titan", "Triton", "Pluto", "Charon", "Ceres", "Eris"] {
            assert!(solid.contains(&name), "{name} has a surface");
        }
        for name in ["Sun", "Jupiter", "Saturn", "Uranus", "Neptune"] {
            assert!(!solid.contains(&name), "{name} has no surface");
        }
        assert!(w.find("Sun").unwrap().relativistic);
        let order: Vec<&str> = w.anchor_order.iter().map(|&i| w.sources[i].name.as_str()).collect();
        assert_eq!(order, ["Moon", "Earth"]);
    }
}
