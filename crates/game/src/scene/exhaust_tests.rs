use super::*;
use sim::catalog::EngineData;
use sim::systems::exhaust::{exit_state, jet, propellants, NozzleSpec};
use std::path::Path;

const SEA_LEVEL: f64 = 101_325.0;

/// The Falcon 9 crew craft's engines: Merlin 1D, Merlin Vacuum, SuperDraco.
fn engines() -> (EngineData, EngineData, EngineData) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let ship = sim::craft::load_definition(&path).unwrap().ship.unwrap();
    let find = |f: &dyn Fn(&EngineData) -> bool| ship.engines.iter().find(|e| f(&e.data)).unwrap().data.clone();
    (find(&|d| d.isp_vac == 311.0), find(&|d| d.isp_vac == 348.0), find(&|d| d.fuel == "MMH"))
}

/// Engine `d`'s plume at full flow in air at `p` Pa.
fn at(d: &EngineData, p: f64) -> Plume {
    let products = propellants().pair(&d.oxidizer, &d.fuel).unwrap();
    let s = exit_state(&NozzleSpec::of(d), products, d.mixture_ratio, 1.0);
    plume(&s, &jet(&s, p), p)
}

fn luminance(c: [f64; 3]) -> f64 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

#[test]
fn plumes_are_drawn_by_the_toggle_or_the_benchmark() {
    // (setting, benchmarking, the benchmark's part) → drawn
    for (setting, bench, part, want) in [
        (false, false, true, false),
        (true, false, true, true),
        (false, true, true, true),
        (true, true, false, false),
        (false, true, false, false),
    ] {
        assert_eq!(drawn(setting, bench, part), want, "{setting} {bench} {part}");
    }
}

#[test]
fn the_boundary_leaves_the_lip_at_the_turning_and_tends_to_the_expanded_radius() {
    // (slope, spread): under-, over-expanded, matched.
    for (k, d) in [(0.3, 2.0), (-0.1, -0.2), (0.0, 0.0)] {
        let p = Plume { r0: 1.0, slope: k, spread: d, ..Plume::default() };
        assert!((boundary(&p, 0.0) - 1.0).abs() < 1e-12);
        let h = 1e-6;
        assert!(((boundary(&p, h) - 1.0) / h - k).abs() < 1e-3, "slope at the lip {k}");
        assert!((boundary(&p, 1e7) - (1.0 + d)).abs() < 1e-3, "asymptote {d}");
    }
    // Continuous through matched: no pop when the jet crosses it.
    let near = |e: f64| boundary(&Plume { r0: 1.0, slope: e, spread: 10.0 * e, ..Plume::default() }, 5.0);
    assert!((near(1e-7) - 1.0).abs() < 1e-5 && (near(-1e-7) - 1.0).abs() < 1e-5);
}

#[test]
fn shocks_reheat_by_the_normal_shock_ratio_and_vanish_when_matched_or_in_vacuum() {
    assert!((normal_shock_temperature(1.0, 1.4) - 1.0).abs() < 1e-12);
    // Air at Mach 2: T₂/T₁ = 1.6875 (NACA 1135's tables).
    assert!((normal_shock_temperature(2.0, 1.4) - 1.6875).abs() < 1e-4);
    assert!(shock_strength(1.0).abs() < 1e-12, "matched");
    assert!(shock_strength(1e9) < 1e-6, "vacuum");
    for r in [0.7, 3.0] {
        assert!(shock_strength(r) > 0.1, "{r}");
    }
}

#[test]
fn planck_colour_is_balanced_at_d65_warm_below_and_close_to_the_true_luminance() {
    let w = planck_rgb(WHITE);
    assert!((w[0] / w[1] - 1.0).abs() < 1e-9 && (w[2] / w[1] - 1.0).abs() < 1e-9);
    assert!((luminance(planck_rgb(PLATINUM)) / PLATINUM_NITS - 1.0).abs() < 1e-9);
    let warm = planck_rgb(1_500.0);
    assert!(warm[0] > warm[1] && warm[1] > warm[2]);
    // Against Planck × CIE V(λ) (a Gaussian fit), relative to platinum.
    let true_lum = |t: f64| {
        (380..=780)
            .map(|nm| {
                let l = f64::from(nm) * 1e-9;
                let v = 1.019 * (-285.4 * (l * 1e6 - 0.559).powi(2)).exp();
                v / (l.powi(5) * ((C2 / (l * t)).exp() - 1.0))
            })
            .sum::<f64>()
    };
    for t in [1_200.0, 1_800.0, 3_000.0, 5_772.0] {
        let ours = luminance(planck_rgb(t)) / PLATINUM_NITS;
        let want = true_lum(t) / true_lum(PLATINUM);
        assert!((ours / want - 1.0).abs() < 0.3, "{t} K: {ours} vs {want}");
    }
}

#[test]
fn merlin_plumes_narrow_at_sea_level_fan_out_in_vacuum_and_never_pop() {
    let (m1d, mvac, superdraco) = engines();
    let sea = at(&m1d, SEA_LEVEL);
    assert!(sea.slope < 0.0 && sea.spread < 0.0, "overexpanded at sea level: {sea:?}");
    assert!(sea.shock > 0.05 && sea.burn > 0.5, "diamonds and afterburning low: {sea:?}");
    assert!(sea.length > 5.0 * sea.r0 && sea.width < 1.5 * sea.r0, "{sea:?}");
    let vac = at(&mvac, 0.0);
    // The lip turns the vacuum jet by ~68°, within what is drawn.
    assert!(vac.slope > 60f64.to_radians().tan() && vac.slope <= MAX_TURN.tan(), "the vacuum fan: {vac:?}");
    assert!(vac.shock < 1e-6 && vac.burn < 1e-6, "{vac:?}");
    assert!(vac.length > 2.0 * vac.r0 && vac.width > 3.0 * vac.r0, "a wide faint fan: {vac:?}");
    // Cooling as it spreads, below the exit temperature.
    assert!(temperature(&vac, vac.length) < 0.5 * vac.temperature);
    // No pop through matched: the plume at just above and below the exit
    // pressure differs little.
    let products = propellants().pair(&m1d.oxidizer, &m1d.fuel).unwrap();
    let pe = exit_state(&NozzleSpec::of(&m1d), products, m1d.mixture_ratio, 1.0).pressure;
    let (a, b) = (at(&m1d, pe * (1.0 - 1e-4)), at(&m1d, pe * (1.0 + 1e-4)));
    for (x, y) in [(a.width, b.width), (a.length, b.length), (a.shock + 1.0, b.shock + 1.0), (a.cell, b.cell)] {
        assert!((x / y - 1.0).abs() < 0.05, "{a:?} vs {b:?}");
    }
    // No soot, nothing drawn (SuperDraco's hypergolic plume is near invisible).
    assert_eq!(at(&superdraco, SEA_LEVEL).length, 0.0);
}

#[test]
fn the_plume_is_bright_where_shocked_or_burning_and_fades_with_altitude() {
    let (m1d, _, _) = engines();
    let hottest = |p: &Plume| (0..200).map(|i| temperature(p, p.length * f64::from(i) / 200.0)).fold(0.0, f64::max);
    let (sea, high) = (at(&m1d, SEA_LEVEL), at(&m1d, 100.0));
    assert!(hottest(&sea) > 1.2 * sea.temperature, "{}", hottest(&sea));
    assert!(hottest(&high) <= high.temperature * 1.01);
    assert!(luminance(planck_rgb(hottest(&sea))) > 10.0 * luminance(planck_rgb(hottest(&high))));
}
