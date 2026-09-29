//! Rigid-body ground contact (realism-1 §3e, D066).
//!
//! The craft's contact points ([`crate::craft::ContactPoint`]: the feet,
//! then hull points on the convex hull) touch the body's solid surface
//! (`BodyPhysical::altitude_above_surface`, with the procedural detail, so
//! what is drawn is what is landed on). Each point that is below the
//! surface gets
//! * a spring-damper along the local terrain normal (normal from finite
//!   differences of the surface): feet are soft with a stroke, and a foot
//!   past its stroke adds the stiff hull spring; hull points are stiff;
//! * Coulomb friction in the tangent plane, regularised below a stick speed
//!   and capped so it can at most stop the point in one substep.
//!
//! Forces and torques go into the vessel's rigid body in fixed substeps
//! ([`SUBSTEPS`] of the 20 ms tick) while any contact point is within
//! [`NEAR`] of the surface. A point past its free travel (a hull point, or
//! a foot past its stroke) still closing faster than the craft's impact
//! limit after the substep's contact force destroys the craft (unless
//! debug mode); the wreck keeps its dynamics (engine and controls cut)
//! until it comes to rest.
//!
//! **Rest** ([`rest_ticks`]): the vessel freezes (into `Landed`, or a wreck
//! into `Crashed`) after [`REST_TICKS`] ticks in a row touching, with no
//! input, with its kinetic energy relative to the ground below
//! [`REST_ENERGY`] per kilogram, and, if intact, standing stably
//! ([`holds`] on the points touching now: at least three, the centre of
//! mass inside their polygon by [`REST_MARGIN`], the ground within their
//! friction). A vessel caught at the top of a bounce or balanced while
//! tipping is not at rest.
//!
//! The rules ([`normal_force`], [`friction_force`], [`impact`],
//! [`ground_normal`], [`holds`], [`kinetic_per_mass`], [`rest_ticks`]) are
//! pure functions with table tests; [`forces`] sums them over the points
//! for one substep, and [`support`] lists the points touching.

use crate::body::BodyPhysical;
use crate::craft::{ContactFile, ContactKind, ContactPoint};
use crate::ephem::Snapshot;
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::world::World;
use glam::{DMat3, DQuat, DVec3};

/// Substeps per 20 ms tick while near the surface (2 ms each; a fixed count,
/// so contact is deterministic).
pub const SUBSTEPS: usize = 10;
/// Contact substeps run while any contact point may be within this height of
/// the surface (m).
pub const NEAR: f64 = 10.0;
/// Extra height a live vessel with its engine off climbs above [`NEAR`]
/// before returning to a coast (m): hysteresis, so it does not alternate.
pub const LEAVE: f64 = 5.0;
/// Rest threshold: kinetic energy relative to the ground per kilogram
/// (J/kg): that of 5 cm/s (translation and rotation together).
pub const REST_ENERGY: f64 = 0.5 * 0.05 * 0.05;
/// Ticks the vessel must stay at rest before freezing (1 s).
pub const REST_TICKS: u32 = 50;
/// Least distance of the centre of mass (seen along gravity) inside the
/// support polygon for a stable stance (m).
pub const REST_MARGIN: f64 = 0.02;
/// A frozen pose's contact points within this clearance are its support
/// when it is checked again (m; they are sunk by millimetres to centimetres).
pub const SUPPORT_CLEARANCE: f64 = 0.01;
/// Finite-difference step for the terrain normal (m).
pub const NORMAL_STEP: f64 = 0.5;

/// Normal force (N, never negative) at a point `depth` into the ground along
/// the normal (m), closing at `closing` (m/s, positive towards the ground).
/// A foot is its own spring up to its stroke; past it, the hull spring and
/// damper take the rest.
pub fn normal_force(kind: ContactKind, c: &ContactFile, depth: f64, closing: f64) -> f64 {
    if depth <= 0.0 {
        return 0.0;
    }
    let (spring, damping) = match kind {
        ContactKind::Foot => {
            let s = c.foot.stroke;
            let past = (depth - s).max(0.0);
            let spring = c.foot.stiffness * depth.min(s) + c.hull.stiffness * past;
            (spring, if past > 0.0 { c.foot.damping + c.hull.damping } else { c.foot.damping })
        }
        ContactKind::Hull => (c.hull.stiffness * depth, c.hull.damping),
    };
    (spring + damping * closing).max(0.0)
}

/// Friction force (N) at a point sliding at `v_t` (in the tangent plane)
/// under normal force `f_n`: Coulomb (`mu·f_n` against the sliding),
/// viscous below `stick_speed`, and at most `max_damping·|v_t|` (so an
/// explicit substep cannot reverse the sliding).
pub fn friction_force(mu: f64, f_n: f64, v_t: DVec3, stick_speed: f64, max_damping: f64) -> DVec3 {
    let s = v_t.length();
    if s == 0.0 || f_n <= 0.0 {
        return DVec3::ZERO;
    }
    -v_t * (mu * f_n / s.max(stick_speed)).min(max_damping)
}

/// Whether a point destroys the craft: a hull point, or a foot past its
/// stroke, closing faster than `max_speed`.
pub fn impact(kind: ContactKind, stroke: f64, depth: f64, closing: f64, max_speed: f64) -> bool {
    let free_travel = match kind {
        ContactKind::Foot => stroke,
        ContactKind::Hull => 0.0,
    };
    depth > free_travel && closing > max_speed
}

/// Unit normal of the surface `clearance(p) = 0` at body-fixed `p` (outward),
/// from central differences of `step` (m) along east and north.
pub fn ground_normal(clearance: impl Fn(DVec3) -> f64, p: DVec3, step: f64) -> DVec3 {
    let up = p.normalize();
    let east = DVec3::Z.cross(up);
    let east = if east.length() < 1e-9 { DVec3::X } else { east.normalize() };
    let north = up.cross(east);
    let slope = |d: DVec3| (clearance(p + d * step) - clearance(p - d * step)) / (2.0 * step);
    (up + east * slope(east) + north * slope(north)).normalize()
}

/// Kinetic energy per kilogram (J/kg) of a rigid body of `mass` and body-axes
/// `inertia` (about its centre of mass) with attitude `q` (body → inertial),
/// moving at `v_rel` and turning at `w_rel` (inertial axes) relative to the
/// ground.
pub fn kinetic_per_mass(mass: f64, inertia: &DMat3, q: DQuat, v_rel: DVec3, w_rel: DVec3) -> f64 {
    let w = q.inverse() * w_rel;
    0.5 * v_rel.length_squared() + 0.5 * w.dot(*inertia * w) / mass
}

/// New rest counter after a tick: counts ticks in a row that touch the
/// ground with no input, kinetic energy `kinetic` (J/kg, relative to the
/// ground) below [`REST_ENERGY`] and a stance that `stable` (see [`holds`];
/// a wreck needs none).
pub fn rest_ticks(count: u32, touching: bool, input: bool, kinetic: f64, stable: bool) -> u32 {
    if touching && !input && kinetic < REST_ENERGY && stable {
        count + 1
    } else {
        0
    }
}

/// Whether a craft standing still holds on the ground: the ground under each
/// support point is no steeper than its friction allows, and its centre of
/// mass, seen along gravity (`down`, unit), is inside the polygon of the
/// support points by at least `margin` (m). `support` is (position, ground
/// normal, friction).
pub fn holds(com: DVec3, down: DVec3, support: &[(DVec3, DVec3, f64)], margin: f64) -> bool {
    let slides = support.iter().any(|&(_, n, mu)| {
        let c = -n.dot(down);
        c <= 0.0 || (1.0 - c * c) > mu * mu * c * c
    });
    if slides || support.len() < 3 {
        return false;
    }
    // Plane axes perpendicular to gravity.
    let a = if down.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let a = (a - down * a.dot(down)).normalize();
    let b = down.cross(a);
    let flat = |p: DVec3| [p.dot(a), p.dot(b)];
    let mut pts: Vec<[f64; 2]> = support.iter().map(|&(p, _, _)| flat(p)).collect();
    let c = flat(com);
    let hull = convex_hull(&mut pts);
    if hull.len() < 3 {
        return false;
    }
    (0..hull.len()).all(|i| {
        let (p, q) = (hull[i], hull[(i + 1) % hull.len()]);
        let edge = ((q[0] - p[0]) * (q[0] - p[0]) + (q[1] - p[1]) * (q[1] - p[1])).sqrt();
        cross(p, q, c) > margin * edge
    })
}

/// z component of `(p − o) × (q − o)`.
fn cross(o: [f64; 2], p: [f64; 2], q: [f64; 2]) -> f64 {
    (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])
}

/// Counter-clockwise convex hull (monotone chain; collinear points dropped).
fn convex_hull(pts: &mut [[f64; 2]]) -> Vec<[f64; 2]> {
    pts.sort_by(|p, q| p[0].total_cmp(&q[0]).then(p[1].total_cmp(&q[1])));
    let chain = |it: &mut dyn Iterator<Item = [f64; 2]>| {
        let mut h: Vec<[f64; 2]> = Vec::new();
        for p in it {
            while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
                h.pop();
            }
            h.push(p);
        }
        h.pop();
        h
    };
    let mut hull = chain(&mut pts.iter().copied());
    hull.extend(chain(&mut pts.iter().rev().copied()));
    hull
}

/// The ground of one body during a substep, seen from the vessel's anchor
/// (inertial axes).
pub struct Ground<'a> {
    pub body: NodeId,
    pub physical: &'a BodyPhysical,
    /// Inertial → body-fixed axes.
    pub to_fixed: DMat3,
    /// Body centre relative to the anchor, and its velocity.
    pub center: DVec3,
    pub center_v: DVec3,
    /// Body angular velocity (inertial axes).
    pub omega: DVec3,
}

impl<'a> Ground<'a> {
    pub fn new(world: &'a World, snap: &Snapshot, anchor: NodeId, body: NodeId) -> Self {
        let physical = world.source(body).and_then(|s| s.physical.as_ref()).expect("a body with a surface");
        let rot = physical.rotation;
        let col = |v: DVec3| rot.to_fixed(Vec3::from_raw(v), snap.t).raw();
        let kin = snap.relative(body, anchor);
        Ground {
            body,
            physical,
            to_fixed: DMat3::from_cols(col(DVec3::X), col(DVec3::Y), col(DVec3::Z)),
            center: kin.r,
            center_v: kin.v,
            omega: rot.omega(snap.t).raw(),
        }
    }

    /// Body-fixed position of an anchor-relative point.
    pub fn fixed(&self, p: DVec3) -> Vec3<BodyFixed> {
        Vec3::from_raw(self.to_fixed * (p - self.center))
    }

    /// Height of an anchor-relative point above the solid surface (m).
    pub fn clearance(&self, p: DVec3) -> f64 {
        self.physical.altitude_above_surface(self.fixed(p))
    }

    /// Velocity of the ground (moving with the body) at an anchor-relative point.
    pub fn velocity(&self, p: DVec3) -> DVec3 {
        self.center_v + self.omega.cross(p - self.center)
    }

    /// Outward surface normal (inertial axes) under a body-fixed point.
    pub fn normal_at_fixed(&self, fixed: DVec3) -> DVec3 {
        let body = self.physical;
        let n = ground_normal(|x| body.altitude_above_surface(Vec3::from_raw(x)), fixed, NORMAL_STEP);
        self.to_fixed.transpose() * n
    }
}

/// The points of `points` within `clearance` (m) of the ground (touching:
/// 0), as `(position, ground normal, friction)` in the anchor frame, for
/// [`holds`].
pub fn support(
    ground: &Ground,
    pose: &Pose,
    points: &[ContactPoint],
    c: &ContactFile,
    clearance: f64,
) -> Vec<(DVec3, DVec3, f64)> {
    points
        .iter()
        .filter_map(|cp| {
            let p = pose.r + pose.q * (cp.pos - pose.com);
            let fixed = ground.fixed(p);
            let mu = match cp.kind {
                ContactKind::Foot => c.foot.friction,
                ContactKind::Hull => c.hull.friction,
            };
            (ground.physical.altitude_above_surface(fixed) <= clearance)
                .then(|| (p, ground.normal_at_fixed(fixed.raw()), mu))
        })
        .collect()
}

/// Closing speed (m/s, positive towards the ground) of contact point `cp`
/// of `pose` along the ground normal under it.
pub fn closing_speed(ground: &Ground, pose: &Pose, cp: &ContactPoint) -> f64 {
    let lever = pose.q * (cp.pos - pose.com);
    let p = pose.r + lever;
    let n = ground.normal_at_fixed(ground.fixed(p).raw());
    -(pose.v + pose.omega.cross(lever) - ground.velocity(p)).dot(n)
}

/// The rigid body touching the ground (anchor frame, inertial axes).
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// Centre of mass and its velocity relative to the anchor.
    pub r: DVec3,
    pub v: DVec3,
    /// Body → inertial, and angular velocity (inertial).
    pub q: DQuat,
    pub omega: DVec3,
    /// Centre of mass (body axes), mass and inertia about it (body axes).
    pub com: DVec3,
    pub mass: f64,
    pub inertia: DMat3,
}

/// A point that hit too fast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    /// Index into the craft's contact points.
    pub point: u32,
    /// Closing speed along the normal (m/s).
    pub speed: f64,
}

/// Contact on a rigid body for one substep.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContactForces {
    /// Total force (N) and torque about the centre of mass (N·m), inertial.
    pub force: DVec3,
    pub torque: DVec3,
    /// Whether any point is on or below the surface.
    pub touching: bool,
    /// The fastest point past its free travel closing above the impact
    /// limit at the start of the substep (a candidate: it destroys the
    /// craft if still too fast after the substep's force).
    pub impact: Option<Impact>,
}

/// Contact force and torque on `pose` from `points` touching `ground`, for
/// a substep of `dt` (friction is capped to what it can stop in `dt`).
pub fn forces(
    ground: &Ground,
    pose: &Pose,
    points: &[ContactPoint],
    c: &ContactFile,
    max_speed: f64,
    dt: f64,
) -> ContactForces {
    struct Touch {
        lever: DVec3,
        n: DVec3,
        f_n: f64,
        v_t: DVec3,
        mu: f64,
    }
    let mut out = ContactForces::default();
    let mut touches = Vec::new();
    for (i, cp) in points.iter().enumerate() {
        let lever = pose.q * (cp.pos - pose.com);
        let p = pose.r + lever;
        let fixed = ground.fixed(p);
        let clear = ground.physical.altitude_above_surface(fixed);
        if clear > 0.0 {
            continue;
        }
        out.touching = true;
        let n = ground.normal_at_fixed(fixed.raw());
        let up = ground.to_fixed.transpose() * fixed.raw().normalize();
        let depth = -clear * n.dot(up);
        let v_point = pose.v + pose.omega.cross(lever) - ground.velocity(p);
        let closing = -v_point.dot(n);
        let (spring, stroke) = match cp.kind {
            ContactKind::Foot => (c.foot, c.foot.stroke),
            ContactKind::Hull => (c.hull, 0.0),
        };
        if impact(cp.kind, stroke, depth, closing, max_speed) && out.impact.is_none_or(|im| closing > im.speed) {
            out.impact = Some(Impact { point: i as u32, speed: closing });
        }
        let f_n = normal_force(cp.kind, c, depth, closing);
        touches.push(Touch { lever, n, f_n, v_t: v_point + n * closing, mu: spring.friction });
    }
    let inv = pose.inertia.inverse();
    let count = touches.len() as f64;
    for t in &touches {
        let mut f = t.n * t.f_n;
        let s = t.v_t.length();
        if s > 0.0 {
            // Effective mass of the point along its sliding direction.
            let u = pose.q.inverse() * t.lever.cross(t.v_t / s);
            let m_eff = 1.0 / (1.0 / pose.mass + u.dot(inv * u));
            f += friction_force(t.mu, t.f_n, t.v_t, c.stick_speed, m_eff / (dt * count));
        }
        out.force += f;
        out.torque += t.lever.cross(f);
    }
    out
}

#[cfg(test)]
mod tests;
