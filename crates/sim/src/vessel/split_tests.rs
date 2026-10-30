//! A split's rule for every field of the vessel (`Vessel::piece`, F1 of
//! the fidelity review): the Falcon 9 in orbit, every field of the vessel
//! off its value at creation, split at its stage separation, gives each
//! piece what its field's rule says (the ship state's own rules are
//! `craft::state_tests`').

use super::*;
use crate::craft::{craft_of, CraftId};
use crate::ephem::Ephemeris;
use crate::kepler::Elements;
use crate::proximity::BubbleState;
use crate::sol;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

/// The Falcon 9 coasting in low orbit with every field of the vessel set
/// off its value at creation.
fn every_field_set(w: &World) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let orbit = Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
    let (r, vel) = orbit.to_state(earth.gm);
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let mut v = Vessel::coasting(w, VesselId(1), t0(), earth.node, r, vel, &craft);
    v.ship.as_mut().unwrap().clamped = false;
    v.attitude.omega = DVec3::new(0.0, 0.01, 0.002);
    v.chute.armed = true;
    v.debug = true;
    let burn = PlannedBurn::delta_v(t0().add_seconds(3_600.0), DVec3::new(10.0, 0.0, 0.0), 981.0e3, 348.0, None);
    v.plan = FlightPlan { burns: vec![burn] };
    v.tick_accel = Some(DVec3::new(0.0, 0.0, 9.0));
    (v.control.hold, v.control.tracking) = (Some(v.attitude.q), true);
    v.rest_ticks = 3;
    v.destroyed = Some(Destruction::Impact { speed: 12.0, point: 0 });
    v.rest_mode = Some(Controls::idle());
    v.clock = ShipClock::at(v.time, -0.25);
    v.limbo.boundary.dynamic_pressure = 0.2;
    v.repoint = true;
    v.bubble = BubbleState { member: true, since_contact: Some(4), lo: DVec3::new(1e-9, 0.0, 0.0), waiting: false };
    v.clear_of = vec![VesselId(99)];
    (v.controls_mut().throttle, v.controls_mut().rotate) = (0.5, DVec3::X);
    v.launch = Some(VesselId(1));
    v
}

/// The parts (indices into the craft's) of the booster and the pad's
/// mount: what the stage separation leaves behind.
fn booster(v: &Vessel) -> Vec<u32> {
    let def = v.ship_def().unwrap();
    let lower =
        |id: u32| def.parts.iter().any(|p| p.part == id && (p.id.starts_with("s1.") || p.id.starts_with("pad.")));
    let parts = &v.craft.design().parts;
    (0..parts.len() as u32).filter(|&i| lower(parts[i as usize].id)).collect()
}

#[test]
fn every_vessel_field_has_its_rule_at_a_split() {
    let w = world();
    let mut v = every_field_set(&w);
    let before = v.clone();
    let plan = v.plan_split(&booster(&v));
    let mut ids = VesselIds::default();
    ids.allocate();
    let mut pieces = v.split(&w, &plan, &plan.crafts(), &mut ids);
    assert_eq!(pieces.len(), 1, "the upper stack and the booster");
    let b = pieces.remove(0);
    // The vessel's piece keeps its id, a new piece gets a new one; each has
    // its own craft.
    assert_eq!(v.id(), before.id());
    assert!(b.id() != v.id() && b.id() != VesselId(99));
    assert!(v.craft.id != before.craft.id && b.craft.id != v.craft.id);
    // The time, attitude, ship clock, debug mode, limbo placeholders,
    // bubble state, launch and destruction: copies in every piece.
    for p in [&v, &b] {
        assert_eq!((p.time, p.attitude, p.clock.offset, p.debug), (before.time, before.attitude, -0.25, true));
        assert_eq!(
            (p.limbo, p.bubble, p.launch, p.destroyed),
            (before.limbo, before.bubble, before.launch, before.destroyed)
        );
        // A new live start, then a coast: nothing of the vessel's ticks.
        assert!(p.tick_accel.is_none() && p.rest_ticks == 0 && p.rest_mode.is_none() && !p.repoint);
        assert!(p.breakup.is_none() && matches!(p.phase, Phase::Coasting { .. }));
        // The ship state with its parts, the propellant its consumables,
        // the temperatures on the piece's own cells.
        assert_eq!(p.propellant, p.ship().unwrap().consumables(&p.ship_def().unwrap()));
        let design = p.craft.design();
        assert_eq!((p.thermal.state.skin.len(), p.thermal.state.nodes.len()), (design.cells.len(), design.nodes()));
    }
    assert!((v.propellant + b.propellant - before.propellant).abs() < 1e-9 * before.propellant);
    // The crew with the seats, the parachutes with their parts.
    assert_eq!((v.crew(), b.crew()), (before.crew(), 0));
    assert_eq!((v.chute, b.chute), (before.chute, split::LOST_CHUTE));
    // The vessel keeps its plan, control (its lattice restarts) and helm; the
    // booster starts with none, and the controls of a piece.
    assert_eq!((v.plan.clone(), b.plan.clone()), (before.plan.clone(), FlightPlan::default()));
    assert_eq!(v.control, AttitudeControl { base: v.control.base, ..before.control });
    assert_eq!(b.control.hold, None);
    assert_eq!((v.controls(), b.controls()), (before.controls(), &before.controls().for_piece()));
    // Clear of each other, not of what the vessel was clear of.
    assert_eq!((v.clear_of.clone(), b.clear_of.clone()), (vec![b.id()], vec![v.id()]));
}
