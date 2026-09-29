//! Burns: thrust laws and flight plans (realism-1 §6a, review sim 11).
//!
//! A [`BurnLaw`] is steady thrust with a constant mass flow along a direction
//! law. A [`FlightPlan`] is a list of [`PlannedBurn`]s; a coasting vessel's
//! trajectory is built from it as coast → burn → coast …, so rails warp only
//! samples a stored trajectory (rule 4) and a planned burn gives the same
//! result at every warp.
//!
//! The `Tracking` direction law names a reference body only to say what
//! "prograde" means (a display choice): the direction is a function of the
//! vessel's state relative to that body, and the physics is unchanged by it
//! (rule 1: no reference bodies).

use crate::ephem::{Kinematics, Snapshot};
use crate::frame::NodeId;
use crate::math;
use crate::time::Epoch;
use glam::DVec3;

/// Standard gravity (m/s²), for specific impulse.
pub const G0: f64 = 9.806_65;

/// Where the thrust points.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DirectionLaw {
    /// A fixed inertial direction (any length; normalised when used).
    Inertial(DVec3),
    /// Components along prograde, normal and radial-out (`x`, `y`, `z` of
    /// `axes`) of the vessel's motion relative to `reference`.
    Tracking { reference: NodeId, axes: DVec3 },
}

impl DirectionLaw {
    /// Unit thrust direction (inertial axes) for a vessel at `(r, v)` relative
    /// to `anchor`. The reference body's state comes from the frame tree
    /// (lowest common ancestor), never from subtracting absolute positions.
    pub fn direction(&self, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> DVec3 {
        self.direction_from(|node| snap.relative(node, anchor), anchor, r, v)
    }

    /// As [`DirectionLaw::direction`], with `relative(node)` giving a
    /// node's state relative to `anchor` (called only for a tracking law
    /// whose reference is not the anchor).
    pub fn direction_from(
        &self,
        relative: impl FnOnce(NodeId) -> Kinematics,
        anchor: NodeId,
        r: DVec3,
        v: DVec3,
    ) -> DVec3 {
        match *self {
            DirectionLaw::Inertial(d) => d.normalize(),
            DirectionLaw::Tracking { reference, axes } => {
                let (rel_r, rel_v) = if reference == anchor {
                    (r, v)
                } else {
                    let k = relative(reference);
                    (r - k.r, v - k.v)
                };
                let (prograde, normal, radial) = prograde_normal_radial(rel_r, rel_v);
                (prograde * axes.x + normal * axes.y + radial * axes.z).normalize()
            }
        }
    }
}

/// Unit prograde, orbit-normal and radial-out axes of a relative state.
pub fn prograde_normal_radial(r: DVec3, v: DVec3) -> (DVec3, DVec3, DVec3) {
    let prograde = v.normalize();
    let normal = r.cross(v).normalize();
    (prograde, normal, prograde.cross(normal))
}

/// Steady thrust along a direction law, burning propellant at a constant rate.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BurnLaw {
    /// Thrust (N).
    pub thrust: f64,
    /// Propellant mass flow (kg/s, ≥ 0).
    pub mass_flow: f64,
    pub direction: DirectionLaw,
}

impl BurnLaw {
    /// A law from thrust and specific impulse (s): `ṁ = F / (Isp·g0)`.
    pub fn from_isp(thrust: f64, isp: f64, direction: DirectionLaw) -> Self {
        BurnLaw { thrust, mass_flow: thrust / (isp * G0), direction }
    }

    /// Whether the numbers can be integrated.
    pub fn is_valid(&self) -> bool {
        self.thrust.is_finite() && self.thrust > 0.0 && self.mass_flow.is_finite() && self.mass_flow >= 0.0
    }
}

/// When a burn stops.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BurnEnd {
    /// After this many seconds.
    Duration(f64),
    /// Once the thrust has delivered this Δv (m/s): the rocket equation gives
    /// the duration from the mass at ignition.
    DeltaV(f64),
}

/// What limits planned burns: the propellant (the mass cannot drop below
/// `dry_mass`; a burn stops at burnout), or none in debug mode (`infinite`:
/// burns take no mass, D064). The default (no dry mass) only requires the
/// mass to stay positive.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BurnLimits {
    pub dry_mass: f64,
    pub infinite: bool,
}

impl BurnLimits {
    /// The law actually flown: no mass flow with infinite propellant.
    pub fn effective(&self, law: BurnLaw) -> BurnLaw {
        if self.infinite {
            BurnLaw { mass_flow: 0.0, ..law }
        } else {
            law
        }
    }
}

impl BurnEnd {
    /// The burn's duration (s) for a vessel of mass `m0` at ignition. `None`
    /// if it cannot be flown (invalid law, or the mass would reach zero).
    pub fn duration(&self, law: &BurnLaw, m0: f64) -> Option<f64> {
        self.duration_limited(law, m0, 0.0)
    }

    /// As [`BurnEnd::duration`], but stopping at burnout when the mass
    /// reaches `dry_mass` (if positive).
    pub fn duration_limited(&self, law: &BurnLaw, m0: f64, dry_mass: f64) -> Option<f64> {
        if !law.is_valid() || !(m0.is_finite() && m0 > 0.0) {
            return None;
        }
        let d = match *self {
            BurnEnd::Duration(d) => d,
            // Δv = (F/ṁ) ln(m0/m1)  ⇒  t = (m0/ṁ)(1 − exp(−Δv ṁ/F)).
            BurnEnd::DeltaV(dv) if law.mass_flow > 0.0 => {
                m0 / law.mass_flow * (1.0 - math::exp(-dv * law.mass_flow / law.thrust))
            }
            BurnEnd::DeltaV(dv) => dv * m0 / law.thrust,
        };
        let d = if dry_mass > 0.0 && law.mass_flow > 0.0 { d.min((m0 - dry_mass).max(0.0) / law.mass_flow) } else { d };
        (d.is_finite() && d >= 0.0 && m0 - law.mass_flow * d > 0.0).then_some(d)
    }
}

/// One burn of a flight plan.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlannedBurn {
    /// Ignition epoch.
    pub t_start: Epoch,
    pub law: BurnLaw,
    pub end: BurnEnd,
}

impl PlannedBurn {
    /// A burn of `dv` (m/s, components in the direction law's axes: inertial,
    /// or prograde/normal/radial) with the given engine.
    pub fn delta_v(t_start: Epoch, dv: DVec3, thrust: f64, isp: f64, tracking: Option<NodeId>) -> Self {
        let direction = match tracking {
            Some(reference) => DirectionLaw::Tracking { reference, axes: dv },
            None => DirectionLaw::Inertial(dv),
        };
        PlannedBurn { t_start, law: BurnLaw::from_isp(thrust, isp, direction), end: BurnEnd::DeltaV(dv.length()) }
    }

    /// A burn of `dv` flown by a craft's engine (full throttle, vacuum).
    pub fn delta_v_with(t_start: Epoch, dv: DVec3, engine: &crate::craft::Engine, tracking: Option<NodeId>) -> Self {
        PlannedBurn::delta_v(t_start, dv, engine.thrust_vac, engine.isp_vac, tracking)
    }
}

/// A vessel's planned burns, in order of ignition (saved with the vessel).
/// Burns stay in the list after they are flown; the trajectory remembers how
/// many it has started.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FlightPlan {
    pub burns: Vec<PlannedBurn>,
}

/// Why a flight plan was not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// Burns must be in order of ignition.
    NotSorted,
    /// The edit changes a burn that has already started (or was skipped).
    Started,
    /// A changed burn ignites before the vessel's current time.
    InThePast,
}

impl FlightPlan {
    pub fn is_sorted(&self) -> bool {
        self.burns.windows(2).all(|w| w[0].t_start.seconds_since(w[1].t_start) <= 0.0)
    }

    /// Index of the first burn that differs between two plans.
    pub fn first_difference(&self, other: &FlightPlan) -> usize {
        let same = self.burns.iter().zip(&other.burns).take_while(|(a, b)| a == b).count();
        if same == self.burns.len() && same == other.burns.len() {
            usize::MAX
        } else {
            same
        }
    }

    /// The first burn at or after index `from` that ignites at or after `t`
    /// (earlier ones were missed), or `burns.len()`.
    pub fn next_from(&self, from: usize, t: Epoch) -> usize {
        let mut i = from.min(self.burns.len());
        while i < self.burns.len() && self.burns[i].t_start.seconds_since(t) < 0.0 {
            i += 1;
        }
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn law(mass_flow: f64) -> BurnLaw {
        BurnLaw { thrust: 600_000.0, mass_flow, direction: DirectionLaw::Inertial(DVec3::X) }
    }

    #[test]
    fn delta_v_duration_follows_the_rocket_equation() {
        // (thrust law, m0, Δv) → duration, then Δv back from the rocket equation.
        for (mdot, m0, dv) in [(174.8, 20_000.0, 100.0), (50.0, 5_000.0, 2_000.0), (1.0, 1_000.0, 0.0)] {
            let l = law(mdot);
            let t = BurnEnd::DeltaV(dv).duration(&l, m0).unwrap();
            let m1 = m0 - mdot * t;
            let back = l.thrust / mdot * math::ln(m0 / m1);
            assert!((back - dv).abs() < 1e-9 * dv.max(1.0), "Δv {dv}: got {back}");
        }
        // No mass flow: constant acceleration.
        assert_eq!(BurnEnd::DeltaV(30.0).duration(&law(0.0), 20_000.0), Some(1.0));
    }

    #[test]
    fn burns_stop_at_burnout() {
        // (end, m0, dry) → duration: 100 kg/s, 5 t of propellant = 50 s.
        let l = law(100.0);
        let cases = [
            (BurnEnd::Duration(20.0), 9_000.0, 4_000.0, Some(20.0)),
            (BurnEnd::Duration(80.0), 9_000.0, 4_000.0, Some(50.0)),
            (BurnEnd::Duration(200.0), 9_000.0, 4_000.0, Some(50.0)),
            (BurnEnd::Duration(10.0), 4_000.0, 4_000.0, Some(0.0)),
            (BurnEnd::DeltaV(1e6), 9_000.0, 4_000.0, Some(50.0)),
        ];
        for (end, m0, dry, expected) in cases {
            assert_eq!(end.duration_limited(&l, m0, dry), expected, "{end:?}");
        }
        // Infinite propellant: no mass flow, the rocket equation degenerates.
        let inf = BurnLimits { dry_mass: 4_000.0, infinite: true }.effective(l);
        assert_eq!(inf.mass_flow, 0.0);
        assert_eq!(BurnEnd::DeltaV(30.0).duration_limited(&inf, 20_000.0, 4_000.0), Some(1.0));
    }

    #[test]
    fn burns_that_cannot_be_flown_have_no_duration() {
        assert_eq!(BurnEnd::Duration(200.0).duration(&law(100.0), 20_000.0), None, "burns all the mass");
        assert_eq!(BurnEnd::Duration(-1.0).duration(&law(100.0), 20_000.0), None);
        assert_eq!(BurnEnd::Duration(1.0).duration(&law(f64::NAN), 20_000.0), None);
        let zero_thrust = BurnLaw { thrust: 0.0, ..law(1.0) };
        assert_eq!(BurnEnd::DeltaV(1.0).duration(&zero_thrust, 20_000.0), None);
    }

    #[test]
    fn tracking_axes_table() {
        // (r, v) → (prograde, normal, radial-out).
        let x = DVec3::X * 7e6;
        let cases = [
            (x, DVec3::Y * 7.5e3, (DVec3::Y, DVec3::Z, DVec3::X)),
            (x, -DVec3::Y * 7.5e3, (-DVec3::Y, -DVec3::Z, DVec3::X)),
            (
                x,
                DVec3::new(1e3, 7.5e3, 0.0),
                (DVec3::new(1e3, 7.5e3, 0.0).normalize(), DVec3::Z, DVec3::new(7.5e3, -1e3, 0.0).normalize()),
            ),
        ];
        for (r, v, (p, n, rad)) in cases {
            let (gp, gn, grad) = prograde_normal_radial(r, v);
            assert!(gp.abs_diff_eq(p, 1e-15) && gn.abs_diff_eq(n, 1e-15) && grad.abs_diff_eq(rad, 1e-15), "{r} {v}");
        }
    }
}
