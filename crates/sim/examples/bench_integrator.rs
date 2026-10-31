//! Coast integration cost and accuracy (release), the COAST workstream's
//! measurements (`docs/technical/scale-performance.md`): for a low orbit
//! (bare and perturbed: thin air, light, held prograde), GEO, a lunar
//! transfer and a heliocentric arc, the µs per step and the steps per day
//! of the segment integrator.
//! `cargo run -p sim --release --example bench_integrator`
//! `BENCH_PROFILE=<case>` repeats one case's segment for 30 s (for `sample`).
//! `BENCH_ACCURACY=1`: the integrators (Dopri5 and DOP853 at the coast
//! tolerance and looser) against a tight DOP853 reference with the same
//! force model: the error at the end, the steps, and the worst error of
//! the quintic Hermite dense output at step midpoints.
//! `BENCH_FLEET=1`: a quiet test craft and a 300-part craft at 300 km,
//! integration against the rest of a 1,000,000x frame.
use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::forces::{ActiveSources, ForceContext};
use sim::frame::NodeId;
use sim::integrate::{hermite5, Dop853, Dopri5, Dynamics, StepSample, Tolerance};
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::limbo::Limbo;
use sim::vessel::{CoastStart, Controls, HoldMode, Segment, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

const RE: f64 = 6_378_137.0;

struct Case {
    name: &'static str,
    seg: Segment,
    /// Simulated time to integrate (s).
    span: f64,
}

fn coast(w: &World, t0: sim::time::Epoch, anchor: sim::frame::NodeId, r: DVec3, v: DVec3) -> Segment {
    Segment::new(
        w,
        t0,
        CoastStart {
            anchor,
            r,
            v,
            drag: None,
            contact_height: 5.0,
            horizon: 1e9,
            fixed_anchor: false,
            proper_time: 0.0,
        },
    )
}

/// A transfer from 200 km whose apogee is just past the Moon, aimed at
/// where the Moon is when it arrives (about five days on).
fn lunar_transfer(w: &World, t0: sim::time::Epoch) -> (DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let moon = w.find("Moon").unwrap();
    let arrive = w.snapshot(t0.add_seconds(4.9 * 86_400.0)).relative(moon.node, earth.node);
    let now = w.snapshot(t0).relative(moon.node, earth.node);
    let normal = now.r.cross(now.v).normalize();
    let rp = RE + 200_000.0;
    let ra = arrive.r.length() + 20_000_000.0;
    let a = 0.5 * (rp + ra);
    let speed = (earth.gm * (2.0 / rp - 1.0 / a)).sqrt();
    let r = -arrive.r.normalize() * rp;
    (r, normal.cross(r).normalize() * speed)
}

fn cases(w: &World) -> Vec<Case> {
    let earth = w.find("Earth").unwrap();
    let sun = w.find("Sun").unwrap();
    let t0 = sol::sol_epoch().add_seconds(86_400.0);
    let el =
        |a: f64, e: f64, i: f64, gm: f64| Elements { a, e, i, raan: 0.3, argp: 0.0, mean_anomaly: 1.0 }.to_state(gm);
    let mut out = Vec::new();
    let (r, v) = el(RE + 250_000.0, 0.001, 0.9, earth.gm);
    out.push(Case { name: "leo", seg: coast(w, t0, earth.node, r, v), span: 86_400.0 });
    // The limbo drag craft of `bench_limbo` (held prograde at 250 km).
    let mut s = Vessel::coasting(w, VesselId(1), t0, earth.node, r, v, sim::craft::small::rocket_ref());
    let c = Controls { sas: true, hold: HoldMode::Prograde, ..Controls::default() };
    s.advance(w, t0.add_seconds(900.0), &c, usize::MAX);
    out.push(Case { name: "leo_perturbed", seg: perturbed_from(w, &s), span: 86_400.0 });
    let (r, v) = el(42_164_000.0, 0.0002, 0.001, earth.gm);
    out.push(Case { name: "geo", seg: coast(w, t0, earth.node, r, v), span: 10.0 * 86_400.0 });
    let (r, v) = lunar_transfer(w, t0);
    out.push(Case { name: "lunar", seg: coast(w, t0, earth.node, r, v), span: 6.0 * 86_400.0 });
    // 0.1 AU outside Earth's orbit, about the Sun.
    let earth_sun = w.snapshot(t0).relative(earth.node, sun.node);
    let r = earth_sun.r * 1.1;
    let v = earth_sun.v * (1.0 / 1.1f64).sqrt();
    out.push(Case { name: "helio", seg: coast(w, t0, sun.node, r, v), span: 365.0 * 86_400.0 });
    out
}

/// A copy of the vessel's current (perturbed) coast, restarted at its start.
fn perturbed_from(w: &World, s: &Vessel) -> Segment {
    let seg = s.segment().unwrap();
    Segment::new_limbo(w, seg.t0, seg.initial(), seg.initial_mass(), seg.limbo().unwrap().clone())
}

fn run(w: &World, case: &Case) -> (usize, f64, u64) {
    let mut seg = case.seg.clone();
    let t = Instant::now();
    seg.extend_until(w, case.span, usize::MAX);
    let dt = t.elapsed().as_secs_f64();
    let mut bytes = Vec::new();
    for s in &seg.samples {
        bytes.extend_from_slice(&s.anchor.0.to_le_bytes());
        for x in [s.s.t, s.s.r.x, s.s.r.y, s.s.r.z, s.s.v.x, s.s.v.y, s.s.v.z] {
            bytes.extend_from_slice(&x.to_bits().to_le_bytes());
        }
    }
    (seg.samples.len() - 1, dt, sim::ephem::fnv1a64(&bytes))
}

/// The segment's force model with a fixed anchor and source set.
struct Model<'a> {
    w: &'a World,
    t0: Epoch,
    anchor: NodeId,
    active: ActiveSources,
    limbo: Option<Limbo>,
    mass: f64,
}

impl Dynamics for Model<'_> {
    fn accel(&self, t: f64, r: DVec3, v: DVec3) -> DVec3 {
        let snap = self.w.snapshot(self.t0.add_seconds(t));
        let f =
            ForceContext { world: self.w, anchor: self.anchor, active: &self.active, drag: None, thrust: DVec3::ZERO };
        let a = f.accel_with(&snap, r, v);
        match &self.limbo {
            Some(l) => a + l.accel(self.w, &snap, self.anchor, r, v, self.mass),
            None => a,
        }
    }
}

#[derive(Clone, Copy)]
enum Method {
    Five,
    Eight,
}

fn propagate(m: &Model, r: DVec3, v: DVec3, method: Method, tol: Tolerance, end: f64) -> Vec<StepSample> {
    let mut out = Vec::new();
    match method {
        Method::Five => {
            let i = Dopri5::new(tol);
            let mut st = i.start(m, 0.0, r, v, 10.0);
            out.push(StepSample { t: 0.0, r, v, a: st.a });
            while st.t < end {
                out.push(i.step(m, &mut st, end).unwrap());
            }
        }
        Method::Eight => {
            let i = Dop853::new(tol);
            let mut st = i.start(m, 0.0, r, v, 10.0);
            out.push(StepSample { t: 0.0, r, v, a: st.a });
            while st.t < end {
                out.push(i.step(m, &mut st, end).unwrap());
            }
        }
    }
    out
}

/// The reference's position at `t` (Hermite on its short steps).
fn at(reference: &[StepSample], t: f64) -> DVec3 {
    let k = reference.partition_point(|s| s.t <= t).clamp(1, reference.len() - 1);
    hermite5(&reference[k - 1], &reference[k], t).0
}

fn accuracy(w: &World, cases: &[Case]) {
    let coast = sim::vessel::coast_tolerance();
    for case in cases {
        let seg = &case.seg;
        let start = seg.initial();
        let snap = w.snapshot(seg.t0);
        let m = Model {
            w,
            t0: seg.t0,
            anchor: start.anchor,
            active: ActiveSources::select(w, &snap, start.anchor, start.r),
            limbo: seg.limbo().cloned(),
            mass: seg.initial_mass(),
        };
        let span = case.span;
        // Reference: tight, and short steps so its Hermite is exact enough.
        let h_ref = match case.name {
            "leo" | "leo_perturbed" => 10.0,
            "lunar" => 60.0,
            _ => 600.0,
        };
        let tight = Tolerance { abs_r: 1e-6, abs_v: 1e-9, rel: 1e-15, h_min: 1e-6, h_max: h_ref };
        let t = Instant::now();
        let reference = propagate(&m, start.r, start.v, Method::Eight, tight, span);
        let r_ref = reference.last().unwrap().r;
        println!("{}: reference {} steps ({:.1} s)", case.name, reference.len() - 1, t.elapsed().as_secs_f64());
        // The coasts' former integrator and tolerance (to 2026-10-26).
        let before = Tolerance { abs_r: 1e-2, abs_v: 1e-5, rel: 1e-11, h_min: 1e-4, h_max: 6.0 * 3600.0 };
        let loose = |k: f64| Tolerance { abs_r: coast.abs_r * k, rel: coast.rel * k, ..coast };
        for (label, method, tol) in [
            ("dopri5, before", Method::Five, before),
            ("dop853, before's tol", Method::Eight, before),
            ("dop853, coast tol", Method::Eight, coast),
            ("dop853, tol x10", Method::Eight, loose(10.0)),
            ("dop853, 6 h steps", Method::Eight, Tolerance { h_max: 21_600.0, ..coast }),
        ] {
            let samples = propagate(&m, start.r, start.v, method, tol, span);
            let err = (samples.last().unwrap().r - r_ref).length();
            let mut dense: f64 = 0.0;
            for p in samples.windows(2) {
                let mid = 0.5 * (p[0].t + p[1].t);
                dense = dense.max((hermite5(&p[0], &p[1], mid).0 - at(&reference, mid)).length());
            }
            let evals = (samples.len() - 1)
                * match method {
                    Method::Five => 6,
                    Method::Eight => 12,
                };
            println!(
                "  {label:>18}: {:6} steps, {:7} evaluations, end error {:9.3e} m, dense output worst {:9.3e} m",
                samples.len() - 1,
                evals,
                err,
                dense
            );
        }
    }
}

/// One quiet craft of the D081 fleet in a 300 km orbit (the test craft
/// and the 300-part stress craft), settled 30 days: the integration and
/// the rest (attitude, thermal and systems lattices) per 1,000,000x frame.
fn fleet_craft(w: &World) {
    let earth = w.find("Earth").unwrap();
    let t0 = sol::sol_epoch().add_seconds(86_400.0);
    let el = Elements { a: RE + 300_000.0, e: 0.001, i: 0.3, raan: 0.4, argp: 0.0, mean_anomaly: 0.7 };
    let (r, v) = el.to_state(earth.gm);
    let big = (*sim::craft::stress::craft(300)).clone();
    let frame = 1e6 / 60.0;
    for (name, craft) in [("small rocket", sim::craft::small::rocket_ref()), ("300-part craft", &big)] {
        let mut s = Vessel::coasting(w, VesselId(1), t0, earth.node, r, v, craft);
        let c = Controls::default();
        s.advance(w, t0.add_seconds(30.0 * 86_400.0), &c, usize::MAX);
        let start = s.time;
        let frames = 60;
        let t = Instant::now();
        s.extend_coast(w, start.add_seconds(frame * frames as f64 + 10.0), usize::MAX);
        let integrate = t.elapsed().as_secs_f64() / frames as f64;
        let t = Instant::now();
        for k in 1..=frames {
            s.advance(w, start.add_seconds(frame * k as f64), &c, usize::MAX);
        }
        let rest = t.elapsed().as_secs_f64() / frames as f64;
        println!(
            "{name:>15}: integration {:.0} µs per 1,000,000x frame, the rest {:.0} µs",
            integrate * 1e6,
            rest * 1e6
        );
    }
}

fn main() {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    let cases = cases(&w);
    if std::env::var("BENCH_FLEET").is_ok() {
        fleet_craft(&w);
        return;
    }
    if std::env::var("BENCH_ACCURACY").is_ok() {
        accuracy(&w, &cases);
        return;
    }
    if let Ok(name) = std::env::var("BENCH_PROFILE") {
        let case = cases.iter().find(|c| c.name == name).expect("a case name");
        let t = Instant::now();
        let mut steps = 0;
        while t.elapsed().as_secs_f64() < 30.0 {
            steps += run(&w, case).0;
        }
        println!("{name}: {steps} steps in 30 s");
        return;
    }
    for case in &cases {
        let (n, mut best, hash) = run(&w, case);
        for _ in 0..4 {
            best = best.min(run(&w, case).1);
        }
        println!(
            "{:>14}: {:7.2} µs/step, {:8.1} steps/day, {:6} steps over {:.1} days, samples {hash:#018x}",
            case.name,
            best / n as f64 * 1e6,
            n as f64 / (case.span / 86_400.0),
            n,
            case.span / 86_400.0
        );
    }
}
