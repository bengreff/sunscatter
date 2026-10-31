//! The landing prediction's engines and mass come from the vessel (D125):
//! a craft of parts' stage estimate of the engines its throttle runs, its
//! own mass and centre of mass.

use super::*;
use crate::craft::small;
use crate::ephem::Ephemeris;
use crate::sol;
use crate::vessel::VesselId;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

/// The small rocket of parts falling at 150 m/s, 3 km over the Cape.
fn falling(w: &World) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
    let fixed = p.surface_point(28.6f64.to_radians(), (-80.6f64).to_radians(), 3_000.0);
    let r = p.rotation.to_inertial(fixed, t).raw();
    let v = p.rotation.omega(t).raw().cross(r) - r.normalize() * 150.0;
    Vessel::coasting(w, VesselId(1), t, earth.node, r, v, &small::rocket())
}

#[test]
fn a_craft_of_parts_predicts_with_its_own_engines_and_mass() {
    let w = world();
    let vessel = falling(&w);
    let (def, _) = vessel.ship_parts().expect("a craft of parts");
    let start = LandingStart::of_vessel(&w, &vessel, 0.0, false, AssumedAttitude::SurfaceRetrograde).unwrap();
    // Its main engines (all: none selected), at full throttle in vacuum.
    let main: Vec<_> = def.engines.iter().filter(|e| e.main).collect();
    let vac: f64 = main.iter().map(|e| e.data.thrust_vac).sum();
    let th = start.thrust;
    println!("thrust {:.0} N vac, exit {:.4} m², {:.2} kg/s, axis {:?}", th.vac, th.exit_area, th.mdot, th.axis);
    assert!((th.vac - vac).abs() < 1e-6 * vac, "{} N of {vac} N", th.vac);
    assert!(th.exit_area > 0.0 && th.output(1.0, P_SEA_LEVEL).0 < th.vac, "less thrust at sea level");
    let axis = main.iter().map(|e| e.data.dir.normalize()).sum::<DVec3>().normalize();
    assert!(th.axis.dot(axis) > 1.0 - 1e-12);
    // Its own mass and centre of mass; the propellant its engines burn.
    assert_eq!(start.mass, (vessel.mass(), vessel.mass_props().com));
    // The small rocket reads no tank levels: its engines are assumed not to
    // run dry, and its mass is held.
    assert_eq!(start.propellant, f64::INFINITY);
    let twr = start.twr(0.0, 9.81);
    assert!((twr - th.vac / (vessel.mass() * 9.81)).abs() < 1e-12);
    // It brakes on them: a braking solution exists and stops it above
    // the ground.
    let craft = &vessel.craft;
    let descent = predict_impact(&w, craft, &start, Limits::two_orbits(&w, &start));
    assert!(descent.impact.is_some(), "it falls to the ground");
    let brake = braking_solution(&w, craft, &descent, 10.0).expect("a braking solution");
    println!("ignite at {:.0} m, stop {:.1} m above the ground", brake.ignite_height, brake.stop_height);
    assert!(brake.stop_height > 0.0);
}
