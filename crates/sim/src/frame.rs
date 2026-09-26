//! Frame-typed vectors.
//!
//! v0.1 repeatedly compared positions in different reference frames (e.g. an
//! absolute position against a parent-relative one, ~1 AU apart). Here every
//! vector carries its frame *kind* in its type, so mixing kinds does not compile:
//!
//! ```compile_fail
//! use sim::frame::{Vec3, Inertial, BodyFixed};
//! let a: Vec3<Inertial> = Vec3::new(1.0, 0.0, 0.0);
//! let b: Vec3<BodyFixed> = Vec3::new(0.0, 1.0, 0.0);
//! let _ = a + b; // error: different frames
//! ```
//!
//! Frames centred on a *particular* node (Earth vs Moon) share the kind
//! [`Inertial`]; the node is carried at runtime by [`Anchored`] and checked on
//! every combination. Converting between frames always goes through an explicit
//! function (see `ephem::Ephemeris::relative_state`), which walks the frame tree
//! through the lowest common ancestor so no precise quantity is ever formed by
//! subtracting two large absolute coordinates.

use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Marker for a frame kind.
pub trait FrameKind: Copy + Clone + std::fmt::Debug + PartialEq + Default + 'static {}

/// Non-rotating (ICRF-aligned) frame centred on some node of the frame tree
/// (the system barycenter, a body, or a barycenter node).
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Inertial;
/// Frame centred on a body and rotating with it (terrain, launch sites).
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct BodyFixed;
/// Vessel body axes, centred on the vessel's centre of mass.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct VesselLocal;

impl FrameKind for Inertial {}
impl FrameKind for BodyFixed {}
impl FrameKind for VesselLocal {}

/// A 3-vector in frame kind `F`.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Vec3<F: FrameKind> {
    v: DVec3,
    #[serde(skip)]
    _f: PhantomData<F>,
}

impl<F: FrameKind> Vec3<F> {
    pub const ZERO: Self = Self { v: DVec3::ZERO, _f: PhantomData };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { v: DVec3::new(x, y, z), _f: PhantomData }
    }

    /// Wraps a raw vector that the caller knows is expressed in `F`.
    pub const fn from_raw(v: DVec3) -> Self {
        Self { v, _f: PhantomData }
    }

    /// The raw components. Use at math boundaries only (integrators, rendering).
    pub const fn raw(self) -> DVec3 {
        self.v
    }

    pub fn length(self) -> f64 {
        self.v.length()
    }

    pub fn dot(self, o: Self) -> f64 {
        self.v.dot(o.v)
    }

    pub fn cross(self, o: Self) -> Self {
        Self::from_raw(self.v.cross(o.v))
    }
}

impl<F: FrameKind> Add for Vec3<F> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::from_raw(self.v + o.v)
    }
}
impl<F: FrameKind> Sub for Vec3<F> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::from_raw(self.v - o.v)
    }
}
impl<F: FrameKind> AddAssign for Vec3<F> {
    fn add_assign(&mut self, o: Self) {
        self.v += o.v;
    }
}
impl<F: FrameKind> SubAssign for Vec3<F> {
    fn sub_assign(&mut self, o: Self) {
        self.v -= o.v;
    }
}
impl<F: FrameKind> Mul<f64> for Vec3<F> {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self::from_raw(self.v * k)
    }
}
impl<F: FrameKind> Neg for Vec3<F> {
    type Output = Self;
    fn neg(self) -> Self {
        Self::from_raw(-self.v)
    }
}

/// Position, velocity and acceleration in one frame kind.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct State<F: FrameKind> {
    pub r: Vec3<F>,
    pub v: Vec3<F>,
}

impl<F: FrameKind> State<F> {
    pub fn new(r: Vec3<F>, v: Vec3<F>) -> Self {
        Self { r, v }
    }

    pub fn from_raw(r: DVec3, v: DVec3) -> Self {
        Self { r: Vec3::from_raw(r), v: Vec3::from_raw(v) }
    }
}

impl<F: FrameKind> Add for State<F> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self { r: self.r + o.r, v: self.v + o.v }
    }
}
impl<F: FrameKind> Sub for State<F> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self { r: self.r - o.r, v: self.v - o.v }
    }
}

/// Identifier of a node in the frame tree (the system barycenter, a body, or a
/// barycenter node). Indices into the ephemeris node table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub u16);

/// An inertial state expressed relative to a specific node.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anchored {
    pub origin: NodeId,
    pub state: State<Inertial>,
}

impl Anchored {
    pub fn new(origin: NodeId, state: State<Inertial>) -> Self {
        Self { origin, state }
    }

    /// Difference of two states *about the same origin*. Panics on mismatched
    /// origins: re-express one of them first via the ephemeris.
    pub fn minus(&self, other: &Anchored) -> State<Inertial> {
        assert_eq!(self.origin, other.origin, "anchored states about different origins");
        self.state - other.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_frame_arithmetic_works() {
        let a: Vec3<Inertial> = Vec3::new(1.0, 2.0, 3.0);
        let b: Vec3<Inertial> = Vec3::new(1.0, 1.0, 1.0);
        assert_eq!((a - b).raw(), DVec3::new(0.0, 1.0, 2.0));
        assert_eq!((a * 2.0).raw(), DVec3::new(2.0, 4.0, 6.0));
    }

    #[test]
    #[should_panic(expected = "different origins")]
    fn anchored_origin_mismatch_panics() {
        let a = Anchored::new(NodeId(1), State::default());
        let b = Anchored::new(NodeId(2), State::default());
        let _ = a.minus(&b);
    }
}
