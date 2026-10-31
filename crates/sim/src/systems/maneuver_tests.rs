use super::*;
use crate::systems::flight::EngineReading;
use crate::systems::Knowledge;
use glam::DQuat;

const MU: f64 = 3.986_004_418e14;

fn t(s: f64) -> Epoch {
    crate::sol::sol_epoch().add_seconds(s)
}

/// A 400 km circular orbit's state.
fn leo() -> (DVec3, DVec3) {
    let r = 6_778_137.0;
    (DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, (MU / r).sqrt(), 0.0))
}

fn node(at: f64, dv: DVec3) -> Node {
    Node { t: t(at), dv, reference: NodeId(3) }
}

fn reading(status: EngineStatus) -> Option<EngineReading> {
    Some(EngineReading { status, level: 1.0, reason: None })
}

fn view(att: Attitude, engines: Vec<Option<EngineReading>>) -> View {
    let (r, v) = leo();
    View { knowledge: Knowledge::perfect(), attitude: Some(att), nav: Some((NodeId(3), r, v)), engines }
}

#[test]
fn before_the_burn_v_go_is_the_node_s_dv() {
    // (Δv prograde/normal/radial, seconds before the node) → v_go is that
    // Δv in the node's axes, whenever it is asked before the burn.
    let (r, v) = leo();
    for (dv, dt) in [(DVec3::new(100.0, 0.0, 0.0), 600.0), (DVec3::new(-20.0, 5.0, 3.0), 1_200.0), (DVec3::ZERO, 10.0)]
    {
        let target = target((r, v), MU, dt, dv).unwrap();
        let go = v_go(target, (r, v), MU, dt).unwrap();
        let (rn, vn) = crate::kepler::propagate(r, v, MU, dt).unwrap();
        let (p, n, radial) = prograde_normal_radial(rn, vn);
        assert!((go - (p * dv.x + n * dv.y + radial * dv.z)).length() < 1e-9, "{dv} {dt}: {go}");
        // Later on the same coast: the same.
        let (r2, v2) = crate::kepler::propagate(r, v, MU, dt * 0.5).unwrap();
        let later = v_go(target, (r2, v2), MU, dt * 0.5).unwrap();
        assert!((later - go).length() < 1e-6, "{later} vs {go}");
    }
}

#[test]
fn aligned_table() {
    // (thrust axis turned from the direction by, rate) → aligned.
    let cases = [(0.0, 0.0, true), (0.03, 0.0, true), (0.04, 0.0, false), (0.0, 0.008, true), (0.0, 0.02, false)];
    for (angle, rate, want) in cases {
        let att = Attitude { q: DQuat::from_rotation_y(angle), omega: DVec3::new(rate, 0.0, 0.0) };
        assert_eq!(aligned(&att, DVec3::Z, DVec3::Z), want, "{angle} rad, {rate} rad/s");
    }
}

#[test]
fn last_ticks_table() {
    // (left after this tick's committed thrust, a tick of full thrust,
    // the tail-off from full level, the level now, the lowest level) →
    // the next tick's throttle, or cut off now.
    let cases = [
        (10.0, 0.5, 0.0, 1.0, 0.4, Some(1.0)),   // more than two ticks left: full
        (0.3, 0.5, 0.0, 1.0, 0.4, Some(0.6)),    // one tick at 60 % ends on it
        (0.15, 0.5, 0.0, 1.0, 0.4, Some(0.4)),   // the lowest level: 0.05 over, not 0.15 short
        (0.1, 0.5, 0.0, 1.0, 0.4, None),         // 0.1 short beats 0.1 over: cut
        (0.75, 0.5, 0.0, 1.0, 0.64, Some(0.75)), // two ticks at 75 %
        (0.55, 0.5, 0.0, 1.0, 0.64, Some(1.0)),  // one full tick (0.05 short) beats two at 64 % (0.09 over)
        (0.35, 0.5, 0.2, 1.0, 0.4, Some(0.5)),   // a tick and its tail-off at 50 %
        (0.08, 0.5, 0.2, 0.4, 0.4, None),        // the tail-off from 40 % gives it
        (-0.2, 0.5, 0.0, 1.0, 0.4, None),        // the committed tick overshoots: cut
        (1.0, 0.0, 0.0, 0.0, 0.4, Some(1.0)),    // no thrust estimate: burn on
        (-1.0, 0.0, 0.0, 0.0, 0.4, None),
    ];
    for (a, d, tail, now, min, want) in cases {
        let got = last_ticks(a, d, tail, now, min);
        let close = got.zip(want).is_some_and(|(g, w)| (g - w).abs() < 1e-12);
        assert!(close || got == want, "{a} {d} {tail} {now} {min}: {got:?}, want {want:?}");
    }
}

#[test]
fn overshot_table() {
    // (v_go, the burn's first direction) → overshot.
    let x = DVec3::X;
    let cases = [
        (x * 10.0, Some(x), false),
        (-x * 3.0, Some(x), true),
        (DVec3::Y * 3.0, Some(x), true), // across the burn
        (x * 3.0, None, false),
    ];
    for (go, first, want) in cases {
        assert_eq!(overshot(go, first), want, "{go}");
    }
}

#[test]
fn steers_table() {
    // (step, |v_go| m/s, acceleration m/s²) → the direction follows v_go.
    let cases = [
        (Step::Turn, 0.5, 0.0, true),
        (Step::Settle, 0.5, 8.0, true),
        (Step::Burn, 100.0, 8.0, true), // 12.5 s to go
        (Step::Burn, 16.0, 8.0, false), // 2 s: held
        (Step::Burn, 1.0, 8.0, false),
        (Step::Burn, 1.0, 0.0, true), // no estimate: still steers
        (Step::TailOff, 100.0, 8.0, false),
    ];
    for (step, go, accel, want) in cases {
        assert_eq!(steers(step, go, accel), want, "{step:?} {go} {accel}");
    }
}

#[test]
fn takeover_table() {
    let base = Controls { sas: true, hold: HoldMode::Stability, ..Controls::default() };
    let burning = Commands { hold: Some(DVec3::X), throttle: Some(1.0), ..Commands::default() };
    let settling =
        Commands { hold: Some(DVec3::X), throttle: Some(0.0), translate: Some(DVec3::Z), ..Commands::default() };
    use HoldMode::*;
    // (the pilot now, what the program commands) → conflicts.
    let cases = [
        (base, &burning, false),                               // no input
        (Controls { throttle: 1.0, ..base }, &burning, false), // agrees
        (Controls { throttle: 0.5, ..base }, &burning, true),  // a different throttle
        (Controls { throttle: 0.5, ..base }, &settling, true),
        (Controls { rotate: DVec3::X, ..base }, &burning, true), // against the hold
        (Controls { sas: false, ..base }, &burning, true),
        (Controls { hold: Maneuver, ..base }, &burning, false), // agrees
        (Controls { hold: Prograde, ..base }, &burning, true),
        (Controls { chute: true, ..base }, &burning, false), // not commanded
        (Controls { speed: crate::vessel::SpeedReference::Surface, ..base }, &burning, false),
        (Controls { translate: DVec3::X, ..base }, &burning, false), // not commanded now
        (Controls { translate: DVec3::X, ..base }, &settling, true), // against the settle
        (Controls { translate: DVec3::Z, ..base }, &settling, false), // agrees
    ];
    for (i, (now, c, want)) in cases.iter().enumerate() {
        assert_eq!(conflicts(&base, now, c), *want, "case {i}: {now:?}");
    }
    // Input the pilot gave before the wake is the baseline, not input.
    let held = Controls { throttle: 0.3, rotate: DVec3::Y, ..base };
    assert!(!conflicts(&held, &held, &burning));
}

#[test]
fn handover_table() {
    use HoldMode::*;
    let base = Controls { sas: true, hold: Prograde, ..Controls::default() };
    let burning = Commands { hold: Some(DVec3::X), throttle: Some(0.8), ..Commands::default() };
    let on = |throttle: f64| Controls { throttle, sas: true, hold: Stability, ..base };
    // (the pilot now, the program's last commands) → the pilot's controls.
    let cases = [
        // A rotation nudge: the burn goes on at the program's throttle,
        // holding where the nudge leaves it.
        (Controls { rotate: DVec3::X, ..base }, &burning, Controls { rotate: DVec3::X, ..on(0.8) }),
        // The pilot's own throttle is kept; the hold is handed over.
        (Controls { throttle: 0.3, ..base }, &burning, on(0.3)),
        // The pilot's SAS off or own hold is kept; the throttle handed over.
        (Controls { sas: false, ..base }, &burning, Controls { sas: false, throttle: 0.8, ..base }),
        (Controls { hold: Retrograde, ..base }, &burning, Controls { hold: Retrograde, throttle: 0.8, ..base }),
        // What it did not command is the pilot's.
        (Controls { chute: true, ..base }, &burning, Controls { chute: true, ..on(0.8) }),
        // Nothing commanded: nothing handed over.
        (base, &Commands::default(), base),
    ];
    for (i, (now, c, want)) in cases.iter().enumerate() {
        assert_eq!(handover(&base, now, c), *want, "case {i}: {now:?}");
    }
}

#[test]
fn schedule_table() {
    // (Δv, thrust, flow, mass, start, turn) → seconds from ignition to the
    // node, and from the wake to ignition.
    let isp_flow = |f: f64, isp: f64| f / (isp * crate::vessel::G0);
    let cases = [
        // 100 m/s at 1 m/s² with no flow: 50 s before (+ half the start).
        (100.0, 1e4, 0.0, 1e4, 2.0, 60.0, 51.0),
        (0.0, 1e4, 0.0, 1e4, 0.0, 60.0, 0.0),
        // No thrust known: ignite at the node.
        (100.0, 0.0, 0.0, 1e4, 0.0, 60.0, 0.0),
    ];
    for (dv, f, mdot, m0, start, turn, lead) in cases {
        let n = node(1_000.0, DVec3::X * dv);
        let (wake, ignite) = schedule(&n, (f, mdot, m0), start, turn);
        assert!((n.t.seconds_since(ignite) - lead).abs() < 1e-9, "{dv}: {}", n.t.seconds_since(ignite));
        assert!((ignite.seconds_since(wake) - (turn + SETTLE_LEAD)).abs() < 1e-9);
    }
    // With flow: half the Δv by the rocket equation, less than half the time.
    let (f, mdot) = (1e5, isp_flow(1e5, 320.0));
    let n = node(1_000.0, DVec3::X * 1_000.0);
    let (_, ignite) = schedule(&n, (f, mdot, 1e4), 0.0, 0.0);
    let half = n.t.seconds_since(ignite);
    let full = 1e4 / mdot * (1.0 - crate::math::exp(-1_000.0 * mdot / f));
    let back = f / mdot * crate::math::ln(1e4 / (1e4 - mdot * half));
    assert!((back - 500.0).abs() < 1e-6 && half > 0.5 * full - 1e-9, "{half} s of {full} s");
}

/// Inputs for a craft coasting from the LEO state at 900 s, its engine 0
/// along body +Z.
fn inputs<'a>(at: f64, view: &'a View, n: &'a Node, settled: Option<bool>) -> Inputs<'a> {
    let (r, v) = leo();
    Inputs {
        t: t(at),
        view,
        node: n,
        rel: crate::kepler::propagate(r, v, MU, at - 900.0),
        mu: MU,
        settled,
        axis: DVec3::Z,
        can_settle: true,
        mass: 1e4,
        thrust: 1e5,
        tail_impulse: 1e3,
        level_min: 0.4,
        dt: 0.02,
    }
}

/// A program woken 100 s before a 50 m/s prograde node, igniting 60 s
/// before it.
fn woken(n: &Node) -> Program {
    let mut p = Program::new(0, (t(900.0), t(940.0)), vec![0]);
    p.wake_up(n, Some(leo()), MU, 100.0, &Controls::default());
    p
}

#[test]
fn the_steps_turn_settle_ignite_burn_and_cut_off() {
    let n = node(1_000.0, DVec3::new(50.0, 0.0, 0.0));
    let mut p = woken(&n);
    assert_eq!(p.step, Step::Turn);
    let dir = |p: &Program| p.dir.unwrap();
    // Turning, not aligned: hold, engines held off.
    let off = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
    let v = view(off, vec![reading(EngineStatus::Off)]);
    let c = step(&inputs(905.0, &v, &n, Some(false)), &mut p);
    assert_eq!((p.step, c.throttle, c.translate), (Step::Turn, Some(0.0), None));
    assert!(c.hold.is_some() && c.ignite.is_empty());
    // Aligned (the thrust axis +Z onto the burn direction): settle.
    let on = Attitude { q: crate::vessel::quat_arc(DVec3::Z, dir(&p)), omega: DVec3::ZERO };
    let v = view(on, vec![reading(EngineStatus::Off)]);
    let c = step(&inputs(925.0, &v, &n, Some(false)), &mut p);
    assert_eq!(p.step, Step::Settle);
    assert_eq!(c.translate, None, "the step changed this tick");
    // Settling: pushing along the thrust axis until settled and due.
    let c = step(&inputs(930.0, &v, &n, Some(false)), &mut p);
    assert_eq!((p.step, c.translate, c.throttle), (Step::Settle, Some(DVec3::Z), Some(0.0)));
    let c = step(&inputs(935.0, &v, &n, Some(true)), &mut p);
    assert_eq!(p.step, Step::Settle, "settled, ignition not yet due");
    assert!(c.ignite.is_empty());
    let c = step(&inputs(940.0, &v, &n, Some(true)), &mut p);
    assert_eq!((p.step, c.throttle, c.ignite.clone()), (Step::Burn, Some(1.0), vec![0]));
    // Burning: full throttle along v_go.
    let v = view(on, vec![reading(EngineStatus::Starting)]);
    let c = step(&inputs(941.0, &v, &n, Some(true)), &mut p);
    assert_eq!((p.step, c.throttle), (Step::Burn, Some(1.0)));
    assert!(p.lit_seen);
    // Near the target: at the node, 0.35 m/s short of the target velocity,
    // with a tick of full thrust (0.2 m/s) committed and a tail-off of 0.1
    // m/s from full level: one tick at 50 % and its tail-off end on it.
    let (rt, vt) = p.target.unwrap();
    let v = view(on, vec![reading(EngineStatus::Running)]);
    let mut x = inputs(1_000.0, &v, &n, Some(true));
    x.rel = Some((rt, vt - dir(&p) * 0.35));
    let c = step(&x, &mut p);
    assert_eq!(p.step, Step::Burn);
    assert!(c.throttle.is_some_and(|u| (u - 0.5).abs() < 1e-6), "{c:?}");
    // Then that tick is committed at 50 %: 0.05 m/s left, the tail-off's:
    // cut off.
    let half = view(on, vec![Some(EngineReading { status: EngineStatus::Running, level: 0.5, reason: None })]);
    let mut x = inputs(1_000.0, &half, &n, Some(true));
    x.rel = Some((rt, vt - dir(&p) * 0.15));
    let c = step(&x, &mut p);
    assert_eq!((p.step, c.throttle), (Step::TailOff, Some(0.0)));
    assert!(p.left.unwrap() < 1e-6, "{:?}", p.left);
    // Tail-off until the engine is off: done.
    let v = view(on, vec![reading(EngineStatus::ShuttingDown)]);
    assert_eq!(step(&inputs(941.1, &v, &n, Some(true)), &mut p).throttle, Some(0.0));
    let v = view(on, vec![reading(EngineStatus::Off)]);
    assert_eq!(step(&inputs(942.0, &v, &n, Some(true)), &mut p), Commands::default());
    assert_eq!(p.step, Step::Done);
    assert_eq!(step(&inputs(943.0, &v, &n, Some(true)), &mut p), Commands::default(), "ended");
}

#[test]
fn settling_without_a_sensor_and_igniting_unsettled() {
    let n = node(1_000.0, DVec3::new(50.0, 0.0, 0.0));
    // No level sensor: settles for the lead, then ignites when due.
    let mut p = woken(&n);
    let v = view(Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO }, vec![reading(EngineStatus::Off)]);
    step(&inputs(940.0, &v, &n, None), &mut p);
    assert_eq!(p.step, Step::Settle, "ignition due: settle whatever the turn");
    step(&inputs(955.0, &v, &n, None), &mut p);
    assert_eq!(p.step, Step::Settle, "15 s of a 20 s lead");
    assert_eq!(step(&inputs(960.0, &v, &n, None), &mut p).ignite, vec![0]);
    // Never settled: ignites SETTLE_WAIT after the ignition epoch.
    let mut p = woken(&n);
    step(&inputs(940.0, &v, &n, Some(false)), &mut p);
    assert!(step(&inputs(999.0, &v, &n, Some(false)), &mut p).ignite.is_empty());
    assert_eq!(step(&inputs(1_000.0, &v, &n, Some(false)), &mut p).ignite, vec![0]);
}

#[test]
fn an_engine_that_stops_or_never_starts_ends_it_short() {
    let n = node(1_000.0, DVec3::new(50.0, 0.0, 0.0));
    let att = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
    let mut p = Program { step: Step::Burn, engines: vec![0], ignited: Some(t(940.0)), ..woken(&n) };
    let running = view(att, vec![reading(EngineStatus::Running)]);
    step(&inputs(941.0, &running, &n, None), &mut p);
    let out = view(
        att,
        vec![Some(EngineReading {
            status: EngineStatus::ShuttingDown,
            level: 0.5,
            reason: Some("propellant out".into()),
        })],
    );
    assert_eq!(step(&inputs(942.0, &out, &n, None), &mut p), Commands::default());
    assert_eq!(p.step, Step::Short);
    // Waiting on floating propellant (off, not yet lit) is not stopped,
    // until the start sequence gave up.
    let mut p = Program { step: Step::Burn, engines: vec![0], ignited: Some(t(940.0)), ..woken(&n) };
    let waiting = view(att, vec![reading(EngineStatus::Off)]);
    step(&inputs(1_000.0, &waiting, &n, None), &mut p);
    assert_eq!(p.step, Step::Burn);
    step(&inputs(1_062.0, &waiting, &n, None), &mut p);
    assert_eq!(p.step, Step::Short);
    // No navigation solution at the wake: it cannot fly.
    let mut p = Program::new(0, (t(900.0), t(940.0)), vec![0]);
    p.wake_up(&n, None, MU, 100.0, &Controls::default());
    assert_eq!(p.step, Step::Short);
}

#[test]
fn commands_lay_over_the_pilot_s_controls() {
    let pilot = Controls { throttle: 0.2, sas: false, chute: true, ..Controls::default() };
    let c = Commands { hold: Some(DVec3::X), throttle: Some(1.0), translate: Some(DVec3::Z), ignite: vec![0] };
    let flown = c.over(&pilot);
    assert_eq!((flown.throttle, flown.sas, flown.hold, flown.translate), (1.0, true, HoldMode::Maneuver, DVec3::Z));
    assert!(flown.chute, "what it does not command stays the pilot's");
    assert_eq!(Commands::default().over(&pilot), pilot);
}
