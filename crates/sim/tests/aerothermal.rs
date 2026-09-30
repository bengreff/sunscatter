//! Aerodynamics and heating of a flying vessel (realism-1 §5, D061, D064,
//! D065): entries from low Earth orbit, sunlight in orbit, drag decay, and
//! determinism (chunked == single pass, save/load, warp independence).

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::thermal::Overheat;
use sim::time::Epoch;
use sim::vessel::{Attitude, CoastStart, Controls, Destruction, Phase, Segment, Vessel, VesselId, VesselIds};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

const RE: f64 = 6_378_137.0;

/// An orbit from `apo` down to `peri` (heights, m), starting at apoapsis.
fn orbit(w: &World, apo: f64, peri: f64) -> (NodeId, DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let (ra, rp) = (RE + apo, RE + peri);
    let el = Elements { a: 0.5 * (ra + rp), e: (ra - rp) / (ra + rp), i: 0.5, raan: 1.0, argp: 0.3, mean_anomaly: 3.0 };
    let (r, v) = el.to_state(earth.gm);
    (earth.node, r, v)
}

/// Where and how hot the skin got.
#[derive(Debug, Default)]
struct Peak {
    skin: f64,
    cell: usize,
    internal: f64,
}

impl Peak {
    fn record(&mut self, v: &Vessel) {
        let skin = &v.thermal().state.skin;
        for (i, &t) in skin.iter().enumerate() {
            if t > self.skin {
                (self.skin, self.cell) = (t, i);
            }
        }
        self.internal = self.internal.max(v.max_node_temperature().0);
    }

    fn describe(&self) -> String {
        let craft = sim::craft::test_craft();
        let c = &craft.cells.cells[self.cell];
        let part = &craft.geometry.primitives[c.primitive as usize].name;
        format!(
            "peak skin {:.0} K at cell {} ({part}, centroid {:.2}, normal {:.2}), hottest node {:.1} K",
            self.skin, self.cell, c.centroid, c.normal, self.internal
        )
    }
}

/// Flies an entry from a 200 × 40 km orbit in 1 s frames until the vessel
/// is destroyed or lands.
fn entry(debug: bool) -> (Vessel, Peak, f64) {
    let w = world();
    let (earth, r, v) = orbit(&w, 200_000.0, 40_000.0);
    let mut ship = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, sim::craft::test_craft());
    ship.set_debug(&w, debug);
    assert!(matches!(ship.phase, Phase::Coasting { .. }));
    let mut peak = Peak::default();
    let mut t = t0();
    let mut live = 0.0;
    for _ in 0..20_000 {
        t = t.add_seconds(1.0);
        ship.advance(&w, t, &Controls::default(), usize::MAX);
        peak.record(&ship);
        if matches!(ship.phase, Phase::Powered { .. }) {
            live += 1.0;
        }
        if matches!(ship.phase, Phase::Crashed { .. } | Phase::Landed { .. }) {
            break;
        }
    }
    (ship, peak, live)
}

#[test]
fn an_entry_from_low_orbit_burns_up_the_test_craft() {
    // No heat shield (D064): the realistic outcome is destruction (D065).
    let (ship, peak, live) = entry(false);
    println!("no debug: {}, {live:.0} s live, {:?}", peak.describe(), ship.phase);
    let Phase::Crashed { cause: Destruction::Overheat { at: Overheat::Cell(cell), temperature }, .. } = ship.phase
    else {
        panic!("expected to burn up: {:?}", ship.phase)
    };
    assert!(temperature > ship.craft.thermal.skin_max_k, "{temperature}");
    assert_eq!(cell as usize, peak.cell);
}

#[test]
fn in_debug_mode_the_entry_is_survived() {
    let (ship, peak, live) = entry(true);
    println!("debug: {}, {live:.0} s live, {:?}", peak.describe(), ship.phase);
    assert!(matches!(ship.phase, Phase::Landed { .. }), "{:?}", ship.phase);
    // It got far hotter than its limit (no heat shield), and cooled on the
    // way down.
    assert!(peak.skin > 1.5 * ship.craft.thermal.skin_max_k, "{}", peak.describe());
    assert!(ship.max_skin_temperature() < peak.skin);
}

/// A far orbit whose plane faces the Sun: never eclipsed.
fn sunlit(w: &World, t: Epoch) -> (NodeId, DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let sun = w.find("Sun").unwrap();
    let to_sun = w.snapshot(t).relative_r(sun.node, earth.node).normalize();
    let a = 1.0e8;
    let x = to_sun.cross(DVec3::Z).normalize();
    let r = x * a;
    let v = to_sun.cross(x) * (earth.gm / a).sqrt();
    (earth.node, r, v)
}

#[test]
fn a_craft_at_rest_in_sunlight_settles() {
    let w = world();
    let t = t0();
    let (earth, r, v) = sunlit(&w, t);
    let mut ship = Vessel::coasting(&w, VesselId(1), t, earth, r, v, sim::craft::test_craft());
    // Side on to the Sun (the nose perpendicular to the light), not rotating.
    let sun_dir = w.snapshot(t).relative_r(w.find("Sun").unwrap().node, earth).normalize();
    let side = sim::vessel::quat_z_to(sun_dir.cross(DVec3::Z).normalize());
    ship.set_attitude(Attitude { q: side, omega: DVec3::ZERO });
    let (mut last_skin, mut last_internal) = (0.0, 0.0);
    for day in 1..=6 {
        let started = std::time::Instant::now();
        ship.advance(&w, t.add_seconds(86_400.0 * day as f64), &Controls::default(), usize::MAX);
        let (skin, internal) = (ship.max_skin_temperature(), ship.max_node_temperature().0);
        let cold = ship.thermal().state.skin.iter().fold(f64::MAX, |m, &x| m.min(x));
        let cold_node = ship.thermal().state.nodes.iter().fold(f64::MAX, |m, &x| m.min(x));
        println!(
            "day {day}: skin {cold:.1}–{skin:.1} K, interior nodes {cold_node:.1}–{internal:.1} K ({:.0} ms)",
            started.elapsed().as_secs_f64() * 1e3
        );
        (last_skin, last_internal) = (skin, internal);
    }
    // Sunlit cells near the flat-plate equilibrium (ε = α: (S/σ)^¼ ≈ 394 K)
    // less what they conduct away; the interior between the lit and dark sides.
    assert!(last_skin > 300.0 && last_skin < 395.0, "{last_skin}");
    assert!(last_internal > 150.0 && last_internal < 360.0, "{last_internal}");
    // Settled, up to the Sun's apparent motion (1°/day against the fixed
    // attitude, which slowly changes the lit side).
    let started = std::time::Instant::now();
    ship.advance(&w, t.add_seconds(86_400.0 * 7.0), &Controls::default(), usize::MAX);
    println!("day 7 in {:.0} ms", started.elapsed().as_secs_f64() * 1e3);
    assert!((ship.max_node_temperature().0 - last_internal).abs() < 2.0);
    assert!((ship.max_skin_temperature() - last_skin).abs() < 2.0);
}

#[test]
fn coast_temperatures_do_not_depend_on_the_frame_rate() {
    // Low orbit (eclipses) with the craft tumbling: 60 fps at 100x against
    // one jump, three hours.
    let w = world();
    let (earth, r, v) = orbit(&w, 420_000.0, 400_000.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, sim::craft::test_craft());
    let q = sim::vessel::quat_z_to(DVec3::new(0.3, 0.5, 0.8).normalize());
    a.set_attitude(Attitude { q, omega: DVec3::new(0.01, -0.02, 0.005) });
    let mut b = a.clone();
    let end = t0().add_seconds(3.0 * 3600.0);
    let mut t = t0();
    while t < end {
        t = t.add_seconds(100.0 / 60.0);
        let t = if t > end { end } else { t };
        a.advance(&w, t, &Controls::default(), usize::MAX);
    }
    b.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!(a.thermal(), b.thermal());
    assert_eq!(a.attitude, b.attitude);
    let skin = a.thermal().state.skin.iter().fold((f64::MAX, 0.0f64), |(lo, hi), &x| (lo.min(x), hi.max(x)));
    println!("after 3 h in LEO: skin {:.1}–{:.1} K, interior {:.1} K", skin.0, skin.1, a.max_node_temperature().0);
}

/// A vessel entering: 100 km, 7.6 km/s, 1.5° down.
fn descending(w: &World) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let r = DVec3::new(RE + 100_000.0, 0.0, 0.0);
    let g = 1.5f64.to_radians();
    let v = DVec3::new(-g.sin(), g.cos(), 0.0) * 7_600.0;
    let mut ship = Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, sim::craft::test_craft());
    ship.set_debug(w, true);
    assert!(matches!(ship.phase, Phase::Powered { .. }), "live in the atmosphere");
    ship
}

#[test]
fn a_live_descent_in_frames_equals_one_jump() {
    let w = world();
    let mut a = descending(&w);
    let mut b = a.clone();
    let controls = Controls { sas: true, ..Default::default() };
    let end = t0().add_seconds(60.0);
    let mut t = t0();
    while t < end {
        t = t.add_seconds(1.0 / 60.0);
        let t = if t > end { end } else { t };
        a.advance(&w, t, &controls, usize::MAX);
    }
    b.advance(&w, end, &controls, usize::MAX);
    assert_eq!(a, b);
    assert!(a.max_skin_temperature() > 500.0, "{}", a.max_skin_temperature());
}

#[test]
fn a_descent_continues_bit_identically_after_save_and_load() {
    let w = world();
    let mut ship = descending(&w);
    let controls = Controls { sas: true, ..Default::default() };
    let mid = t0().add_seconds(30.0);
    ship.advance(&w, mid, &controls, usize::MAX);
    let mut ids = VesselIds::default();
    ids.allocate();
    let text = SaveGame::capture(&w, mid, std::slice::from_ref(&ship), ids, 0, controls).to_ron();
    let mut loaded = SaveGame::from_ron(&text).unwrap().vessels.remove(0);
    assert_eq!(loaded, ship);
    let end = t0().add_seconds(60.0);
    ship.advance(&w, end, &controls, usize::MAX);
    loaded.advance(&w, end, &controls, usize::MAX);
    assert_eq!(loaded, ship);
}

#[test]
fn drag_in_the_upper_atmosphere_decays_an_orbit() {
    // Circular at 130 km, flown live for 300 s; and the coast model's
    // prediction (attitude-independent drag area) from the same state.
    let w = world();
    let earth = w.find("Earth").unwrap();
    let r0 = RE + 130_000.0;
    let (r, v) = (DVec3::new(r0, 0.0, 0.0), DVec3::new(0.0, (earth.gm / r0).sqrt(), 0.0));
    let energy = |r: DVec3, v: DVec3| 0.5 * v.length_squared() - earth.gm / r.length();
    let mut ship = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, sim::craft::test_craft());
    assert!(matches!(ship.phase, Phase::Powered { .. }));
    let dt = 300.0;
    ship.advance(&w, t0().add_seconds(dt), &Controls { sas: true, ..Default::default() }, usize::MAX);
    let (_, r1, v1) = ship.state(&w);
    let lost = energy(r, v) - energy(r1, v1);
    // Specific power ½ρv³·CdA/m, with the density at 130 km and the drag
    // area between the end-on area and free-molecular Cd 2 on the side
    // view (~14.5 m × 2 m plus the fins, skirt and legs: < 40 m²).
    let rho = earth.physical.as_ref().unwrap().atmosphere.as_ref().unwrap().density(130_000.0);
    let v_air = v.length() - 465.0 * 0.0; // equatorial plane: roughly prograde with the rotation
    let per_area = 0.5 * rho * v_air.powi(3) / ship.mass() * dt;
    println!("live: lost {lost:.1} J/kg in {dt} s ({:.1} m² effective Cd·A)", lost / per_area);
    assert!(lost > 0.0 && lost / per_area > 3.0 && lost / per_area < 80.0, "{}", lost / per_area);
    // The coast model (predictions) decays it too.
    let mut seg = Segment::new(
        &w,
        t0(),
        CoastStart {
            anchor: earth.node,
            r,
            v,
            drag: Some(ship.drag()),
            contact_height: 0.0,
            horizon: dt,
            fixed_anchor: false,
            proper_time: 0.0,
        },
    );
    seg.extend(&w, usize::MAX);
    let (_, r2, v2) = seg.eval(dt).unwrap();
    let lost_coast = energy(r, v) - energy(r2, v2);
    println!("coast: lost {lost_coast:.1} J/kg ({:.1} m²)", lost_coast / per_area);
    assert!(lost_coast > 0.0 && (lost_coast / lost).abs() < 5.0 && (lost / lost_coast) < 5.0);
}

#[test]
fn sunlight_is_the_solar_constant_by_day_and_nothing_in_earths_shadow() {
    let w = world();
    let snap = w.snapshot(t0());
    let earth = w.find("Earth").unwrap().node;
    let to_sun = snap.relative_r(w.find("Sun").unwrap().node, earth).normalize();
    let day = sim::light::sunlight(&w, &snap, earth, to_sun * (RE + 400_000.0));
    // Early January: Earth near perihelion (0.983 AU), 1361 / 0.983² ≈ 1408 W/m².
    assert!((day.length() - 1408.0).abs() < 5.0, "{}", day.length());
    assert!(day.normalize().dot(-to_sun) > 0.999_999);
    assert_eq!(sim::light::sunlight(&w, &snap, earth, -to_sun * (RE + 400_000.0)), DVec3::ZERO);
}

/// Node temperatures of a vessel minus another's.
fn node_rise(a: &Vessel, b: &Vessel) -> Vec<f64> {
    a.thermal().state.nodes.iter().zip(&b.thermal().state.nodes).map(|(x, y)| x - y).collect()
}

/// The hottest nozzle cell of a vessel (K).
fn nozzle_max(v: &Vessel) -> f64 {
    let design = v.craft.design();
    design.nozzle_cells.iter().map(|&(i, _)| v.thermal().state.skin[i as usize]).fold(0.0, f64::max)
}

#[test]
fn a_long_burn_heats_the_nozzle_to_radiative_equilibrium_and_barely_the_interior() {
    // D077. Two craft side by side in deep space (1.4 million km out, where
    // the burn changes nothing about the sunlight); one burns five minutes
    // at full thrust (an Apollo LOI-length burn).
    let w = world();
    let t = t0();
    let (earth, r, _) = sunlit(&w, t);
    let (r, v) = (r * 14.0, DVec3::ZERO);
    let mut idle = Vessel::coasting(&w, VesselId(1), t, earth, r, v, sim::craft::test_craft());
    // Debug mode keeps the propellant (and so the nodes' capacities) equal.
    idle.set_debug(&w, true);
    let mut burn = idle.clone();
    let design = sim::craft::test_craft().design().clone();
    let nodes = &design.volume.nodes;
    let (aft, nose) = (
        design.engine_node as usize,
        (0..nodes.len()).fold(0, |b, i| if nodes[i].centre.z > nodes[b].centre.z { i } else { b }),
    );
    let cells = &sim::craft::test_craft().cells.cells;
    let area: f64 = design.nozzle_cells.iter().map(|&(i, _)| cells[i as usize].area).sum();
    let n = design.nozzle_cells.len();
    let g: f64 = design.nozzle_cells.iter().map(|&(i, _)| design.network.to_node[i as usize]).sum();
    println!("nozzle: {n} cells, {area:.2} m², {g:.2} W/K to the mount");
    // Joined at the throat only: about a watt per kelvin (D077).
    assert!(g > 0.5 && g < 2.0, "{g}");
    let full = Controls { throttle: 1.0, sas: true, ..Default::default() };
    let coast = Controls { sas: true, ..Default::default() };
    burn.advance(&w, t.add_seconds(240.0), &full, usize::MAX);
    let before = nozzle_max(&burn);
    burn.advance(&w, t.add_seconds(300.0), &full, usize::MAX);
    idle.advance(&w, t.add_seconds(300.0), &coast, usize::MAX);
    let (nozzle, rise) = (nozzle_max(&burn), node_rise(&burn, &idle));
    let hottest = (0..rise.len()).fold(0, |b, i| if rise[i] > rise[b] { i } else { b });
    println!(
        "after 300 s: nozzle {nozzle:.0} K ({:+.1} K in the last minute), mount node +{:.2} K, nose +{:.5} K",
        nozzle - before,
        rise[aft],
        rise[nose]
    );
    // A radiatively cooled niobium extension runs near 1300–1500 K in
    // equilibrium, below the C-103 limit.
    assert!((1300.0..1500.0).contains(&nozzle), "{nozzle}");
    assert!((nozzle - before).abs() < 10.0, "not yet in equilibrium: {before} → {nozzle}");
    assert!(nozzle < design.skin_max[design.nozzle_cells[0].0 as usize]);
    // Through the ~1 W/K mount only: the interior barely warms.
    assert_eq!(hottest, aft);
    assert!(rise[aft] > 0.1 && rise[aft] < 10.0, "{}", rise[aft]);
    // The nose differs only through the attitude's small differences in
    // sunlight (the burning craft's gimbal).
    assert!(rise[nose].abs() < 0.1, "{}", rise[nose]);
    // An hour later the nozzle has cooled; the heat that soaked in spreads.
    let later = t.add_seconds(3900.0);
    burn.advance(&w, later, &coast, usize::MAX);
    idle.advance(&w, later, &coast, usize::MAX);
    let soaked = node_rise(&burn, &idle);
    println!("an hour later: nozzle {:.0} K, mount node +{:.2} K", nozzle_max(&burn), soaked[aft]);
    assert!(nozzle_max(&burn) - nozzle_max(&idle) < 20.0);
    assert!(soaked.iter().all(|&x| x < 10.0), "{soaked:?}");
}

#[test]
fn a_burning_vessel_in_frames_equals_one_jump() {
    let w = world();
    let t = t0();
    let (earth, r, v) = sunlit(&w, t);
    let mut a = Vessel::coasting(&w, VesselId(1), t, earth, r, v, sim::craft::test_craft());
    let mut b = a.clone();
    let full = Controls { throttle: 1.0, sas: true, ..Default::default() };
    let end = t.add_seconds(20.0);
    let mut clock = t;
    while clock < end {
        clock = clock.add_seconds(1.0 / 60.0);
        a.advance(&w, if clock > end { end } else { clock }, &full, usize::MAX);
    }
    b.advance(&w, end, &full, usize::MAX);
    assert_eq!(a, b);
}

#[test]
fn a_planned_burn_on_rails_heats_the_engine_at_any_frame_rate() {
    use sim::vessel::{BurnEnd, BurnLaw, DirectionLaw, FlightPlan, PlannedBurn};
    let w = world();
    let t = t0();
    let (earth, r, _) = sunlit(&w, t);
    let mut idle = Vessel::coasting(&w, VesselId(1), t, earth, r * 14.0, DVec3::ZERO, sim::craft::test_craft());
    idle.set_debug(&w, true);
    let e = idle.craft.engine;
    let law = BurnLaw::from_isp(e.thrust_vac, e.isp_vac, DirectionLaw::Inertial(DVec3::X));
    let plan =
        FlightPlan { burns: vec![PlannedBurn { t_start: t.add_seconds(90.0), law, end: BurnEnd::Duration(95.0) }] };
    let mut a = idle.clone();
    a.set_plan(&w, plan).unwrap();
    let mut b = a.clone();
    let end = t.add_seconds(600.0);
    let mut clock = t;
    while clock < end {
        clock = clock.add_seconds(100.0 / 60.0);
        a.advance(&w, if clock > end { end } else { clock }, &Controls::default(), usize::MAX);
    }
    b.advance(&w, end, &Controls::default(), usize::MAX);
    idle.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!(a.thermal(), b.thermal());
    let aft = sim::craft::test_craft().design().engine_node as usize;
    let rise = node_rise(&a, &idle);
    let nozzle = nozzle_max(&a) - nozzle_max(&idle);
    println!("planned 95 s burn: nozzle {nozzle:+.1} K, mount node +{:.3} K after 10 min", rise[aft]);
    assert!(rise[aft] > 0.01 && rise[aft] < 10.0, "{}", rise[aft]);
}

#[test]
fn coast_sunlight_is_the_orbit_average_in_low_orbit() {
    let w = world();
    let t = t0();
    let snap = w.snapshot(t);
    let (earth, r, v) = orbit(&w, 400_000.0, 400_000.0);
    let avg = sim::vessel::coast_sunlight(&w, t, earth, r, v);
    let full = sim::light::star_flux(3.828e26, snap.relative_r(w.find("Sun").unwrap().node, earth).length());
    // Sunlit 61–100 % of a 400 km orbit, depending on its beta angle.
    let lit = avg.length() / full;
    assert!(lit > 0.6 && lit <= 1.0, "{lit}");
    // Far out (a period of days) it is the sunlight at the point.
    let (earth, r, v) = sunlit(&w, t);
    assert_eq!(sim::vessel::coast_sunlight(&w, t, earth, r, v), sim::light::sunlight(&w, &snap, earth, r));
}

#[test]
fn a_budgeted_coast_equals_an_unbudgeted_one() {
    // Holding its attitude in low orbit for two days: advanced in calls of
    // a small work budget (the game holds its clock back) against one call.
    let w = world();
    let (earth, r, v) = orbit(&w, 420_000.0, 400_000.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, sim::craft::test_craft());
    let mut b = a.clone();
    let controls = Controls { sas: true, ..Default::default() };
    let end = t0().add_seconds(2.0 * 86_400.0);
    let mut calls = 0;
    while a.time < end {
        a.advance(&w, end, &controls, 60);
        calls += 1;
        assert!(calls < 100_000);
    }
    b.advance(&w, end, &controls, usize::MAX);
    println!("{calls} budgeted calls");
    assert!(calls > 10, "{calls}");
    assert_eq!(a, b);
}

#[test]
fn holding_attitude_in_low_orbit_frames_equal_one_jump() {
    // 60 fps at 1,000x for a day (lattice points passed without evaluation
    // while the attitude holds) against one jump.
    let w = world();
    let (earth, r, v) = orbit(&w, 420_000.0, 400_000.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, sim::craft::test_craft());
    let mut b = a.clone();
    let controls = Controls { sas: true, ..Default::default() };
    let end = t0().add_seconds(86_400.0);
    let mut t = t0();
    while t < end {
        t = t.add_seconds(1000.0 / 60.0);
        a.advance(&w, if t > end { end } else { t }, &controls, usize::MAX);
    }
    b.advance(&w, end, &controls, usize::MAX);
    assert_eq!(a.thermal(), b.thermal());
    assert_eq!(a.attitude, b.attitude);
    let skin = a.thermal().state.skin.iter().fold((f64::MAX, 0.0f64), |(lo, hi), &x| (lo.min(x), hi.max(x)));
    println!(
        "a day in LEO holding attitude: skin {:.1}–{:.1} K, interior {:.1} K",
        skin.0,
        skin.1,
        a.max_node_temperature().0
    );
}
