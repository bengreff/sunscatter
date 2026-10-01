//! The test craft in flight (realism-1 §3b–3d): propellant, the engine's
//! back pressure, debug mode (D064).

use glam::DVec3;
use sim::craft::test_craft;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, HoldMode, Phase, SpeedReference, Vessel, VesselId, TICK};
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
    Vessel::landed_at(w, VesselId(1), "Earth", 28.6082, -80.6041, t0(), test_craft())
}

fn leo(w: &World) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, test_craft())
}

fn fly(w: &World, ship: &mut Vessel, seconds: f64, controls: &Controls) {
    for k in 1..=(seconds * 60.0) as usize {
        ship.advance(w, t0().add_seconds(k as f64 / 60.0), controls, usize::MAX);
    }
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
};

#[test]
fn rcs_burns_propellant_at_its_flow_per_axis() {
    let w = world();
    let mut ship = pad(&w);
    // A full roll command on the pad, engine off (D078).
    let roll = Controls { throttle: 0.0, rotate: DVec3::Z, sas: false, ..FULL };
    fly(&w, &mut ship, 2.0, &roll);
    let ticks = libm::round(ship.time.seconds_since(t0()) / TICK);
    let burned = 16_000.0 - ship.propellant();
    let flow = ship.craft.rcs_flow.z;
    assert!((flow - 8.0 * 445.0 / (290.0 * 9.806_65)).abs() < 1e-12, "{flow}");
    assert!(ticks > 0.0 && (burned / (ticks * flow * TICK) - 1.0).abs() < 1e-9, "{burned} kg in {ticks} ticks");
}

#[test]
fn powered_ticks_burn_propellant_at_the_engine_rate() {
    let w = world();
    let mut ship = pad(&w);
    let mdot = ship.craft.engine.mdot_max();
    // SAS off: the RCS stays quiet and only the engine burns.
    fly(&w, &mut ship, 10.0, &Controls { sas: false, ..FULL });
    assert!(matches!(ship.phase, Phase::Powered { .. }), "lifted off");
    let ticks = libm::round(ship.time.seconds_since(t0()) / TICK);
    let burned = 16_000.0 - ship.propellant();
    // Every tick burns ṁ·TICK (the lift-off frame included).
    assert!((burned / (ticks * mdot * TICK) - 1.0).abs() < 1e-9, "{burned} kg in {ticks} ticks");
    assert_eq!(ship.mass(), 4_000.0 + ship.propellant());
}

#[test]
fn no_propellant_no_liftoff_unless_debug_mode() {
    let w = world();
    let mut ship = pad(&w);
    ship.set_propellant(&w, 0.0);
    fly(&w, &mut ship, 5.0, &FULL);
    assert!(matches!(ship.phase, Phase::Landed { .. }), "no thrust without propellant");
    let mut debug = pad(&w);
    debug.set_propellant(&w, 0.0);
    debug.set_debug(&w, true);
    fly(&w, &mut debug, 5.0, &FULL);
    assert!(matches!(debug.phase, Phase::Powered { .. }), "debug mode: infinite propellant");
    assert_eq!(debug.propellant(), 0.0);
}

#[test]
fn the_engine_runs_dry_and_the_vessel_coasts() {
    let w = world();
    let mut ship = leo(&w);
    let mdot = ship.craft.engine.mdot_max();
    ship.set_propellant(&w, 2.5 * mdot); // 2.5 s of full thrust
    let mut twin = ship.clone();
    fly(&w, &mut ship, 4.0, &FULL);
    fly(&w, &mut twin, 4.0, &Controls::default());
    assert_eq!(ship.propellant(), 0.0);
    assert!(matches!(ship.phase, Phase::Coasting { .. }), "{:?}", ship.phase);
    // Against a twin that coasted: Δv from the rocket equation.
    let dv = ship.craft.engine.isp_vac * sim::vessel::G0 * libm::log((4_000.0 + 2.5 * mdot) / 4_000.0);
    let got = (ship.state(&w).2 - twin.state(&w).2).length();
    assert!((got - dv).abs() < 1e-3 * dv, "{got} vs {dv} m/s");
}

#[test]
fn back_pressure_lowers_thrust_at_sea_level() {
    let w = world();
    let (on_pad, in_orbit) = (pad(&w), leo(&w));
    let ratio = on_pad.thrust_accel(&w, 1.0).length() / in_orbit.thrust_accel(&w, 1.0).length();
    // Isp 280 s at sea level vs 320 s in vacuum (the pad is at ~0 m).
    assert!((ratio - 280.0 / 320.0).abs() < 1e-3, "{ratio}");
}

#[test]
fn a_standing_craft_rests_on_its_feet() {
    let w = world();
    let ship = pad(&w);
    let props = ship.mass_props();
    // The centre of mass is 7.7 m above the feet plus its own height.
    assert!((ship.contact_height() - (props.com.z + 7.7)).abs() < 1e-12);
    assert!(ship.contact_height() > 7.0 && ship.contact_height() < 10.0, "{}", ship.contact_height());
}

#[test]
fn debug_mode_and_propellant_are_saved() {
    use sim::save::SaveGame;
    use sim::vessel::VesselIds;
    let w = world();
    let mut ids = VesselIds::default();
    let earth = w.find("Earth").unwrap();
    let (_, r, v) = leo(&w).state(&w);
    let mut ship = Vessel::coasting(&w, ids.allocate(), t0(), earth.node, r, v, test_craft());
    ship.set_debug(&w, true);
    ship.set_propellant(&w, 1234.5);
    let save = SaveGame::capture(&w, t0(), std::slice::from_ref(&ship), ids, 0, Controls::default());
    let back = SaveGame::from_ron(&save.to_ron()).unwrap();
    assert_eq!(back.vessels[0], ship);
    assert!(back.vessels[0].debug() && back.vessels[0].propellant() == 1234.5);
}

#[test]
fn the_nozzle_bell_cools_in_seconds_after_cutoff() {
    let w = world();
    let mut ship = leo(&w);
    ship.set_debug(&w, true);
    // Two minutes at full thrust bring the bell to ~1400 K (D077).
    let burn = Controls { sas: false, ..FULL };
    fly(&w, &mut ship, 120.0, &burn);
    assert!(ship.max_skin_temperature() > 1300.0, "{}", ship.max_skin_temperature());
    // Off: the 1 mm niobium bell radiates (C/4εσT³ ≈ 5 s at 1400 K); by
    // hand ~830 K after 30 s and ~500 K after 136 s as a bare plate. The
    // coast lattice resolves it, the same at 60 fps and in one jump.
    let coast = Controls { sas: false, ..Controls::default() };
    let mut jump = ship.clone();
    let after = |s: f64| t0().add_seconds(120.0 + s);
    for k in 1..=(30 * 60) {
        ship.advance(&w, after(k as f64 / 60.0), &coast, usize::MAX);
    }
    jump.advance(&w, after(30.0), &coast, usize::MAX);
    let hot = ship.max_skin_temperature();
    assert_eq!(hot, jump.max_skin_temperature());
    assert!(hot > 750.0 && hot < 1000.0, "{hot} K after 30 s");
    ship.advance(&w, after(136.0), &coast, usize::MAX);
    let cooler = ship.max_skin_temperature();
    assert!(cooler > 450.0 && cooler < 650.0, "{cooler} K after 136 s");
}
