//! Heating over the whole body (D085): the boundary layer's heating against
//! the classic plate correlations and Tauber's closed forms, the blunt-nose
//! distribution, hot walls and the free-molecular limit.

use super::*;
use crate::aero::boundary::{self, Stream, LAMINAR_RECOVERY, TURBULENT_RECOVERY};
use crate::aero::{air, bake, AeroBake, BakeOptions};
use crate::craft::{Cells, Shape};
use crate::math;
use std::sync::OnceLock;

/// The free stream at `altitude` (US 1976) at airspeed `v`, flowing along
/// `dir` (body axes), Knudsen number on `length`.
fn earth_flow(altitude: f64, v: f64, dir: DVec3, length: f64) -> Flow {
    let atm = crate::body::earth().atmosphere.expect("Earth has air");
    let a = atm.profile(altitude, air::G0);
    Flow {
        dir,
        q: 0.5 * a.rho * v * v,
        mach: v / a.speed_of_sound(1.4),
        knudsen: a.mean_free_path / length,
        gamma: 1.4,
        reynolds_per_m: a.rho * v / air::sutherland_viscosity(a.temperature),
        temperature: a.temperature,
        speed: v,
        gas: Default::default(),
    }
}

/// Earth air's specific heat at constant pressure (J/(kg·K)).
const CP: f64 = 1.4 * air::GAS_CONSTANT / air::EARTH_AIR_MOLAR_MASS / 0.4;

/// x^e.
fn pow(x: f64, e: f64) -> f64 {
    math::exp(e * math::ln(x))
}

#[test]
fn a_flat_plate_heats_like_the_classic_correlations() {
    // Low speed (M 0.2, 288.15 K), a cool wall (250 K): the local Stanton
    // number against Pohlhausen's laminar plate, Nu = 0.332·Re^½·Pr^⅓, and
    // Colburn's turbulent plate, Nu = 0.0296·Re^⅘·Pr^⅓ (St = Nu/(Re·Pr);
    // Incropera & DeWitt, Fundamentals of Heat and Mass Transfer, §7.2,
    // from memory). Properties: ours at Eckert's reference temperature, theirs
    // at the film temperature (≈ 1 % apart here). Turbulent, White's law
    // runs 4–9 % above the ⅕-power fit from Re 10⁶ to 10⁷ (the ⅕ law falls
    // off at high Re) and Van Driest II adds ~5 % for the cool wall.
    let (t, tw) = (288.15, 250.0);
    let st = |re: f64| {
        let e = Stream::new(re, 0.2, 1.4, t, 0.0).edge(0.0, 0.0);
        let lam = e.laminar_heat(1.0, 1.0, tw, t) / (e.recovery(LAMINAR_RECOVERY, t) - tw);
        let turb = e.turbulent_heat(1.0, tw, t) / (e.recovery(TURBULENT_RECOVERY, t) - tw);
        (lam, turb)
    };
    let pr23 = pow(boundary::PRANDTL, -2.0 / 3.0);
    for re in [1e4, 1e5, 4e5] {
        let (got, want) = (st(re).0, 0.332 / re.sqrt() * pr23);
        println!("Re {re:e}: laminar St {got:.3e} (Pohlhausen {want:.3e})");
        assert!((got / want - 1.0).abs() < 0.02, "Re {re}: {got} vs {want}");
    }
    for re in [1e6, 3e6, 1e7] {
        let (got, want) = (st(re).1, 0.0296 * pow(re, -0.2) * pr23);
        println!("Re {re:e}: turbulent St {got:.3e} (Colburn {want:.3e})");
        assert!((got / want - 1.0).abs() < 0.15, "Re {re}: {got} vs {want}");
    }
    // Across transition the layer's heating joins the two.
    let e = Stream::new(1e6, 0.2, 1.4, t, 0.0).edge(0.0, 0.0);
    let layer = |x: f64| e.layer(x, x, Some(tw), t, true);
    let (before, after) = (layer(0.4), layer(1.2));
    assert_eq!(before.heat_turbulent, 0.0);
    assert!(after.intermittency > 0.99 && after.heat_turbulent > 3.0 * before.heat_laminar);
}

#[test]
fn hypersonic_wedge_heating_is_tauber_s() {
    // Tauber, NASA TP-2914 (1989): an inclined plate (wedge angle δ), cold
    // wall, real-gas properties fitted 3–60 km, 1–11 km/s:
    // laminar (eq. 48) q̇ = 2.42e-5·(ρ cos δ / x)^½·V^3.2·sin δ·(1 − h_w/h_aw),
    // h_aw = h∞ + 0.4·V²; turbulent above 3,962 m/s (eq. 56b) q̇ =
    // 2.45e-5·(ρ sin²δ cos^2.62 δ)^0.8·x^−0.2·V^3.7·(0.9 − h_w/H), and
    // below (56a) 3.72e-4·(ρ sin²δ cos^2.22 δ)^0.8·x^−0.2·(T_w/555)^−¼·
    // V^3.37·(0.9 − h_w/H), ±15 % against his reference-enthalpy solutions.
    // Ours (perfect gas, Sutherland, Van Driest II with White's law; his:
    // Blasius and Schultz-Grunow at the reference enthalpy, equilibrium
    // air) within 20 % laminar, 30 % turbulent at 3.5 km/s; from 4.5 km/s
    // the turbulent layer takes his relation where it is the larger (the
    // perfect gas ran 25–35 % under it at 7 km/s: dissociation raises the
    // real reference state's ρ*·μ*).
    let (x, tw) = (5.0, 1000.0);
    for (h, v) in [(40e3, 3500.0), (55e3, 5000.0), (60e3, 7000.0), (50e3, 7000.0)] {
        for deg in [10.0f64, 20.0, 40.0] {
            let delta = deg.to_radians();
            let s = math::sin(delta);
            let flow = earth_flow(h, v, DVec3::X, 1.0);
            let rho = 2.0 * flow.q / (v * v);
            let scale = rho * v * CP;
            let e = Stream::of(&flow).edge(s, 1.0);
            let (lam, turb) =
                (scale * e.laminar_heat(x, x, tw, flow.temperature), scale * e.turbulent_heat(x, tw, flow.temperature));
            let (hinf, hw) = (CP * flow.temperature, CP * tw);
            let c = math::cos(delta);
            let tauber_lam = 2.42e-5 * (rho * c / x).sqrt() * pow(v, 3.2) * s * (1.0 - hw / (hinf + 0.4 * v * v));
            let big_h = hinf + 0.5 * v * v;
            let tauber_turb = if v > 3962.0 {
                2.45e-5 * pow(rho * s * s * pow(c, 2.62), 0.8) / pow(x, 0.2) * pow(v, 3.7) * (0.9 - hw / big_h)
            } else {
                3.72e-4 * pow(rho * s * s * pow(c, 2.22), 0.8) / (pow(x, 0.2) * pow(tw / 555.0, 0.25))
                    * pow(v, 3.37)
                    * (0.9 - hw / big_h)
            };
            println!(
                "{:.0} km {v} m/s δ {deg}°: laminar {:.2} W/cm² (Tauber {:.2}), turbulent {:.2} (Tauber {:.2})",
                h / 1e3,
                lam / 1e4,
                tauber_lam / 1e4,
                turb / 1e4,
                tauber_turb / 1e4
            );
            // Above 4.5 km/s the turbulent heating is at least Tauber's
            // relation itself (real-gas reference state).
            let turb_tolerance = if v > 4500.0 { 0.01 } else { 0.3 };
            assert!((lam / tauber_lam - 1.0).abs() < 0.2, "laminar {h} {v} {deg}: {lam} vs {tauber_lam}");
            assert!(
                (turb / tauber_turb - 1.0).abs() < turb_tolerance,
                "turbulent {h} {v} {deg}: {turb} vs {tauber_turb}"
            );
        }
    }
}

/// A sphere of radius 1 m (two hemispheres): its cells and bake.
fn sphere() -> &'static (Cells, AeroBake) {
    static S: OnceLock<(Cells, AeroBake)> = OnceLock::new();
    S.get_or_init(|| {
        let cap = |axis: DVec3| Shape::SphereCap { center: DVec3::ZERO, axis, radius: 1.0, height: 1.0 };
        let (s, c) = crate::aero::tests::shape(&[cap(DVec3::Z), cap(-DVec3::Z)]);
        let b = bake(&s, &c, &BakeOptions::default());
        (c, b)
    })
}

/// Each cell's heat flux (W/m²) and body angle from the stagnation point
/// (degrees) on the sphere in `flow` (air along −Z) with the wall at `tw`.
fn sphere_fluxes(flow: &Flow, tw: f64) -> Vec<(f64, f64)> {
    let (cells, b) = sphere();
    let wall = vec![tw; cells.cells.len()];
    let heat = FlowHeat { flow, sutton_graves_k: SUTTON_GRAVES_EARTH, radiative: None, wall: &wall };
    let mut out = vec![0.0; cells.cells.len()];
    let mut scratch = vec![0.0; cells.cells.len()];
    cell_heat_flow(b, &cells.cells, Some(&heat), DVec3::ZERO, &mut scratch, &mut out);
    (cells.cells.iter().zip(&out))
        .map(|(c, q)| (q / c.area, math::acos(c.normal.dot(-flow.dir).clamp(-1.0, 1.0)).to_degrees()))
        .collect()
}

/// The sphere's hot-wall stagnation flux in `flow` (W/m²).
fn stagnation(flow: &Flow, tw: f64) -> f64 {
    let rho = 2.0 * flow.q / (flow.speed * flow.speed);
    let t0 = flow.temperature * (1.0 + 0.2 * flow.mach * flow.mach);
    let rn = sphere().1.nose_radius_at(flow.dir);
    sutton_graves(SUTTON_GRAVES_EARTH, rho, rn, flow.speed) * (1.0 - tw / t0)
}

#[test]
fn a_hemisphere_heats_like_the_cosine_law_and_lees() {
    // Laminar, 6 km/s at 65 km, cold wall. Tauber, NASA TP-2914, fig. 5
    // (hemisphere-cylinder; Kemp et al.'s and Lees' local-similarity
    // solutions and the data): q̇/q̇_s follows cos φ (φ the body angle from
    // the stagnation point) to about 70°, ≈ 0.1–0.2 at 80–90° (ours 0.03–0.15
    // there).
    let flow = earth_flow(65e3, 6000.0, -DVec3::Z, 2.0);
    let q0 = stagnation(&flow, 300.0);
    let mut seen = [0; 3];
    for (q, phi) in sphere_fluxes(&flow, 300.0) {
        let (r, cos) = (q / q0, math::cos(phi.to_radians()));
        if phi < 45.0 {
            // The tilt of the cells (their normals average a patch): 5 %.
            assert!((r - cos).abs() < 0.05, "φ {phi:.0}°: {r} vs {cos}");
            seen[0] += 1;
        } else if phi < 70.0 {
            assert!(r > 0.75 * cos && r < 1.05 * cos, "φ {phi:.0}°: {r} vs {cos}");
            seen[1] += 1;
        } else if (80.0..90.0).contains(&phi) {
            // Low: the layer's pressure history (Lees' integral) is not
            // modelled; each cell takes its local state over its whole run.
            assert!(r > 0.02 && r < 0.25, "φ {phi:.0}°: {r}");
            seen[2] += 1;
        }
        // Leeward (from 6°) and separated: at least the floor.
        if phi > 96.0 {
            assert!(r >= LEEWARD_FRACTION * (1.0 - 1e-9), "φ {phi:.0}°: {r}");
        }
    }
    assert!(seen.iter().all(|&n| n > 3), "{seen:?}");
}

#[test]
fn hot_walls_take_less_heat_and_walls_above_recovery_are_cooled() {
    // The stagnation region scales with 1 − T_w/T₀: at 6 km/s T₀ ≈ 18,000 K
    // (perfect gas), a 2,000 K wall takes ~10 % less than a 300 K one.
    let flow = earth_flow(65e3, 6000.0, -DVec3::Z, 2.0);
    let t0 = flow.temperature * (1.0 + 0.2 * flow.mach * flow.mach);
    let (cold, hot) = (sphere_fluxes(&flow, 300.0), sphere_fluxes(&flow, 2000.0));
    let want = (1.0 - 2000.0 / t0) / (1.0 - 300.0 / t0);
    for ((c, phi), (h, _)) in cold.iter().zip(&hot) {
        if *phi < 30.0 {
            assert!((h / c / want - 1.0).abs() < 1e-9, "φ {phi}: {} vs {want}", h / c);
        }
        assert!(h < c, "φ {phi}");
    }
    // At M 2 near sea level T₀ ≈ 520 K: a 600 K skin is cooled everywhere.
    let flow = earth_flow(1e3, 680.0, -DVec3::Z, 2.0);
    for (q, phi) in sphere_fluxes(&flow, 600.0) {
        assert!(q < 0.0, "φ {phi}: {q}");
    }
}

#[test]
fn rarefied_heating_tends_to_the_free_molecular_flux() {
    // At 120 km (λ ≈ 3 m, Kn on the 1 m nose ≈ 3) Sutton–Graves would put
    // C_H = q̇/(½ρV³) far above 1, more than the air brings; bridged by
    // Wilmoth's function, the stagnation cells take close to the
    // free-molecular ½ρV³ (C_H = 1 at full accommodation; DSMC, Tauber fig. 8).
    let v = 7800.0;
    let flow = earth_flow(120e3, v, -DVec3::Z, 2.0);
    let rho = 2.0 * flow.q / (v * v);
    let fm = 0.5 * rho * v * v * v;
    let sg = sutton_graves(SUTTON_GRAVES_EARTH, rho, 1.0, v);
    assert!(sg > 2.0 * fm, "{sg} {fm}");
    let peak = sphere_fluxes(&flow, 300.0).iter().map(|q| q.0).fold(0.0, f64::max);
    println!("120 km: peak {:.0} W/m², free molecular {fm:.0}, Sutton–Graves {sg:.0}", peak);
    assert!(peak <= fm * (1.0 + 1e-9) && peak > 0.9 * fm, "{peak} vs {fm}");
    // At 70 km (Kn 3e-4) the continuum value.
    let flow = earth_flow(70e3, v, -DVec3::Z, 2.0);
    let peak = sphere_fluxes(&flow, 300.0).iter().map(|q| q.0).fold(0.0, f64::max);
    assert!((peak / stagnation(&flow, 300.0) - 1.0).abs() < 0.03, "{peak}");
}

#[test]
fn a_tick_s_boundary_layer_gives_the_same_heating() {
    let flow = earth_flow(45e3, 3000.0, -DVec3::Z, 2.0);
    let (cells, b) = sphere();
    let wall: Vec<f64> = (0..cells.cells.len()).map(|i| 400.0 + (i % 5) as f64 * 100.0).collect();
    let air = FlowHeat { flow: &flow, sutton_graves_k: SUTTON_GRAVES_EARTH, radiative: None, wall: &wall };
    let n = cells.cells.len();
    let (mut a, mut c, mut scratch) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    cell_heat_flow(b, &cells.cells, Some(&air), DVec3::X * 1361.0, &mut scratch, &mut a);
    let layer = crate::aero::friction::FlowLayer::new(b, &flow, Some(&wall), true);
    cell_heat_layer(b, &cells.cells, &air, &layer, DVec3::X * 1361.0, &mut scratch, &mut c);
    assert_eq!(a, c);
}

#[test]
fn a_sharp_cone_heats_like_tauber_s_cone() {
    // A 10° cone, 3 m long, nose first at 6 km/s, 60 km (laminar), wall at
    // 1000 K. Tauber, NASA TP-2914, eq. 46 (the plate's eq. 48 × √3 by
    // Mangler's transformation, with real-gas properties): q̇ =
    // 4.03e-5·(ρ cos δ / x)^½·V^3.2·sin δ·(1 − h_w/h_aw), h_aw = h∞ + 0.4·V²,
    // x along the surface from the apex. Ours: the plate's layer at the
    // cell's run, relieved by the baked perimeter growth (x_eff ≈ x/3).
    let delta = 10f64.to_radians();
    let length = 3.0;
    let r = length * math::tan(delta);
    let (s, c) = crate::aero::tests::shape(&[Shape::Frustum {
        base: DVec3::ZERO,
        axis: DVec3::Z * length,
        r_base: r,
        r_top: 0.001,
    }]);
    let b = bake(&s, &c, &BakeOptions::default());
    let v = 6000.0;
    let flow = earth_flow(60e3, v, -DVec3::Z, b.length);
    let tw = 1000.0;
    let wall = vec![tw; c.cells.len()];
    let air = FlowHeat { flow: &flow, sutton_graves_k: SUTTON_GRAVES_EARTH, radiative: None, wall: &wall };
    let (mut out, mut scratch) = (vec![0.0; c.cells.len()], vec![0.0; c.cells.len()]);
    cell_heat_flow(&b, &c.cells, Some(&air), DVec3::ZERO, &mut scratch, &mut out);
    let rho = 2.0 * flow.q / (v * v);
    let h_aw = CP * flow.temperature + 0.4 * v * v;
    let (mut n, mut worst, mut sum) = (0, 0.0f64, 0.0);
    for (cell, q) in c.cells.iter().zip(&out) {
        // Flank cells away from the tip and the base.
        let x = (length - cell.centroid.z) / math::cos(delta);
        if cell.normal.z > -0.5 && cell.normal.z < 0.5 && x > 0.6 && x < 2.6 {
            let tauber =
                4.03e-5 * (rho * math::cos(delta) / x).sqrt() * pow(v, 3.2) * math::sin(delta) * (1.0 - CP * tw / h_aw);
            let ratio = q / cell.area / tauber;
            worst = worst.max((ratio - 1.0).abs());
            sum += ratio;
            n += 1;
        }
    }
    let mean = sum / f64::from(n);
    println!("cone flank, {n} cells: mean {mean:.3} of Tauber's cone, worst {worst:.3} from it");
    // Our perfect-gas plate runs ~10 % under his laminar plate at these
    // speeds (hypersonic_wedge_heating_is_tauber_s); the cells' run lengths
    // and the baked relief add ±10 %.
    assert!(n > 20 && (mean - 1.0).abs() < 0.12 && worst < 0.25, "{n} {mean} {worst}");
}

#[test]
fn the_test_craft_s_nose_takes_the_stagnation_flux_and_its_lee_a_little() {
    // Nose first at 65 km, 6 km/s, cold wall (300 K): the cells facing the
    // flow (the nose cap, the fins' tops) take the stagnation flux at the
    // nose radius (+0.5 %: λ/Rn 1.3e-3 there, Wilmoth's bridge begins), the
    // aft-facing half at least the 3 % floor.
    let c = crate::craft::test_craft();
    let (cells, b) = (&c.cells.cells, crate::aero::tests::craft_bake());
    let flow = earth_flow(65e3, 6000.0, -DVec3::Z, b.length);
    let q0 = sutton_graves(SUTTON_GRAVES_EARTH, 2.0 * flow.q / (6000.0 * 6000.0), b.nose_radius_at(flow.dir), 6000.0)
        * (1.0 - 300.0 / (flow.temperature * (1.0 + 0.2 * flow.mach * flow.mach)));
    let wall = vec![300.0; cells.len()];
    let air = FlowHeat { flow: &flow, sutton_graves_k: SUTTON_GRAVES_EARTH, radiative: None, wall: &wall };
    let (mut out, mut scratch) = (vec![0.0; cells.len()], vec![0.0; cells.len()]);
    cell_heat_flow(b, cells, Some(&air), DVec3::ZERO, &mut scratch, &mut out);
    let flux: Vec<f64> = out.iter().zip(cells).map(|(h, c)| h / c.area).collect();
    let max = flux.iter().fold(0.0f64, |m, &f| m.max(f));
    assert!(max > 0.9 * q0 && max <= q0 * 1.01, "{max} vs {q0}");
    for (cell, f) in cells.iter().zip(&flux) {
        if cell.normal.z < -0.1 {
            assert!(*f >= LEEWARD_FRACTION * q0 * (1.0 - 1e-9), "{f}");
        }
    }
}
