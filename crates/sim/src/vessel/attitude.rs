//! Vessel attitude: orientation (body → inertial), angular velocity, and
//! attitude control (realism-1 §3d).
//!
//! The rotation is a rigid body ([`crate::rigid`]): control ticks integrate
//! Euler's equations with the commanded torque; between ticks the vessel
//! rotates torque-free (the exact motion, so an asymmetric body tumbles).
//! Torques come from the attitude-control authority (abstract RCS/wheels,
//! per body axis) and, while thrusting, the engine gimbal. SAS is a
//! controller: rate damping until the rotation stops, then attitude hold on
//! the attitude where it stopped; it outputs a torque command within the
//! authority.
//!
//! Quaternions are built with `sim::math` trig, never glam's
//! `from_axis_angle` (platform trig; see `math`).

use super::{Controls, TICK};
use crate::math;
use crate::rigid::{self, FreeRotation};
use crate::time::Epoch;
use glam::{DMat3, DQuat, DVec3};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Attitude {
    /// Rotation from vessel body axes to inertial axes. Body +Z is the nose.
    pub q: DQuat,
    /// Angular velocity (inertial axes, rad/s).
    pub omega: DVec3,
}

/// Quaternion for a rotation vector (axis × angle).
pub fn quat_from_rotvec(v: DVec3) -> DQuat {
    let angle = v.length();
    if angle < 1e-300 {
        return DQuat::IDENTITY;
    }
    let axis = v / angle;
    let (s, c) = (math::sin(0.5 * angle), math::cos(0.5 * angle));
    DQuat::from_xyzw(axis.x * s, axis.y * s, axis.z * s, c)
}

/// Rotation vector (axis × angle, angle in [0, π]) of a quaternion.
pub fn rotvec_of(q: DQuat) -> DVec3 {
    let q = if q.w < 0.0 { -q } else { q };
    let s = q.xyz().length();
    if s < 1e-300 {
        return DVec3::ZERO;
    }
    q.xyz() / s * (2.0 * math::atan2(s, q.w))
}

/// Quaternion rotating +Z onto `dir` (unit), with a deterministic roll.
pub fn quat_z_to(dir: DVec3) -> DQuat {
    let axis = DVec3::Z.cross(dir);
    let s = axis.length();
    let c = DVec3::Z.dot(dir);
    if s < 1e-12 {
        return if c > 0.0 { DQuat::IDENTITY } else { DQuat::from_xyzw(1.0, 0.0, 0.0, 0.0) };
    }
    quat_from_rotvec(axis / s * math::atan2(s, c))
}

impl Attitude {
    pub fn nose(&self) -> DVec3 {
        self.q * DVec3::Z
    }

    /// Angular velocity in body axes.
    pub fn body_rate(&self) -> DVec3 {
        self.q.inverse() * self.omega
    }
}

/// The last control tick of a coast: the attitude there, its epoch and the
/// inertia the vessel rotates with from it (torque-free).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RotationBase {
    pub epoch: Epoch,
    pub att: Attitude,
    pub inertia: DMat3,
}

impl RotationBase {
    /// The torque-free attitude `dt` seconds after the base.
    pub fn free(&self, dt: f64) -> Attitude {
        FreeRotation::new(&self.att, &self.inertia).at(dt)
    }
}

/// Attitude-control state saved with the vessel.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AttitudeControl {
    /// While coasting: the last control tick. Ticks stay on its lattice
    /// (the remainder carries over to the next frame) and the attitude is
    /// this propagated torque-free, so the result does not depend on frame
    /// rate or warp. `None`: the vessel's attitude is itself the tick state.
    pub base: Option<RotationBase>,
    /// SAS attitude hold: the attitude (body → inertial) to keep.
    pub hold: Option<DQuat>,
}

/// The engine as a torque source during a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gimbal {
    /// From the centre of mass to the engine mount (body axes, m).
    pub lever: DVec3,
    /// Undeflected thrust direction (body axes, unit).
    pub dir: DVec3,
    /// Thrust this tick (N).
    pub thrust: f64,
    /// Gimbal range (rad).
    pub max_angle: f64,
}

/// What can turn the vessel during a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Actuators {
    /// Attitude-control authority per body axis (N·m).
    pub torque: DVec3,
    pub gimbal: Option<Gimbal>,
}

/// SAS natural frequency for attitude hold (rad/s), critically damped.
const HOLD_OMEGA: f64 = 1.0;
/// Hold snaps to its target below these errors (rad, rad/s).
const HOLD_SNAP: f64 = 1e-6;

impl Gimbal {
    /// Largest gimbal torque (N·m), and the part of it about each body axis.
    fn authority(&self) -> (f64, DVec3) {
        let g = self.thrust * math::sin(self.max_angle) * self.lever.length();
        let d = self.dir;
        let per_axis = DVec3::new(1.0 - d.x * d.x, 1.0 - d.y * d.y, 1.0 - d.z * d.z).max(DVec3::ZERO);
        (g, DVec3::new(per_axis.x.sqrt(), per_axis.y.sqrt(), per_axis.z.sqrt()) * g)
    }

    /// Thrust direction (body axes) deflected to give torque `tau` (within
    /// range), and the torque the thrust then exerts about the centre of mass.
    fn deflect(&self, tau: DVec3) -> (DVec3, DVec3) {
        let (g, _) = self.authority();
        let mut dir = self.dir;
        let t = tau.length();
        let lat = tau.cross(self.lever);
        let lat = lat - self.dir * lat.dot(self.dir);
        if t > 0.0 && g > 0.0 && lat.length() > 0.0 {
            let s = (t / (self.thrust * self.lever.length())).min(math::sin(self.max_angle));
            dir = self.dir * (1.0 - s * s).sqrt() + lat.normalize() * s;
        }
        (dir, self.lever.cross(dir * self.thrust))
    }
}

/// Whether a control tick would change anything: rotation input, or SAS
/// still damping or holding.
pub fn needs_ticks(att: &Attitude, control: &AttitudeControl, controls: &Controls) -> bool {
    controls.rotate != DVec3::ZERO
        || (controls.sas && (att.omega != DVec3::ZERO || control.hold.is_some_and(|h| h != att.q)))
}

/// One control tick of `dt`: the pilot's rotation input (a fraction of the
/// authority per body axis) or SAS, then Euler's equations. Returns the new
/// attitude and the thrust direction (body axes, after gimbal).
pub fn control_tick(
    att: &Attitude,
    inertia: &DMat3,
    control: &mut AttitudeControl,
    controls: &Controls,
    act: &Actuators,
    dt: f64,
) -> (Attitude, DVec3) {
    let w = att.body_rate();
    let gimbal_axes = act.gimbal.map_or(DVec3::ZERO, |g| g.authority().1);
    let auth = act.torque + gimbal_axes;
    let sas = controls.sas && controls.rotate == DVec3::ZERO;
    let (u, saturated) = if controls.rotate != DVec3::ZERO {
        control.hold = None;
        (controls.rotate.clamp(DVec3::splat(-1.0), DVec3::splat(1.0)), true)
    } else if sas {
        let gyro = w.cross(*inertia * w);
        let wanted = match control.hold {
            Some(target) => {
                let e = rotvec_of(att.q.inverse() * target);
                *inertia * (e * (HOLD_OMEGA * HOLD_OMEGA) - w * (2.0 * HOLD_OMEGA)) + gyro
            }
            None => *inertia * (-w / dt) + gyro,
        };
        let u = DVec3::select(auth.cmpgt(DVec3::ZERO), wanted / auth, DVec3::ZERO);
        let worst = u.abs().max_element();
        if worst > 1.0 {
            (u / worst, true)
        } else {
            (u, false)
        }
    } else {
        control.hold = None;
        (DVec3::ZERO, true)
    };
    let (thrust_dir, thrust_torque) = match act.gimbal {
        Some(g) => g.deflect(u * gimbal_axes),
        None => (DVec3::Z, DVec3::ZERO),
    };
    let torque = u * act.torque + thrust_torque;
    let mut next = rigid::tick(att, inertia, torque, dt);
    if sas {
        match control.hold {
            None if !saturated => {
                // The damping command was sized to stop the rotation this tick.
                next.omega = DVec3::ZERO;
                control.hold = Some(next.q);
            }
            Some(target)
                if rotvec_of(next.q.inverse() * target).length() < HOLD_SNAP && next.omega.length() < HOLD_SNAP =>
            {
                next = Attitude { q: target, omega: DVec3::ZERO };
            }
            _ => {}
        }
    }
    (next, thrust_dir)
}

/// Attitude over a coast: control ticks on the tick lattice while input or
/// SAS need them, then torque-free rotation from the last tick. The partial
/// tick at the end of a frame is carried to the next one, so 60 fps and one
/// long jump give the same attitude bit for bit. `inertia_at(epoch)` is the
/// inertia at an epoch after the vessel's time (it changes in planned burns).
pub fn advance_coast(
    att: &mut Attitude,
    control: &mut AttitudeControl,
    (time, until): (Epoch, Epoch),
    controls: &Controls,
    torque: DVec3,
    inertia_now: DMat3,
    inertia_at: impl Fn(Epoch) -> DMat3,
) {
    let tick = TICK;
    let mut base = control.base.unwrap_or(RotationBase { epoch: time, att: *att, inertia: inertia_now });
    if needs_ticks(&base.att, control, controls) {
        // After free rotation, ticks resume at the last lattice point before
        // the vessel's time (only when the input changes).
        let behind = time.seconds_since(base.epoch);
        if behind >= tick {
            let whole = libm::floor(behind / tick) * tick;
            base.att = base.free(whole);
            base.epoch = base.epoch.add_seconds(whole);
        }
        let act = Actuators { torque, gimbal: None };
        while needs_ticks(&base.att, control, controls) && base.epoch.add_seconds(tick) <= until {
            let end = base.epoch.add_seconds(tick);
            base.inertia = inertia_at(end);
            base.att = control_tick(&base.att, &base.inertia, control, controls, &act, tick).0;
            base.epoch = end;
        }
    }
    *att = base.free(until.seconds_since(base.epoch));
    control.base = Some(base);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inertia() -> DMat3 {
        DMat3::from_diagonal(DVec3::new(1.3e5, 1.3e5, 4.0e4))
    }

    fn rcs() -> Actuators {
        Actuators { torque: DVec3::new(40e3, 40e3, 20e3), gimbal: None }
    }

    #[test]
    fn sas_stops_a_spin_in_the_time_authority_allows() {
        // Spin about body x at 0.3 rad/s: I·ω / τ = 1.3e5 · 0.3 / 4e4 = 0.975 s.
        for (w, axis_time) in [(DVec3::new(0.3, 0.0, 0.0), 0.975), (DVec3::new(0.0, 0.0, 0.5), 0.5 * 4e4 / 20e3)] {
            let mut a = Attitude { q: DQuat::IDENTITY, omega: w };
            let mut control = AttitudeControl::default();
            let controls = Controls { sas: true, ..Controls::default() };
            let mut ticks = 0;
            while needs_ticks(&a, &control, &controls) {
                a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), 0.02).0;
                ticks += 1;
                assert!(ticks < 1000);
            }
            assert_eq!(a.omega, DVec3::ZERO);
            assert_eq!(control.hold, Some(a.q), "holds where it stopped");
            let expected = libm::ceil(axis_time / 0.02 - 1e-9) as i32;
            assert!((ticks - expected).abs() <= 1, "{ticks} ticks, expected {expected}");
        }
    }

    #[test]
    fn hold_returns_to_its_attitude_and_snaps() {
        let target = DQuat::IDENTITY;
        let mut a = Attitude { q: quat_from_rotvec(DVec3::new(0.02, -0.01, 0.005)), omega: DVec3::ZERO };
        let mut control = AttitudeControl { base: None, hold: Some(target) };
        let controls = Controls { sas: true, ..Controls::default() };
        let mut ticks = 0;
        while needs_ticks(&a, &control, &controls) {
            a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), 0.02).0;
            ticks += 1;
            assert!(ticks < 5000, "hold never settled: {a:?}");
        }
        assert_eq!(a, Attitude { q: target, omega: DVec3::ZERO });
        // Critically damped at 1 rad/s: settles to 1e-6 in about 20 s.
        assert!(ticks > 300 && ticks < 2000, "{ticks} ticks");
    }

    #[test]
    fn input_spins_up_at_authority_over_inertia() {
        let mut a = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
        let mut control = AttitudeControl { base: None, hold: Some(DQuat::IDENTITY) };
        let controls = Controls { rotate: DVec3::new(0.0, 0.0, 1.0), sas: true, ..Controls::default() };
        for _ in 0..50 {
            a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), 0.02).0;
        }
        assert!((a.omega.z - 20e3 / 4e4).abs() < 1e-12, "{}", a.omega);
        assert_eq!(control.hold, None, "input releases the hold");
    }

    #[test]
    fn gimbal_adds_pitch_authority_while_thrusting() {
        // 300 kN, 5°, 3 m below the centre of mass: 78 kN·m of pitch authority.
        let g =
            Gimbal { lever: DVec3::new(0.0, 0.0, -3.0), dir: DVec3::Z, thrust: 300e3, max_angle: 5f64.to_radians() };
        let act = Actuators { gimbal: Some(g), ..rcs() };
        let controls = Controls { rotate: DVec3::X, ..Controls::default() };
        let a = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
        let (next, dir) = control_tick(&a, &inertia(), &mut AttitudeControl::default(), &controls, &act, 0.02);
        let g_max = 300e3 * math::sin(5f64.to_radians()) * 3.0;
        let expected = (40e3 + g_max) / 1.3e5 * 0.02;
        assert!((next.omega.x - expected).abs() < 1e-9, "{} vs {expected}", next.omega.x);
        // Thrust tilted by the full gimbal angle, sideways to give +x torque.
        assert!((dir.z - math::cos(5f64.to_radians())).abs() < 1e-12 && dir.y > 0.0, "{dir}");
        // No roll authority from the gimbal.
        assert_eq!(g.authority().1.z, 0.0);
    }

    #[test]
    fn rotation_vectors_round_trip() {
        for v in [DVec3::new(0.3, -0.2, 0.1), DVec3::new(0.0, 3.0, 0.0), DVec3::ZERO, DVec3::new(-1e-9, 0.0, 2e-9)] {
            assert!((rotvec_of(quat_from_rotvec(v)) - v).length() < 1e-12, "{v}");
        }
    }

    #[test]
    fn z_to_points_the_nose() {
        for d in [DVec3::X, DVec3::new(0.3, -0.4, 0.866).normalize(), -DVec3::Z, DVec3::Z] {
            assert!((quat_z_to(d) * DVec3::Z - d).length() < 1e-12);
        }
    }
}
