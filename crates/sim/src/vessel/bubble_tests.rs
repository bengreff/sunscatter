//! The bubble in flight (D082): scenarios in low Earth orbit with the test
//! craft. Vessel 0 (A) and 1 (B) coast side by side, B drifting onto A at
//! 0.5 m/s; vessel 2 (C), 2 km behind, burns at its least throttle, so it
//! leads a bubble that holds A and B live.

use super::*;
use crate::craft::test_craft;
use crate::ephem::Ephemeris;
use crate::kepler::Elements;
use crate::sol;
use crate::vessel::VesselId;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(86_400.0)
}

/// A 400 km orbit about Earth and its along-track unit vector.
fn leo(w: &World) -> (NodeId, DVec3, DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let along = (v - r * (v.dot(r) / r.length_squared())).normalize();
    (earth.node, r, v, along)
}

/// A and B `gap` m apart along track (B ahead, closing at `closing` m/s),
/// and C 2 km behind A.
fn scene(w: &World, gap: f64, closing: f64) -> Vec<Vessel> {
    let (earth, r, v, along) = leo(w);
    let craft = test_craft();
    let a = Vessel::coasting(w, VesselId(1), t0(), earth, r, v, craft);
    let b = Vessel::coasting(w, VesselId(2), t0(), earth, r + along * gap, v - along * closing, craft);
    let c = Vessel::coasting(w, VesselId(3), t0(), earth, r - along * 2_000.0, v, craft);
    vec![a, b, c]
}

/// C burns at its least throttle (while `burning`); everyone holds its
/// attitude (the game's passive controls).
fn controls(burning: bool) -> impl Fn(usize) -> Controls + Sync {
    move |i| Controls { sas: true, throttle: if i == 2 && burning { 0.1 } else { 0.0 }, ..Default::default() }
}

/// One frame of the game: the bubble first, then every vessel.
fn frame(w: &World, fleet: &mut [Vessel], ctl: ControlsOf, target: Epoch) {
    advance(w, fleet, ctl, target);
    for (i, v) in fleet.iter_mut().enumerate() {
        v.advance(w, target, &ctl(i), COAST_BUDGET);
    }
}

/// Frames of `dt` from the fleet's time for `span` s; calls `each` after
/// every frame.
fn run(w: &World, fleet: &mut [Vessel], ctl: ControlsOf, dt: f64, span: f64, each: &mut dyn FnMut(&[Vessel])) {
    let start = fleet[0].time;
    let frames = (span / dt).round() as usize;
    for k in 1..=frames {
        frame(w, fleet, ctl, start.add_seconds(dt * k as f64));
        each(fleet);
    }
}

fn state(w: &World, v: &Vessel) -> (DVec3, DVec3) {
    let (_, r, vel) = v.state(w);
    (r, vel)
}

/// Two craft drifting together at 0.5 m/s inside a bubble collide and
/// separate: both join at the first epoch, touch, come apart with the
/// approach reversed, and survive (0.5 m/s is far under the 8 m/s limit).
#[test]
fn two_craft_in_a_bubble_collide_and_separate() {
    let w = world();
    let mut fleet = scene(&w, 12.0, 0.5);
    let ctl = controls(true);
    let (mut touched, mut first) = (0, None);
    let mut ticks = 0;
    run(&w, &mut fleet, &ctl, 1.0 / 60.0, 30.0, &mut |f| {
        ticks += 1;
        if f[0].bubble.since_contact == Some(0) {
            touched += 1;
            first.get_or_insert(f[0].time);
        }
    });
    for v in &fleet {
        assert!(v.in_bubble(), "{:?} is a member", v.id());
        assert!(matches!(v.phase, Phase::Powered { .. }));
        assert_eq!(v.destruction(), None);
    }
    assert_eq!(fleet[0].time, fleet[1].time, "members tick in lockstep");
    let ((ra, va), (rb, vb)) = (state(&w, &fleet[0]), state(&w, &fleet[1]));
    let line = (rb - ra).normalize();
    let closing = (vb - va).dot(line);
    eprintln!(
        "touched in {touched} of {ticks} frames from {:.2} s; separating at {closing:.3} m/s, {:.2} m apart",
        first.map_or(f64::NAN, |t| t.seconds_since(t0())),
        (rb - ra).length()
    );
    assert!(touched > 3, "they touched");
    assert!(closing > 0.05 && closing < 0.4, "separating at {closing} m/s");
}

/// Frames of 1/60 s and one call over the same span give the same bits:
/// the bubble's joins, ticks and contacts are on its lattice.
#[test]
fn a_bubble_in_frames_equals_one_call() {
    let w = world();
    let ctl = controls(true);
    let mut frames = scene(&w, 12.0, 0.5);
    run(&w, &mut frames, &ctl, 1.0 / 60.0, 14.0, &mut |_| {});
    let mut once = scene(&w, 12.0, 0.5);
    run(&w, &mut once, &ctl, 14.0, 14.0, &mut |_| {});
    for (a, b) in frames.iter().zip(&once) {
        assert_eq!(a.time, b.time);
        assert_eq!(a.phase, b.phase, "{:?}", a.id());
        assert_eq!(a.attitude, b.attitude);
        assert_eq!(a.bubble, b.bubble);
        assert_eq!(a.thermal, b.thermal);
    }
}

/// Saved and loaded mid-contact (A and B touching), the bubble continues
/// bit for bit: membership, contact counters and the lattice are in the
/// vessels.
#[test]
fn save_and_load_mid_contact_continue_bit_for_bit() {
    let w = world();
    let ctl = controls(true);
    let mut fleet = scene(&w, 12.0, 0.5);
    let start = fleet[0].time;
    let mut k = 0;
    while fleet[0].bubble.since_contact != Some(0) {
        k += 1;
        assert!(k < 60 * 40, "no contact");
        frame(&w, &mut fleet, &ctl, start.add_seconds(k as f64 / 60.0));
    }
    let mut ids = crate::vessel::VesselIds::default();
    while ids.issued(VesselId(1)) && !ids.issued(VesselId(3)) || !ids.issued(VesselId(1)) {
        ids.allocate();
    }
    let save = crate::save::SaveGame::capture(&w, fleet[0].time, &fleet, ids, 0);
    let mut loaded = crate::save::SaveGame::from_ron(&save.to_ron()).expect("loads").vessels;
    assert_eq!(loaded, fleet, "the save round-trips");
    let resume = fleet[0].time;
    for f in 1..=120 {
        let target = resume.add_seconds(f as f64 / 60.0);
        frame(&w, &mut fleet, &ctl, target);
        frame(&w, &mut loaded, &ctl, target);
    }
    assert_eq!(loaded, fleet, "continued alike");
    assert!(fleet[0].bubble.since_contact.is_some());
}

/// The difference a bubble makes to a craft that never touches anything:
/// B 5 km from the leader, flown live for a minute, against B coasting
/// alone: under a millimetre. It is the difference between live ticks and
/// a coast, not the bubble's: a vessel held live alone differs from its
/// coast by the same bits, and the coast from a bare (unperturbed) coast
/// by nothing measurable here (the radiation pressure and thin air the
/// live ticks leave out are ~1e-8 m/s²). Measured: 0.72 mm and 3.9e-5 m/s
/// at 400 km, 0.28 mm and 2.2e-5 m/s at 2,000 km, a steady ~4e-7 m/s²
/// between the live tick's force evaluation and the coast's (reported in
/// `docs/technical/proximity.md`); 1.00 mm at 400 km and 0.14 mm at 2,000 km
/// by 2026-10-26's code, 1.12 mm at 400 km with Earth's field to degree 30
/// (D090): the live substeps' first-order integration of a lumpier field.
#[test]
fn a_minute_in_a_bubble_moves_a_craft_by_under_a_millimetre() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    for (a, most) in [(6_778_137.0, 1.5e-3), (8_378_137.0, 5e-4)] {
        let el = Elements { a, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
        let (r, v) = el.to_state(earth.gm);
        let along = (v - r * (v.dot(r) / r.length_squared())).normalize();
        let craft = test_craft();
        let b = || Vessel::coasting(&w, VesselId(2), t0(), earth.node, r + along * 5_000.0, v, craft);
        let leader = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, craft);
        let ctl = |i: usize| Controls { sas: true, throttle: if i == 0 { 0.1 } else { 0.0 }, ..Default::default() };
        let mut with = vec![leader, b()];
        let mut alone = vec![b()];
        let mut joined = None;
        run(&w, &mut with, &ctl, 0.5, 60.0, &mut |f| {
            if f[1].in_bubble() {
                joined.get_or_insert(f[1].time);
            }
        });
        run(&w, &mut alone, &|_| Controls { sas: true, ..Default::default() }, 0.5, 60.0, &mut |_| {});
        assert!(joined.is_some_and(|t| t.seconds_since(t0()) <= 0.5), "B joins in the first frame");
        assert!(matches!(with[1].phase, Phase::Powered { .. }) && matches!(alone[0].phase, Phase::Coasting { .. }));
        let t = with[1].time;
        let (ra, va) = state(&w, &with[1]);
        let (_, rb, vb) = alone[0].state_at(&w, t);
        let (dr, dv) = ((ra - rb).length(), (va - vb).length());
        eprintln!("a = {:.0} km, 60 s in the bubble: {:.4} mm, {dv:.2e} m/s from the coast", a / 1e3, dr * 1e3);
        assert!(dr < most, "{dr} m");
    }
}

/// When the leader stops burning, its members leave at that epoch and
/// coast on (and the leader with them); nothing is live afterwards.
#[test]
fn members_return_to_coasts_when_the_leader_does() {
    let w = world();
    let mut fleet = scene(&w, 200.0, 0.0);
    run(&w, &mut fleet, &controls(true), 1.0 / 60.0, 2.0, &mut |_| {});
    assert!(fleet.iter().all(|v| v.in_bubble() && matches!(v.phase, Phase::Powered { .. })));
    run(&w, &mut fleet, &controls(false), 1.0 / 60.0, 1.0, &mut |_| {});
    for v in &fleet {
        assert!(!v.in_bubble(), "{:?} left", v.id());
        assert!(matches!(v.phase, Phase::Coasting { .. }), "{:?} coasts", v.id());
    }
    let before = fleet.clone();
    let target = fleet[0].time.add_seconds(10.0);
    let out = advance(&w, &mut fleet, &controls(false), target);
    assert_eq!(out, Advance::default(), "no bubble without a leader");
    assert_eq!(fleet, before, "the bubble pass touched nothing");
}

/// Relative position precision of two members 10 km apart (B drifting
/// sideways at 0.3 m/s): their separation flown live in the bubble (each
/// relative to the shared anchor, rule 3) against the same two craft
/// coasting (compensated integration; the bias between live ticks and
/// coasts is common to both craft and cancels in the difference). A
/// position relative to the anchor is rounded to its distance: Earth's in
/// low orbit (|r| ≈ 7e6 m, ulp 1e-9 m), the barycentre's between the
/// planets at 1 AU (1.5e11 m, 3e-5 m) and at 30 AU (4.5e12 m, 1e-3 m).
/// Members carry each tick's rounding (the low part, compensated
/// summation), so it does not accumulate: without it, 60 s at 30 AU put
/// 0.42 m into the separation (a steady 7 mm/s from the same rounding
/// every tick).
#[test]
fn relative_position_of_members_10_km_apart() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let neptune = w.find("Neptune").unwrap();
    let (r_leo, v_leo) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let root = w.eph.root();
    let snap = w.snapshot(t0());
    // 0.1 AU ahead of a planet on its orbit, moving with it.
    let deep = |planet: NodeId| {
        let k = snap.relative(planet, root);
        (k.r + k.v.normalize() * 1.496e10, k.v)
    };
    let (r_1au, v_1au) = deep(earth.node);
    let (r_30au, v_30au) = deep(neptune.node);
    let cases = [
        // Low orbit: the coasting pair's dense output (one 8th-order step
        // spans t = 43–259 s) is good to ~3 cm, and to ~3e-5 m in their
        // separation, where the two craft's errors nearly cancel.
        ("low orbit", earth.node, r_leo, v_leo, 1e-4),
        ("1 AU", root, r_1au, v_1au, 1e-4),
        ("30 AU", root, r_30au, v_30au, 1e-3),
    ];
    for (name, anchor, r, v, most) in cases {
        let along = (v - r * (v.dot(r) / r.length_squared())).normalize();
        let side = along.cross(r).normalize();
        let craft = test_craft();
        let make = |id: u64, at: DVec3, dv: DVec3| Vessel::coasting(&w, VesselId(id), t0(), anchor, at, v + dv, craft);
        let ctl = |i: usize| Controls { sas: true, throttle: if i == 2 { 0.1 } else { 0.0 }, ..Default::default() };
        let pair = || vec![make(1, r, DVec3::ZERO), make(2, r + along * 10_000.0, side * 0.3)];
        let mut bubble = pair();
        bubble.push(make(3, r + along * 5_000.0 + side * 1_000.0, DVec3::ZERO));
        let mut coast = pair();
        let passive = |_: usize| Controls { sas: true, ..Default::default() };
        run(&w, &mut bubble, &ctl, 0.5, 60.0, &mut |_| {});
        run(&w, &mut coast, &passive, 0.5, 60.0, &mut |_| {});
        assert!(bubble[0].in_bubble() && bubble[1].in_bubble(), "{name}: members");
        let t = bubble[0].time;
        // Live: the positions' low parts (members carry them) added.
        let sep = |f: &[Vessel], live: bool| {
            let (ka, ra, _) = if live { f[0].state(&w) } else { f[0].state_at(&w, t) };
            let (kb, rb, _) = if live { f[1].state(&w) } else { f[1].state_at(&w, t) };
            assert_eq!(ka, kb, "{name}: one anchor");
            (rb - ra) + (f[1].bubble.lo - f[0].bubble.lo)
        };
        let (live, rails) = (sep(&bubble, true), sep(&coast, false));
        let err = (live - rails).length();
        eprintln!("{name}: |r| {:.2e} m, separation {:.3} m, live vs coast {:.3e} m", r.length(), live.length(), err);
        assert!((live.length() - 10_000.0).abs() < 50.0, "{name}: {}", live.length());
        assert!(err < most, "{name}: {err} m");
    }
}

/// A hit faster than the impact limit (8 m/s at any point, D066) destroys
/// both one-part craft; in debug mode (D064) neither breaks.
#[test]
fn a_collision_past_the_impact_limit_destroys_the_craft() {
    let w = world();
    for debug in [false, true] {
        let mut fleet = scene(&w, 12.0, 12.0);
        for v in fleet.iter_mut().take(2) {
            v.set_debug(&w, debug);
        }
        run(&w, &mut fleet, &controls(true), 1.0 / 60.0, 2.0, &mut |_| {});
        for v in fleet.iter().take(2) {
            match (debug, v.destruction()) {
                (false, Some(Destruction::Impact { speed, .. })) => assert!(speed > 8.0, "{speed} m/s"),
                (true, None) => {}
                (_, d) => panic!("debug {debug}: {:?} {d:?}", v.id()),
            }
        }
        assert_eq!(fleet[2].destruction(), None);
    }
}

/// A craft of parts hit too fast loses the parts that were hit (D066 per
/// point, D032): the breakup names them, and the bubble stops at that
/// tick for the split.
#[test]
fn a_craft_of_parts_hit_too_fast_loses_the_part() {
    let w = world();
    let (earth, r, v, along) = leo(&w);
    let craft = crate::craft::registry::craft_of(&crate::craft::stress::id(8));
    let hull = proximity::hull_of(&crate::craft::stress::id(8));
    let gap = 2.0 * hull.radius + 0.5;
    let a = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, &craft);
    let b = Vessel::coasting(&w, VesselId(2), t0(), earth, r + along * gap, v - along * 12.0, &craft);
    let c = Vessel::coasting(&w, VesselId(3), t0(), earth, r - along * 2_000.0, v, test_craft());
    let mut fleet = vec![a, b, c];
    run(&w, &mut fleet, &controls(true), 1.0 / 60.0, 3.0, &mut |_| {});
    for v in fleet.iter().take(2) {
        let b = v.breakup.as_ref().unwrap_or_else(|| panic!("{:?} broke up", v.id()));
        assert!(!b.destroyed.is_empty() && b.destroyed.iter().all(|d| matches!(d.1, Destruction::Impact { .. })));
    }
}

/// The review's scenario: M sits near a leader L in low orbit (Earth its
/// anchor); N, anchored on the Moon, drifts into M at 2 m/s; X, a craft
/// with a lower id, burns in lunar orbit. Bubbles are separate: X's
/// (alone, flown by the fleet) never sets L's frame, nobody is
/// re-anchored, and M and N touch through the frame tree.
#[test]
fn far_apart_leaders_make_separate_bubbles_and_anchors_meet_through_the_frame_tree() {
    let w = world();
    let (earth, r, v, along) = leo(&w);
    let moon = w.find("Moon").unwrap();
    let craft = test_craft();
    let k = w.snapshot(t0()).relative(moon.node, earth);
    let (rm, vm) =
        Elements { a: 1_837_400.0, e: 0.001, i: 0.3, raan: 0.2, argp: 0.0, mean_anomaly: 1.0 }.to_state(moon.gm);
    let x = Vessel::coasting(&w, VesselId(1), t0(), moon.node, rm, vm, craft);
    let l = Vessel::coasting(&w, VesselId(2), t0(), earth, r - along * 2_000.0, v, craft);
    let m = Vessel::coasting(&w, VesselId(3), t0(), earth, r, v, craft);
    // N: 12 m ahead of M along track, closing at 2 m/s, stored on the Moon.
    let n = Vessel::coasting(&w, VesselId(4), t0(), moon.node, r + along * 12.0 - k.r, v - along * 2.0 - k.v, craft);
    assert_eq!(n.state(&w).0, moon.node);
    let mut fleet = vec![x, l, m, n];
    let ctl = |i: usize| Controls { sas: true, throttle: if i <= 1 { 0.1 } else { 0.0 }, ..Default::default() };
    let mut touched = false;
    let mut bubbles = 0;
    let start = fleet[0].time;
    for f in 1..=(12 * 60) {
        let target = start.add_seconds(f as f64 / 60.0);
        let out = advance(&w, &mut fleet, &ctl, target);
        bubbles = bubbles.max(out.bubbles.len());
        assert!(out.bubbles.iter().all(|b| !b.vessels.contains(&0)), "X is not in L's bubble");
        for (i, v) in fleet.iter_mut().enumerate() {
            v.advance(&w, target, &ctl(i), COAST_BUDGET);
        }
        touched |= fleet[2].bubble.since_contact == Some(0);
    }
    assert_eq!(bubbles, 1, "one bubble: L, M and N");
    let anchors: Vec<NodeId> = fleet.iter().map(|v| v.state(&w).0).collect();
    assert_eq!(anchors, vec![moon.node, earth, earth, moon.node], "nobody re-anchored");
    assert!(fleet[2].in_bubble() && fleet[3].in_bubble() && !fleet[0].in_bubble());
    assert!(touched, "M and N touched");
    let (rm, vm) = state_about(&w, &fleet[2], fleet[2].time, earth);
    let (rn, vn) = state_about(&w, &fleet[3], fleet[3].time, earth);
    let line = (rn - rm).normalize();
    eprintln!("M and N {:.2} m apart, separating at {:.3} m/s", (rn - rm).length(), (vn - vm).dot(line));
    assert!((vn - vm).dot(line) > 0.1, "they bounced apart");
}

/// Two bubbles far apart (one in low orbit, one in lunar orbit, each a
/// leader and a neighbour) advance on their own threads exactly as each
/// alone.
#[test]
fn separate_bubbles_advance_as_each_alone() {
    let w = world();
    let (earth, r, v, along) = leo(&w);
    let moon = w.find("Moon").unwrap();
    let (rm, vm) =
        Elements { a: 1_837_400.0, e: 0.001, i: 0.3, raan: 0.2, argp: 0.0, mean_anomaly: 1.0 }.to_state(moon.gm);
    let along_m = (vm - rm * (vm.dot(rm) / rm.length_squared())).normalize();
    let craft = test_craft();
    let make =
        |id: u64, node: NodeId, at: DVec3, vel: DVec3| Vessel::coasting(&w, VesselId(id), t0(), node, at, vel, craft);
    let earth_pair = || vec![make(1, earth, r, v), make(2, earth, r + along * 3_000.0, v - along * 0.2)];
    let moon_pair = || vec![make(3, moon.node, rm, vm), make(4, moon.node, rm + along_m * 3_000.0, vm)];
    let burn = |first: usize| {
        move |i: usize| Controls { sas: true, throttle: if i == first { 0.1 } else { 0.0 }, ..Default::default() }
    };
    let mut both: Vec<Vessel> = earth_pair().into_iter().chain(moon_pair()).collect();
    let both_ctl =
        |i: usize| Controls { sas: true, throttle: if i == 0 || i == 2 { 0.1 } else { 0.0 }, ..Default::default() };
    let mut out = Advance::default();
    run(&w, &mut both, &both_ctl, 1.0 / 30.0, 4.0, &mut |_| {});
    out.bubbles.clear();
    let (mut e, mut m) = (earth_pair(), moon_pair());
    run(&w, &mut e, &burn(0), 1.0 / 30.0, 4.0, &mut |_| {});
    run(&w, &mut m, &burn(0), 1.0 / 30.0, 4.0, &mut |_| {});
    assert!(both.iter().all(|v| v.in_bubble()));
    assert_eq!(both, e.into_iter().chain(m).collect::<Vec<_>>());
    let target = both[0].time.add_seconds(0.1);
    assert_eq!(advance(&w, &mut both, &both_ctl, target).bubbles.len(), 2);
}

/// A breakup in contact: the bubble waits at the breakup's epoch until the
/// split is applied (none of its craft moves), so frames of 1/60 s and of
/// 1/7 s give the same bits through the collision, the split and after.
#[test]
fn a_breakup_in_contact_waits_for_the_split_whatever_the_frames() {
    let w = world();
    let (earth, r, v, along) = leo(&w);
    let id = crate::craft::stress::id(8);
    let craft = crate::craft::registry::craft_of(&id);
    let gap = 2.0 * proximity::hull_of(&id).radius + 0.5;
    let scene = || {
        vec![
            Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, &craft),
            Vessel::coasting(&w, VesselId(2), t0(), earth, r + along * gap, v - along * 12.0, &craft),
            Vessel::coasting(&w, VesselId(3), t0(), earth, r - along * 2_000.0, v, test_craft()),
        ]
    };
    let ctl = controls(true);
    let fly = |dt: f64| {
        let mut fleet = scene();
        let mut ids = crate::vessel::VesselIds::default();
        while !ids.issued(VesselId(3)) {
            ids.allocate();
        }
        let (start, mut split_at) = (t0(), Vec::new());
        for k in 1..=(3.0 / dt).round() as usize {
            frame(&w, &mut fleet, &ctl, start.add_seconds(dt * k as f64));
            // As the game does: split what broke (here at once).
            for i in 0..fleet.len() {
                let Some(plan) = fleet[i].plan_breakup() else { continue };
                // Every craft of the bubble waited at the breakup's epoch.
                let epoch = fleet[i].breakup().expect("broke").epoch;
                assert!(
                    fleet.iter().take(3).all(|v| v.time == epoch),
                    "{:?}",
                    fleet.iter().map(|v| v.time).collect::<Vec<_>>()
                );
                split_at.push(epoch);
                let pieces = fleet[i].split(&w, &plan, &plan.crafts(), &mut ids);
                fleet.extend(pieces);
            }
        }
        (fleet, split_at)
    };
    let (fine, fine_at) = fly(1.0 / 60.0);
    let (coarse, coarse_at) = fly(1.0 / 7.0);
    assert!(!fine_at.is_empty(), "they broke up");
    assert!(fine.len() > 3, "pieces flew off");
    assert_eq!(fine_at, coarse_at);
    assert_eq!(fine, coarse);
}
