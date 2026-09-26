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

pub use chebyshev::{cheb_basis, interpolate_lobatto, lobatto_nodes, ChebTable, MAX_DEGREE};
pub use io::{fnv1a64, EphemerisFormatError};
pub use rails::Rails;

use crate::frame::NodeId;
use crate::time::Epoch;
use glam::DVec3;
use serde::{Deserialize, Serialize};

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
        Ephemeris { generator, start, end, nodes, depth }
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

    /// Kinematics relative to the root (the system/bubble frame). Prefer
    /// [`Ephemeris::relative`] for anything that needs precision.
    pub fn absolute(&self, of: NodeId, t: Epoch) -> Kinematics {
        self.relative(of, self.root(), t)
    }

    /// Evaluates every node relative to its parent once, for many relative
    /// queries at the same time (one force evaluation touches every source).
    pub fn snapshot(&self, t: Epoch) -> Snapshot<'_> {
        Snapshot { eph: self, t, local: self.nodes.iter().map(|n| n.motion.eval(t)).collect() }
    }

    /// Ids of all nodes that are physical bodies.
    pub fn bodies(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.iter().enumerate().filter(|(_, n)| n.kind == NodeKind::Body).map(|(i, _)| NodeId(i as u16))
    }
}

/// All nodes' kinematics relative to their parents at one instant.
pub struct Snapshot<'a> {
    eph: &'a Ephemeris,
    pub t: Epoch,
    local: Vec<Kinematics>,
}

impl Snapshot<'_> {
    /// Same as [`Ephemeris::relative`], from the cached per-node values.
    pub fn relative(&self, of: NodeId, about: NodeId) -> Kinematics {
        let (mut a, mut b) = (of, about);
        let mut acc_a = Kinematics::default();
        let mut acc_b = Kinematics::default();
        while a != b {
            if self.eph.depth[a.0 as usize] >= self.eph.depth[b.0 as usize] {
                acc_a = acc_a + self.local[a.0 as usize];
                a = self.eph.node(a).parent.expect("walked past root");
            } else {
                acc_b = acc_b + self.local[b.0 as usize];
                b = self.eph.node(b).parent.expect("walked past root");
            }
        }
        acc_a - acc_b
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
        for (x, y) in [(2, 1), (2, 3), (3, 2), (0, 2)] {
            assert_eq!(snap.relative(NodeId(x), NodeId(y)), e.relative(NodeId(x), NodeId(y), t));
        }
    }
}
