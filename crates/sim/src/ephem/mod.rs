//! Ephemerides: "where is node X at time t?"
//!
//! Every node of the frame tree (system barycenter, bodies, barycenter nodes)
//! has a [`Motion`] relative to its parent. All motions are pure functions of
//! time, so a body's position never depends on history, generation order or
//! platform (D009, D026).
//!
//! Relative states between any two nodes are composed through their lowest
//! common ancestor ([`Ephemeris::relative`]); nothing is ever computed by
//! subtracting two large absolute positions.

mod chebyshev;
mod io;
mod rails;

#[cfg(test)]
pub(crate) use chebyshev::PAST_END_CALLS;
pub use chebyshev::{cheb_basis, interpolate_lobatto, lobatto_nodes, ChebTable, MAX_DEGREE};
pub use io::{fnv1a64, EphemerisFormatError};
pub use rails::Rails;

use crate::frame::NodeId;
use crate::time::Epoch;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::cell::Cell;

/// Position, velocity and acceleration of one node relative to another
/// (inertial axes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kinematics {
    pub r: DVec3,
    pub v: DVec3,
    pub a: DVec3,
}

impl std::ops::Add for Kinematics {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Kinematics { r: self.r + o.r, v: self.v + o.v, a: self.a + o.a }
    }
}
impl std::ops::Sub for Kinematics {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Kinematics { r: self.r - o.r, v: self.v - o.v, a: self.a - o.a }
    }
}

/// Uniform linear motion (stars and system barycenters in the bubble frame).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Linear {
    pub epoch: Epoch,
    pub r0: DVec3,
    pub v: DVec3,
}

/// How a node moves relative to its parent.
#[derive(Clone, Debug, PartialEq)]
pub enum Motion {
    /// The root of the tree (it defines the frame).
    Root,
    Linear(Linear),
    Rails(Rails),
    Table(ChebTable),
}

impl Motion {
    pub fn eval(&self, t: Epoch) -> Kinematics {
        match self {
            Motion::Root => Kinematics::default(),
            Motion::Linear(l) => {
                let dt = t.seconds_since(l.epoch);
                Kinematics { r: l.r0 + l.v * dt, v: l.v, a: DVec3::ZERO }
            }
            Motion::Rails(r) => r.eval(t),
            Motion::Table(c) => c.eval(t),
        }
    }

    /// Position only (bit-identical to `eval(t).r`).
    pub fn eval_r(&self, t: Epoch) -> DVec3 {
        match self {
            Motion::Root => DVec3::ZERO,
            Motion::Linear(l) => l.r0 + l.v * t.seconds_since(l.epoch),
            Motion::Rails(r) => r.eval(t).r,
            Motion::Table(c) => c.eval_r(t),
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Motion::Root => "root",
            Motion::Linear(_) => "linear",
            Motion::Rails(_) => "rails",
            Motion::Table(_) => "table",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// A barycenter: a point in the tree with no physical body.
    Barycenter,
    Body,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    /// Gravitational parameter (m³/s²). For barycenters: total of the subtree.
    pub gm: f64,
    pub motion: Motion,
}

/// A tree of nodes with their motions.
#[derive(Clone, Debug, PartialEq)]
pub struct Ephemeris {
    /// Identifies the generator that produced this data (for caching).
    pub generator: String,
    /// Validity window.
    pub start: Epoch,
    pub end: Epoch,
    nodes: Vec<Node>,
    depth: Vec<u16>,
    /// Nodes a [`Snapshot`] evaluates on first use ([`lazy_nodes`]): those
    /// inside a moon system and the faint ones (dwarf planets), which a
    /// force evaluation far from them never touches. The others (the
    /// planets) are evaluated up front, cheaper when every query touches them.
    lazy: Vec<bool>,
    /// The node or one of its ancestors is lazy.
    lazy_path: Vec<bool>,
    eager_list: Vec<usize>,
    lazy_list: Vec<usize>,
}

/// A node lighter than this fraction of the root's GM (the whole system's)
/// is faint: evaluated lazily (dwarf planets, ~1e-8 of the Solar System; the
/// Moon is 3.7e-8).
pub const FAINT_GM_FRACTION: f64 = 1e-8;

/// Which nodes are evaluated lazily: the faint ones ([`FAINT_GM_FRACTION`]),
/// and the moon systems grafted after the planets: the longest run of
/// trailing nodes inside a barycenter subtree other than the root (Earth and
/// the Moon, under the Earth–Moon barycenter among the planets, stay eager).
fn lazy_nodes(nodes: &[Node]) -> Vec<bool> {
    let inside = |i: usize| {
        let mut up = nodes[i].parent;
        while let Some(p) = up {
            let node = &nodes[p.0 as usize];
            if node.kind == NodeKind::Barycenter && node.parent.is_some() {
                return true;
            }
            up = node.parent;
        }
        false
    };
    let systems_from = (0..nodes.len()).rev().take_while(|&i| inside(i)).last().unwrap_or(nodes.len());
    (0..nodes.len()).map(|i| i >= systems_from || (i > 0 && nodes[i].gm < FAINT_GM_FRACTION * nodes[0].gm)).collect()
}

impl Ephemeris {
    /// Builds an ephemeris; `nodes[0]` must be the root and every parent must
    /// precede its children.
    pub fn new(generator: String, start: Epoch, end: Epoch, nodes: Vec<Node>) -> Self {
        let mut depth = Vec::with_capacity(nodes.len());
        for (i, n) in nodes.iter().enumerate() {
            match n.parent {
                None => {
                    assert_eq!(i, 0, "only node 0 may be the root");
                    depth.push(0);
                }
                Some(p) => {
                    assert!((p.0 as usize) < i, "parent of {} must precede it", n.name);
                    depth.push(depth[p.0 as usize] + 1);
                }
            }
        }
        let lazy = lazy_nodes(&nodes);
        let mut lazy_path = lazy.clone();
        for i in 1..nodes.len() {
            let p = nodes[i].parent.expect("non-root").0 as usize;
            lazy_path[i] |= lazy_path[p];
        }
        let eager_list = (0..nodes.len()).filter(|&i| !lazy[i]).collect();
        let lazy_list = (0..nodes.len()).filter(|&i| lazy[i]).collect();
        Ephemeris { generator, start, end, nodes, depth, lazy, lazy_path, eager_list, lazy_list }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    pub fn find(&self, name: &str) -> Option<NodeId> {
        self.nodes.iter().position(|n| n.name == name).map(|i| NodeId(i as u16))
    }

    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    /// Kinematics of `of` relative to its parent.
    pub fn about_parent(&self, of: NodeId, t: Epoch) -> Kinematics {
        self.node(of).motion.eval(t)
    }

    /// Kinematics of `of` relative to `about`, composed through their lowest
    /// common ancestor.
    pub fn relative(&self, of: NodeId, about: NodeId, t: Epoch) -> Kinematics {
        let (mut a, mut b) = (of, about);
        let mut acc_a = Kinematics::default();
        let mut acc_b = Kinematics::default();
        while a != b {
            if self.depth[a.0 as usize] >= self.depth[b.0 as usize] {
                acc_a = acc_a + self.about_parent(a, t);
                a = self.node(a).parent.expect("walked past root");
            } else {
                acc_b = acc_b + self.about_parent(b, t);
                b = self.node(b).parent.expect("walked past root");
            }
        }
        acc_a - acc_b
    }

    /// Position of `of` relative to `about`, composed through their lowest
    /// common ancestor: bit-identical to `relative(..).r` (and to a
    /// snapshot's `relative_r`), without derivatives or allocation.
    pub fn relative_r(&self, of: NodeId, about: NodeId, t: Epoch) -> DVec3 {
        let (mut a, mut b) = (of, about);
        let (mut acc_a, mut acc_b) = (DVec3::ZERO, DVec3::ZERO);
        while a != b {
            if self.depth[a.0 as usize] >= self.depth[b.0 as usize] {
                acc_a += self.node(a).motion.eval_r(t);
                a = self.node(a).parent.expect("walked past root");
            } else {
                acc_b += self.node(b).motion.eval_r(t);
                b = self.node(b).parent.expect("walked past root");
            }
        }
        acc_a - acc_b
    }

    /// Kinematics relative to the root (the system/bubble frame). Prefer
    /// [`Ephemeris::relative`] for anything that needs precision.
    pub fn absolute(&self, of: NodeId, t: Epoch) -> Kinematics {
        self.relative(of, self.root(), t)
    }

    /// Every node relative to its parent at one time, for many relative
    /// queries at that time: the planets evaluated up front, moons and
    /// faint bodies on first use (see `lazy_nodes`).
    pub fn snapshot(&self, t: Epoch) -> Snapshot<'_> {
        let local_r =
            self.nodes.iter().zip(&self.lazy).map(|(n, &l)| if l { DVec3::ZERO } else { n.motion.eval_r(t) }).collect();
        Snapshot {
            eph: self,
            t,
            local_r,
            lazy_r: vec![Cell::new(None); self.nodes.len()],
            full: vec![Cell::new(None); self.nodes.len()],
        }
    }

    /// Ids of all nodes that are physical bodies.
    pub fn bodies(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.iter().enumerate().filter(|(_, n)| n.kind == NodeKind::Body).map(|(i, _)| NodeId(i as u16))
    }
}

/// All nodes' positions relative to their parents at one instant, with full
/// kinematics (velocity, acceleration) evaluated lazily for the nodes that
/// need them. Positions from `relative_r` and `relative(..).r` are identical.
/// The planets' positions are evaluated up front (a force evaluation touches
/// every one, and a lazy check per query would cost more); positions inside
/// moon systems and of faint bodies on first use, since far from them
/// nobody asks (D023 clustering, `World::counts`).
pub struct Snapshot<'a> {
    eph: &'a Ephemeris,
    pub t: Epoch,
    /// Eager nodes' positions (lazy nodes' entries unused).
    local_r: Vec<DVec3>,
    /// Lazy nodes' positions, once evaluated (eager nodes' entries unused).
    lazy_r: Vec<Cell<Option<DVec3>>>,
    full: Vec<Cell<Option<Kinematics>>>,
}

impl Snapshot<'_> {
    /// Moves the snapshot to `t` in place (no allocation): the same values
    /// as a new [`Ephemeris::snapshot`] at `t`.
    pub fn set_time(&mut self, t: Epoch) {
        self.t = t;
        for &i in &self.eph.eager_list {
            self.local_r[i] = self.eph.nodes[i].motion.eval_r(t);
        }
        for &i in &self.eph.lazy_list {
            *self.lazy_r[i].get_mut() = None;
        }
        self.full.iter_mut().for_each(|c| *c.get_mut() = None);
    }

    /// Position of node `i` relative to its parent, evaluating a lazy node
    /// on first use.
    fn local_pos(&self, i: usize) -> DVec3 {
        if !self.eph.lazy[i] {
            return self.local_r[i];
        }
        if let Some(r) = self.lazy_r[i].get() {
            return r;
        }
        let r = self.eph.nodes[i].motion.eval_r(self.t);
        self.lazy_r[i].set(Some(r));
        r
    }

    fn local_full(&self, i: usize) -> Kinematics {
        if let Some(k) = self.full[i].get() {
            return k;
        }
        let k = self.eph.nodes[i].motion.eval(self.t);
        self.full[i].set(Some(k));
        k
    }

    /// Walks from both nodes to their lowest common ancestor, summing `f`.
    fn walk<T: Copy + std::ops::Add<Output = T> + std::ops::Sub<Output = T>>(
        &self,
        of: NodeId,
        about: NodeId,
        zero: T,
        f: impl Fn(usize) -> T,
    ) -> T {
        let (mut a, mut b) = (of, about);
        let (mut acc_a, mut acc_b) = (zero, zero);
        while a != b {
            if self.eph.depth[a.0 as usize] >= self.eph.depth[b.0 as usize] {
                acc_a = acc_a + f(a.0 as usize);
                a = self.eph.node(a).parent.expect("walked past root");
            } else {
                acc_b = acc_b + f(b.0 as usize);
                b = self.eph.node(b).parent.expect("walked past root");
            }
        }
        acc_a - acc_b
    }

    /// Same as [`Ephemeris::relative`] (full kinematics).
    pub fn relative(&self, of: NodeId, about: NodeId) -> Kinematics {
        self.walk(of, about, Kinematics::default(), |i| self.local_full(i))
    }

    /// Position of `of` relative to `about` (cheap: no derivatives).
    #[inline]
    pub fn relative_r(&self, of: NodeId, about: NodeId) -> DVec3 {
        // When neither end has a lazy node on its path to the root, neither
        // has one on its path to their common ancestor: no lazy checks on
        // the common path.
        let lazy = &self.eph.lazy_path;
        if !lazy[of.0 as usize] && !lazy[about.0 as usize] {
            return self.walk(of, about, DVec3::ZERO, |i| self.local_r[i]);
        }
        self.relative_r_lazy(of, about)
    }

    #[cold]
    #[inline(never)]
    fn relative_r_lazy(&self, of: NodeId, about: NodeId) -> DVec3 {
        self.walk(of, about, DVec3::ZERO, |i| self.local_pos(i))
    }

    /// The path from `about` to the root, for many positions relative to
    /// it ([`AnchorPath::relative_r`]): the same bits as
    /// [`Self::relative_r`], without walking `about`'s side again for each.
    pub fn path_to(&self, about: NodeId) -> AnchorPath<'_, '_> {
        let mut nodes = [NodeId(0); MAX_PATH];
        let mut acc = [DVec3::ZERO; MAX_PATH];
        let depth = self.eph.depth[about.0 as usize] as usize;
        assert!(depth < MAX_PATH, "tree deeper than {MAX_PATH}");
        // acc[d]: the sum of the local positions from `about` up to (not
        // including) its ancestor at depth d, accumulated bottom-up as `walk`
        // does.
        let (mut b, mut sum) = (about, DVec3::ZERO);
        for d in (0..=depth).rev() {
            nodes[d] = b;
            acc[d] = sum;
            if d > 0 {
                sum += self.local_pos(b.0 as usize);
                b = self.eph.node(b).parent.expect("walked past root");
            }
        }
        AnchorPath { snap: self, depth, nodes, acc }
    }
}

/// Deepest tree an [`AnchorPath`] holds.
const MAX_PATH: usize = 16;

/// A node's path to the root with the sums `walk` would accumulate on its
/// side ([`Snapshot::path_to`]).
pub struct AnchorPath<'s, 'a> {
    snap: &'s Snapshot<'a>,
    depth: usize,
    nodes: [NodeId; MAX_PATH],
    acc: [DVec3; MAX_PATH],
}

impl AnchorPath<'_, '_> {
    /// Position of `of` relative to the path's node: bit-identical to
    /// `Snapshot::relative_r(of, about)` (each side summed bottom-up to the
    /// common ancestor, then subtracted).
    pub fn relative_r(&self, of: NodeId) -> DVec3 {
        let eph = self.snap.eph;
        let (mut a, mut acc_a) = (of, DVec3::ZERO);
        let fast = !eph.lazy_path[of.0 as usize];
        loop {
            let d = eph.depth[a.0 as usize] as usize;
            if d <= self.depth && self.nodes[d] == a {
                return acc_a - self.acc[d];
            }
            let i = a.0 as usize;
            acc_a += if fast { self.snap.local_r[i] } else { self.snap.local_pos(i) };
            a = eph.node(a).parent.expect("walked past root");
        }
    }
}

/// One reusable [`Snapshot`] for a run of evaluations (an integration's
/// stages and the checks after each step): re-evaluated in place only when
/// the time changes, so repeated queries at one time (the step's last stage
/// and the step-end checks) evaluate the ephemeris once, and none allocates.
pub struct SnapshotCache<'a> {
    eph: &'a Ephemeris,
    snap: std::cell::RefCell<Option<Snapshot<'a>>>,
}

impl<'a> SnapshotCache<'a> {
    pub fn new(eph: &'a Ephemeris) -> Self {
        SnapshotCache { eph, snap: std::cell::RefCell::new(None) }
    }

    /// Calls `f` with the snapshot at `t` (`f` must not use this cache).
    pub fn with<R>(&self, t: Epoch, f: impl FnOnce(&Snapshot<'a>) -> R) -> R {
        let mut slot = self.snap.borrow_mut();
        match slot.as_mut() {
            Some(s) if s.t == t => {}
            Some(s) => s.set_time(t),
            None => *slot = Some(self.eph.snapshot(t)),
        }
        f(slot.as_ref().expect("just set"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(r: DVec3, v: DVec3) -> Motion {
        Motion::Linear(Linear { epoch: Epoch::J2000, r0: r, v })
    }

    fn tree() -> Ephemeris {
        let n = |name: &str, parent: Option<u16>, motion| Node {
            name: name.into(),
            kind: NodeKind::Body,
            parent: parent.map(NodeId),
            gm: 1.0,
            motion,
        };
        Ephemeris::new(
            "test".into(),
            Epoch::J2000,
            Epoch::J2000.add_seconds(1e6),
            vec![
                n("root", None, Motion::Root),
                n("a", Some(0), lin(DVec3::new(1e12, 0.0, 0.0), DVec3::new(0.0, 1.0, 0.0))),
                n("a1", Some(1), lin(DVec3::new(0.0, 5.0, 0.0), DVec3::ZERO)),
                n("b", Some(0), lin(DVec3::new(-1e12, 0.0, 0.0), DVec3::ZERO)),
            ],
        )
    }

    #[test]
    fn relative_goes_through_lowest_common_ancestor() {
        let e = tree();
        let t = Epoch::J2000.add_seconds(10.0);
        let a1_about_a = e.relative(NodeId(2), NodeId(1), t);
        assert_eq!(a1_about_a.r, DVec3::new(0.0, 5.0, 0.0)); // exact: never touched the 1e12 offset
        let a1_about_b = e.relative(NodeId(2), NodeId(3), t);
        assert_eq!(a1_about_b.r, DVec3::new(2e12, 15.0, 0.0));
        assert_eq!(e.relative(NodeId(3), NodeId(2), t).r, -a1_about_b.r);
        assert_eq!(e.relative(NodeId(1), NodeId(1), t).r, DVec3::ZERO);
        let snap = e.snapshot(t);
        // A snapshot moved to `t` from another time holds the same values.
        let mut moved = e.snapshot(Epoch::J2000.add_seconds(-5.0));
        moved.relative(NodeId(2), NodeId(3));
        moved.set_time(t);
        for (x, y) in [(2, 1), (2, 3), (3, 2), (0, 2)] {
            assert_eq!(snap.relative(NodeId(x), NodeId(y)), e.relative(NodeId(x), NodeId(y), t));
            assert_eq!(snap.relative_r(NodeId(x), NodeId(y)), e.relative(NodeId(x), NodeId(y), t).r);
            assert_eq!(e.relative_r(NodeId(x), NodeId(y), t), e.relative(NodeId(x), NodeId(y), t).r);
            assert_eq!(snap.path_to(NodeId(y)).relative_r(NodeId(x)), snap.relative_r(NodeId(x), NodeId(y)));
            assert_eq!(moved.relative(NodeId(x), NodeId(y)), snap.relative(NodeId(x), NodeId(y)));
        }
    }

    #[test]
    fn nodes_inside_a_trailing_moon_system_are_evaluated_lazily_with_the_same_values() {
        let n = |name: &str, kind, parent: Option<u16>, motion| Node {
            name: name.into(),
            kind,
            parent: parent.map(NodeId),
            gm: 1.0,
            motion,
        };
        let body = |name: &str, parent: u16, x: f64| {
            n(name, NodeKind::Body, Some(parent), lin(DVec3::new(x, 1.0, 0.0), DVec3::new(0.0, 2.0, 1.0)))
        };
        let nodes = vec![
            n("root", NodeKind::Barycenter, None, Motion::Root),
            body("planet", 0, 1e11),
            n("system", NodeKind::Barycenter, Some(0), lin(DVec3::new(-7e11, 0.0, 0.0), DVec3::ZERO)),
            body("giant", 2, 3e5),
            body("moon", 3, 4e8),
        ];
        let end = Epoch::J2000.add_seconds(1e6);
        let e = Ephemeris::new("test".into(), Epoch::J2000, end, nodes.clone());
        assert_eq!(e.lazy, [false, false, false, true, true], "the giant and its moon are inside the system");
        let t = Epoch::J2000.add_seconds(77.0);
        let mut snap = e.snapshot(Epoch::J2000);
        snap.set_time(t);
        for (x, y) in [(4, 1), (1, 4), (3, 4), (4, 2), (1, 0), (2, 1)] {
            assert_eq!(snap.relative_r(NodeId(x), NodeId(y)), e.relative(NodeId(x), NodeId(y), t).r);
            assert_eq!(snap.relative(NodeId(x), NodeId(y)), e.relative(NodeId(x), NodeId(y), t));
        }
        // A faint body (a dwarf planet) is lazy wherever it is; a system
        // with a planet after it is not trailing, so eager (Earth–Moon).
        let mut later = nodes;
        later.push(Node { gm: 1e-9, ..body("faint", 0, 5e12) });
        later.push(body("far", 0, 6e12));
        let e = Ephemeris::new("test".into(), Epoch::J2000, end, later);
        assert_eq!(e.lazy, [false, false, false, false, false, true, false]);
        let mut snap = e.snapshot(Epoch::J2000);
        snap.set_time(t);
        for (x, y) in [(5, 6), (6, 5), (5, 4), (6, 1), (0, 5), (4, 3), (3, 4), (4, 0), (2, 2)] {
            assert_eq!(snap.relative_r(NodeId(x), NodeId(y)), e.relative(NodeId(x), NodeId(y), t).r);
            assert_eq!(snap.path_to(NodeId(y)).relative_r(NodeId(x)), snap.relative_r(NodeId(x), NodeId(y)));
        }
        assert!(!tree().lazy.iter().any(|&l| l), "no barycenter below the root, nothing faint");
    }
}
