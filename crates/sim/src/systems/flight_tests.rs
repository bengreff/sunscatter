//! The flight software's functions as table tests (F3), and its inputs on
//! the Falcon 9 Crew.

use super::*;
use crate::craft::state::Lost;
use crate::craft::state::ShipState;
use crate::systems::data::{knowledge, provides};

fn knows(quantities: &[&str]) -> Knowledge {
    let mut k = Knowledge::default();
    for q in quantities {
        for (quantity, p) in provides(q, 1.0) {
            k.learn(quantity, p);
        }
    }
    k
}

fn computers(working: &[bool]) -> Health {
    Health { computers: working.to_vec(), ..Health::default() }
}

/// SAS needs a working computer, an attitude reference (attitude and
/// rates) and, for a direction, a navigation solution; debug's perfect
/// knowledge has all three.
#[test]
fn what_sas_can_do_table() {
    let on = |hold| Controls { sas: true, hold, ..Default::default() };
    let off = Controls::default();
    let imu = knows(&["inertial"]);
    let all = knows(&["inertial", "position"]);
    let star = knows(&["attitude", "position"]);
    let perfect = Knowledge::perfect();
    let (one, none) = (computers(&[false, true]), computers(&[false, false]));
    // (knowledge, health, controls, expected)
    let table = [
        (&all, &one, on(HoldMode::Prograde), Sas::Holding),
        (&all, &one, on(HoldMode::Stability), Sas::Holding),
        (&imu, &one, on(HoldMode::Stability), Sas::Holding),
        (&imu, &one, on(HoldMode::Prograde), Sas::NoNavigation),
        (&imu, &one, on(HoldMode::Target), Sas::NoNavigation),
        (&imu, &one, on(HoldMode::Maneuver), Sas::NoNavigation),
        (&star, &one, on(HoldMode::Stability), Sas::NoAttitude),
        (&Knowledge::default(), &one, on(HoldMode::Prograde), Sas::NoAttitude),
        (&all, &one, off, Sas::Off),
        (&all, &none, on(HoldMode::Stability), Sas::NoComputer),
        (&all, &none, off, Sas::NoComputer),
        (&perfect, &one, on(HoldMode::Retrograde), Sas::Holding),
        (&perfect, &none, on(HoldMode::Retrograde), Sas::NoComputer),
        (&Knowledge::default(), &Health::ideal(), on(HoldMode::Stability), Sas::NoAttitude),
        (&perfect, &Health::ideal(), on(HoldMode::Stability), Sas::Holding),
    ];
    for (i, (k, h, c, want)) in table.into_iter().enumerate() {
        assert_eq!(sas(k, h, &c), want, "row {i}");
    }
    assert_eq!(Sas::NoAttitude.text(), "no attitude reference");
    assert!(Sas::NoAttitude.degraded() && !Sas::Holding.degraded() && !Sas::Off.degraded());
}

/// What the attitude controller is let fly: the pilot's controls, the
/// attitude held without navigation, no SAS without a reference, nothing
/// without a computer.
#[test]
fn the_controls_the_attitude_controller_acts_on() {
    let pilot = Controls {
        sas: true,
        hold: HoldMode::Retrograde,
        rotate: DVec3::new(0.5, 0.0, 0.0),
        throttle: 0.7,
        ..Default::default()
    };
    // (status, sas, hold, rotate)
    let table = [
        (Sas::Holding, true, HoldMode::Retrograde, pilot.rotate),
        (Sas::Off, true, HoldMode::Retrograde, pilot.rotate),
        (Sas::NoNavigation, true, HoldMode::Stability, pilot.rotate),
        (Sas::NoAttitude, false, HoldMode::Retrograde, pilot.rotate),
        (Sas::NoComputer, false, HoldMode::Retrograde, DVec3::ZERO),
    ];
    for (status, sas, hold, rotate) in table {
        let c = status.acting(&pilot);
        assert_eq!((c.sas, c.hold, c.rotate), (sas, hold, rotate), "{status:?}");
        assert_eq!(c.throttle, pilot.throttle, "only the attitude control changes");
    }
}

fn running(level: f64) -> Option<EngineReading> {
    Some(EngineReading { status: EngineStatus::Running, level, reason: None })
}

/// The pad's launch commit on the controllers' readouts: wait until every
/// engine reports its rated level; abort on an engine that stopped or
/// reports nothing.
#[test]
fn the_launch_commit_table() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    let merlins: Vec<usize> = (0..def.engines.len()).filter(|&i| def.engines[i].id.starts_with("s1.merlin")).collect();
    let max = def.engines[merlins[0]].data.throttle_max;
    let starting = Some(EngineReading { status: EngineStatus::Starting, level: 0.6, reason: None });
    let failed =
        Some(EngineReading { status: EngineStatus::Failed, level: 0.0, reason: Some("failed: forced".into()) });
    let with = |k: usize, r: Option<EngineReading>| {
        let mut e = vec![running(max); def.engines.len()];
        e[merlins[k]] = r;
        e
    };
    let abort = |k: usize, why: &str| Commit::Abort { engine: merlins[k], why: why.into() };
    // (readouts, expected)
    let table = [
        (with(0, running(max)), Commit::Release),
        (with(3, starting.clone()), Commit::Wait),
        (with(3, running(0.99 * max)), Commit::Wait),
        (with(3, failed), abort(3, "failed: forced")),
        (with(5, None), abort(5, "no thrust reading")),
        (
            with(5, Some(EngineReading { status: EngineStatus::ShuttingDown, level: 0.5, reason: None })),
            abort(5, "stopped"),
        ),
    ];
    for (i, (readouts, want)) in table.into_iter().enumerate() {
        assert_eq!(commit(&def, &readouts, &merlins), want, "row {i}");
    }
}

/// The controllers' safing: a lit engine whose controller works but has no
/// working computer is shut down; a lost controller or a dark engine is not.
#[test]
fn the_controllers_safe_their_engines_without_a_computer() {
    let c = |works, commanded| Controller { works, commanded };
    let health = Health {
        controllers: vec![c(true, true), c(true, false), c(true, false), c(false, false)],
        ..Health::default()
    };
    assert_eq!(safing(&health, &[true, true, false, true]), vec![1]);
    assert_eq!(safing(&health, &[false; 4]), Vec::<usize>::new());
}

#[test]
fn the_landing_prediction_needs_navigation() {
    // (knowledge, inertial, expected)
    let table = [
        (knows(&["position"]), false, true),
        (knows(&["position"]), true, false),
        (knows(&["position", "attitude"]), true, true),
        (knows(&["inertial"]), false, false),
        (Knowledge::perfect(), true, true),
    ];
    for (i, (k, inertial, want)) in table.into_iter().enumerate() {
        assert_eq!(can_predict(&k, inertial), want, "row {i}");
    }
}

/// The Falcon 9 Crew's inputs: every IMU lost leaves no attitude
/// reference (Dragon's star trackers give no rates); a stage computer
/// unpowered leaves its controllers uncommanded and their thrust unknown; a
/// lost controller reports nothing; debug knows everything still.
#[test]
fn the_falcon_9s_health_and_view() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    let fresh = ShipState::initial(&def);
    let h = health(&def, &fresh);
    assert!(h.computing() && !h.ideal && h.computers.iter().all(|&c| c));
    assert!(h.controllers.iter().all(|c| c.works && c.commanded));
    let k = knowledge(&def, &fresh, false);
    assert!(attitude_reference(&k) && navigation(&k));
    let lose = |state: &mut ShipState, id: &str, unpowered: bool| {
        let owner = def.owner_named(id).unwrap();
        let part = def.owner_part(owner).unwrap();
        let l = Lost { part, owner, why: "test".into() };
        if unpowered {
            state.unpowered.push(l)
        } else {
            state.lost.push(l)
        }
    };
    // Every IMU lost.
    let mut state = fresh.clone();
    for s in def.sensors.iter().filter(|s| s.id.contains(".imu#")) {
        lose(&mut state, &s.id, false);
    }
    let k = knowledge(&def, &state, false);
    assert!(k.knows(Quantity::Attitude), "Dragon's star trackers");
    assert!(!attitude_reference(&k) && navigation(&k));
    let controls = Controls { sas: true, ..Default::default() };
    assert_eq!(sas(&k, &health(&def, &state), &controls), Sas::NoAttitude);
    assert_eq!(sas(&knowledge(&def, &state, true), &health(&def, &state), &controls), Sas::Holding, "debug");
    // The first stage's computer unpowered.
    let mut state = fresh.clone();
    lose(&mut state, "s1.computer", true);
    let h = health(&def, &state);
    let k = knowledge(&def, &state, false);
    for (i, e) in def.engines.iter().enumerate() {
        let s1 = e.id.starts_with("s1.");
        assert_eq!(h.controllers[i].commanded, !s1, "{}", e.id);
        assert_eq!(k.knows(Quantity::EngineThrust(i as u32)), !s1, "{}", e.id);
    }
    assert!(h.computing(), "the second stage's and Dragon's work");
    // A lost controller reports nothing; the others do.
    let mut state = fresh.clone();
    lose(&mut state, "s1.merlin.5", false);
    let k = knowledge(&def, &state, false);
    let readings = engine_readings(&def, &state, &k);
    let e5 = def.engines.iter().position(|e| def.owner_id(e.owner) == "s1.merlin.5").unwrap();
    assert!(readings[e5].is_none() && readings.iter().filter(|r| r.is_some()).count() == def.engines.len() - 1);
    assert!(engine_readings(&def, &state, &knowledge(&def, &state, true)).iter().all(Option::is_some), "debug");
}

/// The estimates the player is shown read what the software knows: with the
/// first stage's computer unpowered, its stage can start nothing (no burn),
/// and the Merlins still running show a dash for the burn's thrust,
/// thrust-to-weight and Δv (their readouts and the level sensors reach no
/// computer); the second stage's burn keeps them. Debug mode knows the
/// burn's figures; the computer is dead there too.
#[test]
fn the_estimates_read_what_the_computers_receive() {
    use crate::systems::estimate::{estimate, hull, Inputs};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let craft = crate::craft::load_definition(&path).unwrap();
    let def = craft.ship.as_ref().unwrap();
    let hull = hull(&craft.parts);
    let mut state = ShipState::initial(def);
    for (e, d) in state.engines.iter_mut().zip(&def.engines) {
        if d.id.starts_with("s1.merlin") {
            e.status = EngineStatus::Running;
        }
    }
    let part = def.computers.iter().find(|c| c.id == "s1.computer").unwrap().part;
    let owner = def.owner_named("s1.computer").unwrap();
    state.unpowered.push(Lost { part, owner, why: "test".into() });
    for debug in [false, true] {
        let k = knowledge(def, &state, debug);
        let e = estimate(def, &state, &Inputs { hull: &hull, knowledge: &k, throttle: 1.0, p: 0.0, g: Some(9.81) });
        let now = e.now.as_ref().expect("the Merlins burn now");
        assert_eq!((now.thrust.is_some(), now.twr.is_some(), now.dv.is_some()), (debug, debug, debug), "debug {debug}");
        let burns: Vec<_> = e.stages.iter().filter_map(|(_, b)| b.as_ref()).collect();
        let mvac =
            |b: &&crate::systems::estimate::Burn| b.engines.iter().all(|&i| def.engines[i as usize].id == "s2.mvac");
        assert!(burns.iter().all(mvac), "only the second stage's burn");
        assert!(burns.len() == 1 && burns[0].thrust.is_some() && burns[0].dv.is_some(), "its computer works");
    }
}
