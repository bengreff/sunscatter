//! Entries into other atmospheres (D084, D085): the gas of each atmosphere
//! (`aero::gas`) against published Mars, Titan and Venus entry data.

use glam::DVec3;
use sim::aero::gas::Gas;
use sim::aero::{aero_forces, bake, AeroBake, BakeOptions, Flow};
use sim::craft::cells::build_cells;
use sim::craft::mesh::union_surface;
use sim::craft::{CellOptions, Primitive, Resolution, Shape, Skin};
use sim::thermal::heating::{sutton_graves_mixture, Species};
use sim::thermal::sutton_graves;

const SKIN: Skin = Skin {
    areal_mass: 8.1,
    specific_heat: 900.0,
    emissivity: 0.8,
    conductivity: 150.0,
    thickness: 0.003,
    max_k: None,
    coupling: None,
};

/// A blunted cone facing −Z: half-angle `cone` (°), nose radius `rn`, base
/// radius `rb` (m), and a conical backshell to radius `rb`/2 behind it.
fn sphere_cone(cone: f64, rn: f64, rb: f64) -> AeroBake {
    let c = cone.to_radians();
    // The cap's edge meets the cone tangentially at radius rn·cos c.
    let (r1, cap_h) = (rn * c.cos(), rn * (1.0 - c.sin()));
    let cone_h = (rb - r1) / c.tan();
    let shapes = [
        Shape::SphereCap { center: DVec3::Z * rn, axis: -DVec3::Z, radius: rn, height: cap_h },
        Shape::Frustum { base: DVec3::Z * (cap_h + cone_h), axis: -DVec3::Z * cone_h, r_base: rb, r_top: r1 },
        Shape::Frustum { base: DVec3::Z * (cap_h + cone_h), axis: DVec3::Z * rb, r_base: rb, r_top: 0.5 * rb },
    ];
    let prims: Vec<Primitive> = (shapes.iter().enumerate())
        .map(|(i, &shape)| Primitive { name: format!("p{i}"), shape, foot: false, skin: None, nozzle: false })
        .collect();
    let s = union_surface(&prims, &Resolution::default());
    let cells = build_cells(&s, &prims, &SKIN, &CellOptions::default());
    bake(&s, &cells, &BakeOptions::default())
}

/// A hypersonic continuum flow along +Z (onto the nose) at `speed` and
/// `rho` in `gas` with ratio of specific heats `gamma` at 200 K.
fn flow(speed: f64, rho: f64, gas: Gas, gamma: f64, molar_mass: f64) -> Flow {
    let t = 200.0;
    let a = (gamma * 8.314_462_618 * t / molar_mass).sqrt();
    Flow {
        dir: DVec3::Z,
        q: 0.5 * rho * speed * speed,
        mach: speed / a,
        knudsen: 1e-5,
        gamma,
        reynolds_per_m: rho * speed / gas.viscosity(t),
        temperature: t,
        speed,
        gas,
        roughness: 0.0,
    }
}

#[test]
fn mars_pathfinder_has_its_hypersonic_axial_force() {
    // Mars Pathfinder: a 70° sphere-cone, nose radius 0.6625 m, diameter
    // 2.65 m. Gnoffo et al., AIAA 98-2445, Table 1 (equilibrium CO₂ CFD on
    // the flight trajectory, α 2°): C_A 1.707 at 6.0 km/s (ρ 4.53e-4), 1.714
    // at 4.27 (1.00e-3), 1.712 at 4.00 (1.13e-3), 1.698 at 3.65 (1.26e-3);
    // "C_A ≈ 1.7 over the hypersonic continuum". Ours: modified Newtonian
    // with the equilibrium CO₂ shock density ratio (NASA SP-3100) in Mars
    // air (γ 1.29, M 0.04334): 1.721–1.727, 0.8–1.4 % high (in air it
    // would be 1.686–1.706: CO₂'s denser shock layer raises Cp,max ≈ 2 −
    // ρ₁/ρ₂ by 1–2 % at 3.6–4.3 km/s).
    let b = sphere_cone(70.0, 0.6625, 1.325);
    let area = std::f64::consts::PI * 1.325 * 1.325;
    for (v, rho, want) in
        [(6000.0, 4.53e-4, 1.707), (4270.0, 1.00e-3, 1.714), (3996.0, 1.13e-3, 1.712), (3648.0, 1.26e-3, 1.698)]
    {
        let ca = |gas: Gas, gamma: f64, m: f64| {
            let f = flow(v, rho, gas, gamma, m);
            aero_forces(&b, &f, 1.0).0.z / (f.q * area)
        };
        let (co2, air) = (ca(Gas::CarbonDioxide, 1.29, 0.043_34), ca(Gas::Air, 1.4, 0.028_964_4));
        println!("Pathfinder {v} m/s: C_A {co2:.3} in CO₂ ({air:.3} if it were air), CFD {want}");
        assert!((co2 / want - 1.0).abs() < 0.02, "{v}: {co2}");
        assert!(co2 > air, "{v}: CO₂ {co2} air {air}");
    }
}

#[test]
fn sutton_graves_constants_follow_their_mixing_rule() {
    // TR R-376 eq. 44 with Table II: Mars reproduces the constant in use
    // (1.9027e-4, NASA TFAWS 2012); Venus and Titan from their compositions.
    let mars = sutton_graves_mixture(&[(Species::CO2, 0.953), (Species::N2, 0.027), (Species::Ar, 0.016)]);
    assert!((mars / 1.9027e-4 - 1.0).abs() < 5e-4, "{mars}");
    let venus = sutton_graves_mixture(&[(Species::CO2, 0.965), (Species::N2, 0.035)]);
    assert!((venus / 1.8967e-4 - 1.0).abs() < 1e-3, "{venus}");
    let titan = sutton_graves_mixture(&[(Species::N2, 0.985), (Species::CH4, 0.015)]);
    assert!((titan / 1.7400e-4 - 1.0).abs() < 1e-3, "{titan}");
}

#[test]
fn stagnation_heating_at_mars_and_titan_against_published_points() {
    // Mars Pathfinder at its peak-heating point (6.592 km/s, ρ 3.24e-4,
    // Rn 0.6625 m): 106 W/cm² convective (Spencer et al. 1999), 120 with
    // radiation (Milos et al., AIAA 98-2681), reconstructed ≈ 90 (West &
    // Brandis, NTRS 20200002354); Sutton–Graves for Mars 120.5, within the
    // published spread (+14 % on Spencer's).
    let k_mars = sutton_graves_mixture(&[(Species::CO2, 0.953), (Species::N2, 0.027), (Species::Ar, 0.016)]);
    let q = sutton_graves(k_mars, 3.24e-4, 0.6625, 6592.0) / 1e4;
    println!("Pathfinder peak: {q:.1} W/cm² (published 90–120)");
    assert!(q > 90.0 && q < 125.0 && (q / 106.0 - 1.0).abs() < 0.15, "{q}");
    // Huygens at t = 189 s (5.126 km/s, ρ 2.96e-4, Rn 1.25 m): laminar
    // convective 46.3 (LAURA) and 45.5 W/cm² (DPLR), Hollis et al., AIAA
    // 2005-4816, Tables 1–2. Sutton–Graves for Titan's N₂–CH₄ gives 36:
    // 22 % low (its fit is to air and CO₂ data; the 10 % accuracy does not
    // carry to Titan). Its radiation (CN, 60–143 W/cm² there) is not modelled.
    let k_titan = sutton_graves_mixture(&[(Species::N2, 0.985), (Species::CH4, 0.015)]);
    let q = sutton_graves(k_titan, 2.96e-4, 1.25, 5126.3) / 1e4;
    println!("Huygens t = 189 s: {q:.1} W/cm² (CFD 45.5–46.3)");
    assert!(q < 46.3 && (q / 45.9 - 1.0).abs() < 0.25, "{q}");
}
