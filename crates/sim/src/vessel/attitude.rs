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

use super::hold::{self, quat_arc, Aim, AimKey, HoldMode};
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
    /// Stability: the attitude (body → inertial) to keep. A direction hold:
    /// the target attitude of the last tick.
    pub hold: Option<DQuat>,
    /// `hold` is a direction hold's target ([`Aim::Direction`]).
    #[serde(default)]
    pub tracking: bool,
    /// The direction hold whose last tick snapped onto its target: on rails
    /// it is followed without control ticks while its rule stays the same.
    #[serde(default)]
    pub settled: Option<AimKey>,
    /// A direction hold asked for while its direction is undefined (too
    /// slow for prograde, no target, no burn): on rails its base steps on
    /// the follow lattice until the direction appears.
    #[serde(default)]
    pub waiting: bool,
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

/// Whether a control tick would change anything: rotation input, SAS
/// still damping or holding, or a direction hold not yet settled on `aim`.
pub fn needs_ticks(att: &Attitude, control: &AttitudeControl, controls: &Controls, aim: &Aim) -> bool {
    if controls.rotate != DVec3::ZERO {
        return true;
    }
    if !controls.sas {
        return false;
    }
    match aim {
        Aim::Stability => control.tracking || att.omega != DVec3::ZERO || control.hold.is_some_and(|h| h != att.q),
        Aim::Direction { key, .. } => control.settled != Some(*key),
    }
}

/// A control tick's command: what the pilot's input or SAS asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Command {
    /// Body-axes torque from attitude control and the gimbal (N·m).
    pub torque: DVec3,
    /// Thrust direction (body axes, after gimbal).
    pub thrust_dir: DVec3,
    /// Attitude-control use per body axis, a fraction of full command in
    /// [0, 1] (what the RCS fires).
    pub rcs: DVec3,
    sas: bool,
    saturated: bool,
    /// A direction hold: where the attitude is at the tick's end if it
    /// tracks its target exactly (the target carried at its rate).
    snap_to: Option<(Attitude, AimKey)>,
}

/// Direction holds snap onto their moving target below these errors (rad,
/// rad/s): pointing within 10 µrad is exact for every purpose.
const TRACK_SNAP: f64 = 1e-5;

/// The rate (body axes) at which a hold closes an attitude error `e`
/// (rotation vector, body axes): `ω/2·|e|` near the target (critically
/// damped with the gain `2ω`), at most `√(α·|e|)` far from it so the
/// rotation can stop in time braking at half the angular acceleration
/// `alpha`.
fn closing_rate(e: DVec3, alpha: f64) -> DVec3 {
    let m = e.length();
    if m == 0.0 {
        return DVec3::ZERO;
    }
    let linear = 0.5 * HOLD_OMEGA * m;
    let rate = if alpha > 0.0 { linear.min((alpha * m).sqrt()) } else { linear };
    e * (rate / m)
}

/// The command for a tick of `dt`: the pilot's rotation input (a fraction
/// of the authority per body axis) or SAS, aiming at `aim`.
pub fn command(
    att: &Attitude,
    inertia: &DMat3,
    control: &mut AttitudeControl,
    controls: &Controls,
    act: &Actuators,
    aim: &Aim,
    dt: f64,
) -> Command {
    let w = att.body_rate();
    let gimbal_axes = act.gimbal.map_or(DVec3::ZERO, |g| g.authority().1);
    let auth = act.torque + gimbal_axes;
    let sas = controls.sas && controls.rotate == DVec3::ZERO;
    let mut snap_to = None;
    let (u, saturated) = if sas {
        let gyro = w.cross(*inertia * w);
        let (target, w_target) = match *aim {
            Aim::Stability => {
                if control.tracking {
                    // From a direction hold: damp, then hold where it stops.
                    control.hold = None;
                    control.tracking = false;
                }
                control.settled = None;
                (control.hold, DVec3::ZERO)
            }
            Aim::Direction { dir, axis, key } => {
                let prev = control.hold.filter(|_| control.tracking);
                let from = prev.unwrap_or(att.q);
                let target = (quat_arc(from * axis, dir) * from).normalize();
                // The target's rate from its last tick (inertial), and the
                // attitude a perfect tracker has at the end of this tick.
                let (rate, end) = match prev {
                    Some(p) => {
                        let step = rotvec_of(p.inverse() * target);
                        (target * step / dt, (target * quat_from_rotvec(step)).normalize())
                    }
                    None => (DVec3::ZERO, target),
                };
                control.hold = Some(target);
                control.tracking = true;
                snap_to = Some((Attitude { q: end, omega: rate }, key));
                (Some(target), att.q.inverse() * rate)
            }
        };
        let wanted = match target {
            Some(target) => {
                let e = rotvec_of(att.q.inverse() * target);
                let diag = DVec3::new(inertia.x_axis.x, inertia.y_axis.y, inertia.z_axis.z);
                let alpha = DVec3::select(auth.cmpgt(DVec3::ZERO), auth / diag, DVec3::splat(f64::INFINITY));
                let alpha = alpha.min_element();
                let alpha = if alpha.is_finite() { alpha } else { 0.0 };
                *inertia * ((w_target + closing_rate(e, alpha) - w) * (2.0 * HOLD_OMEGA)) + gyro
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
        control.tracking = false;
        control.settled = None;
        (controls.rotate.clamp(DVec3::splat(-1.0), DVec3::splat(1.0)), true)
    };
    let (thrust_dir, thrust_torque) = match act.gimbal {
        Some(g) => g.deflect(u * gimbal_axes),
        None => (DVec3::Z, DVec3::ZERO),
    };
    let rcs = DVec3::select(act.torque.cmpgt(DVec3::ZERO), u.abs(), DVec3::ZERO);
    Command { torque: u * act.torque + thrust_torque, thrust_dir, rcs, sas, saturated, snap_to }
}

/// SAS bookkeeping after a tick flown with `cmd`. `alone`: the command was
/// the only torque, so a damping command that did not saturate stopped the
/// rotation exactly and the hold may snap to its target; with other torques
/// (ground contact, air) SAS only starts holding where it is, and a
/// direction hold never snaps.
pub fn finish(mut next: Attitude, control: &mut AttitudeControl, cmd: &Command, alone: bool) -> Attitude {
    if !cmd.sas {
        return next;
    }
    if let Some((end, key)) = cmd.snap_to {
        let close =
            rotvec_of(next.q.inverse() * end.q).length() < TRACK_SNAP && (next.omega - end.omega).length() < TRACK_SNAP;
        if alone && close {
            next = end;
            control.settled = Some(key);
        } else {
            control.settled = None;
        }
        return next;
    }
    match control.hold {
        None if !cmd.saturated => {
            if alone {
                // The damping command was sized to stop the rotation this tick.
                next.omega = DVec3::ZERO;
            }
            control.hold = Some(next.q);
        }
        Some(target)
            if alone
                && rotvec_of(next.q.inverse() * target).length() < HOLD_SNAP
                && next.omega.length() < HOLD_SNAP =>
        {
            next = Attitude { q: target, omega: DVec3::ZERO };
        }
        _ => {}
    }
    next
}

/// One control tick of `dt`: the command, then Euler's equations. Returns
/// the new attitude and the thrust direction (body axes, after gimbal).
pub fn control_tick(
    att: &Attitude,
    inertia: &DMat3,
    control: &mut AttitudeControl,
    controls: &Controls,
    act: &Actuators,
    aim: &Aim,
    dt: f64,
) -> (Attitude, DVec3) {
    let cmd = command(att, inertia, control, controls, act, aim, dt);
    let next = rigid::tick(att, inertia, cmd.torque, dt);
    (finish(next, control, &cmd, true), cmd.thrust_dir)
}

/// Step of the lattice on which a settled direction hold is followed on
/// rails (s): the attitude there is the last one turned by the smallest
/// rotation onto the direction ([`hold::follow`]).
pub const FOLLOW_STEP: f64 = 60.0;
/// A followed direction that moves more than this in one step (rad, about
/// 11°) is a jump (a new reference, say): the hold turns with ticks.
const FOLLOW_MAX_TURN: f64 = 0.2;

/// A direction hold's target one tick before `att` rotating at its rate:
/// ticks resuming from it (after following, or at a burn's ignition) start
/// with the target moving at the rate the craft turns.
pub fn resume_hold(att: &Attitude) -> DQuat {
    (quat_from_rotvec(-att.omega * TICK) * att.q).normalize()
}

/// Whether a settled direction hold is being followed.
fn following(control: &AttitudeControl, controls: &Controls, aim: &Aim) -> bool {
    controls.sas
        && controls.rotate == DVec3::ZERO
        && matches!(aim, Aim::Direction { key, .. } if control.settled == Some(*key))
}

/// The attitude `dt` after the base: followed if `aim` (at the end) keeps a
/// settled direction hold, else torque-free.
fn drift(base: &RotationBase, control: &AttitudeControl, controls: &Controls, aim: &Aim, dt: f64) -> Attitude {
    match aim {
        Aim::Direction { dir, axis, .. } if following(control, controls, aim) => hold::follow(base, *axis, *dir),
        _ => base.free(dt),
    }
}

/// Attitude over a coast: control ticks on the tick lattice while input or
/// SAS need them; a settled direction hold followed on the
/// [`FOLLOW_STEP`] lattice; otherwise torque-free rotation from the last
/// tick. The partial step at the end of a frame is carried to the next one,
/// so 60 fps and one long jump give the same attitude bit for bit.
/// `inertia_at(epoch)` is the inertia at an epoch after the vessel's time
/// (it changes in planned burns); `aim_at(epoch)` what SAS aims at then.
#[allow(clippy::too_many_arguments)]
pub fn advance_coast(
    att: &mut Attitude,
    control: &mut AttitudeControl,
    (time, until): (Epoch, Epoch),
    controls: &Controls,
    torque: DVec3,
    inertia_now: DMat3,
    inertia_at: impl Fn(Epoch) -> DMat3,
    aim_at: impl Fn(Epoch) -> Aim,
) {
    let tick = TICK;
    let mut base = control.base.unwrap_or(RotationBase { epoch: time, att: *att, inertia: inertia_now });
    if !controls.sas {
        control.hold = None;
        control.tracking = false;
        control.settled = None;
    }
    let act = Actuators { torque, gimbal: None };
    // After free rotation (or a stability hold at rest), ticks resume at
    // the last lattice point before the vessel's time: only the controls
    // change that, at a frame. A followed hold's base is at most a follow
    // step behind, and ticks resume from it (a time-dependent change, the
    // same at every frame rate).
    let mut may_skip = control.settled.is_none() && !control.waiting;
    let wants_direction = controls.sas && controls.rotate == DVec3::ZERO && controls.hold != HoldMode::Stability;
    loop {
        let aim = aim_at(base.epoch);
        if needs_ticks(&base.att, control, controls, &aim) {
            control.waiting = false;
            let behind = time.seconds_since(base.epoch);
            if may_skip && behind >= tick {
                may_skip = false;
                let whole = libm::floor(behind / tick) * tick;
                let end = base.epoch.add_seconds(whole);
                base.att = drift(&base, control, controls, &aim_at(end), whole);
                base.epoch = end;
                continue;
            }
            may_skip = false;
            let end = base.epoch.add_seconds(tick);
            if end > until {
                break;
            }
            base.inertia = inertia_at(end);
            base.att = control_tick(&base.att, &base.inertia, control, controls, &act, &aim, tick).0;
            base.epoch = end;
        } else if following(control, controls, &aim) {
            control.waiting = false;
            let end = base.epoch.add_seconds(FOLLOW_STEP);
            if end > until {
                break;
            }
            let next = aim_at(end);
            match next {
                Aim::Direction { dir, axis, .. }
                    if following(control, controls, &next)
                        && (base.att.q * axis).dot(dir) > math::cos(FOLLOW_MAX_TURN) =>
                {
                    base.att = hold::follow(&base, axis, dir);
                    base.inertia = inertia_at(end);
                    base.epoch = end;
                    control.hold = Some(resume_hold(&base.att));
                }
                // A jump or a new rule: turn with ticks from here.
                _ => control.settled = None,
            }
        } else if wants_direction {
            // Holding still until the direction is defined: look for it on
            // the follow lattice.
            control.waiting = true;
            let end = base.epoch.add_seconds(FOLLOW_STEP);
            if end > until {
                break;
            }
            base.att = base.free(FOLLOW_STEP);
            base.inertia = inertia_at(end);
            base.epoch = end;
            may_skip = false;
        } else {
            control.waiting = false;
            break;
        }
    }
    *att = drift(&base, control, controls, &aim_at(until), until.seconds_since(base.epoch));
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
            while needs_ticks(&a, &control, &controls, &Aim::Stability) {
                a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), &Aim::Stability, 0.02).0;
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
        let mut control = AttitudeControl { hold: Some(target), ..AttitudeControl::default() };
        let controls = Controls { sas: true, ..Controls::default() };
        let mut ticks = 0;
        while needs_ticks(&a, &control, &controls, &Aim::Stability) {
            a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), &Aim::Stability, 0.02).0;
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
        let mut control = AttitudeControl { hold: Some(DQuat::IDENTITY), ..AttitudeControl::default() };
        let controls = Controls { rotate: DVec3::new(0.0, 0.0, 1.0), sas: true, ..Controls::default() };
        for _ in 0..50 {
            a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), &Aim::Stability, 0.02).0;
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
        let (next, dir) =
            control_tick(&a, &inertia(), &mut AttitudeControl::default(), &controls, &act, &Aim::Stability, 0.02);
        let g_max = 300e3 * math::sin(5f64.to_radians()) * 3.0;
        let expected = (40e3 + g_max) / 1.3e5 * 0.02;
        assert!((next.omega.x - expected).abs() < 1e-9, "{} vs {expected}", next.omega.x);
        // Thrust tilted by the full gimbal angle, sideways to give +x torque.
        assert!((dir.z - math::cos(5f64.to_radians())).abs() < 1e-12 && dir.y > 0.0, "{dir}");
        // No roll authority from the gimbal.
        assert_eq!(g.authority().1.z, 0.0);
    }

    fn key() -> AimKey {
        AimKey::BURN
    }

    #[test]
    fn a_direction_hold_turns_and_settles_within_the_turn_lead() {
        // From rest, onto directions up to a half turn away (roll free).
        let lead = hold::turn_lead(&inertia(), rcs().torque);
        let controls = Controls { sas: true, ..Controls::default() };
        let dirs = [DVec3::new(1.0, 0.0, 1.0).normalize(), DVec3::X, DVec3::new(0.2, 0.1, -1.0).normalize(), -DVec3::Z];
        for dir in dirs {
            let aim = Aim::Direction { dir, axis: DVec3::Z, key: key() };
            let mut a = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
            let mut control = AttitudeControl::default();
            let mut ticks = 0;
            while needs_ticks(&a, &control, &controls, &aim) {
                a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), &aim, 0.02).0;
                ticks += 1;
                assert!(ticks < 100_000, "never settled onto {dir}: {a:?}");
            }
            let t = ticks as f64 * 0.02;
            assert!(t <= lead, "{dir}: settled after {t} s, lead {lead} s");
            assert!((a.nose() - dir).length() < 1e-12, "{dir}: nose {}", a.nose());
            assert_eq!(control.settled, Some(key()));
        }
    }

    #[test]
    fn a_direction_hold_tracks_a_turning_direction_exactly() {
        // A direction turning at an orbit's rate: after settling, every
        // tick snaps onto it (no lag).
        let rate = 1.1e-3;
        let dir_at = |k: usize| {
            let a = rate * 0.02 * k as f64;
            DVec3::new(math::cos(a), math::sin(a), 0.0)
        };
        let controls = Controls { sas: true, ..Controls::default() };
        let mut a = Attitude { q: quat_z_to(DVec3::new(1.0, -0.3, 0.2).normalize()), omega: DVec3::ZERO };
        let mut control = AttitudeControl::default();
        let mut settled_at = None;
        for k in 0..5_000 {
            let aim = Aim::Direction { dir: dir_at(k), axis: DVec3::Z, key: key() };
            a = control_tick(&a, &inertia(), &mut control, &controls, &rcs(), &aim, 0.02).0;
            match (settled_at, control.settled) {
                (None, Some(_)) => settled_at = Some(k),
                (Some(_), s) => assert_eq!(s, Some(key()), "lost the target at tick {k}"),
                _ => {}
            }
        }
        assert!(settled_at.is_some_and(|k| k < 2_000), "settled at {settled_at:?}");
        // The nose leads the last target by one tick of its motion.
        assert!((a.nose() - dir_at(5_000)).length() < 1e-9, "{}", a.nose());
        assert!((a.omega - DVec3::Z * rate).length() < 1e-9, "{}", a.omega);
    }

    #[test]
    fn input_releases_a_direction_hold_and_stability_holds_where_it_stops() {
        let aim = Aim::Direction { dir: DVec3::X, axis: DVec3::Z, key: key() };
        let sas = Controls { sas: true, ..Controls::default() };
        let mut a = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
        let mut control = AttitudeControl::default();
        for _ in 0..100 {
            a = control_tick(&a, &inertia(), &mut control, &sas, &rcs(), &aim, 0.02).0;
        }
        assert!(control.tracking && a.omega != DVec3::ZERO);
        let input = Controls { rotate: DVec3::Y, ..sas };
        a = control_tick(&a, &inertia(), &mut control, &input, &rcs(), &aim, 0.02).0;
        assert_eq!((control.hold, control.tracking, control.settled), (None, false, None));
        // Stability: damps the turn, then holds.
        let mut ticks = 0;
        while needs_ticks(&a, &control, &sas, &Aim::Stability) {
            a = control_tick(&a, &inertia(), &mut control, &sas, &rcs(), &Aim::Stability, 0.02).0;
            ticks += 1;
            assert!(ticks < 5_000);
        }
        assert_eq!(a.omega, DVec3::ZERO);
        assert_eq!(control.hold, Some(a.q));
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
