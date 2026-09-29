//! Planned burns flown by the attitude (D075).
//!
//! A burn segment advances on the tick lattice from its ignition, like live
//! powered flight: each tick the maneuver hold aims the engine's axis at
//! the burn law's direction ([`hold::Aim`]) with the attitude control and
//! the engine gimbal ([`attitude::control_tick`]), the rigid body turns,
//! and the thrust acts along the craft's actual attitude (after gimbal)
//! for the tick. The translation integrates that constant thrust
//! acceleration over the tick with the segment's integrator. The attitude
//! at every tick end is stored with the segment, so a planned burn is the
//! same at every warp and after a save, and the vessel's attitude during
//! the burn is read from it.
//!
//! The thrust acceleration of a tick is the mean over the tick of `F/m(t)`
//! with the mass falling linearly, `(F/ṁ)·ln(m_a/m_b)/dt`, so a burn ended
//! by Δv delivers exactly the rocket equation's Δv along the directions it
//! actually thrusted in (turning lag and gimbal deflection included).
//!
//! A trajectory with no craft (tests, bare predictions) points the thrust
//! along the law exactly, still tick by tick.

use super::attitude::{self, Actuators, Attitude, AttitudeControl, Gimbal};
use super::burn::BurnLaw;
use super::hold::{quat_arc, Aim, AimKey};
use super::{quat_from_rotvec, Controls};
use crate::craft::{CraftParams, Engine, MassModel};
use crate::frame::NodeId;
use crate::math;
use crate::time::Epoch;
use crate::world::World;
use glam::{DQuat, DVec3};

/// What a burn needs of the craft to fly it by the attitude.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BurnCraft {
    pub mass: MassModel,
    /// Attitude-control authority per body axis (N·m).
    pub torque: DVec3,
    pub engine: Engine,
}

impl BurnCraft {
    pub fn of(craft: &CraftParams) -> Self {
        BurnCraft { mass: craft.mass, torque: craft.torque, engine: craft.engine }
    }

    /// The maneuver hold's lead before ignition at mass `mass` (s):
    /// [`super::hold::turn_lead`].
    pub fn turn_lead(&self, mass: f64) -> f64 {
        super::hold::turn_lead(&self.mass.at(mass - self.mass.dry_mass).inertia, self.torque)
    }

    /// An estimate of the attitude at ignition when a burn is predicted
    /// ahead of time: the engine's axis turned from `from` onto `dir` by
    /// the smallest rotation (as the maneuver hold turns), rotating at
    /// `rate` (the direction's), settled. The vessel checks it at ignition
    /// and flies the burn again from its actual attitude if they differ
    /// ([`super::Trajectory::refly`]).
    pub fn settled_on(&self, from: DQuat, dir: DVec3, rate: DVec3) -> (Attitude, AttitudeControl) {
        let q = (quat_arc(from * self.engine.mount_dir, dir) * from).normalize();
        let control =
            AttitudeControl { base: None, hold: Some(q), tracking: true, settled: Some(AimKey::BURN), waiting: false };
        (Attitude { q, omega: rate }, control)
    }
}

/// A burn law's direction at `t` for a vessel at `(r, v)` relative to
/// `anchor` with acceleration `a`, and the direction's angular velocity
/// (inertial): from the direction a tick later on the state carried with
/// that acceleration. What a craft settled on the burn turns at.
pub fn direction_and_rate(
    world: &World,
    law: &BurnLaw,
    t: Epoch,
    (anchor, r, v): (NodeId, DVec3, DVec3),
    a: DVec3,
) -> (DVec3, DVec3) {
    let h = super::TICK;
    let dir = law.direction.direction(&world.snapshot(t), anchor, r, v);
    let (r1, v1) = (r + v * h + a * (0.5 * h * h), v + a * h);
    let next = law.direction.direction(&world.snapshot(t.add_seconds(h)), anchor, r1, v1);
    (dir, dir.cross(next) / h)
}

/// The attitude side of a burn segment.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Flight {
    /// `None`: thrust exactly along the law (no attitude).
    pub craft: Option<BurnCraft>,
    /// Attitude and control at ignition.
    pub start: (Attitude, AttitudeControl),
    /// Attitude and control after the last tick.
    pub att: Attitude,
    pub control: AttitudeControl,
    /// The acceleration at ignition before thrust (the coast's last), for
    /// the burn direction's rate there ([`direction_and_rate`]).
    #[serde(default)]
    pub accel0: DVec3,
    /// Ticks flown.
    pub ticks: u64,
    /// Thrust acceleration of the last tick (inertial, m/s²).
    pub thrust: DVec3,
    /// The attitude at ignition and at each tick's end (local time).
    attitudes: Vec<(f64, Attitude)>,
}

impl Flight {
    /// A burn flown from `att` and `control` at ignition (the coast
    /// lattice base is not part of a burn).
    /// A direction hold's target restarts one tick behind the attitude
    /// ([`attitude::resume_hold`]).
    pub fn new(craft: Option<BurnCraft>, att: Attitude, control: AttitudeControl, accel0: DVec3) -> Self {
        let hold = if control.tracking { Some(attitude::resume_hold(&att)) } else { control.hold };
        let control = AttitudeControl { base: None, hold, ..control };
        Flight {
            craft,
            start: (att, control),
            att,
            control,
            accel0,
            ticks: 0,
            thrust: DVec3::ZERO,
            attitudes: vec![(0.0, att)],
        }
    }

    /// Local end time of the next tick of a burn lasting `horizon` seconds.
    pub fn next_tick_end(&self, horizon: f64) -> f64 {
        ((self.ticks + 1) as f64 * super::TICK).min(horizon)
    }

    /// Flies one tick of `dt` ending at local time `t_end`: the maneuver
    /// hold aims at `dir` (the law's direction at the tick's start), the
    /// mass falls from `m_a` to `m_b`. Returns the tick's thrust
    /// acceleration (inertial, m/s²).
    pub fn tick(&mut self, law: &BurnLaw, dir: DVec3, (m_a, m_b): (f64, f64), dt: f64, t_end: f64) -> DVec3 {
        let thrust_dir = match self.craft {
            Some(c) => {
                let props = c.mass.at(m_a - c.mass.dry_mass);
                let gimbal = Gimbal {
                    lever: c.engine.mount_pos - props.com,
                    dir: c.engine.mount_dir,
                    thrust: law.thrust,
                    max_angle: c.engine.gimbal,
                };
                let act = Actuators { torque: c.torque, gimbal: Some(gimbal) };
                let aim = Aim::Direction { dir, axis: c.engine.mount_dir, key: AimKey::BURN };
                let controls = Controls { sas: true, ..Controls::default() };
                let (next, body_dir) =
                    attitude::control_tick(&self.att, &props.inertia, &mut self.control, &controls, &act, &aim, dt);
                self.att = next;
                next.q * body_dir
            }
            None => dir,
        };
        self.ticks += 1;
        self.attitudes.push((t_end, self.att));
        let accel = if law.mass_flow > 0.0 && m_b < m_a {
            law.thrust / law.mass_flow * math::ln(m_a / m_b) / dt
        } else {
            law.thrust / m_a
        };
        self.thrust = thrust_dir * accel;
        self.thrust
    }

    /// The attitude at local time `t`: the last tick's, turned at its rate
    /// (display between ticks). `None` before ignition.
    pub fn attitude_at(&self, t: f64) -> Option<Attitude> {
        let i = self.attitudes.partition_point(|(ts, _)| *ts <= t).checked_sub(1)?;
        let (ts, att) = self.attitudes[i];
        if t == ts {
            return Some(att);
        }
        let q = (quat_from_rotvec(att.omega * (t - ts)) * att.q).normalize();
        Some(Attitude { q, omega: att.omega })
    }

    /// Drops attitudes before the one at or before `t`.
    pub fn prune_before(&mut self, t: f64) {
        let first_after = self.attitudes.partition_point(|(ts, _)| *ts <= t);
        if first_after > 1 {
            self.attitudes.drain(..first_after - 1);
        }
    }
}
