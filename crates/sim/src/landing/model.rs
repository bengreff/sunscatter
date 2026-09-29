//! The landing prediction's dynamics and its integration loop.

use super::{AssumedAttitude, Impact, LandingStart};
use crate::contact::Ground;
use crate::craft::{CraftDesign, CraftParams};
use crate::ephem::{Snapshot, SnapshotCache};
use crate::forces::{ambient_pressure, ActiveSources, DragModel, ForceContext};
use crate::frame::NodeId;
use crate::integrate::{hermite5, Dopri5, Dynamics, StepSample, Tolerance};
use crate::time::Epoch;
use crate::vessel::{air_at, quat_z_to, AeroTick};
use crate::world::World;
use glam::{DQuat, DVec3};

/// Integration tolerance (position 1 cm, velocity 0.1 mm/s).
fn tolerance() -> Tolerance {
    Tolerance { abs_r: 1e-2, abs_v: 1e-4, rel: 1e-11, h_min: 1e-4, h_max: 600.0 }
}

/// Longest step (s) while thrusting or within [`LOW`] of a surface: contact
/// is checked at step ends, and the braking stop is found within a step.
const H_LOW: f64 = 2.0;
/// Below this height above a surface steps are at most [`H_LOW`] (m).
const LOW: f64 = 20_000.0;
/// Below this surface-relative speed a braking burn has stopped (m/s).
pub(super) const REST_SPEED: f64 = 0.05;
/// Surfaces farther than their radius plus this are not checked for contact (m).
const NEAR: f64 = 200_000.0;
/// Bisection iterations for contact and the stop.
const BISECT: usize = 60;

/// One integration run: from `(r, v)` at `t0` with `propellant` aboard and
/// the throttle held.
pub(super) struct Run {
    pub t0: Epoch,
    pub anchor: NodeId,
    pub r: DVec3,
    pub v: DVec3,
    pub propellant: f64,
    pub throttle: f64,
}

impl Run {
    pub fn from_start(s: &LandingStart, throttle: f64) -> Self {
        Run { t0: s.t, anchor: s.anchor, r: s.r, v: s.v, propellant: s.propellant, throttle }
    }
}

/// How a run ended (local times).
pub(super) enum End {
    /// The lowest contact point met `body`'s surface.
    Surface {
        t: f64,
        body: NodeId,
    },
    /// The surface-relative speed reached zero (braking runs).
    Stopped {
        t: f64,
    },
    Horizon,
    Steps,
    Failed,
}

pub(super) struct Flight {
    /// Accepted steps; the last one is at the end's time.
    pub samples: Vec<StepSample>,
    pub end: End,
}

/// The craft, the assumptions and the world of a prediction.
pub(super) struct Model<'a> {
    world: &'a World,
    craft: &'a CraftParams,
    design: &'a CraftDesign,
    body: NodeId,
    attitude: AssumedAttitude,
    chute: bool,
    infinite: bool,
}

/// The dynamics of one run.
struct Forces<'a, 'w> {
    model: &'a Model<'a>,
    run: &'a Run,
    snaps: &'a SnapshotCache<'w>,
    active: &'a ActiveSources,
}

impl Dynamics for Forces<'_, '_> {
    fn accel(&self, t: f64, r: DVec3, v: DVec3) -> DVec3 {
        let e = self.run.t0.add_seconds(t);
        self.snaps.with(e, |snap| self.model.accel(snap, self.run, self.active, t, r, v))
    }
}

impl<'a> Model<'a> {
    pub fn new(world: &'a World, craft: &'a CraftParams, start: &LandingStart) -> Self {
        Model {
            world,
            craft,
            design: craft.design(),
            body: start.body,
            attitude: start.attitude,
            chute: start.chute,
            infinite: start.infinite,
        }
    }

    /// Propellant at local time `t` of a run.
    fn propellant(&self, run: &Run, t: f64) -> f64 {
        let mdot = self.craft.engine.output(run.throttle, 0.0).mdot;
        if self.infinite || mdot == 0.0 {
            run.propellant
        } else {
            (run.propellant - mdot * t).max(0.0)
        }
    }

    /// Velocity relative to the reference body's surface.
    fn surface_velocity(&self, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> DVec3 {
        v - Ground::new(self.world, snap, anchor, self.body).velocity(r)
    }

    /// The assumed attitude (body → inertial) at `(r, v)`.
    fn attitude(&self, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> DQuat {
        match self.attitude {
            AssumedAttitude::Inertial(q) => q,
            AssumedAttitude::SurfaceRetrograde | AssumedAttitude::SurfacePrograde => {
                let v_srf = self.surface_velocity(snap, anchor, r, v);
                let sign = if self.attitude == AssumedAttitude::SurfacePrograde { 1.0 } else { -1.0 };
                let dir = if v_srf.length() > 1e-3 {
                    v_srf.normalize() * sign
                } else {
                    (r - snap.relative_r(self.body, anchor)).normalize()
                };
                (quat_z_to(dir) * quat_z_to(self.craft.engine.mount_dir).inverse()).normalize()
            }
        }
    }

    /// Acceleration at local time `t` of `run`.
    pub(super) fn accel(
        &self,
        snap: &Snapshot,
        run: &Run,
        active: &ActiveSources,
        t: f64,
        r: DVec3,
        v: DVec3,
    ) -> DVec3 {
        let propellant = self.propellant(run, t);
        let props = self.craft.mass.at(propellant);
        let q = self.attitude(snap, run.anchor, r, v);
        let running = run.throttle > 0.0 && (self.infinite || propellant > 0.0);
        let thrust = if running {
            let p = ambient_pressure(self.world, snap, run.anchor, r);
            self.craft.engine.output(run.throttle, p).thrust
        } else {
            0.0
        };
        let chute = self.chute.then_some((self.craft.chute_cd_area, self.craft.chute_mount));
        let air = air_at(self.world, snap, run.anchor, r, v);
        let aero = air.as_ref().and_then(|a| AeroTick::new(self.design, a, props.com, chute));
        let (cd_area, lift) = aero.map_or((0.0, DVec3::ZERO), |a| a.drag_and_lift(q, props.mass));
        let thrust = q * self.craft.engine.mount_dir * (thrust / props.mass) + lift;
        let drag = Some(DragModel { cd_area, mass: props.mass });
        ForceContext { world: self.world, anchor: run.anchor, active, drag, thrust }.accel_with(snap, r, v)
    }

    /// Height of the lowest contact point above `body`'s surface at the
    /// assumed attitude, given the centre of mass at `(r, v)`. Unless
    /// `exact`, the height of the centre of mass minus the contact reach
    /// when that is certainly positive (cheap).
    fn clearance(
        &self,
        snap: &Snapshot,
        anchor: NodeId,
        body: NodeId,
        (r, v): (DVec3, DVec3),
        prop: f64,
        exact: bool,
    ) -> f64 {
        let ground = Ground::new(self.world, snap, anchor, body);
        let h = ground.clearance(r) - self.craft.contact_reach(prop);
        if h > 1.0 && !exact {
            return h;
        }
        let q = self.attitude(snap, anchor, r, v);
        let com = self.craft.mass.at(prop).com;
        (self.craft.contacts.iter()).map(|c| ground.clearance(r + q * (c.pos - com))).fold(f64::INFINITY, f64::min)
    }

    /// The exact lowest-point clearance above the reference body at local
    /// time `t` of `run`.
    pub fn clearance_exact(&self, run: &Run, t: f64, r: DVec3, v: DVec3) -> f64 {
        let snap = self.world.snapshot(run.t0.add_seconds(t));
        self.clearance(&snap, run.anchor, self.body, (r, v), self.propellant(run, t), true)
    }

    /// The impact on `body` of the centre of mass at `(r, v)` at `t`.
    pub fn impact(&self, t: Epoch, anchor: NodeId, body: NodeId, r: DVec3, v: DVec3) -> Impact {
        let snap = self.world.snapshot(t);
        let ground = Ground::new(self.world, &snap, anchor, body);
        let fixed = ground.fixed(r).raw();
        let (lat, lon) = crate::terrain::lat_lon(fixed);
        let height = ground.physical.surface_height_latlon(lat, lon);
        let up = (r - ground.center).normalize();
        let v_srf = v - ground.velocity(r);
        let v_vertical = v_srf.dot(up);
        Impact {
            body,
            t,
            ground: ground.physical.surface_point(lat, lon, height),
            lat,
            lon,
            height,
            v_vertical,
            v_horizontal: (v_srf - up * v_vertical).length(),
            speed: v_srf.length(),
        }
    }

    /// Integrates `run` until contact, the stop (with `stop`), `horizon`
    /// seconds or `max_steps` steps.
    pub fn fly(&self, run: &Run, stop: bool, horizon: f64, max_steps: usize) -> Flight {
        let world = self.world;
        let snaps = SnapshotCache::new(&world.eph);
        let mut active = snaps.with(run.t0, |s| ActiveSources::select(world, s, run.anchor, run.r));
        let integ = Dopri5::new(tolerance());
        let mut st = {
            let f = Forces { model: self, run, snaps: &snaps, active: &active };
            integ.start(&f, 0.0, run.r, run.v, 1.0)
        };
        let mut samples = vec![StepSample { t: 0.0, r: run.r, v: run.v, a: st.a }];
        let first = samples[0];
        if let Some(body) = self.below(run, &first) {
            return Flight { samples, end: End::Surface { t: 0.0, body } };
        }
        for _ in 0..max_steps {
            if st.t >= horizon {
                return Flight { samples, end: End::Horizon };
            }
            let prev = *samples.last().expect("a first sample");
            if run.throttle > 0.0 || self.low(run, &prev) {
                st.h = st.h.min(H_LOW);
            }
            let step = {
                let f = Forces { model: self, run, snaps: &snaps, active: &active };
                integ.step(&f, &mut st, horizon)
            };
            let Ok(new) = step else { return Flight { samples, end: End::Failed } };
            if let Some((t, body)) = self.contact(run, &prev, &new) {
                samples.push(self.sample_at(run, &active, &prev, &new, t));
                return Flight { samples, end: End::Surface { t, body } };
            }
            if stop {
                if let Some(t) = self.stopped(run, &prev, &new) {
                    samples.push(self.sample_at(run, &active, &prev, &new, t));
                    return Flight { samples, end: End::Stopped { t } };
                }
            }
            samples.push(new);
            let grew = snaps.with(run.t0.add_seconds(st.t), |s| active.grow(world, s, run.anchor, new.r));
            if grew {
                let f = Forces { model: self, run, snaps: &snaps, active: &active };
                st.a = f.accel(st.t, new.r, new.v);
            }
        }
        Flight { samples, end: End::Steps }
    }

    /// The interpolated sample at local `t` between two steps.
    fn sample_at(&self, run: &Run, active: &ActiveSources, a: &StepSample, b: &StepSample, t: f64) -> StepSample {
        let (r, v) = hermite5(a, b, t);
        let snap = self.world.snapshot(run.t0.add_seconds(t));
        StepSample { t, r, v, a: self.accel(&snap, run, active, t, r, v) }
    }

    /// Whether the sample is within [`LOW`] of the reference surface.
    fn low(&self, run: &Run, s: &StepSample) -> bool {
        let snap = self.world.snapshot(run.t0.add_seconds(s.t));
        Ground::new(self.world, &snap, run.anchor, self.body).clearance(s.r) < LOW
    }

    /// The surfaces near the sample at its time.
    fn near(&self, run: &Run, s: &StepSample) -> Vec<NodeId> {
        let snap = self.world.snapshot(run.t0.add_seconds(s.t));
        (self.world.surfaces())
            .filter(|src| {
                let radius = src.physical.as_ref().map_or(0.0, |p| p.radius_eq);
                (s.r - snap.relative_r(src.node, run.anchor)).length() <= radius + NEAR
            })
            .map(|src| src.node)
            .collect()
    }

    /// Lowest-point clearance above `body` of the sample at local time `t`.
    fn clearance_run(&self, run: &Run, body: NodeId, t: f64, r: DVec3, v: DVec3) -> f64 {
        let snap = self.world.snapshot(run.t0.add_seconds(t));
        self.clearance(&snap, run.anchor, body, (r, v), self.propellant(run, t), false)
    }

    /// A surface the sample is already below.
    fn below(&self, run: &Run, s: &StepSample) -> Option<NodeId> {
        self.near(run, s).into_iter().find(|&b| self.clearance_run(run, b, s.t, s.r, s.v) < 0.0)
    }

    /// First contact between two samples, by bisection on the interpolated
    /// clearance of the lowest contact point.
    fn contact(&self, run: &Run, a: &StepSample, b: &StepSample) -> Option<(f64, NodeId)> {
        let body = self.below(run, b)?;
        let at = |t: f64| {
            let (r, v) = hermite5(a, b, t);
            self.clearance_run(run, body, t, r, v)
        };
        let (mut lo, mut hi) = (a.t, b.t);
        for _ in 0..BISECT {
            let mid = 0.5 * (lo + hi);
            if at(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some((hi, body))
    }

    /// Where the surface-relative speed reaches zero between two samples:
    /// the speed is below [`REST_SPEED`] at `b`, or the surface-relative
    /// velocity reverses (bisection on its component along the velocity at `a`).
    fn stopped(&self, run: &Run, a: &StepSample, b: &StepSample) -> Option<f64> {
        let v_srf = |t: f64, r: DVec3, v: DVec3| {
            let snap = self.world.snapshot(run.t0.add_seconds(t));
            self.surface_velocity(&snap, run.anchor, r, v)
        };
        let va = v_srf(a.t, a.r, a.v);
        let vb = v_srf(b.t, b.r, b.v);
        if vb.length() < REST_SPEED {
            return Some(b.t);
        }
        if va.dot(vb) > 0.0 {
            return None;
        }
        let (mut lo, mut hi) = (a.t, b.t);
        for _ in 0..BISECT {
            let mid = 0.5 * (lo + hi);
            let (r, v) = hermite5(a, b, mid);
            if v_srf(mid, r, v).dot(va) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(hi)
    }
}
