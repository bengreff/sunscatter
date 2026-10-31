//! The stage estimates on the Falcon 9 (`data/craft/falcon9-crew`): its
//! sequence's two burns, the burn now, what unmeasured tanks do, a feed
//! list drained tank after tank, a setting without flow, the pieces a
//! separation leaves.

use super::*;
use crate::craft::command::Stage;
use crate::systems::data;

fn falcon9() -> (ShipDef, ShipState, Vec<Hull>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let craft = crate::craft::load_definition(&path).unwrap();
    let def = (**craft.ship.as_ref().unwrap()).clone();
    let state = ShipState::initial(&def);
    (def, state, hull(&craft.parts))
}

fn inputs<'a>(hull: &'a [Hull], k: &'a Knowledge, p: f64) -> Inputs<'a> {
    Inputs { hull, knowledge: k, throttle: 1.0, p, g: Some(9.81) }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs())
}

/// The burn's Δv is the rocket equation's at its constant Isp, its time
/// what it spent over its flow.
fn consistent(b: &Burn) {
    let (m0, m1) = (b.mass.value, b.burnout_mass.unwrap().value);
    let dv = b.isp * G0 * math::ln(m0 / m1);
    assert!(close(b.dv.unwrap().value, dv, 1e-9), "{} vs {dv}", b.dv.unwrap().value);
    let mdot = b.thrust.unwrap().value / (b.isp * G0);
    assert!(close(b.time.unwrap().value, (m0 - m1) / mdot, 1e-9));
}

#[test]
fn on_the_pad_the_sequence_has_two_burns() {
    let (def, state, hull) = falcon9();
    let k = Knowledge::perfect();
    let e = estimate(&def, &state, &inputs(&hull, &k, 0.0));
    assert!(e.now.is_none(), "nothing burns on the pad");
    let burns: Vec<(usize, bool)> = e.stages.iter().map(|(j, b)| (*j, b.is_some())).collect();
    let names: Vec<&str> = state.stages.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(burns.len(), names.len());
    for (j, burns) in burns {
        let want = matches!(names[j], "Ignition" | "Second stage ignition");
        assert_eq!(burns, want, "{}", names[j]);
    }
    let s1 = e.stages[0].1.as_ref().unwrap();
    let s2 = e.stages[4].1.as_ref().unwrap();
    assert_eq!((s1.engines.len(), s2.engines.len()), (9, 1));
    for b in [s1, s2] {
        let fluids: Vec<&str> = b.bars.iter().map(|b| b.fluid.as_str()).collect();
        assert_eq!(fluids.len(), 2, "{fluids:?}");
        for bar in &b.bars {
            assert!(close(bar.kg.unwrap().value, bar.capacity, 1e-9), "{}: loaded full", bar.fluid);
            assert_eq!(bar.kg.unwrap().precision, 0.0, "debug knows exactly");
        }
        consistent(b);
    }
    let (m0, dv1, dv2) = (s1.mass.value, s1.dv.unwrap().value, s2.dv.unwrap().value);
    println!(
        "liftoff {m0:.0} kg, s1 {dv1:.0} m/s in {:.0} s, s2 {dv2:.0} m/s in {:.0} s",
        s1.time.unwrap().value,
        s2.time.unwrap().value
    );
    assert!((540e3..580e3).contains(&m0), "Falcon 9 with Crew Dragon: ~549 t plus Dragon: {m0}");
    let left = s1.burnout_mass.unwrap().value - s2.mass.value;
    assert!((20e3..40e3).contains(&left), "the first stage leaves at separation: {left}");
    assert!((3_000.0..5_500.0).contains(&dv1) && (5_000.0..8_000.0).contains(&dv2), "{dv1} {dv2}");
}

#[test]
fn the_burn_now_carries_thrust_weight_ratio_and_mean_isp() {
    let (def, mut state, hull) = falcon9();
    state.next_stage(&def).unwrap();
    for r in state.engines.iter_mut() {
        r.level = 1.0;
    }
    let k = Knowledge::perfect();
    let p = 101_325.0;
    let air = estimate(&def, &state, &inputs(&hull, &k, p));
    let vac = estimate(&def, &state, &inputs(&hull, &k, 0.0));
    let now = air.now.as_ref().expect("the first stage burns");
    let want: f64 = (state.engines.iter().zip(&def.engines))
        .filter(|(e, _)| e.status == EngineStatus::Running)
        .map(|(_, d)| propulsion::steady(&d.data, propulsion::setting(&d.data, 1.0, 1.0), p).0)
        .sum();
    assert!(close(now.thrust.unwrap().value, want, 1e-12));
    let twr = now.twr.unwrap().value;
    assert!(close(twr, want / (now.mass.value * 9.81), 1e-12));
    assert!((1.1..1.6).contains(&twr), "Falcon 9 lifts off at ~1.4: {twr}");
    assert!((270.0..290.0).contains(&now.isp), "Merlin 1D at sea level: {}", now.isp);
    consistent(now);
    let vac_now = vac.now.as_ref().unwrap();
    assert!(vac_now.dv.unwrap().value > now.dv.unwrap().value, "the vacuum's Δv is the larger");
    assert_eq!(vac_now.time, now.time, "the flow does not depend on the air");
    // The sequence's next stage (Liftoff) has nothing left to burn.
    assert_eq!(air.stages[0].0, 1);
    assert!(air.stages[0].1.is_none());
}

/// Knowledge as the real one less the level of `skip`.
fn without(k: &Knowledge, skip: Quantity) -> Knowledge {
    let mut out = Knowledge::default();
    for (q, p) in k.iter().filter(|(q, _)| *q != skip) {
        out.learn(q, p);
    }
    out
}

#[test]
fn unmeasured_tanks_widen_or_hide_the_estimates() {
    let (def, state, hull) = falcon9();
    let k = data::knowledge(&def, &state, false);
    let e = estimate(&def, &state, &inputs(&hull, &k, 0.0));
    let s1 = e.stages[0].1.as_ref().unwrap();
    assert!(s1.bars.iter().all(|b| b.kg.is_some_and(|r| r.precision > 0.0)), "level sensors: to their accuracy");
    let dv = s1.dv.unwrap();
    // Dragon's tanks have no level sensor (falcon9.md §5.2): its
    // propellant is anywhere from empty to full.
    let unmeasured: f64 =
        (0..def.tanks.len()).filter(|&t| !k.knows(Quantity::TankLevel(t as u32))).map(|t| def.tanks[t].capacity).sum();
    assert!(unmeasured > 0.0);
    assert!(s1.mass.precision >= unmeasured * 0.99, "{} vs {unmeasured}", s1.mass.precision);
    assert!(dv.precision > 0.0 && dv.precision < 0.05 * dv.value, "{dv:?}");
    // Without the first stage's LOX level the burn is not known at all.
    let lox = def.tanks.iter().position(|t| t.id == "s1.tank:lox").unwrap() as u32;
    let blind = without(&k, Quantity::TankLevel(lox));
    let e = estimate(&def, &state, &inputs(&hull, &blind, 0.0));
    let s1 = e.stages[0].1.as_ref().unwrap();
    let known: Vec<(String, bool)> = s1.bars.iter().map(|b| (b.fluid.clone(), b.kg.is_some())).collect();
    assert_eq!(known.iter().filter(|(_, k)| !k).count(), 1, "{known:?}");
    assert!(s1.time.is_none() && s1.dv.is_none() && s1.burnout_mass.is_none());
    assert!(e.stages[4].1.as_ref().unwrap().dv.is_some(), "the second stage's own tanks are measured");
}

#[test]
fn a_feed_list_drains_tank_after_tank() {
    let (mut def, mut state, hull) = falcon9();
    let tank = |id: &str| def.tanks.iter().position(|t| t.id == id).unwrap() as u32;
    let mvac = def.engines.iter().position(|e| e.id == "s2.mvac").unwrap();
    let feeds = [vec![tank("s1.tank:lox"), tank("s2.tank:lox")], vec![tank("s1.tank:rp1"), tank("s2.tank:rp1")]];
    def.engines[mvac].feeds = feeds.clone();
    state.clamped = false;
    state.stages = vec![Stage {
        name: "MVac".into(),
        key: None,
        actions: vec![ShipCommand::Ignite("s2".into())],
        pinned: false,
        abort: false,
    }];
    let k = Knowledge::perfect();
    let e = estimate(&def, &state, &inputs(&hull, &k, 0.0));
    let b = e.stages[0].1.as_ref().unwrap();
    assert_eq!(b.engines, [mvac as u32]);
    let tanks: Vec<Vec<u32>> = b.bars.iter().map(|b| b.tanks.clone()).collect();
    assert_eq!(tanks.len(), 2);
    for list in &feeds {
        let mut list = list.clone();
        list.sort_unstable();
        assert!(tanks.contains(&list), "{tanks:?}");
    }
    consistent(b);
    let one_stage: f64 = feeds.iter().flatten().map(|&t| state.tanks[t as usize].liquid).sum();
    let used = b.mass.value - b.burnout_mass.unwrap().value;
    assert!(used > 0.5 * one_stage, "it burned through the first tanks into the next: {used} of {one_stage}");
}

#[test]
fn a_setting_without_flow_never_burns_out() {
    let (mut def, mut state, hull) = falcon9();
    let mvac = def.engines.iter().position(|e| e.id == "s2.mvac").unwrap();
    def.engines[mvac].data.throttle_min = 0.0;
    state.clamped = false;
    state.stages = vec![Stage {
        name: "MVac".into(),
        key: None,
        actions: vec![ShipCommand::Ignite("s2".into())],
        pinned: false,
        abort: false,
    }];
    let k = Knowledge::perfect();
    let idle = Inputs { throttle: 0.0, ..inputs(&hull, &k, 0.0) };
    let e = estimate(&def, &state, &idle);
    let b = e.stages[0].1.as_ref().unwrap();
    assert_eq!(b.time.unwrap().value, f64::INFINITY);
    assert_eq!(b.thrust.unwrap().value, 0.0);
    let full = estimate(&def, &state, &inputs(&hull, &k, 0.0));
    assert_eq!(b.dv, full.stages[0].1.as_ref().unwrap().dv, "the full throttle's Δv");
}

/// Parts, the joints holding them, the pieces they make.
type PieceCase = (&'static [u32], &'static [(u32, u32)], &'static [&'static [u32]]);

#[test]
fn pieces_are_the_groups_joints_hold_together() {
    let cases: [PieceCase; 4] = [
        (&[0, 1, 2], &[(0, 1), (1, 2)], &[&[0, 1, 2]]),
        (&[0, 1, 2], &[(0, 1)], &[&[0, 1], &[2]]),
        (&[0, 1, 2, 3], &[(2, 3), (0, 3)], &[&[0, 2, 3], &[1]]),
        (&[4, 7], &[(4, 9)], &[&[4], &[7]]),
    ];
    for (parts, joined, want) in cases {
        let got = pieces(parts, joined);
        let want: Vec<Vec<u32>> = want.iter().map(|g| g.to_vec()).collect();
        assert_eq!(got, want, "{parts:?} {joined:?}");
    }
}
