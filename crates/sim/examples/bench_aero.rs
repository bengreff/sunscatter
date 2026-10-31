//! Timing of the aerodynamic bake and the per-tick aero and thermal work
//! for the test craft (release): `cargo run -p sim --release --example bench_aero`;
//! `… bench_aero cells` times the per-cell work (forces, the boundary layer,
//! whole-body heating) and the bake on the test craft and stress craft of
//! 50 and 300 parts (`sim::craft::stress`); `… bench_aero cells 1000` adds
//! larger ones.
use glam::DVec3;
use sim::aero::{aero_forces, bake, BakeOptions, Flow};
use sim::craft::small;
use sim::thermal::heating::SUTTON_GRAVES_EARTH;
use sim::thermal::{cell_heat_flow, FlowHeat, ThermalState};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("cells") {
        let mut sizes = vec![50, 300];
        sizes.extend(args.iter().skip(1).filter_map(|a| a.parse::<usize>().ok()));
        per_cell("small rocket", &small::rocket_ref().surface, &small::rocket_ref().cells);
        let one = sim::craft::stress::craft(1);
        per_cell("stress craft, 1 part", &one.surface, &one.cells);
        for n in sizes {
            let s = sim::craft::stress::craft(n);
            per_cell(&format!("stress craft, {n} parts"), &s.surface, &s.cells);
        }
        return;
    }
    let c = small::rocket_ref();
    println!("small rocket: {} triangles, {} cells", c.surface.triangles.len(), c.cells.cells.len());
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
    d.node_capacity(&sim::craft::design::Aboard::Fill(c.spec.propellant.capacity), &mut cap);
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
        let flow = Flow {
            dir,
            q: 20e3,
            mach: 12.0,
            knudsen: 1e-4,
            gamma: 1.4,
            reynolds_per_m: 1e6,
            temperature: 230.0,
            speed: 3600.0,
            gas: Default::default(),
            roughness: 0.0,
        };
        let (f, m) = aero_forces(&b, &flow, 0.8);
        sink += f + m;
        let air = FlowHeat { flow: &flow, sutton_graves_k: SUTTON_GRAVES_EARTH, radiative: None, wall: &state.skin };
        cell_heat_flow(&b, &c.cells.cells, Some(&air), DVec3::new(1361.0, 0.0, 0.0), &mut exposure, &mut heat);
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
        let flow = Flow {
            dir,
            q: 20e3,
            mach: 12.0,
            knudsen: 1e-4,
            gamma: 1.4,
            reynolds_per_m: 1e6,
            temperature: 230.0,
            speed: 3600.0,
            gas: Default::default(),
            roughness: 0.0,
        };
        sink += aero_forces(&b, &flow, 0.8).0;
    });
    let flow = Flow {
        dir,
        q: 20e3,
        mach: 12.0,
        knudsen: 1e-4,
        gamma: 1.4,
        reynolds_per_m: 1e6,
        temperature: 230.0,
        speed: 3600.0,
        gas: Default::default(),
        roughness: 0.0,
    };
    let wall = state.skin.clone();
    let air = FlowHeat { flow: &flow, sutton_graves_k: SUTTON_GRAVES_EARTH, radiative: None, wall: &wall };
    let sun = DVec3::new(1361.0, 0.0, 0.0);
    time("cell_heat_flow (flow + sun)", &mut || {
        cell_heat_flow(&b, &c.cells.cells, Some(&air), sun, &mut exposure, &mut heat)
    });
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
    let mut ship = Vessel::coasting(&w, VesselId(1), t0, earth.node, r, v, small::rocket_ref());
    ship.set_debug(&w, true);
    let controls = Controls { sas: true, ..Default::default() };
    let secs = 60.0;
    let clock = Instant::now();
    ship.advance(&w, t0.add_seconds(secs), &controls, usize::MAX);
    let us = clock.elapsed().as_secs_f64() * 1e6 / (secs / 0.02);
    println!("vessel live tick in the atmosphere (aero, heating, integration): {us:.1} µs");
    // Ascent through the turning air at 12 km, M 1.2 (turbulent): the test
    // craft and a 300-part stress craft (`… bench_aero ticks`).
    if std::env::args().any(|a| a == "ticks") {
        let stress = sim::craft::stress::craft(300);
        for (name, craft) in [("small rocket", small::rocket_ref()), ("stress craft, 300 parts", &*stress)] {
            let r = DVec3::new(re + 12_000.0, 0.0, 0.0);
            let v =
                sim::body::earth().rotation.omega(t0).raw().cross(r) + DVec3::new(0.3, 1.0, 0.0).normalize() * 354.0;
            let mut ship = Vessel::coasting(&w, VesselId(2), t0, earth.node, r, v, craft);
            ship.set_debug(&w, true);
            ship.advance(&w, t0.add_seconds(0.02), &controls, usize::MAX);
            let secs = 2.0;
            let clock = Instant::now();
            ship.advance(&w, t0.add_seconds(0.02 + secs), &controls, usize::MAX);
            let us = clock.elapsed().as_secs_f64() * 1e6 / (secs / 0.02);
            println!("ascent tick, 12 km, M 1.2, {name}: {us:.0} µs");
        }
    }
    let (r, v) = (DVec3::new(re + 400_000.0, 0.0, 0.0), DVec3::new(0.0, (earth.gm / (re + 400_000.0)).sqrt(), 0.0));
    let mut ship = Vessel::coasting(&w, VesselId(1), t0, earth.node, r, v, small::rocket_ref());
    ship.set_attitude(Attitude { q: sim::vessel::quat_z_to(DVec3::X), omega: DVec3::new(0.01, 0.02, 0.0) });
    let secs = 86_400.0;
    let clock = Instant::now();
    ship.advance(&w, t0.add_seconds(secs), &Controls::default(), usize::MAX);
    let us = clock.elapsed().as_secs_f64() * 1e6 / (secs / sim::vessel::LATTICE);
    println!("coast in LEO, tumbling, per lattice point (integration, attitude, sunlight, thermal): {us:.1} µs");
}

/// The free stream of a flight condition: altitude (m), airspeed (m/s),
/// direction (body axes) on Earth (US 1976), Knudsen number on `length`.
fn earth_flow(altitude: f64, v: f64, dir: DVec3, length: f64) -> Flow {
    let atm = sim::body::earth().atmosphere.unwrap();
    let a = atm.profile(altitude, sim::aero::air::G0);
    Flow {
        dir: dir.normalize(),
        q: 0.5 * a.rho * v * v,
        mach: v / a.speed_of_sound(1.4),
        knudsen: a.mean_free_path / length,
        gamma: 1.4,
        reynolds_per_m: a.rho * v / sim::aero::gas::Gas::Air.viscosity(a.temperature),
        temperature: a.temperature,
        speed: v,
        gas: Default::default(),
        roughness: 0.0,
    }
}

/// Per-call and per-cell cost of the aerodynamics and heating on one craft.
fn per_cell(name: &str, surface: &sim::craft::Surface, cells: &sim::craft::Cells) {
    let n = cells.cells.len();
    let clock = Instant::now();
    let b = bake(surface, cells, &BakeOptions::default());
    let bake_ms = clock.elapsed().as_secs_f64() * 1e3;
    let clock = Instant::now();
    bake(surface, cells, &BakeOptions { level: 2, ..BakeOptions::default() });
    let coarse_ms = clock.elapsed().as_secs_f64() * 1e3;
    let clock = Instant::now();
    bake(surface, cells, &BakeOptions { level: 2, raster: 32 });
    println!("  (162 directions, 32 × 32 pixels: {:.1} ms)", clock.elapsed().as_secs_f64() * 1e3);
    let tris = surface.triangles.len();
    println!(
        "{name}: {tris} triangles, {n} cells, {} fins; bake {bake_ms:.0} ms (162 directions: {coarse_ms:.0} ms)",
        b.fins.len()
    );
    let wall = vec![900.0; n];
    let mut heat = vec![0.0; n];
    let mut scratch = vec![0.0; n];
    let dir = DVec3::new(0.05, 0.03, -1.0);
    let cases = [
        ("entry, 70 km, 7.4 km/s (laminar)", earth_flow(70e3, 7400.0, dir, b.length)),
        ("entry, 45 km, 3 km/s (turbulent aft)", earth_flow(45e3, 3000.0, dir, b.length)),
        ("ascent, 12 km, M 1.2", earth_flow(12e3, 354.0, dir, b.length)),
        ("low, 1 km, M 0.3", earth_flow(1e3, 100.0, dir, b.length)),
    ];
    let mut sink = 0.0;
    for (what, flow) in cases {
        let reps = (2_000_000 / n).max(20);
        let time = |f: &mut dyn FnMut()| {
            let clock = Instant::now();
            for _ in 0..reps {
                f();
            }
            clock.elapsed().as_secs_f64() * 1e6 / reps as f64
        };
        let adiabatic = time(&mut || sink += aero_forces(&b, &flow, 0.8).0.x);
        let walled = time(&mut || sink += sim::aero::aero_forces_wall(&b, &flow, 0.8, Some(&wall)).0.x);
        let stream = sim::aero::boundary::Stream::of(&flow);
        let d = flow.dir;
        let (e, relief) = (b.exposure_weights(d), b.relief_at(d));
        let friction = time(&mut || {
            sink +=
                sim::aero::friction::friction(&b.geometry, d, &stream, |i| e.at(i), |x| relief.at(x), Some(&wall)).0.x
        });
        let air = sim::thermal::FlowHeat {
            flow: &flow,
            sutton_graves_k: sim::thermal::heating::SUTTON_GRAVES_EARTH,
            radiative: Some(sim::thermal::TauberSutton::Earth),
            wall: &wall,
        };
        let heating = time(&mut || {
            sim::thermal::cell_heat_flow(&b, &cells.cells, Some(&air), DVec3::X * 1361.0, &mut scratch, &mut heat);
            sink += heat[0];
        });

        let ns = |us: f64| us * 1e3 / n as f64;
        println!(
            "  {what}: aero_forces {adiabatic:.0} µs ({:.0} ns/cell), with skin temperatures {walled:.0} µs ({:.0}), \
             of it friction {friction:.0} µs ({:.0}); cell_heat_flow {heating:.0} µs ({:.0})",
            ns(adiabatic),
            ns(walled),
            ns(friction),
            ns(heating)
        );
        println!(
            "    a live tick's aerothermal share (5 force evaluations, one heating): {:.0} µs",
            5.0 * walled + heating
        );
    }
    println!("  ({sink:.0})");
}
