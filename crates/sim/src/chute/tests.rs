use super::*;

fn spec() -> ChuteSpec {
    crate::craft::test_craft().params().chute
}

const STILL: Flow = Flow { q: 0.0, speed: 0.0, height: 10_000.0 };

/// Steps `state` for `seconds` in a constant `flow`, returning the events.
fn run(state: &mut ChuteState, spec: &ChuteSpec, flow: Flow, seconds: f64) -> Vec<ChuteEvent> {
    let mut events = Vec::new();
    for _ in 0..(seconds / 0.02).round() as usize {
        events.extend(state.step(spec, true, flow, 0.02, false));
    }
    events
}

#[test]
fn nothing_happens_until_the_command() {
    let s = spec();
    let mut c = ChuteState::default();
    assert!(c.step(&s, false, STILL, 0.02, false).is_empty());
    assert_eq!(c, ChuteState::default());
    assert_eq!(c.step(&s, true, STILL, 0.02, false), vec![ChuteEvent::DrogueOpened]);
    assert!(c.armed);
}

#[test]
fn the_drogue_fills_at_v_over_n_to_its_reefed_stage_then_full() {
    let s = spec();
    let d = s.drogue.clone().unwrap();
    let mut c = ChuteState::default();
    let flow = Flow { q: 2_000.0, speed: 60.0, height: 7_000.0 };
    run(&mut c, &s, flow, 0.1);
    // The mouth grows at V/n: 60/10 = 6 m/s, 0.6 m after 0.1 s.
    let CanopyState::Open { diameter, .. } = c.drogue else { panic!("{c:?}") };
    assert!((diameter - 0.6).abs() < 1e-9, "{diameter}");
    run(&mut c, &s, flow, 5.0);
    assert!((c.cd_area(&s) - 0.570 * d.cd_area).abs() < 1e-9);
    run(&mut c, &s, flow, 6.0);
    assert!((c.cd_area(&s) - d.cd_area).abs() < 1e-9);
    assert_eq!(c.main, CanopyState::Stowed); // above the main's height
}

#[test]
fn the_main_opens_below_its_height_after_the_drogue_disreefs_and_cuts_it_away() {
    let s = spec();
    let mut c = ChuteState::default();
    let low = Flow { q: 300.0, speed: 22.0, height: 2_000.0 };
    // Commanded low: the drogue first, the main only after its 10 s reef.
    let events = run(&mut c, &s, low, 9.9);
    assert_eq!(events, vec![ChuteEvent::DrogueOpened]);
    let events = run(&mut c, &s, low, 0.2);
    assert_eq!(events, vec![ChuteEvent::MainOpened]);
    assert_eq!(c.drogue, CanopyState::Released);
    // Reefed stages on the cutters' timers, then full.
    let m = &s.main;
    run(&mut c, &s, low, 5.0);
    assert!((c.cd_area(&s) - 0.0679 * m.cd_area).abs() < 1e-6);
    run(&mut c, &s, low, 4.5);
    assert!((c.cd_area(&s) - 0.2571 * m.cd_area).abs() < 1e-6, "{c:?} {}", c.cd_area(&s));
    run(&mut c, &s, low, 10.0);
    assert!((c.cd_area(&s) - m.cd_area).abs() < 1e-6);
}

#[test]
fn an_overload_tears_the_canopy_and_nothing_refuses_the_command() {
    let s = spec();
    let d = s.drogue.clone().unwrap();
    // (q, fails): the reefed drogue breaks at max_load / (0.570·Cd·A).
    let breaking = d.max_load / (0.570 * d.cd_area);
    assert!(breaking > d.deploy_max_q);
    for (q, fails) in [(d.deploy_max_q, false), (0.99 * breaking, false), (1.01 * breaking, true)] {
        let mut c = ChuteState::default();
        let events = run(&mut c, &s, Flow { q, speed: 150.0, height: 9_000.0 }, 2.0);
        assert_eq!(events.contains(&ChuteEvent::DrogueFailed), fails, "{q}");
        assert_eq!(c.drogue == CanopyState::Failed, fails);
        // Debug mode: nothing breaks.
        let mut c = ChuteState::default();
        for _ in 0..100 {
            c.step(&s, true, Flow { q, speed: 150.0, height: 9_000.0 }, 0.02, true);
        }
        assert!(matches!(c.drogue, CanopyState::Open { .. }));
    }
}

#[test]
fn without_a_drogue_the_main_opens_on_the_command_and_a_failed_drogue_lets_it_open_low() {
    let mut s = spec();
    let mut c = ChuteState::default();
    run(&mut c, &s, Flow { q: 1e6, speed: 300.0, height: 9_000.0 }, 0.1);
    assert_eq!(c.drogue, CanopyState::Failed);
    assert_eq!(c.main, CanopyState::Stowed);
    let events = run(&mut c, &s, Flow { q: 500.0, speed: 30.0, height: 3_000.0 }, 0.02);
    assert_eq!(events, vec![ChuteEvent::MainOpened]);
    s.drogue = None;
    let mut c = ChuteState::default();
    assert_eq!(run(&mut c, &s, STILL, 0.02), vec![ChuteEvent::MainOpened]);
}

#[test]
fn predictions_assume_the_canopy_the_sequence_reaches() {
    let s = spec();
    let (d, m) = (s.drogue.as_ref().unwrap().cd_area, s.main.cd_area);
    let armed = ChuteState { armed: true, ..Default::default() };
    let failed_main = ChuteState { main: CanopyState::Failed, ..armed };
    let open_main = ChuteState { main: CanopyState::Open { age: 0.0, diameter: 0.0 }, ..armed };
    // (state, height, area)
    let cases = [
        (ChuteState::default(), 1_000.0, 0.0),
        (armed, 5_000.0, d),
        (armed, 3_000.0, m),
        (ChuteState { drogue: CanopyState::Failed, ..armed }, 5_000.0, 0.0),
        (open_main, 5_000.0, m),
        (failed_main, 1_000.0, 0.0),
    ];
    for (state, h, want) in cases {
        assert_eq!(state.predicted_cd_area(&s, h), want, "{state:?} at {h}");
    }
}
