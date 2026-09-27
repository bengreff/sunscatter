use super::heating::{LEEWARD_FRACTION, SUTTON_GRAVES_EARTH, SUTTON_GRAVES_MARS};
use super::*;
use crate::aero::{bake, AeroBake, BakeOptions};
use crate::craft::test_craft;
use crate::math;
use glam::DVec3;
use std::sync::OnceLock;

fn craft_bake() -> &'static AeroBake {
    static B: OnceLock<AeroBake> = OnceLock::new();
    B.get_or_init(|| bake(&test_craft().surface, &test_craft().cells, &BakeOptions::default()))
}

/// One interior node of 4e6 J/K, 2 W/(m²·K) to every cell.
fn one_node(cells: &[Cell]) -> ThermalNetwork {
    ThermalNetwork::new(cells, &Interior::single(cells.len(), 2.0))
}
const CAP: [f64; 1] = [4.0e6];
const NO_HEAT: [f64; 1] = [0.0];

fn one_cell(emissivity: f64) -> Cell {
    Cell {
        centroid: DVec3::ZERO,
        normal: DVec3::Z,
        area: 0.5,
        primitive: 0,
        areal_mass: 8.1,
        specific_heat: 900.0,
        emissivity,
        contact: false,
        neighbours: Vec::new(),
    }
}

#[test]
fn an_isolated_cell_relaxes_to_radiative_equilibrium() {
    let (q, eps) = (5.0e4, 0.8);
    let want = math::sqrt_pos(math::sqrt_pos(q / (eps * STEFAN_BOLTZMANN)));
    let cell = one_cell(eps);
    let net = ThermalNetwork::new(std::slice::from_ref(&cell), &Interior::single(1, 0.0));
    for (dt, steps) in [(0.02, 100_000), (600.0, 20)] {
        let mut s = ThermalState::uniform(1, 1, 290.0);
        for _ in 0..steps {
            net.step(&mut s, &[q * cell.area], &NO_HEAT, &[1.0], 0.0, dt, DEFAULT_SWEEPS);
        }
        assert!((s.skin[0] / want - 1.0).abs() < 1e-9, "dt {dt}: {} vs {want}", s.skin[0]);
    }
    assert!((want - 1024.6).abs() < 0.5, "{want}");
}

fn wavy_start(n: usize) -> ThermalState {
    ThermalState {
        skin: (0..n).map(|i| 250.0 + 300.0 * (math::sin(i as f64 * 0.7) + 1.0)).collect(),
        nodes: vec![280.0],
    }
}

#[test]
fn conduction_conserves_energy() {
    let cells = &test_craft().cells.cells;
    let mut net = one_node(cells);
    net.radiation.fill(0.0);
    let zero = vec![0.0; cells.len()];
    for (dt, steps) in [(0.02, 200), (60.0, 30)] {
        let mut s = wavy_start(cells.len());
        let e0 = net.heat_content(&s, &CAP);
        let spread0 = s.skin.iter().fold(0.0f64, |m, t| m.max((t - 550.0).abs()));
        for _ in 0..steps {
            net.step(&mut s, &zero, &NO_HEAT, &CAP, 0.0, dt, DEFAULT_SWEEPS);
        }
        let e = net.heat_content(&s, &CAP);
        assert!((e / e0 - 1.0).abs() < 1e-9, "dt {dt}: {e} vs {e0}");
        // And it did conduct: the skin is smoother than it started.
        let spread = s.skin.iter().fold(0.0f64, |m, t| m.max((t - 550.0).abs()));
        assert!(spread < spread0, "dt {dt}: {spread} vs {spread0}");
    }
}

#[test]
fn sutton_graves_matches_published_points() {
    // MER at peak heating (Wright, NASA TFAWS 2012 aerothermodynamics
    // course, lecture 1): Mars, ρ = 3.11e-4 kg/m³, V = 4.61 km/s,
    // Rn = 0.6625 m → 40.4 W/cm²; CFD values in the literature 40–44 W/cm².
    let q = sutton_graves(SUTTON_GRAVES_MARS, 3.11e-4, 0.6625, 4610.0) / 1e4;
    assert!((q - 40.4).abs() < 0.1, "{q}");
    assert!((q / 42.0 - 1.0).abs() < 0.10, "{q} vs CFD 40–44");
    // Same source, Earth: ρ = 3.1459e-4, V = 3.535 km/s, Rn = 1 m → 13.6 W/cm².
    let q = sutton_graves(SUTTON_GRAVES_EARTH, 3.1459e-4, 1.0, 3535.0) / 1e4;
    assert!((q - 13.6).abs() < 0.05, "{q}");
    // Scaling: √ρ, 1/√Rn, V³; nothing in vacuum.
    let base = sutton_graves(SUTTON_GRAVES_EARTH, 1e-4, 1.0, 7000.0);
    assert!((sutton_graves(SUTTON_GRAVES_EARTH, 4e-4, 1.0, 7000.0) / base - 2.0).abs() < 1e-12);
    assert!((sutton_graves(SUTTON_GRAVES_EARTH, 1e-4, 4.0, 7000.0) / base - 0.5).abs() < 1e-12);
    assert!((sutton_graves(SUTTON_GRAVES_EARTH, 1e-4, 1.0, 14000.0) / base - 8.0).abs() < 1e-12);
    assert_eq!(sutton_graves(SUTTON_GRAVES_EARTH, 0.0, 1.0, 7000.0), 0.0);
}

#[test]
fn large_steps_are_stable() {
    let cells = &test_craft().cells.cells;
    let net = one_node(cells);
    let b = craft_bake();
    let mut heat = vec![0.0; cells.len()];
    let mut scratch = vec![0.0; cells.len()];
    let input = HeatInput { flow_dir: -DVec3::Z, q_stag: 3.0e5, sun: DVec3::new(-1361.0, 0.0, 0.0) };
    cell_heat(b, cells, &input, &mut scratch, &mut heat);
    // Upper bound: every cell at its own radiative equilibrium under the
    // largest flux any cell receives.
    let peak = heat.iter().zip(cells).map(|(h, c)| h / c.area).fold(0.0, f64::max);
    let bound = math::sqrt_pos(math::sqrt_pos(peak / (0.8 * STEFAN_BOLTZMANN)));
    // Same end state (steady) from 20 ms and 600 s steps; bounded and
    // positive throughout the big steps.
    let mut fine = ThermalState::uniform(cells.len(), 1, 290.0);
    for _ in 0..(3600.0 / 0.5) as usize {
        net.step(&mut fine, &heat, &NO_HEAT, &CAP, 3.0, 0.5, DEFAULT_SWEEPS);
    }
    let mut coarse = ThermalState::uniform(cells.len(), 1, 290.0);
    for _ in 0..6 {
        net.step(&mut coarse, &heat, &NO_HEAT, &CAP, 3.0, 600.0, DEFAULT_SWEEPS);
        assert!(coarse.skin.iter().all(|&t| t.is_finite() && t > 0.0 && t < 1.05 * bound), "{bound}");
        assert!(coarse.nodes[0].is_finite() && coarse.nodes[0] > 0.0);
    }
    for i in 0..cells.len() {
        assert!((coarse.skin[i] / fine.skin[i] - 1.0).abs() < 0.02, "cell {i}: {} vs {}", coarse.skin[i], fine.skin[i]);
    }
    // A hot craft with nothing heating it cools monotonically at 600 s steps.
    let mut s = ThermalState::uniform(cells.len(), 1, 2000.0);
    let zero = vec![0.0; cells.len()];
    let mut last = f64::MAX;
    for _ in 0..20 {
        net.step(&mut s, &zero, &NO_HEAT, &CAP, 3.0, 600.0, DEFAULT_SWEEPS);
        let hottest = s.skin.iter().fold(0.0f64, |m, &t| m.max(t));
        assert!(hottest < last && s.skin.iter().all(|&t| t > 3.0), "{hottest}");
        last = hottest;
    }
}

#[test]
fn stagnation_cells_get_the_full_flux_and_leeward_cells_a_little() {
    let cells = &test_craft().cells.cells;
    let b = craft_bake();
    let mut heat = vec![0.0; cells.len()];
    let mut scratch = vec![0.0; cells.len()];
    let q = 1.0e6;
    cell_heat(b, cells, &HeatInput { flow_dir: -DVec3::Z, q_stag: q, sun: DVec3::ZERO }, &mut scratch, &mut heat);
    let flux: Vec<f64> = heat.iter().zip(cells).map(|(h, c)| h / c.area).collect();
    let max = flux.iter().fold(0.0f64, |m, &f| m.max(f));
    assert!(max > 0.97 * q && max <= q * (1.0 + 1e-12), "{max}");
    for (i, c) in cells.iter().enumerate() {
        // The whole back half (normals facing aft) gets the leeward fraction.
        if c.normal.z < -0.1 {
            assert!((flux[i] / q - LEEWARD_FRACTION).abs() < 1e-12, "cell {i}: {}", flux[i]);
        }
        assert!(flux[i] >= LEEWARD_FRACTION * q * (1.0 - 1e-12));
    }
}

#[test]
fn sunlight_is_absorbed_on_the_lit_silhouette() {
    let cells = &test_craft().cells.cells;
    let b = craft_bake();
    let mut heat = vec![0.0; cells.len()];
    let mut scratch = vec![0.0; cells.len()];
    // Light travelling −Z (onto the nose); the bell is hidden behind the body.
    let s = 1361.0;
    cell_heat(
        b,
        cells,
        &HeatInput { flow_dir: DVec3::Z, q_stag: 0.0, sun: DVec3::new(0.0, 0.0, -s) },
        &mut scratch,
        &mut heat,
    );
    let total: f64 = heat.iter().sum();
    let want = 0.8 * s * b.sums_at(-DVec3::Z).area;
    assert!((total / want - 1.0).abs() < 0.01, "{total} vs {want}");
    for (i, c) in cells.iter().enumerate() {
        if c.normal.z < 0.0 {
            assert_eq!(heat[i], 0.0, "cell {i} faces away from the light");
        }
    }
}

#[test]
fn overheat_reports_the_hottest_cell_then_the_interior() {
    let s = |skin: Vec<f64>, node: f64| ThermalState { skin, nodes: vec![300.0, node] };
    let cases = [
        (s(vec![300.0, 900.0, 1000.0], 350.0), None),
        (s(vec![300.0, 1200.0, 1500.0], 350.0), Some(Overheat::Cell(2))),
        (s(vec![1500.0, 1200.0, 1500.0], 500.0), Some(Overheat::Cell(0))),
        (s(vec![300.0, 900.0, 1000.0], 450.0), Some(Overheat::Node(1))),
        (s(vec![], 450.0), Some(Overheat::Node(1))),
    ];
    for (state, want) in cases {
        assert_eq!(check(&state, 1100.0, 400.0), want, "{state:?}");
    }
}

#[test]
fn the_cell_balance_is_solved_from_any_guess() {
    // (a, r, b): a·T + r·T⁴ = b.
    let cases = [(1.0, 0.0, 300.0), (0.0, 1e-8, 1e4), (50.0, 2e-8, 2e5), (1e6, 4.5e-8, 3e8), (1e-3, 4.5e-8, 1e7)];
    for (a, r, b) in cases {
        for guess in [0.0, 1.0, 290.0, 5000.0, 1e6] {
            let t = cell_temperature(a, r, b, guess);
            let residual = a * t + r * t * t * t * t - b;
            assert!(t > 0.0 && residual.abs() < 1e-12 * b, "{a} {r} {b} from {guess}: {t}, {residual}");
        }
    }
    assert_eq!(cell_temperature(1.0, 1e-8, 0.0, 300.0), 0.0);
}

#[test]
fn live_ticks_converge_in_a_few_sweeps() {
    let cells = &test_craft().cells.cells;
    let net = one_node(cells);
    let mut s = wavy_start(cells.len());
    let heat: Vec<f64> = cells.iter().map(|c| 2.0e4 * c.area).collect();
    for _ in 0..10 {
        let sweeps = net.step(&mut s, &heat, &NO_HEAT, &CAP, 3.0, 0.02, DEFAULT_SWEEPS);
        assert!(sweeps <= 4, "{sweeps}");
    }
}

/// The test craft's network over its interior volume nodes, and the nodes'
/// capacities with a full tank.
fn with_nodes() -> (&'static ThermalNetwork, Vec<f64>) {
    let d = test_craft().design();
    let mut cap = vec![0.0; d.nodes()];
    d.node_capacity(test_craft().spec.propellant.capacity, &mut cap);
    (&d.network, cap)
}

#[test]
fn node_conduction_conserves_energy() {
    let (net, cap) = with_nodes();
    let mut net = net.clone();
    net.radiation.fill(0.0);
    let (n, m) = (net.cells(), net.nodes());
    let zero = vec![0.0; n];
    for (dt, steps) in [(0.02, 100), (60.0, 30), (3600.0, 5)] {
        let mut s = wavy_start(n);
        s.nodes = (0..m).map(|j| 200.0 + 150.0 * (math::cos(j as f64 * 1.3) + 1.0)).collect();
        let e0 = net.heat_content(&s, &cap);
        for _ in 0..steps {
            net.step(&mut s, &zero, &vec![0.0; m], &cap, 0.0, dt, DEFAULT_SWEEPS);
        }
        let e = net.heat_content(&s, &cap);
        assert!((e / e0 - 1.0).abs() < 1e-9, "dt {dt}: {e} vs {e0}");
    }
}

#[test]
fn a_heated_node_settles_at_the_analytic_temperature() {
    // One cell (radiating) over one node heated by Q through a coupling G:
    // steady state εσA·T_c⁴ = Q and T_n = T_c + Q/G.
    let cell = one_cell(0.8);
    let (q, coupling) = (500.0, 20.0);
    let net = ThermalNetwork::new(std::slice::from_ref(&cell), &Interior::single(1, coupling));
    let g = coupling * cell.area;
    let t_cell = math::sqrt_pos(math::sqrt_pos(q / (0.8 * STEFAN_BOLTZMANN * cell.area)));
    let mut s = ThermalState::uniform(1, 1, 290.0);
    for _ in 0..200 {
        net.step(&mut s, &[0.0], &[q], &[5.0e4], 0.0, 3600.0, DEFAULT_SWEEPS);
    }
    assert!((s.skin[0] / t_cell - 1.0).abs() < 1e-9, "{} vs {t_cell}", s.skin[0]);
    assert!((s.nodes[0] / (t_cell + q / g) - 1.0).abs() < 1e-9, "{} vs {}", s.nodes[0], t_cell + q / g);
}

#[test]
fn the_node_network_is_stable_at_large_steps_and_fast_at_ticks() {
    let (net, cap) = with_nodes();
    let d = test_craft().design();
    let cells = &test_craft().cells.cells;
    let mut heat = vec![0.0; cells.len()];
    let mut scratch = vec![0.0; cells.len()];
    let input = HeatInput { flow_dir: DVec3::ZERO, q_stag: 0.0, sun: DVec3::new(-1361.0, 0.0, 0.0) };
    cell_heat(craft_bake(), cells, &input, &mut scratch, &mut heat);
    let mut node_heat = vec![0.0; net.nodes()];
    node_heat[d.engine_node as usize] = 1.0e3;
    let mut s = ThermalState::uniform(cells.len(), net.nodes(), 290.0);
    for _ in 0..20 {
        let sweeps = net.step(&mut s, &heat, &node_heat, &cap, 2.725, 3600.0, DEFAULT_SWEEPS);
        assert!(sweeps < DEFAULT_SWEEPS, "converged: {sweeps}");
        assert!(s.skin.iter().chain(&s.nodes).all(|&t| t.is_finite() && t > 100.0 && t < 1000.0));
    }
    for _ in 0..10 {
        let sweeps = net.step(&mut s, &heat, &node_heat, &cap, 2.725, 0.02, DEFAULT_SWEEPS);
        assert!(sweeps <= 4, "{sweeps}");
    }
}
