use super::*;
use crate::craft::engine::P_SEA_LEVEL;

/// The Falcon 9 crew craft's engines: Merlin 1D, Merlin Vacuum, SuperDraco.
fn engines() -> (EngineData, EngineData, EngineData) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let ship = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    let find = |f: &dyn Fn(&EngineData) -> bool| ship.engines.iter().find(|e| f(&e.data)).unwrap().data.clone();
    (find(&|d| d.isp_vac == 311.0), find(&|d| d.isp_vac == 348.0), find(&|d| d.fuel == "MMH"))
}

fn seed(d: &EngineData, level: f64) -> ExitState {
    let products = propellants().pair(&d.oxidizer, &d.fuel).expect("a propellant pair");
    exit_state(&NozzleSpec::of(d), products, d.mixture_ratio, level)
}

#[test]
fn the_table_has_the_craft_files_propellants() {
    let t = propellants();
    for (o, f) in [("LOX", "RP-1"), ("NTO", "MMH"), ("LOX", "LH2")] {
        assert!(t.pair(o, f).is_some(), "{o}/{f}");
    }
    assert!(t.pair("RP-1", "LOX").is_none(), "oxidizer first");
    assert!(Propellants::parse(
        "{\"X/Y\": (gamma: 0.9, molar_mass: 20.0, chamber_temperature: 3000.0, \
         mixture_ratio: 2.0, stoichiometric_ratio: 3.0, water: 0.3, soot: 0.0, alumina: 0.0, source: \"\")}"
    )
    .is_err());
}

#[test]
fn the_area_mach_relation_round_trips() {
    // (area ratio, γ) → Mach: textbook values (γ 1.4: ε 1.6875 at M 2).
    assert!((supersonic_mach(1.6875, 1.4) - 2.0).abs() < 1e-6);
    assert!((supersonic_mach(1.0, 1.4) - 1.0).abs() < 1e-6);
    for (eps, g) in [(16.0, 1.24), (165.0, 1.24), (9.77, 1.25), (80.0, 1.26)] {
        let m = supersonic_mach(eps, g);
        assert!(m > 1.0 && (area_ratio(m, g) / eps - 1.0).abs() < 1e-9, "{eps} {g}: {m}");
    }
    // ν(2) = 26.38° in air; the vacuum limit (π/2)(√((γ+1)/(γ−1)) − 1) = 130.45°.
    assert!((prandtl_meyer(2.0, 1.4).to_degrees() - 26.38).abs() < 0.01);
    assert!((prandtl_meyer(1e6, 1.4).to_degrees() - 130.45).abs() < 0.01);
}

/// The exit state against the published thrust: `F_vac = ṁ·vₑ + pₑ·Aₑ`
/// exactly at every level; the chamber temperature it implies within 10 %
/// of the handbook's for the Merlins (their Isp and nozzles are published).
#[test]
fn the_seed_gives_the_published_thrust() {
    let (m1d, mvac, superdraco) = engines();
    for d in [&m1d, &mvac, &superdraco] {
        let n = NozzleSpec::of(d);
        for level in [1.0, 0.64, 0.4] {
            let s = seed(d, level);
            let thrust = s.mdot * s.velocity + s.pressure * n.exit_area;
            assert!((thrust / (level * d.thrust_vac) - 1.0).abs() < 1e-12, "{}: {thrust}", d.isp_vac);
            assert!((s.pressure / seed(d, 1.0).pressure / level - 1.0).abs() < 1e-12, "pₑ ∝ level");
            assert_eq!(s.temperature, seed(d, 1.0).temperature);
            assert!((n.level(level * d.thrust_vac - 1e3 * n.exit_area, 1e3) - level).abs() < 1e-12);
        }
    }
    for d in [&m1d, &mvac] {
        let t = chamber_temperature(&seed(d, 1.0));
        assert!((t / 3677.0 - 1.0).abs() < 0.10, "{}: Tc {t}", d.isp_vac);
    }
}

/// Each engine at full flow: (exit pressure kPa range, exit temperature K
/// range, exit Mach range). Merlin 1D's ~0.7–0.8 atm is the figure quoted
/// for it (SpaceX does not publish it); the vacuum bell's few kPa;
/// SuperDraco (an abort engine) near matched at sea level.
#[test]
fn the_seed_table() {
    let (m1d, mvac, superdraco) = engines();
    let cases = [
        (&m1d, (60.0, 90.0), (1200.0, 1500.0), (3.6, 3.9)),
        (&mvac, (2.5, 5.0), (650.0, 900.0), (5.5, 5.8)),
        (&superdraco, (60.0, 200.0), (700.0, 1300.0), (3.2, 3.6)),
    ];
    for (d, p, t, m) in cases {
        let s = seed(d, 1.0);
        let within = |x: f64, (lo, hi): (f64, f64)| x >= lo && x <= hi;
        assert!(within(s.pressure / 1e3, p), "{}: pₑ {} kPa", d.isp_vac, s.pressure / 1e3);
        assert!(within(s.temperature, t), "{}: Tₑ {}", d.isp_vac, s.temperature);
        assert!(within(s.mach, m), "{}: Mₑ {}", d.isp_vac, s.mach);
        assert!(s.velocity < d.isp_vac * crate::vessel::G0, "vₑ below the effective velocity");
    }
    // Fuel-rich kerolox leaves a third of its fuel to afterburn; soot only from carbon.
    let s = seed(&m1d, 1.0).composition;
    assert!((s.unburned - (1.0 - 2.33 / 3.41)).abs() < 1e-12 && s.soot > 0.0);
    assert_eq!(seed(&superdraco, 1.0).composition.soot, 0.0);
    assert_eq!(seed(&m1d, 0.0).mdot, 0.0);
}

/// The jet: underexpanded in thin air (turning out, wider than the exit),
/// overexpanded at sea level for the vacuum bell (turning in, narrower);
/// continuous through the matched pressure, and vacuum the limit.
#[test]
fn the_jet_in_the_local_air() {
    let (m1d, mvac, _) = engines();
    let s = seed(&m1d, 1.0);
    let matched = jet(&s, s.pressure);
    assert!((matched.mach - s.mach).abs() < 1e-9 && matched.turn.abs() < 1e-9);
    assert!((matched.radius / s.radius - 1.0).abs() < 1e-9);
    let sea = jet(&s, P_SEA_LEVEL);
    assert!(sea.pressure_ratio < 1.0 && sea.turn < 0.0 && sea.radius < s.radius);
    let high = jet(&s, 1e3);
    assert!(high.turn > 0.0 && high.radius > s.radius && high.cell > sea.cell);
    // Continuity: a 1 Pa change near the match moves the turning by a hair.
    let (a, b) = (jet(&s, s.pressure - 0.5), jet(&s, s.pressure + 0.5));
    assert!((a.turn - b.turn).abs() < 1e-4);
    // Vacuum: the turning approaches ν_max − ν(Mₑ); the floor is the limit.
    let v = jet(&seed(&mvac, 1.0), 0.0);
    let g: f64 = (1.24 + 1.0) / (1.24 - 1.0);
    let limit = math::PI / 2.0 * g.sqrt() - math::PI / 2.0 - prandtl_meyer(seed(&mvac, 1.0).mach, 1.24);
    assert!(v.turn > 0.0 && v.turn < limit && limit - v.turn < 0.2, "{} vs {limit}", v.turn);
    assert_eq!(jet(&seed(&mvac, 1.0), -1.0), v);
    // The shock-cell length (Prandtl–Pack) at sea level: about a few exit diameters.
    let d_e = 2.0 * s.radius;
    assert!(sea.cell > 2.0 * d_e && sea.cell < 6.0 * d_e, "{} m", sea.cell);
}
