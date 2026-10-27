use super::*;
use crate::state::SimState;
use crate::trajectory::{LineFrame, Plotter};

/// A craft flying live 25 km above the Cape: 1,200 m/s north over the
/// ground and 300 m/s up (falling back into the air, as after a cut-off
/// on a steep ascent).
fn climbing(sim: &mut SimState) -> Vessel {
    let earth = sim.world.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let rot = p.rotation;
    let fixed = p.surface_point(28.6f64.to_radians(), (-80.6f64).to_radians(), 25_000.0);
    let r = rot.to_inertial(fixed, sim.clock).raw();
    let up = r.normalize();
    let north = (rot.pole(sim.clock) - up * rot.pole(sim.clock).dot(up)).normalize();
    let v = rot.omega(sim.clock).raw().cross(r) + north * 1_200.0 + up * 300.0;
    let id = sim.vessel_ids.allocate();
    let vessel = Vessel::coasting(&sim.world, id, sim.clock, earth.node, r, v, sim::craft::test_craft());
    assert!(matches!(vessel.phase, Phase::Powered { .. }), "flown live in the air");
    vessel
}

/// Points of `segs` drawn in `frame`, centred on Earth (300 per segment).
fn drawn(sim: &SimState, segs: &[&Segment], frame: LineFrame) -> Vec<DVec3> {
    let earth = sim.world.find("Earth").unwrap().node;
    let plotter = Plotter::at(&sim.world, sim.clock, earth, DVec3::ZERO, frame);
    let mut out = Vec::new();
    for seg in segs {
        let end = seg.computed_until();
        for k in 0..=300 {
            let t = end * f64::from(k) / 300.0;
            if let Some((anchor, r, _)) = seg.eval(t) {
                out.push(plotter.plot(anchor, r, seg.t0.add_seconds(t)));
            }
        }
    }
    out
}

/// The largest turn (deg) of the line seen from above its start: the
/// heading of each chord's horizontal part (those over 50 m) against the
/// first one's.
fn largest_turn(points: &[DVec3]) -> f64 {
    let up = points[0].normalize();
    let flat: Vec<DVec3> =
        points.windows(2).map(|w| (w[1] - w[0]) - up * (w[1] - w[0]).dot(up)).filter(|h| h.length() > 50.0).collect();
    flat.iter().map(|h| flat[0].angle_between(*h).to_degrees()).fold(0.0, f64::max)
}

/// The owner's kink (2026-10-26), reproduced: the old prediction coasted
/// through the air with the thin-air drag, which stops the craft against
/// the air within seconds; the air then carries it east with the ground at
/// 400 m/s while it falls. Drawn without the ground's rotation, the line
/// runs north-north-east, then turns ~70° east into a shorter straight
/// piece. Over the ground (SURFACE) the same path does not turn.
#[test]
fn the_line_through_the_air_kinked_where_the_air_carried_the_craft() {
    let mut sim = SimState::new();
    let vessel = climbing(&mut sim);
    let earth = sim.world.find("Earth").unwrap().node;
    let mut old = vessel.coast_from_now(&sim.world);
    extend_to_line_end(&sim.world, &mut old, &OrbitSettings::default(), COAST_STEPS);
    let about = largest_turn(&drawn(&sim, &[&old], LineFrame::About(earth)));
    let ground = largest_turn(&drawn(&sim, &[&old], LineFrame::Ground(earth)));
    println!("old prediction: turns {about:.1}° about Earth, {ground:.1}° over the ground");
    assert!(about > 45.0, "{about}°");
    assert!(ground < 10.0, "{ground}°");
}

/// The path through the air is the vessel's own force model: drag against
/// the air turning with the planet (against still air the craft would be
/// pushed west over the ground, a turn), so over the ground it slows and
/// falls without turning; it ends on the ground where the landing
/// prediction (the Landing menu's marker) puts the impact.
#[test]
fn the_path_through_the_air_is_the_vessels_own_model() {
    let mut sim = SimState::new();
    let vessel = climbing(&mut sim);
    let earth = sim.world.find("Earth").unwrap().node;
    let controls = Controls::default();
    let path = path(&sim.world, &vessel, &controls, &OrbitSettings::default());
    assert_eq!(path.after, None);
    let air = &path.segments[0];
    let Some(sim::vessel::SegmentEnd { kind: EndKind::Surface { body }, t: t_hit }) = air.end else {
        panic!("{:?}", air.end)
    };
    assert_eq!((body, path.segments.len()), (earth, 1), "it falls back to the ground");
    let segs: Vec<&Segment> = path.segments.iter().collect();
    let ground = largest_turn(&drawn(&sim, &segs, LineFrame::Ground(earth)));
    println!("air path: turns {ground:.1}° over the ground, lands after {t_hit:.0} s");
    assert!(ground < 10.0, "{ground}°");
    // Slowed against the air: well under its 1,200 m/s over the ground.
    let (anchor, r, v) = air.eval(t_hit).unwrap();
    let snap = sim.world.snapshot(air.t0.add_seconds(t_hit));
    let m = landing::surface_motion(&sim.world, &snap, anchor, earth, r, v);
    println!("over the ground at impact: {:.0} m/s horizontal, {:.0} m/s vertical", m.v_horizontal, m.v_vertical);
    assert!(m.v_horizontal < 600.0, "{} m/s over the ground", m.v_horizontal);
    // The landing prediction from the same start meets the ground at the
    // same time and place.
    let start = LandingStart::of_vessel(
        &sim.world,
        &vessel,
        0.0,
        false,
        crate::landing::assumed_attitude(&controls, vessel.attitude.q),
    )
    .unwrap();
    let hit = landing::predict_impact(&sim.world, &vessel.craft, &start, Limits::two_orbits(&sim.world, &start))
        .impact
        .unwrap();
    assert!((hit.t.seconds_since(air.t0.add_seconds(t_hit))).abs() < 1e-6);
}

/// A coast into the air hands over at its entry: the stored segment is cut
/// there and the path through the air follows from the same state.
#[test]
fn a_coast_into_the_air_hands_over_at_its_entry() {
    let mut sim = SimState::new();
    let earth = sim.world.find("Earth").unwrap();
    let re = earth.physical.as_ref().unwrap().radius_eq;
    // From 200 km, a perigee 40 km below the ground: it enters the air.
    let (ra, rp) = (re + 200_000.0, re - 40_000.0);
    let el = sim::kepler::Elements {
        a: 0.5 * (ra + rp),
        e: (ra - rp) / (ra + rp),
        i: 0.5,
        raan: 0.0,
        argp: 0.0,
        mean_anomaly: std::f64::consts::PI,
    };
    let (r, v) = el.to_state(earth.gm);
    let id = sim.vessel_ids.allocate();
    let mut vessel = Vessel::coasting(&sim.world, id, sim.clock, earth.node, r, v, sim::craft::test_craft());
    vessel.extend_coast(&sim.world, sim.clock.add_seconds(3_000.0), 200_000);
    assert!(needed(&vessel));
    let last = vessel.trajectory().unwrap().last().clone();
    let (entry, _) = last.entry.expect("an entry");
    let path = path(&sim.world, &vessel, &Controls::default(), &OrbitSettings::default());
    assert_eq!(path.after, Some((vessel.id(), last.t0)));
    let (cut, air) = (&path.segments[0], &path.segments[1]);
    assert_eq!(cut.computed_until(), entry);
    assert_eq!(air.t0, last.t0.add_seconds(entry));
    let (a, r_cut, v_cut) = cut.eval(entry).unwrap();
    let (b, r_air, v_air) = air.eval(0.0).unwrap();
    assert_eq!(a, b);
    assert!((r_cut - r_air).length() < 1e-6 && (v_cut - v_air).length() < 1e-9);
    assert!(matches!(air.end.unwrap().kind, EndKind::Surface { .. }));
}
