//! Timing of the aerodynamic bake and the per-tick aero and thermal work
//! for the test craft (release): `cargo run -p sim --release --example bench_aero`
use glam::DVec3;
use sim::aero::{aero_forces, bake, BakeOptions, Flow};
use sim::craft::test_craft;
use sim::thermal::{cell_heat, HeatInput, ThermalState};
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
    // The design's network: skin cells and interior volume nodes (full tank).
    let d = c.design();
    let net = &d.network;
    let mut cap = vec![0.0; d.nodes()];
    d.node_capacity(c.spec.propellant.capacity, &mut cap);
    let node_heat = vec![0.0; d.nodes()];
    println!("interior: {} volume nodes", d.nodes());
    let mut state = ThermalState::uniform(c.cells.cells.len(), d.nodes(), 290.0);
    let mut heat = vec![0.0; c.cells.cells.len()];
    let mut exposure = vec![0.0; c.cells.cells.len()];
    let n = 10_000;
    let clock = Instant::now();
    let mut sink = DVec3::ZERO;
    for i in 0..n {
        let a = i as f64 * 1e-3;
        let dir = DVec3::new(sim::math::sin(a) * 0.2, 0.1, -1.0).normalize();
        let flow =
            Flow { dir, q: 20e3, mach: 12.0, knudsen: 1e-4, gamma: 1.4, reynolds_per_m: 1e6, temperature: 230.0 };
        let (f, m) = aero_forces(&b, &flow, 0.8);
        sink += f + m;
        let input = HeatInput { flow_dir: dir, q_stag: 1e6, sun: DVec3::new(1361.0, 0.0, 0.0) };
        cell_heat(&b, &c.cells.cells, &input, &mut exposure, &mut heat);
        net.step(&mut state, &heat, &node_heat, &cap, 3.0, 0.02, sim::thermal::DEFAULT_SWEEPS);
    }
    let us = clock.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("per tick (aero forces + cell heating + thermal step, 512 cells + nodes): {us:.1} µs  ({sink:.0?})");
    let dir = DVec3::new(0.1, 0.1, -1.0).normalize();
    let time = |what: &str, f: &mut dyn FnMut()| {
        let clock = Instant::now();
        for _ in 0..n {
            f();
        }
        println!("  {what}: {:.1} µs", clock.elapsed().as_secs_f64() * 1e6 / n as f64);
    };
    time("aero_forces", &mut || {
        let flow =
            Flow { dir, q: 20e3, mach: 12.0, knudsen: 1e-4, gamma: 1.4, reynolds_per_m: 1e6, temperature: 230.0 };
        sink += aero_forces(&b, &flow, 0.8).0;
    });
    let input = HeatInput { flow_dir: dir, q_stag: 1e6, sun: DVec3::new(1361.0, 0.0, 0.0) };
    time("cell_heat (flow + sun)", &mut || cell_heat(&b, &c.cells.cells, &input, &mut exposure, &mut heat));
    let mut sweeps = 0;
    time("thermal step (20 ms)", &mut || {
        sweeps = net.step(&mut state, &heat, &node_heat, &cap, 3.0, 0.02, sim::thermal::DEFAULT_SWEEPS)
    });
    println!("  sweeps per 20 ms step: {sweeps}");
    for dt in [60.0, 600.0] {
        let mut s = state.clone();
        println!(
            "  sweeps per {dt} s step: {}",
            net.step(&mut s, &heat, &node_heat, &cap, 3.0, dt, sim::thermal::DEFAULT_SWEEPS)
        );
    }
    println!("({sink:.0?})");
    vessel_ticks();
}

/// A whole live tick of a vessel entering (100 km, 7.6 km/s), and a
/// coast's thermal lattice in low orbit (eclipses, tumbling: no skips).
fn vessel_ticks() {
    use sim::vessel::{Attitude, Controls, Vessel, VesselId};
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sim::sol::EPHEMERIS_PATH);
    let eph = sim::ephem::Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let w = sim::world::World::sol(std::sync::Arc::new(eph));
    let earth = w.find("Earth").unwrap();
    let t0 = sim::sol::sol_epoch();
    let re = 6_378_137.0;
    let g = 1.5f64.to_radians();
    let (r, v) = (DVec3::new(re + 100_000.0, 0.0, 0.0), DVec3::new(-g.sin(), g.cos(), 0.0) * 7_600.0);
    let mut ship = Vessel::coasting(&w, VesselId(1), t0, earth.node, r, v, test_craft());
    ship.set_debug(&w, true);
    let controls = Controls { sas: true, ..Default::default() };
    let secs = 60.0;
    let clock = Instant::now();
    ship.advance(&w, t0.add_seconds(secs), &controls, usize::MAX);
    let us = clock.elapsed().as_secs_f64() * 1e6 / (secs / 0.02);
    println!("vessel live tick in the atmosphere (aero, heating, integration): {us:.1} µs");
    let (r, v) = (DVec3::new(re + 400_000.0, 0.0, 0.0), DVec3::new(0.0, (earth.gm / (re + 400_000.0)).sqrt(), 0.0));
    let mut ship = Vessel::coasting(&w, VesselId(1), t0, earth.node, r, v, test_craft());
    ship.set_attitude(Attitude { q: sim::vessel::quat_z_to(DVec3::X), omega: DVec3::new(0.01, 0.02, 0.0) });
    let secs = 86_400.0;
    let clock = Instant::now();
    ship.advance(&w, t0.add_seconds(secs), &Controls::default(), usize::MAX);
    let us = clock.elapsed().as_secs_f64() * 1e6 / (secs / sim::vessel::LATTICE);
    println!("coast in LEO, tumbling, per lattice point (integration, attitude, sunlight, thermal): {us:.1} µs");
}
