//! The attitude inside a coast segment as a pure function of time and
//! state (`docs/features/limbo.md` §1), so forces that depend on the
//! attitude (drag, radiation pressure, low thrust along the engine) can be
//! integrated into a stored segment and any warp only samples it (rule 4).
//!
//! A segment's [`Pointing`] comes from the attitude regime when the segment
//! starts ([`lawful`]): SAS off is torque-free rotation (exact,
//! [`crate::rigid::FreeRotation`]); a stability hold at rest is a fixed
//! attitude; a settled direction hold is the held axis on the rule's
//! direction, inertial or fixed in the orbital frame (prograde, normal,
//! radial) of the motion relative to a body, with the roll fixed in that
//! frame (Principia's Frenet-fixed burns). Anything else (a turn under way,
//! rotation input, a target hold, which follows a target outside the
//! trajectory) is [`Pointing::Mean`]: forces then use orientation-averaged
//! areas. The vessel restarts its coast when the regime changes
//! (`super::coast`), so a segment's pointing holds for its whole life.
//!
//! The reference body of a frame rule is a control choice (the navball's,
//! D075), never physics (rule 1): it only says where the craft points.

use super::attitude::{needs_ticks, resume_hold, Attitude, AttitudeControl, RotationBase};
use super::burn::{prograde_normal_radial, DirectionLaw};
use super::hold::{quat_arc, Aim, AimKey, HoldMode, SpeedReference};
use super::Controls;
use crate::ephem::Snapshot;
use crate::frame::NodeId;
use crate::rigid::FreeRotation;
use crate::time::Epoch;
use crate::world::World;
use glam::{DMat3, DQuat, DVec3};
use std::sync::OnceLock;

/// What the attitude control holds, as far as a segment's forces care:
/// the regime the controls ask for. Two segments with the same rule and the
/// same controls continue each other; a different rule restarts the coast.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Rule {
    /// Under active control, or following something outside the trajectory.
    Mean,
    /// SAS off: torque-free.
    Free,
    /// SAS stability: held where the rotation stopped.
    Stability,
    /// A direction hold on a fixed inertial direction: body axis `axis` on `dir`.
    Inertial { dir: DVec3, axis: DVec3 },
    /// A direction hold in a body's orbital frame.
    Track(TrackRule),
}

/// A direction fixed in the orbital frame of the motion relative to
/// `reference`: columns prograde, normal (r × v), radial-out; `surface`
/// takes the velocity relative to the body's rotating surface.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrackRule {
    pub reference: NodeId,
    pub surface: bool,
    /// The held direction in the frame (unit): (1, 0, 0) prograde,
    /// (−1, 0, 0) retrograde, a burn law's axes.
    pub dir: DVec3,
    /// The body axis held on it (+Z the nose, or the engine's axis).
    pub axis: DVec3,
    /// `reference` is the automatic one ([`super::hold::hold_reference`],
    /// the player named none): the segment ends where that body changes
    /// (`super::segment`), at an integrator step, never at a frame.
    #[serde(default)]
    pub auto: bool,
}

/// The attitude law of a segment.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Pointing {
    /// Unknown: forces use orientation averages.
    Mean,
    /// A fixed attitude (body → inertial).
    Fixed { q: DQuat, rule: Rule },
    /// Torque-free rotation from `att` at `epoch` with `inertia` (body axes).
    Free {
        epoch: Epoch,
        att: Attitude,
        inertia: DMat3,
        #[serde(skip)]
        cache: FreeCache,
    },
    /// The attitude `P(t)·rel`, P the rule's orbital frame.
    Track { rule: TrackRule, rel: DQuat },
}

/// The torque-free solution of a [`Pointing::Free`], built on first use
/// (a pure function of the pointing; never saved, compares equal).
#[derive(Clone, Default)]
pub struct FreeCache(OnceLock<FreeRotation>);

impl PartialEq for FreeCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for FreeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeCache")
    }
}

/// The orbital frame of `(r, v)` as a rotation (columns prograde, normal,
/// radial-out), or `None` where it is undefined (no motion, or radial).
fn frame(r: DVec3, v: DVec3) -> Option<DQuat> {
    let h = r.cross(v);
    if !(v.length_squared() > 0.0 && h.length_squared() > 1e-18 * r.length_squared() * v.length_squared()) {
        return None;
    }
    let (p, n, rad) = prograde_normal_radial(r, v);
    Some(DQuat::from_mat3(&DMat3::from_cols(p, n, rad)).normalize())
}

impl TrackRule {
    /// The vessel's motion relative to the rule's body, the velocity taken
    /// relative to its surface if asked, at the snapshot's time; and the
    /// frame's rotation rate (the orbit's, `r × v / r²`).
    fn motion(&self, world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> (DVec3, DVec3) {
        let (rel_r, rel_v) = if self.reference == anchor {
            (r, v)
        } else {
            let k = snap.relative(self.reference, anchor);
            (r - k.r, v - k.v)
        };
        if !self.surface {
            return (rel_r, rel_v);
        }
        let spin = (world.source(self.reference).and_then(|s| s.physical.as_ref()))
            .map_or(DVec3::ZERO, |p| p.rotation.omega(snap.t).raw());
        (rel_r, rel_v - spin.cross(rel_r))
    }
}

impl Pointing {
    /// The rule this pointing holds.
    pub fn rule(&self) -> Rule {
        match self {
            Pointing::Mean => Rule::Mean,
            Pointing::Fixed { rule, .. } => *rule,
            Pointing::Free { .. } => Rule::Free,
            Pointing::Track { rule, .. } => Rule::Track(*rule),
        }
    }

    /// Whether the attitude is known (not [`Pointing::Mean`]).
    pub fn is_known(&self) -> bool {
        !matches!(self, Pointing::Mean)
    }

    /// The attitude at the snapshot's time for a vessel at `(r, v)`
    /// relative to `anchor`; `None` for [`Pointing::Mean`] (and for a frame
    /// rule whose frame is undefined there).
    pub fn attitude(&self, world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> Option<Attitude> {
        match self {
            Pointing::Mean => None,
            Pointing::Fixed { q, .. } => Some(Attitude { q: *q, omega: DVec3::ZERO }),
            Pointing::Free { epoch, att, inertia, cache } => {
                let free = cache.0.get_or_init(|| FreeRotation::new(att, inertia));
                Some(free.at(snap.t.seconds_since(*epoch)))
            }
            Pointing::Track { rule, rel } => {
                let (rr, rv) = rule.motion(world, snap, anchor, r, v);
                let p = frame(rr, rv)?;
                let omega = rr.cross(rv) / rr.length_squared();
                Some(Attitude { q: (p * *rel).normalize(), omega })
            }
        }
    }

    /// A torque-free pointing's attitude at `t` (`None` for the others):
    /// [`Self::attitude`] without a snapshot.
    pub fn free_at(&self, t: Epoch) -> Option<Attitude> {
        match self {
            Pointing::Free { epoch, att, inertia, cache } => {
                Some(cache.0.get_or_init(|| FreeRotation::new(att, inertia)).at(t.seconds_since(*epoch)))
            }
            _ => None,
        }
    }

    /// The attitude-control state that continues this pointing at `epoch`
    /// with attitude `att` (when the segment ends and the controller takes
    /// over again): a held attitude, a settled direction hold with its
    /// target a tick behind, or torque-free from the law's own base.
    pub fn control(&self, key: Option<AimKey>, epoch: Epoch, att: Attitude, inertia: DMat3) -> AttitudeControl {
        match self {
            Pointing::Mean => AttitudeControl::default(),
            Pointing::Fixed { q, rule: Rule::Stability } => AttitudeControl {
                base: Some(RotationBase { epoch, att: Attitude { q: *q, omega: DVec3::ZERO }, inertia }),
                hold: Some(*q),
                ..AttitudeControl::default()
            },
            Pointing::Free { epoch: e0, att: a0, inertia: i0, .. } => AttitudeControl {
                base: Some(RotationBase { epoch: *e0, att: *a0, inertia: *i0 }),
                ..AttitudeControl::default()
            },
            Pointing::Fixed { .. } | Pointing::Track { .. } => AttitudeControl {
                base: Some(RotationBase { epoch, att, inertia }),
                hold: Some(resume_hold(&att)),
                tracking: true,
                settled: key,
                waiting: false,
            },
        }
    }
}

impl Rule {
    /// Whether two rules are the same regime: equal, except that two
    /// automatic frame rules hold whatever the automatic reference is (the
    /// segment's own event marks where it changes, not a frame).
    pub fn same_regime(&self, other: &Rule) -> bool {
        match (self, other) {
            (Rule::Track(a), Rule::Track(b)) if a.auto && b.auto => {
                (a.surface, a.dir, a.axis) == (b.surface, b.dir, b.axis)
            }
            _ => self == other,
        }
    }
}

/// The rule the controls ask for: `controls` (with the maneuver hold
/// forced during a planned burn's turn), `burn` the law of the next
/// planned burn (the maneuver hold's direction), `auto` the automatic
/// reference body (asked only if the controls name none,
/// [`super::hold::hold_reference`]), `engine_axis` the thrust axis in body
/// axes.
pub fn wanted(
    controls: &Controls,
    burn: Option<&DirectionLaw>,
    auto: impl FnOnce() -> NodeId,
    engine_axis: DVec3,
) -> Rule {
    if controls.rotate != DVec3::ZERO {
        return Rule::Mean;
    }
    if !controls.sas {
        return Rule::Free;
    }
    match controls.hold {
        HoldMode::Prograde | HoldMode::Retrograde => {
            let surface = match controls.speed {
                SpeedReference::Orbit => false,
                SpeedReference::Surface => true,
                // Relative to the target: it moves outside the trajectory.
                SpeedReference::Target => return Rule::Mean,
            };
            let dir = if controls.hold == HoldMode::Prograde { DVec3::X } else { DVec3::NEG_X };
            let (reference, auto) = match controls.reference {
                Some(body) => (body, false),
                None => (auto(), true),
            };
            Rule::Track(TrackRule { reference, surface, dir, axis: DVec3::Z, auto })
        }
        _ => held(controls, burn).with_axis(engine_axis),
    }
}

/// The rules of the holds that do not use the speed reference.
fn held(controls: &Controls, burn: Option<&DirectionLaw>) -> Rule {
    match controls.hold {
        HoldMode::Stability => Rule::Stability,
        HoldMode::Target | HoldMode::AntiTarget => Rule::Mean,
        HoldMode::Maneuver => match burn {
            // No burn: the maneuver hold has no direction and holds still.
            None => Rule::Stability,
            Some(DirectionLaw::Inertial(d)) => Rule::Inertial { dir: d.normalize(), axis: DVec3::Z },
            Some(&DirectionLaw::Tracking { reference, axes }) => {
                Rule::Track(TrackRule { reference, surface: false, dir: axes.normalize(), axis: DVec3::Z, auto: false })
            }
        },
        // Handled by the caller.
        HoldMode::Prograde | HoldMode::Retrograde => Rule::Mean,
    }
}

impl Rule {
    /// The maneuver hold points the engine's axis.
    fn with_axis(self, axis: DVec3) -> Rule {
        match self {
            Rule::Inertial { dir, .. } => Rule::Inertial { dir, axis },
            Rule::Track(t) => Rule::Track(TrackRule { axis, ..t }),
            r => r,
        }
    }
}

/// The pointing that holds `rule` from now, if the attitude is already
/// there: torque-free always; a hold once the controller needs no more
/// ticks for `aim` (SAS at rest on stability, or settled on its direction).
/// `epoch` and `(r, v)` relative to `anchor` locate the vessel; `inertia`
/// is its inertia now.
#[allow(clippy::too_many_arguments)]
pub fn lawful(
    rule: &Rule,
    (att, control): (&Attitude, &AttitudeControl),
    controls: &Controls,
    aim: &Aim,
    world: &World,
    snap: &Snapshot,
    (anchor, r, v): (NodeId, DVec3, DVec3),
    inertia: DMat3,
) -> Option<Pointing> {
    match rule {
        Rule::Mean => None,
        Rule::Free => Some(Pointing::Free { epoch: snap.t, att: *att, inertia, cache: FreeCache::default() }),
        _ if needs_ticks(att, control, controls, aim) => None,
        Rule::Stability => matches!(aim, Aim::Stability).then_some(Pointing::Fixed { q: att.q, rule: *rule }),
        Rule::Inertial { dir, axis } => matches!(aim, Aim::Direction { .. })
            .then(|| Pointing::Fixed { q: snap_axis(att.q, *axis, *dir), rule: *rule }),
        Rule::Track(t) => {
            if !matches!(aim, Aim::Direction { .. }) {
                return None;
            }
            let (rr, rv) = t.motion(world, snap, anchor, r, v);
            let p = frame(rr, rv)?;
            let q = snap_axis(att.q, t.axis, p * t.dir);
            Some(Pointing::Track { rule: *t, rel: (p.inverse() * q).normalize() })
        }
    }
}

/// The pointing of a low-thrust burn on direction law `law` flown from
/// attitude `att` (the maneuver hold settled on it, D075): body axis
/// `axis` (the engine's) on the law's direction, inertially fixed or fixed
/// in the law's orbital frame, the roll taken from `att`. The vessel is at
/// `(r, v)` relative to `anchor` at the snapshot's time.
pub fn on_law(
    law: &DirectionLaw,
    att: &Attitude,
    axis: DVec3,
    world: &World,
    snap: &Snapshot,
    (anchor, r, v): (NodeId, DVec3, DVec3),
) -> Pointing {
    match *law {
        DirectionLaw::Inertial(d) => {
            let dir = d.normalize();
            Pointing::Fixed { q: snap_axis(att.q, axis, dir), rule: Rule::Inertial { dir, axis } }
        }
        DirectionLaw::Tracking { reference, axes } => {
            let rule = TrackRule { reference, surface: false, dir: axes.normalize(), axis, auto: false };
            let (rr, rv) = rule.motion(world, snap, anchor, r, v);
            match frame(rr, rv) {
                Some(p) => {
                    let q = snap_axis(att.q, axis, p * rule.dir);
                    Pointing::Track { rule, rel: (p.inverse() * q).normalize() }
                }
                // No orbital frame (radial motion): along the law now.
                None => {
                    let dir = law.direction(snap, anchor, r, v);
                    Pointing::Fixed { q: snap_axis(att.q, axis, dir), rule: Rule::Inertial { dir, axis } }
                }
            }
        }
    }
}

/// `q` turned by the smallest rotation that puts body axis `axis` exactly
/// on `dir` (a settled hold is within 10 µrad of it).
fn snap_axis(q: DQuat, axis: DVec3, dir: DVec3) -> DQuat {
    (quat_arc(q * axis, dir.normalize()) * q).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ephem::Ephemeris;
    use crate::kepler::Elements;
    use crate::sol;
    use std::sync::Arc;

    fn world() -> World {
        let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
        World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
    }

    #[test]
    fn wanted_rules_table() {
        let earth = NodeId(3);
        let moon = NodeId(5);
        let z = DVec3::Z;
        let sas = Controls { sas: true, ..Controls::default() };
        let law = DirectionLaw::Tracking { reference: moon, axes: DVec3::new(0.0, 0.0, 2.0) };
        let inertial = DirectionLaw::Inertial(DVec3::new(0.0, 3.0, 4.0));
        let track = |reference, surface, dir| {
            Rule::Track(TrackRule { reference, surface, dir, axis: z, auto: reference == earth })
        };
        let cases = [
            (Controls { rotate: DVec3::X, ..sas }, None, Rule::Mean),
            (Controls { sas: false, ..sas }, None, Rule::Free),
            (sas, None, Rule::Stability),
            (Controls { hold: HoldMode::Prograde, ..sas }, None, track(earth, false, DVec3::X)),
            (
                Controls { hold: HoldMode::Retrograde, reference: Some(moon), ..sas },
                None,
                track(moon, false, -DVec3::X),
            ),
            (
                Controls { hold: HoldMode::Prograde, speed: SpeedReference::Surface, ..sas },
                None,
                track(earth, true, DVec3::X),
            ),
            (Controls { hold: HoldMode::Prograde, speed: SpeedReference::Target, ..sas }, None, Rule::Mean),
            (Controls { hold: HoldMode::Target, ..sas }, None, Rule::Mean),
            (Controls { hold: HoldMode::Maneuver, ..sas }, None, Rule::Stability),
            (Controls { hold: HoldMode::Maneuver, ..sas }, Some(&law), track(moon, false, DVec3::Z)),
            (
                Controls { hold: HoldMode::Maneuver, ..sas },
                Some(&inertial),
                Rule::Inertial { dir: DVec3::new(0.0, 3.0, 4.0).normalize(), axis: z },
            ),
        ];
        for (controls, burn, expected) in cases {
            assert_eq!(wanted(&controls, burn, || earth, z), expected, "{controls:?} {burn:?}");
        }
    }

    #[test]
    fn a_track_pointing_holds_its_axis_on_the_rule_with_the_roll_fixed_in_the_frame() {
        let w = world();
        let earth = w.find("Earth").unwrap();
        let t0 = sol::sol_epoch();
        let el = Elements { a: 6.9e6, e: 0.01, i: 0.7, raan: 0.3, argp: 0.2, mean_anomaly: 0.0 };
        let (r, v) = el.to_state(earth.gm);
        let rule = TrackRule { reference: earth.node, surface: false, dir: DVec3::X, axis: DVec3::Z, auto: false };
        // A settled prograde hold, nose 3 µrad off the velocity, some roll.
        let q0 = (quat_arc(DVec3::Z, (v.normalize() + r.normalize() * 3e-6).normalize())
            * DQuat::from_xyzw(0.0, 0.0, 0.2, 1.0).normalize())
        .normalize();
        let snap = w.snapshot(t0);
        let p = lawful(
            &Rule::Track(rule),
            (&Attitude { q: q0, omega: DVec3::ZERO }, &AttitudeControl { settled: Some(key()), ..Default::default() }),
            &Controls { sas: true, hold: HoldMode::Prograde, ..Controls::default() },
            &Aim::Direction { dir: v.normalize(), axis: DVec3::Z, key: key() },
            &w,
            &snap,
            (earth.node, r, v),
            DMat3::IDENTITY,
        )
        .unwrap();
        let roll_axis = |q: DQuat, r: DVec3, v: DVec3| (q * DVec3::X).dot(r.cross(v).normalize());
        let roll0 = roll_axis(q0, r, v);
        // Half an orbit later the nose is on the new velocity exactly and
        // the roll relative to the orbit normal is unchanged.
        let (r1, v1) = Elements { mean_anomaly: 3.0, ..el }.to_state(earth.gm);
        let a = p.attitude(&w, &w.snapshot(t0.add_seconds(2_800.0)), earth.node, r1, v1).unwrap();
        assert!((a.nose() - v1.normalize()).length() < 1e-12, "{}", a.nose());
        assert!((roll_axis(a.q, r1, v1) - roll0).abs() < 1e-5, "{} vs {roll0}", roll_axis(a.q, r1, v1));
        assert!(a.omega.dot(r1.cross(v1).normalize()) > 0.0);
    }

    fn key() -> AimKey {
        AimKey { mode: HoldMode::Prograde, speed: SpeedReference::Orbit, reference: None }
    }

    #[test]
    fn free_and_fixed_pointings_are_pure_functions_of_time() {
        let w = world();
        let t0 = sol::sol_epoch();
        let att =
            Attitude { q: DQuat::from_xyzw(0.1, 0.2, 0.3, 0.9).normalize(), omega: DVec3::new(0.01, -0.02, 0.05) };
        let inertia = DMat3::from_diagonal(DVec3::new(1.3e5, 1.1e5, 4.0e4));
        let free = Pointing::Free { epoch: t0, att, inertia, cache: FreeCache::default() };
        let at = |p: &Pointing, dt: f64| p.attitude(&w, &w.snapshot(t0.add_seconds(dt)), NodeId(3), DVec3::X, DVec3::Y);
        let direct = FreeRotation::new(&att, &inertia).at(1234.5);
        assert_eq!(at(&free, 1234.5), Some(direct));
        // Evaluated out of order, or by a clone, the same bits.
        let other = free.clone();
        assert_eq!(at(&other, 77.0), at(&free, 77.0));
        let fixed = Pointing::Fixed { q: att.q, rule: Rule::Stability };
        assert_eq!(at(&fixed, 1e7), Some(Attitude { q: att.q, omega: DVec3::ZERO }));
        assert_eq!(at(&Pointing::Mean, 0.0), None);
    }
}
