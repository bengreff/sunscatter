//! Cost and accuracy of the thermal network solve (hot spot 1 of
//! `docs/archive/research/scale-baseline-2026-10-26.md`; results in
//! `docs/technical/scale-performance.md`), release only:
//!
//! `cargo run -p sim --release --example bench_thermal [parts=0,50,300,1000] [runs=3] [solve]`
//!
//! Part count 0 is the test craft. Per craft: 600 s coast solves from a
//! uniform 290 K in sunlight (the first and the 10th), coast solves under
//! a turning Sun after the craft has settled (a held craft in LEO without
//! orbit averaging: the input changes at every step), 20 ms live steps
//! under entry heating; each with its sweeps, its time, its energy residual
//! (the step's balance Σ C·ΔT/dt − heat in + radiated, relative to the
//! heat flowing) and its error against the same step converged to 1e-14.
//! Then (unless `solve`) whole coast lattice points of a vessel in LEO
//! (`Vessel::advance`, integrated ahead): tumbling and held prograde.
use glam::DVec3;
use sim::craft::{stress, test_craft, CraftDesign};
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::thermal::{cell_heat_flow, ThermalState, COAST_CONVERGED, CONVERGED, DEFAULT_SWEEPS, STEFAN_BOLTZMANN};
use sim::vessel::{quat_z_to, Attitude, Controls, HoldMode, Vessel, VesselId, LATTICE, TICK};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

const SINK: f64 = 2.725;

struct Case {
    design: std::sync::Arc<CraftDesign>,
    capacity: Vec<f64>,
    node_heat: Vec<f64>,
}

impl Case {
    fn heat(&self, sun: DVec3, q_stag: f64) -> Vec<f64> {
        let d = &self.design;
        let n = d.cells.len();
        let (mut heat, mut scratch) = (vec![0.0; n], vec![0.0; n]);
        cell_heat_flow(&d.bake, &d.cells, None, sun, &mut scratch, &mut heat);
        // The stagnation flux spread nose-first as f·sin^1.5 θ, at least 3 %
        // (the heating's distribution before D085; a fixed load for timing).
        d.bake.exposure_at(-DVec3::Z, &mut scratch);
        for ((h, c), &f) in heat.iter_mut().zip(&d.cells).zip(&scratch) {
            let s = c.normal.z.max(0.0);
            *h += q_stag * (f * s * s.sqrt()).max(0.03) * c.area;
        }
        heat
    }

    /// One step: (sweeps, µs).
    fn step(&self, s: &mut ThermalState, heat: &[f64], dt: f64, tol: f64) -> (usize, f64) {
        let t = Instant::now();
        let k = self.design.network.step_to(s, heat, &self.node_heat, &self.capacity, SINK, dt, DEFAULT_SWEEPS, tol);
        (k, t.elapsed().as_secs_f64() * 1e6)
    }

    /// The step's energy balance from `old` to `new` over `dt`, relative to
    /// the heat flowing in and out.
    fn residual(&self, old: &ThermalState, new: &ThermalState, heat: &[f64], dt: f64) -> f64 {
        let net = &self.design.network;
        let stored = net.heat_content(new, &self.capacity) - net.heat_content(old, &self.capacity);
        let env4 = SINK * SINK * SINK * SINK;
        let radiated: f64 = (net.radiation.iter().zip(&new.skin)).map(|(r, t)| r * (t * t * t * t - env4)).sum();
        let input: f64 = heat.iter().sum::<f64>() + self.node_heat.iter().sum::<f64>();
        (stored / dt - input + radiated).abs() / (input.abs() + radiated.abs()).max(1e-30)
    }

    /// Largest |ΔT| (K) of `s` against the same step converged to 1e-14.
    fn error(&self, old: &ThermalState, s: &ThermalState, heat: &[f64], dt: f64) -> f64 {
        let mut exact = old.clone();
        self.design.network.step_to(&mut exact, heat, &self.node_heat, &self.capacity, SINK, dt, 100_000, 1e-14);
        (s.skin.iter().zip(&exact.skin))
            .chain(s.nodes.iter().zip(&exact.nodes))
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
    }
}

#[derive(Default, Clone, Copy)]
struct Stat {
    us: f64,
    sweeps: f64,
    residual: f64,
    error: f64,
    n: usize,
}

impl Stat {
    fn add(&mut self, (k, us): (usize, f64), residual: f64, error: f64) {
        self.us += us;
        self.sweeps += k as f64;
        self.residual = self.residual.max(residual);
        self.error = self.error.max(error);
        self.n += 1;
    }
    fn show(&self) -> String {
        let n = self.n.max(1) as f64;
        format!(
            "{:.0} µs, {:.1} sweeps, residual {:.1e}, error {:.1e} K",
            self.us / n,
            self.sweeps / n,
            self.residual,
            self.error
        )
    }
}

/// A day in LEO (400 km) per coast lattice point (µs), integrated ahead:
/// tumbling (SAS off: the isotropic sunlight) and held prograde (the
/// sunlight averaged over the orbit), each from a uniform 290 K and after a
/// settling day.
fn lattice(w: &World, craft: &sim::craft::Craft, runs: usize) -> [f64; 4] {
    let earth = w.find("Earth").unwrap();
    let t0 = sim::sol::sol_epoch().add_seconds(86_400.0);
    let el = Elements { a: 6_378_137.0 + 400e3, e: 0.001, i: 0.9, raan: 0.3, argp: 0.0, mean_anomaly: 0.6 };
    let (r, v) = el.to_state(earth.gm);
    let day = 86_400.0;
    let one = |tumbling: bool, settle: f64| {
        let mut times: Vec<f64> = (0..runs.max(1))
            .map(|_| {
                let mut s = Vessel::coasting(w, VesselId(1), t0, earth.node, r, v, craft);
                let controls = if tumbling {
                    s.set_attitude(Attitude { q: quat_z_to(r.normalize()), omega: DVec3::new(0.01, 0.02, 0.0) });
                    Controls::default()
                } else {
                    Controls { sas: true, hold: HoldMode::Prograde, ..Default::default() }
                };
                s.advance(w, t0.add_seconds(1.0), &controls, usize::MAX);
                s.extend_coast(w, t0.add_seconds(settle + day + 7_200.0), usize::MAX);
                s.advance(w, t0.add_seconds(settle), &controls, usize::MAX);
                let t = Instant::now();
                s.advance(w, t0.add_seconds(settle + day), &controls, usize::MAX);
                t.elapsed().as_secs_f64() * 1e6 / (day / LATTICE)
            })
            .collect();
        times.sort_by(f64::total_cmp);
        times[times.len() / 2]
    };
    [one(true, 1.0), one(true, day), one(false, 1.0), one(false, day)]
}

fn run(name: &str, design: std::sync::Arc<CraftDesign>, propellant: f64, runs: usize) {
    let m = design.nodes();
    let mut capacity = vec![0.0; m];
    design.node_capacity(&sim::craft::design::Aboard::Fill(propellant), &mut capacity);
    let case = Case { design, capacity, node_heat: vec![0.0; m] };
    let (n, unknowns) = (case.design.cells.len(), case.design.cells.len() + m);
    let sun = DVec3::new(1361.0, 0.0, 0.0);
    let lit = case.heat(sun, 0.0);
    // Fresh: from 290 K, ten 600 s steps.
    let (mut first, mut tenth) = (Stat::default(), Stat::default());
    for _ in 0..runs {
        let mut s = ThermalState::uniform(n, m, 290.0);
        for k in 0..10 {
            let old = s.clone();
            let r = case.step(&mut s, &lit, LATTICE, COAST_CONVERGED);
            let (res, err) = (case.residual(&old, &s, &lit, LATTICE), case.error(&old, &s, &lit, LATTICE));
            match k {
                0 => first.add(r, res, err),
                9 => tenth.add(r, res, err),
                _ => {}
            }
        }
    }
    // Turning Sun: settle 2 days, then 60 steps with the Sun turning by a
    // LEO orbit's 600 s (39°) each.
    let mut s = ThermalState::uniform(n, m, 290.0);
    for _ in 0..288 {
        case.step(&mut s, &lit, LATTICE, COAST_CONVERGED);
    }
    let mut turning = Stat::default();
    let rate = 2.0 * std::f64::consts::PI / 5_550.0;
    for k in 0..60 {
        let a = rate * LATTICE * k as f64;
        let heat = case.heat(DVec3::new(a.cos(), a.sin(), 0.3).normalize() * 1361.0, 0.0);
        let old = s.clone();
        let r = case.step(&mut s, &heat, LATTICE, COAST_CONVERGED);
        turning.add(r, case.residual(&old, &s, &heat, LATTICE), case.error(&old, &s, &heat, LATTICE));
    }
    // Live: 20 ms steps under 300 kW/m² at the stagnation point.
    let hot = case.heat(sun, 3.0e5);
    let mut live = Stat::default();
    let mut s = ThermalState::uniform(n, m, 290.0);
    for k in 0..200 {
        let old = s.clone();
        let r = case.step(&mut s, &hot, TICK, CONVERGED);
        let check = k % 20 == 19;
        let (res, err) =
            if check { (case.residual(&old, &s, &hot, TICK), case.error(&old, &s, &hot, TICK)) } else { (0.0, 0.0) };
        live.add(r, res, err);
    }
    let hottest = s.skin.iter().fold(0.0f64, |a, &b| a.max(b));
    let _ = STEFAN_BOLTZMANN;
    println!("{name}: {n} cells + {m} nodes = {unknowns} unknowns");
    println!("  coast 600 s from 290 K, first: {}", first.show());
    println!("  coast 600 s from 290 K, 10th:  {}", tenth.show());
    println!("  coast 600 s, turning Sun:      {}", turning.show());
    println!("  live 20 ms, entry:             {} (hottest {hottest:.0} K)", live.show());
    println!(
        "  ns per unknown per sweep: coast {:.1}, live {:.1}",
        turning.us * 1e3 / (turning.sweeps * unknowns as f64),
        live.us * 1e3 / (live.sweeps * unknowns as f64)
    );
}

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sim::sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn show_lattice(w: &World, craft: &sim::craft::Craft, runs: usize) {
    let [a, b, c, d] = lattice(w, craft, runs);
    println!(
        "  per coast lattice point over a day in LEO: tumbling {a:.0} µs fresh, {b:.0} settled; held prograde {c:.0} µs fresh, {d:.0} settled"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| args.iter().find_map(|a| a.strip_prefix(key).map(str::to_owned));
    let parts: Vec<usize> =
        value("parts=").map_or(vec![0, 50, 300], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let runs = value("runs=").map_or(3, |s| s.parse().unwrap());
    let with_lattice = !args.iter().any(|a| a == "solve");
    let w = world();
    for p in parts {
        if p == 0 {
            let c = test_craft();
            run("test craft", c.design().clone(), c.spec.propellant.capacity, runs);
            if with_lattice {
                show_lattice(&w, c, runs);
            }
        } else {
            let s = stress::stress_craft(p);
            let cap = s.craft.spec.propellant.capacity;
            run(&format!("stress {p}"), s.craft.design().clone(), cap, runs);
            if with_lattice {
                show_lattice(&w, &s.craft, runs);
            }
        }
    }
}
