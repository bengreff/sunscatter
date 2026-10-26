//! Boundary-layer transition and acreage heating against flight data
//! (D085): the windward centreline of the Space Shuttle orbiter on STS-1,
//! as tabulated by Tauber, "A Review of High-Speed, Convective,
//! Heat-Transfer Computation Methods", NASA TP-2914 (1989), fig. 14 (from
//! Tauber & Adelman): heating against x/L at three points of the entry,
//! where the transition front moved forward as the orbiter descended.

use glam::DVec3;
use sim::aero::air::{self, G0};
use sim::aero::boundary::{Stream, LAMINAR_RECOVERY};
use sim::aero::gas::Gas;
use sim::aero::Flow;
use sim::thermal::STEFAN_BOLTZMANN;

/// The orbiter's length (m).
const L: f64 = 32.77;
/// Emissivity of the lower surface's tiles (reaction-cured glass, ~0.85).
const EMISSIVITY: f64 = 0.85;
/// Earth air's specific heat at constant pressure (J/(kg·K)).
const CP: f64 = 1.4 * air::GAS_CONSTANT / air::EARTH_AIR_MOLAR_MASS / 0.4;

/// One flight point: altitude (m), airspeed (m/s), angle of attack (°), and
/// the measured heating (x/L, W/cm²) read from fig. 14.
struct Point {
    altitude: f64,
    speed: f64,
    alpha: f64,
    data: [(f64, f64); 7],
}

const STS1: [Point; 3] = [
    Point {
        altitude: 52.9e3,
        speed: 3536.0,
        alpha: 39.2,
        data: [(0.12, 5.0), (0.17, 3.3), (0.19, 3.4), (0.27, 2.9), (0.32, 2.7), (0.42, 2.6), (0.50, 3.0)],
    },
    Point {
        altitude: 47.5e3,
        speed: 2640.0,
        alpha: 34.1,
        data: [(0.12, 2.7), (0.16, 1.8), (0.19, 1.8), (0.27, 1.6), (0.32, 2.2), (0.42, 5.1), (0.50, 5.2)],
    },
    Point {
        altitude: 44.2e3,
        speed: 2275.0,
        alpha: 30.8,
        data: [(0.12, 1.85), (0.16, 1.15), (0.19, 1.35), (0.27, 3.85), (0.32, 3.75), (0.42, 3.7), (0.50, 3.8)],
    },
];

/// The windward centreline as a wedge at the angle of attack (Tauber's
/// assumption for this comparison), in the US 1976 air: the edge state,
/// the free stream's ρ·V·c_p and temperature.
fn centreline(p: &Point) -> (sim::aero::boundary::Edge, f64, f64) {
    let atm = sim::body::earth().atmosphere.expect("Earth has air");
    let a = atm.profile(p.altitude, G0);
    let flow = Flow {
        dir: DVec3::X,
        q: 0.5 * a.rho * p.speed * p.speed,
        mach: p.speed / a.speed_of_sound(1.4),
        knudsen: 0.0,
        gamma: 1.4,
        reynolds_per_m: a.rho * p.speed / Gas::Air.viscosity(a.temperature),
        temperature: a.temperature,
        speed: p.speed,
        gas: Default::default(),
        roughness: 0.0,
    };
    let s = p.alpha.to_radians().sin();
    (Stream::of(&flow).edge(s, 1.0), a.rho * p.speed * CP, a.temperature)
}

/// Heating (W/m²) at x/L with the wall in radiative equilibrium, and the
/// turbulent share: (laminar only, the layer).
fn heating(p: &Point, xl: f64) -> (f64, f64, f64) {
    let (edge, scale, t) = centreline(p);
    let x = xl * L;
    // Radiative equilibrium q̇(T_w) = εσT_w⁴ by bisection.
    let equilibrium = |q: &dyn Fn(f64) -> f64| {
        let (mut lo, mut hi) = (100.0, edge.recovery(LAMINAR_RECOVERY, t));
        for _ in 0..80 {
            let tw = 0.5 * (lo + hi);
            if q(tw) > EMISSIVITY * STEFAN_BOLTZMANN * tw * tw * tw * tw {
                lo = tw;
            } else {
                hi = tw;
            }
        }
        q(0.5 * (lo + hi))
    };
    let laminar = equilibrium(&|tw| scale * edge.laminar_heat(x, x, tw, t));
    let layer = |tw: f64| edge.layer(x, x, Some(tw), t, true);
    let total = equilibrium(&|tw| {
        let l = layer(tw);
        scale * ((1.0 - l.intermittency) * l.heat_laminar + l.intermittency * l.heat_turbulent)
    });
    let onset = layer(500.0).intermittency;
    (laminar, total, onset)
}

/// Where the layer starts to turn turbulent (x/L), wall at 1000 K.
fn onset(p: &Point) -> f64 {
    let (edge, _, t) = centreline(p);
    (1..=100)
        .map(|k| k as f64 * 0.01)
        .find(|&xl| edge.layer(xl * L, xl * L, Some(1000.0), t, false).intermittency > 0.0)
        .unwrap_or(1.0)
}

#[test]
fn the_transition_front_moves_forward_as_on_sts_1() {
    // Fig. 14: at 52.9 km the centreline is laminar to x/L 0.42 and turns
    // at about 0.45–0.5; at 47.5 km the rise starts at 0.27–0.32 and is
    // complete by 0.42; at 44.2 km it starts at 0.16–0.19 and is complete
    // by 0.27.
    let bands = [(0.32, 0.50), (0.19, 0.32), (0.12, 0.20)];
    let mut last = 1.0;
    for (p, (lo, hi)) in STS1.iter().zip(bands) {
        let x = onset(p);
        println!("{:.1} km, {} m/s, α {}°: onset at x/L {x:.2} (flight {lo}–{hi})", p.altitude / 1e3, p.speed, p.alpha);
        assert!(x >= lo && x <= hi, "{}: {x}", p.altitude);
        assert!(x < last);
        last = x;
    }
}

#[test]
fn acreage_heating_matches_sts_1_laminar_and_turbulent() {
    // Laminar ahead of the front and turbulent behind it, within 35 %
    // (Tauber's own calculation was within ~20 % laminar and "definitely
    // too high" turbulent; engineering methods ±20–30 %). Measured: our
    // laminar runs 15–30 % low (a 2D wedge: no lateral spreading of the
    // centreline flow), turbulent 20–35 % high.
    let laminar_points = [(0, &[0.19, 0.27, 0.32][..]), (1, &[0.16, 0.19, 0.27][..]), (2, &[0.16][..])];
    for (k, xs) in laminar_points {
        let p = &STS1[k];
        for &xl in xs {
            let want = p.data.iter().find(|d| d.0 == xl).expect("data").1;
            let (lam, _, _) = heating(p, xl);
            println!("{:.1} km x/L {xl}: laminar {:.2} W/cm² (flight {want})", p.altitude / 1e3, lam / 1e4);
            assert!((lam / 1e4 / want - 1.0).abs() < 0.35, "{} {xl}: {}", p.altitude, lam / 1e4);
        }
    }
    let turbulent_points = [(1, &[0.42, 0.50][..]), (2, &[0.32, 0.42, 0.50][..])];
    for (k, xs) in turbulent_points {
        let p = &STS1[k];
        for &xl in xs {
            let want = p.data.iter().find(|d| d.0 == xl).expect("data").1;
            let (_, total, gamma) = heating(p, xl);
            println!(
                "{:.1} km x/L {xl}: {:.2} W/cm² (flight {want}), turbulent share {gamma:.2}",
                p.altitude / 1e3,
                total / 1e4
            );
            assert!(gamma > 0.9, "{} {xl}: {gamma}", p.altitude);
            assert!((total / 1e4 / want - 1.0).abs() < 0.35, "{} {xl}: {}", p.altitude, total / 1e4);
        }
    }
}

#[test]
fn transition_raises_the_acreage_heating_several_times() {
    // STS-1 at 47.5 and 44.2 km: from 1.6–1.8 W/cm² laminar to 5.1–5.2
    // turbulent, and 1.15 to 3.7–3.85 (×3.0–3.3; Tauber: "turbulent
    // boundary layer heating can be several times greater than laminar").
    // Ours, the same points: laminar just ahead of the front against
    // turbulent behind it, ×3–6 (our laminar low, turbulent high).
    for (k, (ahead, behind)) in [(1, (0.27, 0.42)), (2, (0.16, 0.32))] {
        let p = &STS1[k];
        let (lam, _, _) = heating(p, ahead);
        let (_, turb, _) = heating(p, behind);
        let ratio = turb / lam;
        let flight = p.data.iter().find(|d| d.0 == behind).unwrap().1 / p.data.iter().find(|d| d.0 == ahead).unwrap().1;
        println!("{:.1} km: ×{ratio:.1} (flight ×{flight:.1})", p.altitude / 1e3);
        assert!(ratio > 2.5 && ratio < 6.5, "{ratio}");
    }
}

/// A cell shaded to exposure f gets f of the impact pressure (the edge
/// state), and so of Tauber's ρ∞ sin² δ in the real-gas turbulent heating:
/// at 40 km and 4.5 km/s a cell at f = 0.3 is not heated like one at f = 1
/// (review of 2026-10-26, AERO 4: both got ~1.2e6 W/m²).
#[test]
fn a_shaded_cell_gets_its_share_of_the_real_gas_heating() {
    let atm = sim::body::earth().atmosphere.expect("Earth has air");
    let (h, speed) = (40e3, 4_500.0);
    let a = atm.profile(h, G0);
    let flow = Flow {
        dir: DVec3::X,
        q: 0.5 * a.rho * speed * speed,
        mach: speed / a.speed_of_sound(1.4),
        knudsen: 0.0,
        gamma: 1.4,
        reynolds_per_m: a.rho * speed / Gas::Air.viscosity(a.temperature),
        temperature: a.temperature,
        speed,
        gas: Default::default(),
        roughness: 0.0,
    };
    let s = 40f64.to_radians().sin();
    let stream = Stream::of(&flow);
    let (x, tw) = (5.0, 1_000.0);
    let tauber = |f: f64| stream.edge(s, f).tauber_turbulent(x, tw);
    let (full, shaded) = (tauber(1.0), tauber(0.3));
    let scale = a.rho * speed * CP;
    println!("Tauber: f 1 {:.3e} W/m², f 0.3 {:.3e} W/m²", full * scale, shaded * scale);
    assert!(full * scale > 5e5, "{}", full * scale);
    // (ρ f sin² δ)^0.8.
    assert!((shaded / full / 0.3f64.powf(0.8) - 1.0).abs() < 1e-9, "{}", shaded / full);
    assert_eq!(tauber(0.0), 0.0);
    // The layer's turbulent heating: the larger of Tauber and the perfect
    // gas, each with the cell's share.
    let layer = |f: f64| stream.edge(s, f).layer(x, x, Some(tw), a.temperature, true).heat_turbulent;
    assert!(layer(0.3) < 0.8 * layer(1.0), "{} {}", layer(0.3), layer(1.0));
}
