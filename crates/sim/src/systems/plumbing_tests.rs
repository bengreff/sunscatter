//! Plumbing against its sources (ship-model.md §11; falcon9.md §3): tank
//! pressures, settling, NPSP at liftoff and near MECO, the regulator's
//! helium budget over a first-stage burn, the pad's warming of densified
//! LOX, boil-off at the relief pressure.

use super::*;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;

fn falcon9() -> (ShipDef, ShipState) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    let state = ShipState::initial(&def);
    ((*def).clone(), state)
}

fn tank(def: &ShipDef, id: &str) -> usize {
    def.tanks.iter().position(|t| t.id == id).unwrap()
}

/// As loaded: the pressurant's ideal-gas pressure at the loading pressure
/// plus the liquid's vapour pressure (densified LOX: 3.4 kPa at 66.5 K,
/// falcon9.md §3.1).
#[test]
fn a_tank_holds_its_loading_pressure_plus_its_vapour() {
    let (def, state) = falcon9();
    for (id, vapour) in [("s1.tank:lox", 3.4e3), ("s2.tank:lox", 3.4e3)] {
        let i = tank(&def, id);
        let p = pressure(&def.tanks[i], &state.tanks[i]);
        assert!((p - 3.5e5 - vapour).abs() < 300.0, "{id}: {p}");
    }
    let g = geometry(&def.tanks[tank(&def, "s1.tank:lox")], &state.tanks[tank(&def, "s1.tank:lox")]);
    assert!((g.density - 1_253.0).abs() < 2.0, "densified LOX: {}", g.density);
    assert!(g.ullage / def.tanks[tank(&def, "s1.tank:lox")].volume < 0.03, "about 2 % ullage");
}

/// Settling (falcon9.md §3.5): under thrust the liquid stays on its
/// outlet; a small deceleration moves it ½·a·t² (MECO's 3 s of drag:
/// centimetres); in free fall surface tension disperses it; the nitrogen
/// thrusters' 0.015 m/s² settle it again in about √(2x/a).
#[test]
fn settling_table() {
    let (def, state) = falcon9();
    let i = tank(&def, "s2.tank:lox");
    let (t, mut s) = (def.tanks[i].clone(), state.tanks[i].clone());
    let g = geometry(&t, &s);
    let run0 = TankRun::default();
    // (acceleration m/s², seconds) → drift (m), covered
    let mut run = run0.clone();
    for _ in 0..150 {
        settle(&mut run, &t, &g, 13.6, 0.02);
    }
    assert_eq!((run.drift, covered(&run, &g, s.liquid)), (0.0, true));
    let mut run = run0.clone();
    for _ in 0..150 {
        settle(&mut run, &t, &g, -0.008, 0.02);
    }
    assert!((run.drift - 0.5 * 0.008 * 9.0).abs() < 1e-3, "{}", run.drift);
    assert!(covered(&run, &g, s.liquid), "a few centimetres after MECO");
    // Nearly empty after the burn, then ten minutes of free fall.
    s.liquid = 2_000.0;
    let g = geometry(&t, &s);
    let mut run = run0.clone();
    for _ in 0..10 {
        settle(&mut run, &t, &g, 1e-7, 60.0);
    }
    assert!(!covered(&run, &g, s.liquid), "floating after a coast: {} m", run.drift);
    let x = run.drift;
    let mut seconds = 0.0;
    while !covered(&run, &g, s.liquid) {
        settle(&mut run, &t, &g, 0.015, 0.02);
        seconds += 0.02;
    }
    let expect = crate::math::sqrt_pos(2.0 * (x - cover(&g)) / 0.015);
    assert!((seconds - expect).abs() < 0.5, "settled in {seconds} s, √(2x/a) = {expect}");
}

/// NPSP at the Merlins' LOX inlets (falcon9.md §3.4): at liftoff (1.38 g of
/// thrust, a full tank above a 16 m downcomer) about 10 bar of inlet
/// pressure; near MECO (4 g, a metre of liquid) about 4 bar; both far
/// above the 1.7 bar the pumps need.
#[test]
fn npsp_at_liftoff_and_near_meco() {
    let (def, mut state) = falcon9();
    let i = tank(&def, "s1.tank:lox");
    let e9 = def.engines.iter().find(|e| e.id == "s1.merlin.9").unwrap();
    let a = |a: f64| -> Vec<f64> { vec![a; def.tanks.len()] };
    let inlet = |state: &ShipState, acc: f64| {
        let f = feeds(&def, state, &a(acc))[i];
        (f.npsp(1.0, e9.data.pos) + f.vapour, f.npsp(1.0, e9.data.pos))
    };
    let (p, npsp) = inlet(&state, 13.6);
    println!("liftoff: inlet {:.2} bar, NPSP {:.2} bar", p / 1e5, npsp / 1e5);
    assert!((p / 1e5 - 10.1).abs() < 1.0, "falcon9.md §3.4: 10.1 bar; {}", p / 1e5);
    let g = geometry(&def.tanks[i], &state.tanks[i]);
    state.tanks[i].liquid = g.density * crate::math::PI * g.radius * g.radius * 1.0;
    let (p, npsp) = inlet(&state, 4.0 * 9.80665);
    println!("near MECO: inlet {:.2} bar, NPSP {:.2} bar", p / 1e5, npsp / 1e5);
    assert!(npsp > e9.data.npsp_ox && p / 1e5 > 3.0 && p / 1e5 < 12.0);
}

/// The first stage's burn (150 s at the nine Merlins' full flow): the
/// regulator holds both tanks at 3.5 bar from the six COPVs, with heated
/// helium; falcon9.md §1.2 derives ~260 kg of helium at MECO at 250 K
/// (colder ullage gas, as here, takes more).
#[test]
fn the_regulator_holds_the_tanks_through_the_burn() {
    let (def, mut state) = falcon9();
    let (lox, rp1) = (tank(&def, "s1.tank:lox"), tank(&def, "s1.tank:rp1"));
    for (e, _) in def.engines.iter().enumerate().filter(|(_, e)| e.id.starts_with("s1.merlin")) {
        state.engines[e].status = EngineStatus::Running;
    }
    let flow = 9.0 * 299.7;
    let (ox, fuel) = (flow * 2.33 / 3.33, flow / 3.33);
    let (dt, mut lowest) = (0.02, f64::INFINITY);
    let a = vec![20.0; def.tanks.len()];
    for _ in 0..(150.0 / dt) as usize {
        let before = ullages(&def, &state);
        state.tanks[lox].liquid = (state.tanks[lox].liquid - ox * dt).max(0.0);
        state.tanks[rp1].liquid = (state.tanks[rp1].liquid - fuel * dt).max(0.0);
        fast(&def, &mut state, &before, &a, dt);
        slow(&def, &mut state, &Surroundings { sun: DVec3::ZERO, air_k: None }, dt);
        lowest = lowest.min(pressure(&def.tanks[lox], &state.tanks[lox]));
    }
    let helium: f64 = [lox, rp1].iter().map(|&i| state.runtime.tanks[i].pressurant_in).sum::<f64>() * 0.0040026;
    let left: f64 = (def.bottles.iter().zip(&state.bottles))
        .filter(|(b, _)| b.id.starts_with("s1.helium"))
        .map(|(_, s)| s.moles * 0.0040026)
        .sum();
    println!("lowest LOX ullage {:.3} bar, helium let in {helium:.0} kg, {left:.0} kg left", lowest / 1e5);
    assert!(lowest > 3.45e5, "held at 3.5 bar: {lowest}");
    assert!((200.0..600.0).contains(&helium), "{helium} kg");
    assert!(left > 0.0, "the bottles last the burn");
    let p_rp1 = pressure(&def.tanks[rp1], &state.tanks[rp1]);
    assert!((p_rp1 - 3.5e5).abs() < 0.1e5, "{p_rp1}");
}

/// On the pad (falcon9.md §3.6): the second stage's densified LOX warms
/// ~0.6 K in 10 minutes from the air through its frosted wall (DATA: 2 kW/m²
/// on ~62 m², 0.97 mK/s; here 0.72 K, the bottom dome wetted too); RP-1 at
/// 266.5 K warms far less.
#[test]
fn densified_lox_warms_on_the_pad() {
    let (def, mut state) = falcon9();
    let (lox, rp1) = (tank(&def, "s2.tank:lox"), tank(&def, "s2.tank:rp1"));
    let air = Surroundings { sun: DVec3::ZERO, air_k: Some(288.15) };
    let (t0, f0) = (state.tanks[lox].liquid_k, state.tanks[rp1].liquid_k);
    for _ in 0..10 {
        slow(&def, &mut state, &air, 60.0);
    }
    let (dl, df) = (state.tanks[lox].liquid_k - t0, state.tanks[rp1].liquid_k - f0);
    println!("10 min on the pad: LOX +{dl:.2} K, RP-1 +{df:.3} K");
    assert!((0.4..1.0).contains(&dl), "{dl}");
    assert!(df > 0.0 && df < 0.2 * dl, "{df}");
    // Not boiling: the expanding liquid squeezes its 2 % ullage, which
    // vents a little pressurant at the relief pressure (4 bar).
    let p = pressure(&def.tanks[lox], &state.tanks[lox]);
    assert!(p <= def.tanks[lox].pressure + 1.0, "{p}");
    assert!(state.runtime.tanks[lox].vented < 5.0, "{} kg", state.runtime.tanks[lox].vented);
}

/// A liquid at saturation at its relief pressure vents what heats it: Q/L.
#[test]
fn at_saturation_the_heat_boils_off() {
    let (def, mut state) = falcon9();
    let i = tank(&def, "s2.tank:lox");
    let l = liquid_of(&def.tanks[i]).unwrap();
    state.tanks[i].liquid_k = l.saturation_temperature(def.tanks[i].pressure);
    state.tanks[i].ullage_moles = 0.0;
    let air = Surroundings { sun: DVec3::ZERO, air_k: Some(288.15) };
    let q = heat_in(&def, &state, i, &air);
    let m0 = state.tanks[i].liquid;
    let vented = slow(&def, &mut state, &air, 60.0);
    let boiled = m0 - state.tanks[i].liquid;
    let expect = q * 60.0 / l.latent_heat;
    assert!((boiled / expect - 1.0).abs() < 0.02, "{boiled} kg vs Q·t/L {expect} kg");
    assert!(vented >= boiled);
}

#[test]
fn the_bottles_feed_their_tanks() {
    let (def, _) = falcon9();
    let names =
        |t: &str| bottles_of(&def, tank(&def, t)).iter().map(|&b| def.bottles[b].id.clone()).collect::<Vec<_>>();
    assert_eq!(names("s2.tank:lox"), ["s2.helium#1", "s2.helium#2", "s2.helium#3"]);
    assert_eq!(names("s1.tank:rp1").len(), 6);
    assert_eq!(names("dragon.mmh#2:main"), ["dragon.helium#1", "dragon.helium#2"]);
}

/// A tank burned dry (the review's case: the booster's LOX after a burn to
/// depletion read 0 kg, no pressurant and an 8.7·10⁸ K ullage, from the
/// sunlight's heat in a 10⁻¹⁵ kg residual; in a 10⁻¹⁰–10⁻⁶ kg residual a
/// vent cooled the liquid under 0 K and its vapour pressure read 42 bar).
/// The first stage's tanks drained at full flow, then a residual from none
/// to a gram left in the LOX tank, coasting two minutes in full sunlight:
/// the liquid stays between its freezing and critical points, the ullage
/// under 1,000 K, every pressure at most its relief's and nothing bursts.
#[test]
fn a_tank_burned_dry_stays_physical() {
    let (def, mut state) = falcon9();
    let (lox, rp1) = (tank(&def, "s1.tank:lox"), tank(&def, "s1.tank:rp1"));
    let flow = 9.0 * 299.7;
    let (ox, fuel, dt) = (flow * 2.33 / 3.33, flow / 3.33, 0.02);
    let a = vec![20.0; def.tanks.len()];
    while state.tanks[lox].liquid > 0.0 || state.tanks[rp1].liquid > 0.0 {
        let before = ullages(&def, &state);
        drain(&mut state.tanks[lox], ox * dt);
        drain(&mut state.tanks[rp1], fuel * dt);
        fast(&def, &mut state, &before, &a, dt);
        slow(&def, &mut state, &Surroundings { sun: DVec3::ZERO, air_k: None }, dt);
    }
    let sun = Surroundings { sun: DVec3::new(816.6, 0.0, -1_088.8), air_k: None };
    let at = crate::time::Epoch::new(845_000_000, 0.0);
    for residual in [0.0, 1e-15, 1e-9, 2e-6, 1e-3] {
        let mut s = state.clone();
        s.tanks[lox].liquid = residual;
        let free = vec![0.0; def.tanks.len()];
        for _ in 0..6_000 {
            let before = ullages(&def, &s);
            fast(&def, &mut s, &before, &free, dt);
            slow(&def, &mut s, &sun, dt);
            for i in [lox, rp1] {
                let (t, x) = (&def.tanks[i], &s.tanks[i]);
                let l = liquid_of(t).unwrap();
                let p = pressure(t, x);
                let sane = (l.freezing_k..=l.critical_k).contains(&x.liquid_k) && x.ullage_k > 1.0 && x.ullage_k < 1e3;
                assert!(sane && p <= t.pressure + 1.0, "{} with {residual} kg: {x:?}, {p} Pa", t.id);
            }
            let burst = crate::systems::failure::physical::bursts(&def, &mut s, 0.0, at);
            assert!(burst.is_empty(), "{residual} kg: {burst:?}");
        }
        println!(
            "{residual} kg left: LOX {:?}, {:.2} bar",
            s.tanks[lox],
            pressure(&def.tanks[lox], &s.tanks[lox]) / 1e5
        );
    }
}
