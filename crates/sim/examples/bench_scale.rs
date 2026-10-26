//! Scale baseline (D081; results and reading in
//! `docs/plans/scale-baseline-2026-10-26.md`): what the craft pipeline and
//! the game's frame loop cost as craft grow to 1,000 parts (synthetic stress
//! craft, `sim::craft::stress`) and fleets to 50 craft. Release only:
//!
//! `cargo run -p sim --release --example bench_scale [craft] [fleet] [parts=1,50,300,1000] [fleets=1,10,50] [runs=3]`
//!
//! No section named: both. Each timing is the median of `runs` runs.
//! `profile=<bake|design|build|live|contact|lattice|fleet|settled>,<parts>` repeats
//! one measurement for about 30 s, for a sampling profiler (`sample <pid>`).
use glam::DVec3;
use sim::aero::{aero_forces, bake, BakeOptions, Flow};
use sim::craft::design::{mean_drag_area, CraftDesign};
use sim::craft::stress::{self, layout, StressCraft};
use sim::craft::volume::VolumeGrid;
use sim::craft::{cells, mesh, test_craft, CellOptions, Craft, Resolution};
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::thermal::heating::SUTTON_GRAVES_EARTH;
use sim::thermal::{cell_heat_flow, FlowHeat, ThermalState, COAST_CONVERGED, DEFAULT_SWEEPS};
use sim::time::Epoch;
use sim::vessel::{quat_z_to, Attitude, Controls, Phase, Vessel, VesselId, LATTICE, TICK};
use sim::world::World;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The game's budgets and look-ahead (`crates/game/src/state.rs`).
const COAST_STEPS_PER_FRAME: usize = 1_200;
const LOOKAHEAD_STEPS_PER_FRAME: usize = 1_500;
const OTHER_LOOKAHEAD_STEPS_PER_FRAME: usize = 300;
/// Look-ahead span (s): about one low orbit, as `bench_fleet`.
const LOOKAHEAD: f64 = 6_000.0;
/// Threads of Bevy's compute pool on a 10-core machine (io 3, async 3,
/// compute the remaining 4).
const POOL: usize = 4;
const RE: f64 = 6_378_137.0;
/// The game's pad (`crates/game/src/state.rs`).
const PAD: (f64, f64) = (28.6082, -80.66);

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(86_400.0)
}

/// Runs `f` and returns its result and wall time (ms).
fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let clock = Instant::now();
    let out = f();
    (out, clock.elapsed().as_secs_f64() * 1e3)
}

/// Median over `runs` runs of `f` (which returns a time).
fn median(runs: usize, mut f: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..runs.max(1)).map(|_| f()).collect();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Cells whose triangles' normals nearly cancel (|Σ n·a| < ½ Σ a: a tube
/// made one cell) and their share of the area.
fn tubes(surface: &sim::craft::Surface, cells: &sim::craft::Cells) -> (usize, f64) {
    let mut sums = vec![(DVec3::ZERO, 0.0); cells.cells.len()];
    for t in 0..surface.triangles.len() {
        let (_, n, a) = surface.triangle(t);
        let s = &mut sums[cells.tri_cell[t] as usize];
        *s = (s.0 + n * a, s.1 + a);
    }
    let total: f64 = sums.iter().map(|s| s.1).sum();
    let bad: Vec<_> = sums.iter().filter(|(v, a)| v.length() < 0.5 * a).collect();
    (bad.len(), bad.iter().map(|s| s.1).sum::<f64>() / total)
}

/// Heap bytes of a design (arrays by length × element size; the volume
/// grid's private index and the network's private node arrays estimated
/// from their counts).
fn design_bytes(d: &CraftDesign) -> usize {
    let v = |n: usize, size: usize| n * size;
    let b = &d.bake;
    let neighbours: usize = d.cells.iter().map(|c| c.neighbours.len()).sum();
    let fins: usize = b.fins.iter().map(|f| f.cells.len()).sum();
    let tank: usize = d.volume.nodes.iter().map(|n| n.tank.len()).sum();
    let (cells, nodes, links) = (d.cells.len(), d.nodes(), d.volume.links.len());
    let bake = b.exposure.len()
        + v(b.geometry.len(), std::mem::size_of::<sim::aero::CellGeometry>())
        + v(b.grid.dirs.len(), 24 + 3 * 8)
        + v(b.grid.faces.len(), 12 + 8)
        + v(b.sections.area.len() + fins, 8);
    let network = v(cells, 3 * 8 + 4 + 4 + 4 + 4) + v(neighbours, 16) + v(2 * links, 16) + v(nodes, 4);
    let volume = v(nodes, std::mem::size_of::<sim::craft::VolumeNode>() + 40) + v(tank, 8) + v(links, 16);
    let rest = v(cells, std::mem::size_of::<sim::craft::Cell>() + 8) + v(neighbours, 16) + v(nodes, 8);
    // The limbo surface table: 6 cube faces of 17² entries of 8 f64.
    let surfaces = 6 * 17 * 17 * 64;
    bake + network + volume + rest + surfaces
}

/// Heap bytes of a loaded craft (surface, render mesh, cells).
fn craft_bytes(c: &Craft) -> usize {
    let s = &c.surface;
    let neighbours: usize = c.cells.cells.iter().map(|c| c.neighbours.len()).sum();
    s.positions.len() * 48
        + s.triangles.len() * (12 + 4 + 4)
        + s.patches.len() * 4
        + c.mesh.positions.len() * 24
        + c.mesh.indices.len() * 4
        + c.cells.cells.len() * std::mem::size_of::<sim::craft::Cell>()
        + neighbours * 16
}

/// Heap bytes of a vessel (thermal state, trajectory samples, contacts;
/// the design is shared) and its save size (RON).
fn vessel_bytes(v: &Vessel) -> (usize, usize) {
    let samples: usize = v.trajectory().map_or(0, |t| t.segments.iter().map(|s| s.samples.len()).sum());
    let th = &v.thermal().state;
    let heap = std::mem::size_of::<Vessel>()
        + 8 * (th.skin.len() + th.nodes.len())
        + samples * std::mem::size_of::<sim::vessel::Sample>()
        + v.craft.contacts.len() * std::mem::size_of::<sim::craft::ContactPoint>();
    (heap, ron::to_string(v).map_or(0, |s| s.len()))
}

/// A vessel of `craft` in a circular orbit of altitude `h` (m), inclination `i`.
fn orbiting(w: &World, craft: &Craft, id: u64, h: f64, i: f64, phase: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let el = Elements { a: RE + h, e: 0.001, i, raan: phase, argp: 0.0, mean_anomaly: 2.0 * phase };
    let (r, v) = el.to_state(earth.gm);
    Vessel::coasting(w, VesselId(id), t0(), earth.node, r, v, craft)
}

/// On a lunar transfer: perigee 200 km, apogee 400,000 km.
fn transfer(w: &World, craft: &Craft, id: u64, phase: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (rp, ra) = (RE + 200e3, 4.0e8);
    let el =
        Elements { a: 0.5 * (rp + ra), e: (ra - rp) / (ra + rp), i: 0.5, raan: phase, argp: 0.0, mean_anomaly: 0.0 };
    let (r, v) = el.to_state(earth.gm);
    Vessel::coasting(w, VesselId(id), t0(), earth.node, r, v, craft)
}

fn tumble(v: &mut Vessel, w: &World) {
    let (_, r, _) = v.state(w);
    v.set_attitude(Attitude { q: quat_z_to(r.normalize()), omega: DVec3::new(0.01, 0.02, 0.0) });
}

/// The live scenarios: (vessel, controls, seconds flown).
fn live_case(w: &World, craft: &Craft, case: usize, secs: f64) -> (Vessel, Controls, f64) {
    let earth = w.find("Earth").unwrap();
    let sas = Controls { sas: true, ..Default::default() };
    let at = |h: f64, v: DVec3| Vessel::coasting(w, VesselId(1), t0(), earth.node, DVec3::X * (RE + h), v, craft);
    let mut out = match case {
        // Entry: 100 km, 7.6 km/s inertial (7.1 through the air, M 25),
        // 1.5° down (hypersonic, partly rarefied).
        0 => (at(100e3, DVec3::new(-0.026, 0.9997, 0.0) * 7_600.0), sas, secs),
        // Ascent: 12 km, 354 m/s through the turning air (M 1.2), climbing
        // (wave drag, slender body, fins).
        1 => {
            let rotation = earth.physical.as_ref().unwrap().rotation;
            let air = rotation.omega(t0()).raw().cross(DVec3::X * (RE + 12e3));
            (at(12e3, air + DVec3::new(100.0, 340.0, 0.0)), sas, secs)
        }
        // Vacuum: 400 km, engine at full thrust.
        2 => {
            let v = DVec3::Y * (earth.gm / (RE + 400e3)).sqrt();
            let mut s = at(400e3, v);
            s.set_propellant(w, s.craft.mass.capacity);
            (s, Controls { throttle: 1.0, ..sas }, secs)
        }
        // Pad: engine at minimum throttle, held down (contact substeps).
        _ => {
            let s = Vessel::landed_at(w, VesselId(1), "Earth", PAD.0, PAD.1, t0(), craft);
            (s, Controls { throttle: 0.1, ..sas }, secs)
        }
    };
    out.0.set_debug(w, true);
    out
}

/// Live tick cost (µs) of scenario `case`.
fn live_tick(w: &World, craft: &Craft, case: usize, secs: f64) -> f64 {
    let (mut v, controls, secs) = live_case(w, craft, case, secs);
    let ((), ms) = timed(|| {
        v.advance(w, t0().add_seconds(secs), &controls, usize::MAX);
    });
    assert!(matches!(v.phase, Phase::Powered { .. }), "case {case}: not live at the end: {:?}", v.phase);
    ms * 1e3 / (secs / TICK)
}

/// Per call (µs): aero forces (M 12 and M 1.2), cell heating, a 20 ms
/// thermal step.
fn pieces(d: &CraftDesign, propellant: f64, n: usize) -> [f64; 4] {
    let dir = DVec3::new(0.1, 0.1, -1.0).normalize();
    let flow = |mach: f64| Flow {
        dir,
        q: 20e3,
        mach,
        knudsen: 1e-4,
        gamma: 1.4,
        reynolds_per_m: 1e6,
        temperature: 230.0,
        speed: 300.0 * mach,
        gas: Default::default(),
        roughness: 0.0,
    };
    let per = |f: &mut dyn FnMut()| {
        let (_, ms) = timed(|| (0..n).for_each(|_| f()));
        ms * 1e3 / n as f64
    };
    let mut sink = DVec3::ZERO;
    let hyper = per(&mut || sink += aero_forces(&d.bake, &flow(12.0), 0.45).0);
    let trans = per(&mut || sink += aero_forces(&d.bake, &flow(1.2), 0.45).0);
    let (c, m) = (d.cells.len(), d.nodes());
    let (mut heat, mut scratch) = (vec![0.0; c], vec![0.0; c]);
    // Whole-body heating at the hypersonic flow's conditions (D085), the
    // skin at 900 K.
    let wall = vec![900.0; c];
    let hot = flow(12.0);
    let air = FlowHeat { flow: &hot, sutton_graves_k: SUTTON_GRAVES_EARTH, radiative: None, wall: &wall };
    let sun = DVec3::new(1361.0, 0.0, 0.0);
    let heating = per(&mut || cell_heat_flow(&d.bake, &d.cells, Some(&air), sun, &mut scratch, &mut heat));
    let mut cap = vec![0.0; m];
    d.node_capacity(propellant, &mut cap);
    let mut state = ThermalState::uniform(c, m, 290.0);
    let node_heat = vec![0.0; m];
    let step = per(&mut || {
        d.network.step(&mut state, &heat, &node_heat, &cap, 2.725, TICK, DEFAULT_SWEEPS);
    });
    std::hint::black_box(sink);
    [hyper, trans, heating, step]
}

/// Coast costs of one craft.
#[derive(Default)]
struct Coast {
    /// µs per coast integration step (LEO).
    step: f64,
    /// A 600 s lattice solve from 290 K in sunlight, and the 10th in a row:
    /// (µs, sweeps).
    first: (f64, usize),
    tenth: (f64, usize),
    /// A tumbling day in LEO per lattice point (µs; integrated ahead).
    tumbling: f64,
    /// 10⁶ s in LEO holding attitude (ms; integrated ahead).
    holding: f64,
}

fn coast(w: &World, craft: &Craft, runs: usize) -> Coast {
    let samples = |v: &Vessel| v.trajectory().map_or(0, |t| t.segments.iter().map(|s| s.samples.len()).sum::<usize>());
    let step = median(runs, || {
        let mut v = orbiting(w, craft, 1, 400e3, 0.9, 0.0);
        let before = samples(&v);
        let ((), ms) = timed(|| v.extend_coast(w, t0().add_seconds(30.0 * 86_400.0), 20_000));
        ms * 1e3 / (samples(&v) - before).max(1) as f64
    });
    let d = craft.design();
    let (c, m) = (d.cells.len(), d.nodes());
    let (mut heat, mut scratch) = (vec![0.0; c], vec![0.0; c]);
    cell_heat_flow(&d.bake, &d.cells, None, DVec3::new(1361.0, 0.0, 0.0), &mut scratch, &mut heat);
    let mut cap = vec![0.0; m];
    d.node_capacity(craft.spec.propellant.capacity, &mut cap);
    let node_heat = vec![0.0; m];
    let mut state = ThermalState::uniform(c, m, 290.0);
    let mut solve = || {
        let (sweeps, ms) = timed(|| {
            d.network.step_to(&mut state, &heat, &node_heat, &cap, 2.725, LATTICE, DEFAULT_SWEEPS, COAST_CONVERGED)
        });
        (ms * 1e3, sweeps)
    };
    let first = solve();
    let tenth = (0..9).map(|_| solve()).last().unwrap();
    let lattice = |hold: bool, secs: f64| {
        median(runs, || {
            let mut v = orbiting(w, craft, 1, 400e3, 0.9, 0.3);
            if !hold {
                tumble(&mut v, w);
            }
            // One second first: the coast restarts on the pointing the
            // controls give (perturbed coasts); then integrated ahead.
            let controls = Controls { sas: hold, ..Default::default() };
            v.advance(w, t0().add_seconds(1.0), &controls, usize::MAX);
            v.extend_coast(w, t0().add_seconds(secs + 7_200.0), usize::MAX);
            timed(|| v.advance(w, t0().add_seconds(secs), &controls, usize::MAX)).1
        })
    };
    let tumbling = lattice(false, 86_401.0) * 1e3 / (86_400.0 / LATTICE);
    Coast { step, first, tenth, tumbling, holding: lattice(true, 1e6 + 1.0) }
}

/// Everything measured for one part count.
#[derive(Default)]
struct Row {
    parts: usize,
    primitives: usize,
    triangles: usize,
    cells: usize,
    tubes: (usize, f64),
    /// With the default cell options (512): cells and tubes.
    default_cells: (usize, usize, f64),
    nodes: usize,
    links: usize,
    contacts: (usize, usize),
    fins: usize,
    size: (f64, f64),
    /// Shell mass model's dry centre against the parts' (m).
    shell_offset: f64,
    /// ms: union surface, whole build (union + cells + mass), aero bake,
    /// volume grid, whole design, mean drag area.
    times: [f64; 6],
    /// KB: design, craft, vessel, save.
    memory: [f64; 4],
    /// µs per live tick: entry, ascent, vacuum burn, pad.
    live: [f64; 4],
    /// µs: aero forces M 12, M 1.2, cell heating, 20 ms thermal step.
    pieces: [f64; 4],
    coast: Coast,
}

fn craft_row(w: &World, n: usize, runs: usize) -> Row {
    let res = Resolution::default();
    let opts = stress::cell_options(n);
    let l = layout(n);
    let union = median(runs, || timed(|| mesh::union_surface(&l.geometry.primitives, &res)).1);
    let mut built: Option<StressCraft> = None;
    let build = median(runs, || {
        let (s, ms) = timed(|| layout(n).build(&res, &opts));
        built = Some(s);
        ms
    });
    let s = built.unwrap();
    let c = &s.craft;
    let defaults = cells::build_cells(&c.surface, &c.geometry.primitives, &c.geometry.skin, &CellOptions::default());
    let default_tubes = tubes(&c.surface, &defaults);
    let (lo, hi) = c
        .surface
        .positions
        .iter()
        .fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |b, p| (b.0.min(*p), b.1.max(*p)));
    let bake_ms = median(runs, || timed(|| bake(&c.surface, &c.cells, &BakeOptions::default())).1);
    let t = &c.spec.thermal;
    let volume =
        median(runs, || timed(|| VolumeGrid::new(&c.geometry.primitives, (lo, hi), &c.geometry.tank, t.node_size)).1);
    let design_ms = median(runs, || timed(|| CraftDesign::new(c)).1);
    let d = c.design();
    let drag = median(runs, || timed(|| mean_drag_area(&d.bake, d.cd0)).1);
    let secs = if n >= 300 { 4.0 } else { 20.0 };
    let live = [0, 1, 2, 3].map(|case| median(runs, || live_tick(w, c, case, secs)));
    let v = orbiting(w, c, 1, 400e3, 0.9, 0.0);
    let (vessel, save) = vessel_bytes(&v);
    let feet = c.cells.contacts.iter().filter(|p| p.kind == sim::craft::ContactKind::Foot).count();
    let kb = |b: usize| b as f64 / 1024.0;
    Row {
        parts: n,
        primitives: c.geometry.primitives.len(),
        triangles: c.surface.triangles.len(),
        cells: c.cells.cells.len(),
        tubes: tubes(&c.surface, &c.cells),
        default_cells: (defaults.cells.len(), default_tubes.0, default_tubes.1),
        nodes: d.nodes(),
        links: d.volume.links.len(),
        contacts: (feet, c.cells.contacts.len() - feet),
        fins: d.bake.fins.len(),
        size: (hi.z - lo.z, (hi - lo).truncate().max_element()),
        shell_offset: (c.mass.dry_com - stress::volume_com(&c.def())).length(),
        times: [union, build, bake_ms, volume, design_ms, drag],
        memory: [kb(design_bytes(d)), kb(craft_bytes(c)), kb(vessel), kb(save)],
        live,
        pieces: pieces(d, c.spec.propellant.capacity, if n >= 300 { 200 } else { 2_000 }),
        coast: coast(w, c, runs),
    }
}

fn print_rows(rows: &[Row]) {
    println!("\n| parts | primitives | triangles | cells (tubes, area) | default 512: cells (tubes, area) | nodes | node links | feet + hull contacts | fins | height × width (m) | shell CoM off (m) |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for r in rows {
        let (dc, dt, da) = r.default_cells;
        println!(
            "| {} | {} | {} | {} ({}, {:.1} %) | {dc} ({dt}, {:.1} %) | {} | {} | {} + {} | {} | {:.1} × {:.1} | {:.2} |",
            r.parts, r.primitives, r.triangles, r.cells, r.tubes.0, 100.0 * r.tubes.1, 100.0 * da, r.nodes,
            r.links, r.contacts.0, r.contacts.1, r.fins, r.size.0, r.size.1, r.shell_offset
        );
    }
    println!("\n| parts | union surface | build (union + cells + mass) | aero bake | volume grid | design (bake + grid + network + drag) | mean drag area | design KB | craft KB | vessel KB | save KB |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for r in rows {
        let t = r.times;
        let m = r.memory;
        println!(
            "| {} | {:.1} ms | {:.1} ms | {:.1} ms | {:.1} ms | {:.1} ms | {:.1} ms | {:.0} | {:.0} | {:.1} | {:.1} |",
            r.parts, t[0], t[1], t[2], t[3], t[4], t[5], m[0], m[1], m[2], m[3]
        );
    }
    println!("\n| parts | live tick: entry | ascent M 1.2 | vacuum burn | pad (contact) | aero forces M 12 | M 1.2 | cell heating | thermal step 20 ms |");
    println!("|---|---|---|---|---|---|---|---|---|");
    for r in rows {
        let (l, p) = (r.live, r.pieces);
        println!(
            "| {} | {:.0} µs | {:.0} µs | {:.0} µs | {:.0} µs | {:.1} µs | {:.1} µs | {:.1} µs | {:.1} µs |",
            r.parts, l[0], l[1], l[2], l[3], p[0], p[1], p[2], p[3]
        );
    }
    println!("\n| parts | coast step | lattice solve 600 s, first (sweeps) | tenth (sweeps) | tumbling day, per lattice point | 10⁶ s holding attitude |");
    println!("|---|---|---|---|---|---|");
    for r in rows {
        let c = &r.coast;
        println!(
            "| {} | {:.2} µs | {:.0} µs ({}) | {:.0} µs ({}) | {:.0} µs | {:.1} ms |",
            r.parts, c.step, c.first.0, c.first.1, c.tenth.0, c.tenth.1, c.tumbling, c.holding
        );
    }
}

/// A fleet of `size` (1, 10 or 50): vessels and their controls. Limbo
/// stand-ins (10 when size ≥ 10): tumbling with SAS off (torque-free);
/// half 300-part craft, two on lunar transfers. Quiet craft hold attitude
/// with SAS on (the game's passive controls): test craft and 300-part craft
/// in LEO, four on lunar transfers, one test craft on the pad. Vessel 0
/// (300 parts, LEO) is active. `high`: 20,000 km circular orbits instead of
/// the lunar transfers (nothing re-enters while a fleet settles).
fn fleet(w: &World, size: usize, big: &Craft, high: bool) -> (Vec<Vessel>, Vec<Controls>) {
    let small = test_craft();
    let (mut ships, mut controls) = (Vec::new(), Vec::new());
    for k in 0..size {
        let limbo = size >= 10 && k < 10;
        let craft = if limbo { [big, small][k % 2] } else { [small, big][usize::from((30..45).contains(&k))] };
        let craft = if size == 1 { big } else { craft };
        let id = k as u64 + 1;
        let leo = |c: &Craft| orbiting(w, c, id, 300e3 + 5e3 * (k % 40) as f64, 0.1 + 0.02 * k as f64, 0.37 * k as f64);
        let far = |c: &Craft| {
            if high {
                orbiting(w, c, id, 20_000e3, 0.5, k as f64)
            } else {
                transfer(w, c, id, k as f64)
            }
        };
        let mut v = match k {
            8 | 9 if limbo => far(craft),
            45..=48 => far([small, big][k % 2]),
            49 => Vessel::landed_at(w, VesselId(id), "Earth", PAD.0, PAD.1, t0(), small),
            _ => leo(craft),
        };
        if limbo {
            tumble(&mut v, w);
        }
        controls.push(Controls { sas: !limbo, ..Default::default() });
        ships.push(v);
    }
    (ships, controls)
}

/// Whether a vessel's motion ended for good (`state.rs` `stopped`).
fn stopped(v: &Vessel) -> bool {
    matches!(&v.phase, Phase::Coasting { trajectory } if trajectory.finished())
}

/// What a run of the frame loop measured.
#[derive(Default)]
struct Frames {
    /// Mean and worst frame (ms, advance plus look-ahead), the mean over
    /// the frames at the full warp (not capped), look-ahead (ms).
    mean: f64,
    full: f64,
    worst: f64,
    ahead: f64,
    /// Frames held back by the budgets, frames at physics warp because a
    /// vessel was live, and the warp reached.
    limited: usize,
    capped: usize,
    reached: f64,
    /// The slowest single advance: ms, vessel, its phase afterwards.
    slowest: (f64, usize, String),
}

/// The vessel's phase, with the body for landed and crashed vessels.
fn phase_name(w: &World, v: &Vessel) -> String {
    let on = |body| w.source(body).map_or("?", |s| s.name.as_str()).to_owned();
    match v.phase {
        Phase::Landed { body, .. } => format!("landed on {}", on(body)),
        Phase::Powered { .. } => "live".into(),
        Phase::Coasting { .. } => "coasting".into(),
        Phase::Crashed { body, .. } => format!("crashed on {}", on(body)),
    }
}

/// The game's frame loop (`state.rs` `advance`) for `frames` frames of
/// 1/60 s at `warp`, the vessels advanced on `threads` threads (1: in turn).
/// As the game, rails warp falls to physics warp (4×) while any vessel is
/// live (`RailsBlock::OtherLive`), judged at the frame's start.
fn frame_loop(
    w: &World,
    ships: &mut [Vessel],
    controls: &[Controls],
    warp: f64,
    frames: usize,
    threads: usize,
) -> Frames {
    let start = ships[0].time;
    let mut clock = start;
    let mut f = Frames::default();
    for frame in 0..frames {
        let live = ships.iter().any(|v| matches!(v.phase, Phase::Powered { .. }));
        let warp = if live && warp > 4.0 { 4.0 } else { warp };
        f.capped += usize::from(live);
        let target = clock.add_seconds(warp / 60.0);
        let target = if target > w.end() { w.end() } else { target };
        let (reached, ms) = timed(|| {
            let out = Mutex::new(vec![(target, false, 0.0); ships.len()]);
            let one = |i: usize, v: &mut Vessel, c: &Controls| {
                let (r, ms) = timed(|| v.advance(w, target, c, COAST_STEPS_PER_FRAME));
                out.lock().unwrap()[i] = (r, stopped(v), ms);
            };
            if threads <= 1 {
                for (i, v) in ships.iter_mut().enumerate() {
                    one(i, v, &controls[i]);
                }
            } else {
                let queue = Mutex::new(ships.iter_mut().zip(controls).enumerate());
                std::thread::scope(|s| {
                    for _ in 0..threads {
                        s.spawn(|| loop {
                            let next = queue.lock().unwrap().next();
                            let Some((i, (v, c))) = next else { break };
                            one(i, v, c);
                        });
                    }
                });
            }
            out.into_inner().unwrap()
        });
        let mut next = target;
        for (i, &(r, stop, ms)) in reached.iter().enumerate() {
            if !stop && r.seconds_since(target) < -TICK && r < next {
                next = r;
            }
            if ms > f.slowest.0 {
                f.slowest = (ms, i, phase_name(w, &ships[i]));
            }
        }
        f.limited += usize::from(next < target);
        clock = next;
        let ((), look) = timed(|| {
            ships[0].extend_coast(w, clock.add_seconds(LOOKAHEAD), LOOKAHEAD_STEPS_PER_FRAME);
            if ships.len() > 1 {
                let other = 1 + frame % (ships.len() - 1);
                ships[other].extend_coast(w, clock.add_seconds(LOOKAHEAD), OTHER_LOOKAHEAD_STEPS_PER_FRAME);
            }
        });
        f.mean += (ms + look) / frames as f64;
        if !live {
            f.full += ms + look;
        }
        f.ahead += look / frames as f64;
        f.worst = f.worst.max(ms + look);
    }
    f.reached = clock.seconds_since(start) / (frames as f64 / 60.0);
    f.full /= (frames - f.capped).max(1) as f64;
    f
}

fn fleets(w: &World, sizes: &[usize]) {
    let (big, ms) = timed(|| stress::stress_craft(300));
    let (_, bake_ms) = timed(|| big.craft.design().clone());
    println!(
        "\n300-part craft for the fleets: built in {ms:.0} ms, design in {bake_ms:.0} ms (shared by every vessel)"
    );
    println!("\n| fleet | warp | frame, in turn: mean at full warp, all (worst) | look-ahead | compute-limited | at 4× (a vessel live) | warp reached | slowest advance (vessel, phase) | {POOL} threads: mean at full warp, all (worst) | warp reached | fleet KB |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    // Fresh from launch (uniform 290 K), then settled for 30 days first.
    for (size, settled) in sizes.iter().flat_map(|&n| [(n, false), (n, true)]) {
        let (mut ships, controls) = fleet(w, size, &big.craft, settled);
        let ((), settle_ms) = timed(|| {
            for (v, c) in ships.iter_mut().zip(&controls).filter(|_| settled) {
                v.advance(w, t0().add_seconds(30.0 * 86_400.0), c, usize::MAX);
            }
        });
        for v in &mut ships {
            let t = v.time.add_seconds(LOOKAHEAD);
            v.extend_coast(w, t, usize::MAX);
        }
        let kb: usize = ships.iter().map(|v| vessel_bytes(v).0).sum();
        let label = if settled { format!("{size} settled ({:.0} s)", settle_ms / 1e3) } else { size.to_string() };
        let warps: &[(f64, usize)] =
            if settled { &[(1e3, 600), (1e6, 240)] } else { &[(1.0, 600), (1e3, 600), (1e6, 240)] };
        for &(warp, frames) in warps {
            let serial = frame_loop(w, &mut ships.clone(), &controls, warp, frames, 1);
            let pool = frame_loop(w, &mut ships.clone(), &controls, warp, frames, POOL);
            let (a, b) = (serial, pool);
            println!(
                "| {label} | {warp:.0}× | {:.2}, {:.2} ms ({:.1}) | {:.2} ms | {}/{frames} | {} | {:.0}× | {:.1} ms (#{}, {}) | {:.2}, {:.2} ms ({:.1}) | {:.0}× | {:.0} |",
                a.full, a.mean, a.worst, a.ahead, a.limited, a.capped, a.reached, a.slowest.0, a.slowest.1, a.slowest.2,
                b.full, b.mean, b.worst, b.reached, kb as f64 / 1024.0
            );
        }
    }
}

/// Repeats one measurement for ~30 s (for `sample <pid>`).
fn profile(w: &World, what: &str, n: usize) {
    let clock = Instant::now();
    let mut runs = 0;
    let s = stress::stress_craft(if what == "fleet" { 300 } else { n });
    let c = Some(&s.craft);
    while clock.elapsed().as_secs_f64() < 30.0 {
        match (what, c) {
            ("build", _) => drop(stress::stress_craft(n)),
            ("bake", Some(c)) => drop(bake(&c.surface, &c.cells, &BakeOptions::default())),
            ("design", Some(c)) => drop(CraftDesign::new(c)),
            ("live", Some(c)) => drop(live_tick(w, c, 0, 2.0)),
            ("contact", Some(c)) => drop(live_tick(w, c, 3, 2.0)),
            ("lattice", Some(c)) => drop(coast(w, c, 1)),
            ("fleet", Some(c)) => {
                let (mut ships, controls) = fleet(w, n, c, false);
                frame_loop(w, &mut ships, &controls, 1e6, 240, 1);
            }
            ("settled", Some(c)) => {
                let (mut ships, controls) = fleet(w, n, c, true);
                for (v, c) in ships.iter_mut().zip(&controls) {
                    v.advance(w, t0().add_seconds(30.0 * 86_400.0), c, usize::MAX);
                }
                frame_loop(w, &mut ships, &controls, 1e6, 2_400, 1);
            }
            _ => panic!("profile=<bake|design|build|live|contact|lattice|fleet|settled>,<parts>"),
        }
        runs += 1;
    }
    println!("{what} {n}: {runs} runs in {:.1} s", clock.elapsed().as_secs_f64());
}

/// The machine's load averages (other processes skew the timings).
fn load() -> String {
    let out = std::process::Command::new("sysctl").args(["-n", "vm.loadavg"]).output();
    out.map_or_else(|_| "?".into(), |o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| args.iter().find_map(|a| a.strip_prefix(key).map(str::to_owned));
    let w = world();
    if let Some(p) = value("profile=") {
        let (what, n) = p.split_once(',').unwrap_or((&p, "300"));
        return profile(&w, what, n.parse().unwrap());
    }
    let parts: Vec<usize> =
        value("parts=").map_or(vec![1, 50, 300, 1000], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let runs = value("runs=").map_or(3, |s| s.parse().unwrap());
    let (craft, fleet) = (args.iter().any(|a| a == "craft"), args.iter().any(|a| a == "fleet"));
    println!("bench_scale: {runs} runs per timing (median), release");
    if craft || !fleet {
        let mut rows = Vec::new();
        for &n in &parts {
            let (row, ms) = timed(|| craft_row(&w, n, runs));
            println!("{n} parts measured in {:.1} s (load {})", ms / 1e3, load());
            rows.push(row);
        }
        print_rows(&rows);
    }
    if fleet || !craft {
        let sizes = value("fleets=").map_or(vec![1, 10, 50], |s| s.split(',').map(|x| x.parse().unwrap()).collect());
        fleets(&w, &sizes);
        println!("load after the fleets: {}", load());
    }
}
