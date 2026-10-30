//! A small craft of parts in flight (`sim::craft::small`, D121): its tanks
//! drained by its engines, the engine's back pressure, running dry, debug
//! mode (D064), the nozzle's heat (D077).

use glam::DVec3;
use sim::craft::small;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, HoldMode, Phase, SpeedReference, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

fn pad(w: &World) -> Vessel {
    Vessel::landed_at(w, VesselId(1), "Earth", 28.6082, -80.6041, t0(), &small::rocket())
}

fn leo(w: &World) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, &small::rocket())
}

/// Flies from the vessel's time for `seconds` at 60 fps.
fn fly(w: &World, ship: &mut Vessel, seconds: f64, controls: &Controls) {
    let from = ship.time;
    for k in 1..=(seconds * 60.0) as usize {
        ship.advance(w, from.add_seconds(k as f64 / 60.0), controls, usize::MAX);
    }
}

/// The sequence's first stage: the four engines start.
fn ignite(w: &World, ship: &mut Vessel) {
    assert_eq!(ship.next_stage(w).unwrap().0, "Ignition");
}

const FULL: Controls = Controls {
    throttle: 1.0,
    rotate: DVec3::ZERO,
    sas: true,
    chute: false,
    hold: HoldMode::Stability,
    speed: SpeedReference::Orbit,
    reference: None,
    target: None,
    translate: DVec3::ZERO,
    rcs: sim::systems::rcs::RcsMode { on: false, only: sim::systems::rcs::RcsOnly::Both, fine: false },
};

#[test]
fn empty_tanks_no_liftoff_unless_debug_mode() {
    let w = world();
    let mut ship = pad(&w);
    ship.set_fill(&w, 0.0);
    ignite(&w, &mut ship);
    fly(&w, &mut ship, 5.0, &FULL);
    assert!(matches!(ship.phase, Phase::Landed { .. }), "no thrust without propellant");
    let mut debug = pad(&w);
    debug.set_fill(&w, 0.0);
    debug.set_debug(&w, true);
    let before = debug.propellant();
    ignite(&w, &mut debug);
    fly(&w, &mut debug, 5.0, &FULL);
    assert!(matches!(debug.phase, Phase::Powered { .. }), "debug mode: infinite propellant");
    assert_eq!(debug.propellant(), before);
}

#[test]
fn the_engines_run_dry_and_the_vessel_coasts() {
    let w = world();
    let mut ship = leo(&w);
    // Tanks at 2 %: some seconds of full thrust.
    ship.set_fill(&w, 0.02);
    let mut twin = ship.clone();
    let m0 = ship.mass();
    ignite(&w, &mut ship);
    fly(&w, &mut ship, 60.0, &FULL);
    fly(&w, &mut twin, 60.0, &Controls::default());
    assert!(!ship.engine_running(&FULL), "dry: the engines are out");
    assert!(matches!(ship.phase, Phase::Coasting { .. }), "{:?}", ship.phase);
    // Against a twin that coasted: Δv from the rocket equation on the
    // mass the engines took.
    let isp = ship.ship_def().unwrap().engines[0].data.isp_vac;
    let dv = isp * sim::vessel::G0 * libm::log(m0 / ship.mass());
    let got = (ship.state(&w).2 - twin.state(&w).2).length();
    assert!(dv > 10.0 && (got - dv).abs() < 1e-2 * dv, "{got} vs {dv} m/s");
}

#[test]
fn back_pressure_lowers_thrust_at_sea_level() {
    let w = world();
    let mut ship = pad(&w);
    ignite(&w, &mut ship);
    fly(&w, &mut ship, 1.1, &FULL);
    let thrust = |p: f64| ship.engine_thrusts(1.0, p).unwrap()[0];
    let (sl, vac) = (thrust(101_325.0), thrust(0.0));
    // The ambient pressure on the nozzle's 0.4 m exit; the sea-level Isp
    // near the entry's 280 s against 320 s in vacuum.
    let exit = std::f64::consts::PI * 0.2 * 0.2;
    assert!((vac - sl - 101_325.0 * exit).abs() < 1.0, "{sl} vs {vac}");
    assert!(((sl / vac) - 280.0 / 320.0).abs() < 1e-2, "{sl} vs {vac}");
}

#[test]
fn a_standing_craft_rests_on_its_feet() {
    let w = world();
    let ship = pad(&w);
    let props = ship.mass_props();
    // The centre of mass above the bottom of the feet.
    assert!((ship.contact_height() - (props.com.z - ship.craft.bottom_z)).abs() < 1e-12);
    assert!(ship.contact_height() > 1.0 && ship.contact_height() < 10.0, "{}", ship.contact_height());
}

#[test]
fn debug_mode_and_the_tanks_are_saved() {
    use sim::save::SaveGame;
    use sim::vessel::VesselIds;
    let w = world();
    let mut ids = VesselIds::default();
    let earth = w.find("Earth").unwrap();
    let (_, r, v) = leo(&w).state(&w);
    let mut ship = Vessel::coasting(&w, ids.allocate(), t0(), earth.node, r, v, &small::rocket());
    ship.set_debug(&w, true);
    ship.set_fill(&w, 0.1);
    let save = SaveGame::capture(&w, t0(), std::slice::from_ref(&ship), ids, 0);
    let back = SaveGame::from_ron(&save.to_ron()).unwrap();
    assert_eq!(back.vessels[0], ship);
    assert!(back.vessels[0].debug() && back.vessels[0].propellant() == ship.propellant());
}

#[test]
fn the_nozzle_bell_cools_in_seconds_after_cutoff() {
    let w = world();
    let mut ship = leo(&w);
    ship.set_debug(&w, true);
    ignite(&w, &mut ship);
    // Two minutes at full thrust heat the bells (D077).
    let burn = Controls { sas: false, ..FULL };
    fly(&w, &mut ship, 120.0, &burn);
    let burning = ship.max_skin_temperature();
    assert!(burning > 1000.0, "{burning}");
    // Off: the thin bells radiate within a minute or two (at ~1,350 K the
    // cells' time constant is ~30 s), the same at 60 fps and in one jump on
    // the coast lattice.
    let coast = Controls { sas: false, throttle: 0.0, ..Controls::default() };
    let mut jump = ship.clone();
    let from = ship.time;
    for k in 1..=(90 * 60) {
        ship.advance(&w, from.add_seconds(k as f64 / 60.0), &coast, usize::MAX);
    }
    jump.advance(&w, from.add_seconds(90.0), &coast, usize::MAX);
    let hot = ship.max_skin_temperature();
    assert_eq!(hot, jump.max_skin_temperature());
    assert!(hot < burning - 200.0, "{hot} K after 90 s from {burning} K");
}
