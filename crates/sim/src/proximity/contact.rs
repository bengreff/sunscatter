//! Craft-to-craft contact forces (D082; D066's contact between two
//! rigid bodies instead of a body and the ground).
//!
//! **Where they touch** ([`touches`]): every sample point of one craft's
//! hull inside the other's solid ([`Hull::penetration`]), both ways (A's
//! points in B, then B's in A), so a corner of either craft is caught.
//! Broad phase: the hulls' bounding spheres; mid phase: each primitive's
//! points' sphere against the other hull's primitive boxes; only the points
//! of primitives that may touch are tested.
//!
//! **The force** at each touching point, along the other solid's normal:
//! a spring-damper `(k·depth + c·closing)⁺` with `k = MODULUS × the
//! point's area` (so the stiffness grows with the touching area, not with
//! how densely the surface is sampled), the pair's damping
//! `C = 2ζ√(K·m_red)` shared in proportion to `k` (K the touching points'
//! total), and regularised Coulomb friction as on the ground
//! ([`crate::contact::friction_force`], capped to what one substep can
//! stop at the point's effective mass). The total stiffness is capped at
//! `m_red·(OMEGA_STEP/dt)²`, so an explicit substep stays stable for any
//! pair (a small piece's contact is softer: it sinks `v·dt/OMEGA_STEP`,
//! millimetres at docking speeds). Following Catto ("Soft Constraints",
//! GDC 2011; Box2D's soft step) in stating a contact by frequency and
//! damping ratio rather than raw stiffness.
//!
//! **Equal and opposite**: the force on B is exactly `−force`, applied at
//! the same point, so the pair's linear and angular momentum change only
//! by round-off (the torques are about each centre of mass: `τ_A + τ_B +
//! d × F_B = 0`). Positions are relative to a local origin both bodies
//! share (the bubble's leader, `sim::vessel::bubble`), so they are metres,
//! not orbit radii (rule 3). Points are visited in a fixed order (A's
//! primitives and points, then B's), so the sums are deterministic.

use super::hull::Hull;
use crate::contact::friction_force;
use glam::{DMat3, DQuat, DVec3};

/// Stiffness of craft contact per unit of touching area (N/m per m²): a
/// flat square metre pressed 1 cm pushes back with 500 kN; two 20 t craft
/// meeting at 0.5 m/s on a curved patch of ~0.5 m² sink ~1 cm.
pub const MODULUS: f64 = 5.0e7;
/// Damping ratio of a contact: restitution ≈ e^(−ζπ/√(1−ζ²)) = 0.37.
pub const DAMPING_RATIO: f64 = 0.3;
/// Most the pair's contact frequency times the substep may be (rad).
pub const OMEGA_STEP: f64 = 0.5;

/// One craft in a contact test: its hull and rigid-body state, positions
/// relative to the local origin the two share (inertial axes).
#[derive(Clone, Copy, Debug)]
pub struct Body<'a> {
    pub hull: &'a Hull,
    /// Centre of mass and its velocity.
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

impl Body<'_> {
    /// The centre of the hull's bounding sphere (relative to the origin).
    pub fn sphere_center(&self) -> DVec3 {
        self.r + self.q * (self.hull.center - self.com)
    }
}

/// The gap between two bodies' bounding spheres (m; negative where they
/// overlap): contact is possible only below zero.
pub fn sphere_gap(a: &Body, b: &Body) -> f64 {
    (b.sphere_center() - a.sphere_center()).length() - a.hull.radius - b.hull.radius
}

/// A sample point of one craft inside the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    /// Relative to A's centre of mass (inertial).
    pub at: DVec3,
    /// Unit normal out of the solid the point is in (inertial): the force
    /// on the point's craft pushes along it.
    pub n: DVec3,
    /// Along the normal (m).
    pub depth: f64,
    /// The point's area (m²).
    pub area: f64,
    /// Whether the point is A's (inside B), else B's (inside A).
    pub on_a: bool,
    /// The parts of A and of B where they touch.
    pub part_a: u32,
    pub part_b: u32,
}

/// The sample points of `a` inside `b` and of `b` inside `a`, in that
/// order (see the module).
pub fn touches(a: &Body, b: &Body) -> Vec<Touch> {
    let mut out = Vec::new();
    if sphere_gap(a, b) > 0.0 {
        return out;
    }
    let d = b.r - a.r;
    // `x` relative to A's centre of mass: the point of `from` (body axes)
    // and where it is in `into`'s body axes.
    let mut one_way = |from: &Body, into: &Body, offset: DVec3, on_a: bool| {
        let into_inv = into.q.inverse();
        for i in 0..from.hull.shapes.len() {
            let (c, radius) = from.hull.spheres[i];
            let x = from.q * (c - from.com) + offset;
            let into_offset = if on_a { d } else { DVec3::ZERO };
            if !into.hull.near_sphere(into_inv * (x - into_offset) + into.com, radius) {
                continue;
            }
            for p in from.hull.points_of(i) {
                let x = from.q * (p.pos - from.com) + offset;
                let local = into_inv * (x - into_offset) + into.com;
                if let Some((depth, n, j)) = into.hull.penetration(local) {
                    let (own, other) = (from.hull.part[i], into.hull.part[j as usize]);
                    let (part_a, part_b) = if on_a { (own, other) } else { (other, own) };
                    out.push(Touch { at: x, n: into.q * n, depth, area: p.area, on_a, part_a, part_b });
                }
            }
        }
    };
    one_way(a, b, DVec3::ZERO, true);
    one_way(b, a, d, false);
    out
}

/// The fastest closing touch of a substep: speed (m/s) along the normal
/// and the parts that met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairImpact {
    pub speed: f64,
    pub part_a: u32,
    pub part_b: u32,
}

/// Contact between two craft for one substep.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PairForces {
    /// Force on A (N, inertial); B takes exactly `−force`.
    pub force: DVec3,
    /// Torques about each centre of mass (N·m, inertial).
    pub torque_a: DVec3,
    pub torque_b: DVec3,
    /// Touching points, and the deepest (m).
    pub count: u32,
    pub depth: f64,
    pub impact: Option<PairImpact>,
}

/// Effective mass of a body pair at lever arms `la`, `lb` (from each
/// centre of mass, inertial) along unit `dir`.
fn effective_mass(a: &Body, b: &Body, la: DVec3, lb: DVec3, dir: DVec3) -> f64 {
    let ua = a.q.inverse() * la.cross(dir);
    let ub = b.q.inverse() * lb.cross(dir);
    1.0 / (1.0 / a.mass + 1.0 / b.mass + ua.dot(a.inertia.inverse() * ua) + ub.dot(b.inertia.inverse() * ub))
}

/// The contact forces between `a` and `b` for a substep of `dt` (s), with
/// friction coefficient `mu` and stick speed `stick` (m/s).
pub fn pair_forces(a: &Body, b: &Body, mu: f64, stick: f64, dt: f64) -> PairForces {
    let touching = touches(a, b);
    let mut out = PairForces::default();
    if touching.is_empty() {
        return out;
    }
    let d = b.r - a.r;
    let m_red = a.mass * b.mass / (a.mass + b.mass);
    let stiffness: f64 = touching.iter().map(|t| MODULUS * t.area).sum();
    let cap = m_red * (OMEGA_STEP / dt) * (OMEGA_STEP / dt);
    let scale = if stiffness > cap { cap / stiffness } else { 1.0 };
    let total = stiffness * scale;
    let damping = 2.0 * DAMPING_RATIO * (total * m_red).sqrt();
    let count = touching.len() as f64;
    for t in &touching {
        let (la, lb) = (t.at, t.at - d);
        let va = a.v + a.omega.cross(la);
        let vb = b.v + b.omega.cross(lb);
        // The point's velocity relative to the other craft's material there.
        let v_rel = if t.on_a { va - vb } else { vb - va };
        let closing = -v_rel.dot(t.n);
        let k = MODULUS * t.area * scale;
        let c = damping * k / total;
        let f_n = (k * t.depth + c * closing).max(0.0);
        let mut f = t.n * f_n;
        let v_t = v_rel + t.n * closing;
        let s = v_t.length();
        if s > 0.0 {
            let m_eff = effective_mass(a, b, la, lb, v_t / s);
            f += friction_force(mu, f_n, v_t, stick, m_eff / (dt * count));
        }
        let on_a = if t.on_a { f } else { -f };
        out.force += on_a;
        out.torque_a += la.cross(on_a);
        out.torque_b -= lb.cross(on_a);
        out.depth = out.depth.max(t.depth);
        if out.impact.is_none_or(|im| closing > im.speed) {
            out.impact = Some(PairImpact { speed: closing, part_a: t.part_a, part_b: t.part_b });
        }
    }
    out.count = touching.len() as u32;
    out
}

#[cfg(test)]
#[path = "contact_tests.rs"]
mod tests;
