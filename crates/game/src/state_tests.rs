use super::*;

#[test]
fn rails_floor_blocks_low_orbits_on_earth_and_low_passes_over_the_moon() {
    let sim = SimState::new();
    let w = &sim.world;
    let t = sim.clock;
    let earth = w.find("Earth").unwrap().node;
    let moon = w.find("Moon").unwrap().node;
    let re = w.find("Earth").unwrap().physical.as_ref().unwrap().radius_eq;
    let rm = w.find("Moon").unwrap().physical.as_ref().unwrap().radius_eq;
    // (anchor, distance from its centre along x, blocked by)
    let cases = [
        (earth, re + 120_000.0, Some(earth)),
        (earth, re + 200_000.0, None),
        (moon, rm + 10_000.0, Some(moon)),
        (moon, rm + 15_000.0, None),
    ];
    for (anchor, d, expected) in cases {
        let got = below_rails_floor(w, t, anchor, DVec3::X * d).map(|b| b.0);
        assert_eq!(got, expected, "{d}");
    }
}

#[test]
fn throttle_keys_ramp_and_clamp() {
    // (start, up, down, fine, dt, expected)
    let cases = [
        (0.0, true, false, false, 0.5, 0.3),
        (0.0, true, false, true, 1.0, 0.05),
        (0.5, false, true, false, 0.5, 0.2),
        (0.5, true, true, false, 1.0, 0.5),
        (0.95, true, false, false, 1.0, 1.0),
        (0.02, false, true, true, 1.0, 0.0),
    ];
    for (t, up, down, fine, dt, want) in cases {
        let got = throttle_after(t, up, down, fine, dt);
        assert!((got - want).abs() < 1e-12, "{t} {up} {down} {fine} {dt}: {got}");
    }
}

#[test]
fn lookahead_takes_turns_and_skips_the_active_vessel() {
    assert_eq!(lookahead_turn(0, 1, 0), None);
    assert_eq!(lookahead_turn(0, 3, 0), Some(1));
    assert_eq!(lookahead_turn(2, 3, 0), Some(2));
    assert_eq!(lookahead_turn(3, 3, 0), Some(1), "wraps past the active vessel");
    assert_eq!(lookahead_turn(5, 3, 2), Some(0));
}

/// D081: rails warp is refused while any craft is live, with the
/// reason; craft in limbo (perturbed coasts: thin-air drag, light, a
/// low-thrust burn under way) never refuse it.
#[test]
fn only_live_craft_refuse_rails_warp_and_limbo_craft_do_not() {
    use sim::vessel::{BurnEnd, BurnLaw, DirectionLaw, FlightPlan, PlannedBurn, SegmentKind};
    let mut sim = SimState::new();
    let w = sim.world.clone();
    let earth = w.find("Earth").unwrap();
    let re = earth.physical.as_ref().unwrap().radius_eq;
    let circular = |h: f64| {
        let r = re + h;
        (DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, (earth.gm / r).sqrt(), 0.0))
    };
    let craft = |sim: &mut SimState, h: f64| {
        let (r, v) = circular(h);
        let id = sim.vessel_ids.allocate();
        Vessel::coasting(&w, id, sim.clock, earth.node, r, v, test_craft())
    };
    // On the pad, nothing else flying: allowed.
    assert_eq!(sim.rails_block(), None);
    // Limbo: drag decay at 300 km, and a 0.5 N burn spiralling up.
    let decaying = craft(&mut sim, 300_000.0);
    let mut spiral = craft(&mut sim, 700_000.0);
    let law = BurnLaw::from_isp(0.5, 3_000.0, DirectionLaw::Tracking { reference: earth.node, axes: DVec3::X });
    let burn = PlannedBurn { t_start: sim.clock, law, end: BurnEnd::Duration(30.0 * 86_400.0) };
    spiral.set_plan(&w, FlightPlan { burns: vec![burn] }).unwrap();
    spiral.advance(&w, sim.clock.add_seconds(3_600.0), &Controls { sas: true, ..Default::default() }, 1_200);
    assert!(matches!(spiral.segment().unwrap().kind, SegmentKind::Burn(_)), "burning on rails");
    sim.fleet.extend([decaying, spiral]);
    assert_eq!(sim.rails_block(), None, "limbo craft do not refuse rails warp");
    // A craft in the air (live below the boundary, 100 km): refused, named.
    let entering = craft(&mut sim, 100_000.0);
    let id = entering.id();
    assert!(matches!(entering.phase, Phase::Powered { .. }));
    sim.fleet.push(entering);
    assert_eq!(sim.rails_block(), Some(RailsBlock::OtherLive(id)));
    // The active vessel's own reasons come first.
    sim.controls_mut().throttle = 0.5;
    assert_eq!(sim.rails_block(), Some(RailsBlock::Thrust));
    sim.controls_mut().throttle = 0.0;
    sim.active = sim.fleet.len() - 1;
    assert_eq!(sim.rails_block(), Some(RailsBlock::Live));
}

/// D082: a craft held live in the bubble of a live controllable craft
/// names it as the reason rails warp is refused.
#[test]
fn a_craft_held_in_a_bubble_names_its_leader() {
    let mut sim = SimState::new();
    let w = sim.world.clone();
    let earth = w.find("Earth").unwrap();
    let r = earth.physical.as_ref().unwrap().radius_eq + 400_000.0;
    let (pos, vel) = (DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, (earth.gm / r).sqrt(), 0.0));
    let craft = |sim: &mut SimState, along: f64| {
        let id = sim.vessel_ids.allocate();
        Vessel::coasting(&w, id, sim.clock, earth.node, pos + DVec3::Y * along, vel, test_craft())
    };
    let (ship, other) = (craft(&mut sim, 0.0), craft(&mut sim, 2_000.0));
    let other_id = other.id();
    sim.fleet = vec![ship, other];
    sim.active = 0;
    assert_eq!(sim.rails_block(), None, "both coasting");
    // The other craft burns: a bubble forms around it and holds the
    // active one live.
    let burn = |i: usize| Controls { sas: true, throttle: if i == 1 { 0.1 } else { 0.0 }, ..Default::default() };
    sim::vessel::bubble::advance(&w, &mut sim.fleet, &burn, sim.clock.add_seconds(0.1));
    assert!(sim.fleet[0].in_bubble() && matches!(sim.fleet[0].phase, Phase::Powered { .. }));
    assert_eq!(sim.rails_block(), Some(RailsBlock::Near(other_id)));
}

#[test]
fn the_startup_craft_is_the_wanted_one_else_the_first_available() {
    let both = ["falcon9-crew", "test-craft"];
    // (wanted, available, demo, expected)
    let cases: [(Option<&str>, &[&str], bool, &str); 6] = [
        (None, &both, false, "falcon9-crew"),
        (None, &both, true, "test-craft"),
        (Some("test-craft"), &both, false, "test-craft"),
        (Some("falcon9-crew"), &both, true, "falcon9-crew"),
        (Some("no-such-craft"), &both, false, "falcon9-crew"),
        (None, &[], false, "test-craft"),
    ];
    for (wanted, available, demo, want) in cases {
        assert_eq!(startup_craft(wanted, available, demo), want, "{wanted:?} {available:?} {demo}");
    }
    let only_test = craft_choices(|id| id == "test-craft");
    assert_eq!(only_test, [("test-craft", "Test craft (debug)")]);
    assert_eq!(craft_choices(|_| true).len(), CRAFT_CHOICES.len(), "the Falcon 9 first");
    assert!(craft_exists("test-craft") && !craft_exists("no-such-craft"));
}

/// A new flight puts a new vessel of the chosen craft on the pad, fuelled at
/// its loading; a reset puts back a new one of the active vessel's craft.
#[test]
fn a_new_flight_and_a_reset_bring_the_chosen_craft_to_the_pad() {
    let mut sim = SimState::new();
    let first = sim.ship().id();
    let rocket = crate::test_rocket::rocket();
    sim.new_flight(&rocket);
    assert_eq!(sim.ship().craft.id, rocket.id);
    assert!(sim.ship().id() != first && matches!(sim.ship().phase, Phase::Landed { .. }));
    let def = sim.ship().ship_def().unwrap();
    let tanks = &sim.ship().ship().unwrap().tanks;
    let loaded = def.tanks.iter().zip(tanks).all(|(d, s)| (s.liquid - d.capacity * d.load.fill).abs() < 1e-9);
    assert!(loaded, "every tank fuelled at its loading");
    assert_eq!(sim.ship().crew(), 1);
    let before = sim.ship().id();
    sim.reset();
    assert_eq!((sim.ship().craft.id.clone(), sim.fleet.len()), (rocket.id.clone(), 1));
    assert!(sim.ship().id() != before);
    // A craft that does not build gives the test craft.
    assert_eq!(craft_or_test("no-such-craft").id.0, "test-craft");
}

/// Held on the pad with its engines lit (at their minimum with the throttle
/// closed) or the release armed, a craft is ticked in place every 20 ms:
/// rails warp is refused, for it and while another vessel is active.
#[test]
fn a_craft_held_live_on_the_pad_refuses_rails_warp() {
    let mut sim = SimState::with_craft(&craft_or_test("falcon9-crew"));
    let (a, f9) = (sim.active, sim.ship().id());
    assert!(sim.ship().clamped(), "the Falcon 9 starts on the pad's clamps");
    assert_eq!(sim.rails_block(), None, "on the pad, nothing lit: allowed");
    // Ignition with the throttle closed: the nine engines spool to their minimum.
    let s = &mut sim;
    s.fleet[a].next_stage(&s.world).expect("the ignition");
    assert_eq!(sim.controls().throttle, 0.0);
    assert!(held_live(sim.ship()));
    assert_eq!(sim.rails_block(), Some(RailsBlock::Held));
    sim.warp = WARP_LEVELS.len() - 1;
    assert_eq!(sim.effective_warp(), MAX_PHYSICS_WARP, "capped at physics warp");
    // Another vessel active (in orbit): the held one is named.
    let w = sim.world.clone();
    let earth = w.find("Earth").unwrap();
    let r = earth.physical.as_ref().unwrap().radius_eq + 400_000.0;
    let (pos, vel) = (DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, (earth.gm / r).sqrt(), 0.0));
    let id = sim.vessel_ids.allocate();
    sim.fleet.push(Vessel::coasting(&w, id, sim.clock, earth.node, pos, vel, test_craft()));
    sim.active = sim.fleet.len() - 1;
    assert_eq!(sim.rails_block(), Some(RailsBlock::OtherHeld(f9)));
    // A craft of parts on the pad with nothing lit is not held live.
    let idle = SimState::with_craft(&craft_or_test("falcon9-crew"));
    assert!(idle.ship().clamped() && !held_live(idle.ship()));
}

/// D111: rails warp is refused once thrust is produced, not by where the
/// throttle sits (table): the test craft's engine with its throttle open
/// and propellant; a craft of parts only with an engine lit (or settling)
/// or the pilot's RCS translation with RCS on.
#[test]
fn thrust_not_the_throttle_refuses_rails_warp() {
    use sim::systems::rcs::RcsMode;
    let test = SimState::new();
    let w = test.world.clone();
    let mut dry = test.ship().clone();
    dry.set_propellant(&w, 0.0);
    let f9 = SimState::with_craft(&craft_or_test("falcon9-crew"));
    let mut lit = f9.ship().clone();
    lit.next_stage(&w).expect("the ignition");
    let open = Controls { throttle: 1.0, ..Controls::default() };
    let rcs = RcsMode { on: true, ..RcsMode::default() };
    let translate = Controls { translate: DVec3::Z, ..Controls::default() };
    // (vessel, controls, produces thrust)
    let table = [
        (test.ship(), Controls::default(), false),
        (test.ship(), open, true),
        (&dry, open, false),
        (f9.ship(), open, false),
        (&lit, Controls::default(), true),
        (f9.ship(), translate, false),
        (f9.ship(), Controls { rcs, ..translate }, true),
    ];
    for (i, (v, c, want)) in table.into_iter().enumerate() {
        assert_eq!(produces_thrust(v, &c), want, "case {i}");
    }
    // The throttle open with nothing lit on the Falcon 9 on its pad: rails
    // warp is allowed.
    let mut f9 = f9;
    f9.controls_mut().throttle = 1.0;
    assert_eq!(f9.rails_block(), None);
}
