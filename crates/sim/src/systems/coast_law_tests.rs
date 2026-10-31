//! The coast's law of the tanks (D119) against the lattice it replaces on
//! coasts (`plumbing::slow` stepped every 60 s) and against the hand.

use super::*;
use crate::systems::plumbing::{heat_in, Surroundings};
use glam::DVec3;

fn falcon9() -> (ShipDef, ShipState) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    let mut state = ShipState::initial(&def);
    state.runtime.refit(&def);
    ((*def).clone(), state)
}

fn tank(def: &ShipDef, id: &str) -> usize {
    def.tanks.iter().position(|t| t.id == id).unwrap()
}

/// Sunlight from the side of the stage, at an angle to its axis (body Z).
fn sun() -> DVec3 {
    DVec3::new(1.0, 0.2, 0.4).normalize() * 1_361.0
}

/// Steps `state` like a coast's lattice before the law: `plumbing::slow`
/// every 60 s for `seconds`.
fn stepped(def: &ShipDef, state: &mut ShipState, env: &Surroundings, seconds: f64) {
    for _ in 0..(seconds / 60.0) as usize {
        plumbing::slow(def, state, env, 60.0);
    }
}

/// The heat law is the lattice's heat (`plumbing::heat_in`) for one
/// sunlight sample, in vacuum and in air, at any height and temperature
/// (its terms regrouped: equal to rounding).
#[test]
fn the_heat_law_is_the_lattices_heat() {
    let (def, mut state) = falcon9();
    for id in ["s2.tank:lox", "s2.tank:rp1", "s1.tank:lox"] {
        let i = tank(&def, id);
        for air_k in [None, Some(250.0)] {
            let law = plumbing::heat_law(&def, i, &[sun()], air_k);
            for (fill, k) in [(1.0, 70.0), (0.3, 95.0), (0.02, 300.0)] {
                state.tanks[i].liquid = def.tanks[i].capacity * fill;
                state.tanks[i].liquid_k = k;
                let g = plumbing::geometry(&def.tanks[i], &state.tanks[i]);
                let lattice = heat_in(&def, &state, i, &Surroundings { sun: sun(), air_k });
                let mine = law.at(g.radius, g.height, k);
                assert!((mine - lattice).abs() <= 1e-9 * lattice.abs(), "{id} {air_k:?} {fill}: {mine} vs {lattice}");
            }
        }
    }
}

/// Boiling at saturation (3), against the hand and the lattice: the second
/// stage's LOX at a third of its load (what is left after orbit insertion)
/// at saturation at its relief pressure, no helium left, in steady
/// sunlight in vacuum for a day.
///
/// By hand: at T_s the heat is linear in the height h = m/(ρ_s·πR²),
/// Q = a + b·m with a = α·F_end·πR² − εσT_s⁴·πR² (the end) and
/// b = (α·F_side·2R − εσT_s⁴·2πR)/(ρ_s·πR²) (the side); L·dm/dt = −Q(m) is
/// linear, so m(t) = (m₀ + a/b)·e^(−b·t/L) − a/b, written
/// m₀ − (a + b·m₀)·(t/L)·φ(b·t/L) with φ(x) = (1 − e⁻ˣ)/x for b → 0. Here
/// b·t/L ≈ 5·10⁻³ over the day: the boil-off is nearly Q(m₀)·t/L, a little
/// less as the wetted side shrinks. The lattice steps the same heat by
/// Euler (Q at the step's start for 60 s, boiled at once: an error of
/// b·dt/(2L) ≈ 10⁻⁷ of the boil-off per step's share): the two agree within
/// 10⁻³ of what boiled.
#[test]
fn boiling_at_saturation_agrees_with_the_lattice_and_the_hand() {
    let (def, mut state) = falcon9();
    let i = tank(&def, "s2.tank:lox");
    let t = &def.tanks[i];
    let l = plumbing::liquid_of(t).unwrap();
    let t_s = l.saturation_temperature(t.pressure);
    state.tanks[i].liquid *= 0.3;
    state.tanks[i].liquid_k = t_s;
    state.tanks[i].ullage_moles = 0.0;
    let env = Surroundings { sun: sun(), air_k: None };
    let heat: Vec<_> = (0..def.tanks.len()).map(|j| plumbing::heat_law(&def, j, &[sun()], None)).collect();
    let law = CoastLaw::start(&def, &state, &heat);
    let day = 86_400.0;
    let m0 = state.tanks[i].liquid;
    let mut lattice = state.clone();
    stepped(&def, &mut lattice, &env, day);
    let mut by_law = state.clone();
    law.apply(&def, &mut by_law, day);
    // By hand from the tank's geometry and fluid.
    let (r, alpha, eps_sigma) = (t.region.radius, heat[i].side / (2.0 * t.region.radius), heat[i].emit);
    let area = crate::math::PI * r * r;
    let radiated = eps_sigma * crate::math::powi(t_s, 4);
    let a = heat[i].end - radiated * area;
    let b = (alpha * 2.0 * r - radiated * 2.0 * crate::math::PI * r) / (l.density(t_s) * area);
    let hand = (m0 + a / b) * crate::math::exp(-b * day / l.latent_heat) - a / b;
    let (boiled, lattice_boiled) = (m0 - by_law.tanks[i].liquid, m0 - lattice.tanks[i].liquid);
    println!(
        "a {a:.1} W, b {b:.4} W/kg, b·t/L {:.2e}: boiled by the law {boiled:.3} kg, by hand {:.3} kg, \
         by the lattice {lattice_boiled:.3} kg",
        b * day / l.latent_heat,
        m0 - hand
    );
    assert!(((m0 - hand) / boiled - 1.0).abs() < 1e-9, "the hand");
    assert!((lattice_boiled / boiled - 1.0).abs() < 1e-3, "the lattice");
    assert_eq!(by_law.tanks[i].liquid_k, t_s);
    assert_eq!(by_law.tanks[i].vented, boiled, "the vented ledger");
    // Bit for bit: the law's total is the ship state's consumables there.
    assert_eq!(law.total(&def, day).to_bits(), by_law.consumables(&def).to_bits());
}

/// Warming (1) and venting the pressurant (2), against the lattice: a
/// tenth of the second stage's densified LOX (66.5 K) and its helium in
/// steady sunlight in vacuum for two days, sealed until its pressure
/// reaches the relief (about 20 h), then venting the helium as the vapour
/// pressure rises. Sealed, the temperatures agree within 0.01 K. Venting,
/// the lattice also lets out the vapour mixed with the helium, cooling the
/// liquid by its latent heat, which the law leaves to the boil-off at
/// saturation: after two days the law is 1.3 K warmer, and the heat taken
/// in, m·c·(T − T₀) + L·(liquid lost), agrees within 1 %.
#[test]
fn warming_and_relief_agree_with_the_lattice() {
    let (def, mut state) = falcon9();
    let i = tank(&def, "s2.tank:lox");
    // A tenth of the load: it warms ten times as fast.
    state.tanks[i].liquid *= 0.1;
    let env = Surroundings { sun: sun(), air_k: None };
    let heat: Vec<_> = (0..def.tanks.len()).map(|j| plumbing::heat_law(&def, j, &[sun()], None)).collect();
    // Both from the law's start (the gas at the liquid's temperature, the
    // regulator's top-up), the law taking a span an hour as on a coast.
    plumbing::equilibrate(&def, &mut state.tanks, &mut state.bottles);
    let l = plumbing::liquid_of(&def.tanks[i]).unwrap();
    let (k0, m0) = (state.tanks[i].liquid_k, state.tanks[i].liquid);
    let mut law = CoastLaw::start(&def, &state, &heat);
    for h in 1..48 {
        law.add_span(&def, f64::from(h) * SPAN, &heat);
    }
    let (mut lattice, mut by_law) = (state.clone(), state.clone());
    let mut done = 0.0;
    for hours in [1.0, 6.0, 24.0, 48.0] {
        stepped(&def, &mut lattice, &env, (hours - done) * 3_600.0);
        done = hours;
        law.apply(&def, &mut by_law, hours * 3_600.0);
        let (a, b) = (&lattice.tanks[i], &by_law.tanks[i]);
        println!(
            "{hours} h: {:.3} K and {:.1} mol by the lattice, {:.3} K and {:.1} mol by the law, p {:.0} Pa",
            a.liquid_k,
            a.ullage_moles,
            b.liquid_k,
            b.ullage_moles,
            plumbing::pressure(&def.tanks[i], b)
        );
        let taken = |s: &TankState| s.liquid * l.specific_heat * (s.liquid_k - k0) + l.latent_heat * (m0 - s.liquid);
        println!("  heat taken {:.4e} J by the lattice, {:.4e} J by the law", taken(a), taken(b));
        if plumbing::pressure(&def.tanks[i], b) < def.tanks[i].pressure {
            assert!((a.liquid_k - b.liquid_k).abs() < 0.01, "{hours} h");
        }
        assert!((taken(b) / taken(a) - 1.0).abs() < 0.01, "{hours} h");
    }
    // The helium vented by then: within 15 % of the lattice's (the law
    // warms a little faster, the vapour's heat kept in the liquid).
    let n0 = state.tanks[i].ullage_moles;
    let (lattice_out, out) = (n0 - lattice.tanks[i].ullage_moles, n0 - by_law.tanks[i].ullage_moles);
    assert!(out > 0.3 * n0 && (out / lattice_out - 1.0).abs() < 0.15, "{out} mol vs {lattice_out} mol");
}

/// The warming's closed form inverted: F(x) at the x Newton's method finds
/// is the target, on both sides of the equilibrium.
#[test]
fn the_warmings_inverse_finds_its_temperature() {
    for (x0, delta) in [(0.1, 1e-3), (0.2, 0.5), (0.9, 2.0), (0.999, 1e-6), (1.5, 0.1), (3.0, 1.0)] {
        let target = big_f(x0) + delta;
        let x = invert_f(x0, target);
        assert!((big_f(x) - target).abs() < 1e-12 * target.abs().max(1.0), "{x0}: {x}");
        assert_eq!(x > 1.0, x0 > 1.0);
    }
}
