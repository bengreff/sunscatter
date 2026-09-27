//! Timing of the aerodynamic bake and the per-tick aero and thermal work
//! for the test craft (release): `cargo run -p sim --release --example bench_aero`
use glam::DVec3;
use sim::aero::{aero_forces, bake, BakeOptions, Flow};
use sim::craft::test_craft;
use sim::thermal::{cell_heat, HeatInput, InternalNode, ThermalNetwork, ThermalState};
use std::time::Instant;

fn main() {
    let c = test_craft();
    println!("test craft: {} triangles, {} cells", c.surface.triangles.len(), c.cells.cells.len());
    for level in [3, 4] {
        let opts = BakeOptions { level, ..BakeOptions::default() };
        let clock = Instant::now();
        let b = bake(&c.surface, &c.cells, &opts);
        let ms = clock.elapsed().as_secs_f64() * 1e3;
        println!(
            "bake level {level}: {} directions, {:.1} ms, exposure {} KB, hash {:#x}",
            b.grid.dirs.len(),
            ms,
            b.exposure.len() / 1024,
            b.hash()
        );
    }
    let b = bake(&c.surface, &c.cells, &BakeOptions::default());
    let net = ThermalNetwork::new(&c.cells.cells, InternalNode { capacity: 4.0e6, coefficient: 2.0 });
    let mut state = ThermalState::uniform(c.cells.cells.len(), 290.0);
    let mut heat = vec![0.0; c.cells.cells.len()];
    let mut exposure = vec![0.0; c.cells.cells.len()];
    let n = 10_000;
    let clock = Instant::now();
    let mut sink = DVec3::ZERO;
    for i in 0..n {
        let a = i as f64 * 1e-3;
        let dir = DVec3::new(sim::math::sin(a) * 0.2, 0.1, -1.0).normalize();
        let flow = Flow { dir, q: 20e3, mach: 12.0, knudsen: 1e-4, gamma: 1.4 };
        let (f, m) = aero_forces(&b, &flow, 0.8);
        sink += f + m;
        let input = HeatInput { flow_dir: dir, q_stag: 1e6, sun: DVec3::new(1361.0, 0.0, 0.0) };
        cell_heat(&b, &c.cells.cells, &input, &mut exposure, &mut heat);
        net.step(&mut state, &heat, 0.0, 3.0, 0.02, sim::thermal::DEFAULT_SWEEPS);
    }
    let us = clock.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("per tick (aero forces + cell heating + thermal step, 512 cells): {us:.1} µs  ({sink:.0?})");
    let dir = DVec3::new(0.1, 0.1, -1.0).normalize();
    let time = |what: &str, f: &mut dyn FnMut()| {
        let clock = Instant::now();
        for _ in 0..n {
            f();
        }
        println!("  {what}: {:.1} µs", clock.elapsed().as_secs_f64() * 1e6 / n as f64);
    };
    time("aero_forces", &mut || {
        let flow = Flow { dir, q: 20e3, mach: 12.0, knudsen: 1e-4, gamma: 1.4 };
        sink += aero_forces(&b, &flow, 0.8).0;
    });
    let input = HeatInput { flow_dir: dir, q_stag: 1e6, sun: DVec3::new(1361.0, 0.0, 0.0) };
    time("cell_heat (flow + sun)", &mut || cell_heat(&b, &c.cells.cells, &input, &mut exposure, &mut heat));
    let mut sweeps = 0;
    time("thermal step (20 ms)", &mut || {
        sweeps = net.step(&mut state, &heat, 0.0, 3.0, 0.02, sim::thermal::DEFAULT_SWEEPS)
    });
    println!("  sweeps per 20 ms step: {sweeps}");
    for dt in [60.0, 600.0] {
        let mut s = state.clone();
        println!("  sweeps per {dt} s step: {}", net.step(&mut s, &heat, 0.0, 3.0, dt, sim::thermal::DEFAULT_SWEEPS));
    }
    println!("({sink:.0?})");
}
