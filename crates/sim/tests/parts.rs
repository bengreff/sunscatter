//! The part layer in flight (`docs/technical/parts.md`, D032, D083): splits
//! conserve momentum and carry their state, a split is deterministic (frames
//! equal one jump), breakage triggers, saves of craft of parts and of their
//! pieces, and the debris rule over a fleet.

use glam::{DMat3, DVec3};
use sim::craft::{craft_of, stress, Craft, CraftId};
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::debris::{debris, separation};
use sim::vessel::{Attitude, Controls, Destruction, Phase, Vessel, VesselId, VesselIds};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

fn orbit(w: &World, a: f64, phase: f64) -> (sim::frame::NodeId, DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let (r, v) = Elements { a, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: phase }.to_state(earth.gm);
    (earth.node, r, v)
}

/// The placeholder stress layout of `n` parts, registered as
/// `placeholder-<n>`: these tests were written on its parts and numbers,
/// and move to `stress-<n>` (catalog parts) with the made-up path's
/// deletion (D121).
fn placeholder(n: usize) -> CraftId {
    let id = CraftId(format!("placeholder-{n}"));
    sim::craft::registry::register(&id, stress::layout(n).def());
    id
}

fn craft(n: usize) -> Arc<Craft> {
    craft_of(&placeholder(n))
}

/// The parts of a craft whose names start with `prefix`.
fn named(c: &Craft, prefix: &str) -> Vec<u32> {
    (0..c.parts.len() as u32).filter(|&i| c.parts[i as usize].name.starts_with(prefix)).collect()
}

/// Mass, centre of mass, momentum and angular momentum about the centre of
/// mass of vessels at their (common) time and anchor.
fn totals(w: &World, vessels: &[&Vessel]) -> (f64, DVec3, DVec3, DVec3) {
    let states: Vec<_> = vessels.iter().map(|v| (v.state(w), v.mass_props(), v.attitude)).collect();
    assert!(states.iter().all(|s| s.0 .0 == states[0].0 .0), "one anchor");
    let mass: f64 = states.iter().map(|s| s.1.mass).sum();
    let com = states.iter().map(|s| s.0 .1 * s.1.mass).sum::<DVec3>() / mass;
    let momentum = states.iter().map(|s| s.0 .2 * s.1.mass).sum::<DVec3>();
    let vcom = momentum / mass;
    let angular = (states.iter())
        .map(|((_, r, v), p, att)| {
            let rot = DMat3::from_quat(att.q);
            rot * p.inertia * rot.transpose() * att.omega + (*r - com).cross(*v - vcom) * p.mass
        })
        .sum::<DVec3>();
    (mass, com, momentum, angular)
}

fn assert_conserved(before: (f64, DVec3, DVec3, DVec3), after: (f64, DVec3, DVec3, DVec3)) {
    assert!((after.0 / before.0 - 1.0).abs() < 1e-12, "mass {} vs {}", after.0, before.0);
    assert!((after.1 - before.1).length() < 1e-6, "centre of mass moved {}", (after.1 - before.1).length());
    let dp = (after.2 - before.2).length() / before.2.length();
    assert!(dp < 1e-13, "momentum {dp:e}");
    // Relative to the spin's, or to 1 m²/s per kilogram for a craft at rest.
    let dl = (after.3 - before.3).length() / before.3.length().max(before.0);
    assert!(dl < 1e-9, "angular momentum {dl:e}: {} vs {}", after.3, before.3);
}

/// A 40-part stress craft tumbling in a low orbit, 60 % full, loses a
/// booster (strut, tank, nose, engine, strut): the vessel keeps the core,
/// the booster flies as a new vessel; mass, centre of mass, momentum and
/// angular momentum are those of the craft; each piece's tanks are at the
/// craft's fill; both restart coasts at the split epoch with the same
/// attitude and angular velocity.
#[test]
fn a_split_in_a_coast_conserves_momentum_and_restarts_coasts() {
    let w = world();
    let c = craft(40);
    let (anchor, r, v) = orbit(&w, 6_778_137.0, 0.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), anchor, r, v, &c);
    a.set_propellant(&w, 0.6 * c.mass.capacity);
    a.set_attitude(Attitude { q: a.attitude.q, omega: DVec3::new(0.02, -0.01, 0.03) });
    let at = t0().add_seconds(30.0);
    a.advance(&w, at, &Controls::default(), usize::MAX);
    let before = totals(&w, &[&a]);
    let spin = (a.attitude.q, a.attitude.omega);
    let booster = named(&c, "booster 1.0 ");
    assert_eq!(booster.len(), 5);
    let plan = a.plan_detach(&booster[..1]);
    assert_eq!(plan.pieces.len(), 2);
    assert!(booster.iter().all(|p| plan.pieces[1].contains(p)), "{:?}", plan.pieces);
    let mut ids = VesselIds::default();
    ids.allocate();
    let pieces = a.split(&w, &plan, &plan.crafts(), &mut ids);
    assert_eq!(pieces.len(), 1);
    let b = &pieces[0];
    assert_eq!((a.id(), b.id()), (VesselId(1), VesselId(2)));
    assert_conserved(before, totals(&w, &[&a, b]));
    for v in [&a, b] {
        assert_eq!(v.time, at);
        assert_eq!((v.attitude.q, v.attitude.omega), spin);
        let Phase::Coasting { trajectory } = &v.phase else { panic!("not coasting") };
        assert_eq!(trajectory.current().t0, at, "a new coast from the split epoch");
        let fill = v.propellant() / v.craft.mass.capacity;
        assert!((fill - 0.6).abs() < 1e-12, "fill {fill}");
    }
    assert_eq!(b.craft.id, CraftId::piece("placeholder-40", &plan.pieces[1]));
    assert_eq!(b.craft.id.0, "placeholder-40#12-16,32,36", "the booster and the small parts on it");
    // The booster has an engine but no control part; the core keeps the pod.
    assert!(b.craft.engine.thrust_vac > 0.0 && !b.controllable() && a.controllable());
    assert_eq!((a.crew(), b.crew()), (3, 0));
}

/// Live: a 40-part craft burning in vacuum for 20 s (every bell heats)
/// splits off a booster; momentum is conserved at the split, the pieces
/// stay live, and each piece's skin and interior temperatures are those of
/// its part before (the booster's hot bell included).
#[test]
fn a_live_split_conserves_momentum_and_carries_temperatures() {
    let w = world();
    let c = craft(40);
    let (anchor, r, v) = orbit(&w, 6_778_137.0, 1.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), anchor, r, v, &c);
    let burn = Controls { throttle: 1.0, sas: true, ..Default::default() };
    a.advance(&w, t0().add_seconds(20.0), &burn, usize::MAX);
    assert!(matches!(a.phase, Phase::Powered { .. }));
    let old = a.craft.design().clone();
    let skin = a.thermal().state.skin.clone();
    let booster = named(&c, "booster 1.0 ");
    let plan = a.plan_detach(&booster[..1]);
    assert_eq!(plan.pieces.len(), 2);
    let before = totals(&w, &[&a]);
    let pieces = a.split(&w, &plan, &plan.crafts(), &mut VesselIds::default());
    let b = &pieces[0];
    assert_conserved(before, totals(&w, &[&a, b]));
    assert!(matches!(a.phase, Phase::Powered { .. }) && matches!(b.phase, Phase::Powered { .. }));
    // Each part's hottest cell is as hot as it was.
    for v in [&a, b] {
        let d = v.craft.design();
        for (k, part) in d.parts.iter().enumerate() {
            let was = (0..skin.len()).filter(|&i| old.parts[old.cell_part[i] as usize].id == part.id).map(|i| skin[i]);
            let now = (0..d.cells.len()).filter(|&i| d.cell_part[i] as usize == k).map(|i| v.thermal().state.skin[i]);
            assert_eq!(now.fold(0.0, f64::max), was.fold(0.0, f64::max), "{}", part.name);
        }
    }
    let bell = b.thermal().state.skin.iter().fold(0.0f64, |m, &t| m.max(t));
    assert!(bell > 400.0, "the booster's bell after a 20 s burn: {bell} K");
}

/// Advances a fleet to `end` in steps of `dt`, splitting every vessel that
/// broke up (its pieces join the fleet) and advancing again to the step.
fn fly(w: &World, fleet: &mut Vec<Vessel>, ids: &mut VesselIds, end: Epoch, dt: f64, controls: &Controls) {
    let passive = Controls::default();
    let mut t = fleet[0].time;
    while t < end {
        t = t.add_seconds(dt);
        if t > end {
            t = end;
        }
        loop {
            for (i, v) in fleet.iter_mut().enumerate() {
                v.advance(w, t, if i == 0 { controls } else { &passive }, usize::MAX);
            }
            // The earliest breakup first (ties in fleet order), so ids and
            // the fleet's order do not depend on the step.
            let first = (0..fleet.len())
                .filter_map(|i| fleet[i].breakup().map(|b| (b.epoch, i)))
                .min_by(|a, b| a.0.seconds_since(b.0).total_cmp(&0.0).then(a.1.cmp(&b.1)));
            let Some((_, i)) = first else { break };
            let plan = fleet[i].plan_breakup().unwrap();
            let pieces = fleet[i].split(w, &plan, &plan.crafts(), ids);
            fleet.extend(pieces);
        }
    }
}

/// A 12-part craft (core, fins, legs) entering broadside at 7.6 km/s from
/// 85 km: a part's skin passes its limit, the part is destroyed and leaves
/// as a wreck, the rest flies on. Flown in 1× frames (60 fps) and in one
/// jump, the fleets are the same bits. In debug mode nothing breaks.
#[test]
fn an_overheated_part_breaks_off_and_frames_equal_one_jump() {
    let w = world();
    let c = craft(12);
    let earth = w.find("Earth").unwrap();
    let r = DVec3::X * (earth.physical.as_ref().unwrap().radius_eq + 85e3);
    let v = DVec3::new(-0.035, 1.0, 0.0).normalize() * 7_600.0;
    let start = |debug: bool| {
        let mut s = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, &c);
        s.set_debug(&w, debug);
        vec![s]
    };
    let end = t0().add_seconds(70.0);
    let mut jump = start(false);
    let mut ids = VesselIds::default();
    ids.allocate();
    let mut frames = (jump.clone(), ids);
    fly(&w, &mut jump, &mut ids, end, 70.0, &Controls::default());
    assert!(jump.len() >= 2, "no part broke off: {} vessels", jump.len());
    let wrecks: Vec<&Vessel> = jump.iter().filter(|v| v.destruction().is_some()).collect();
    assert!(!wrecks.is_empty());
    for wreck in &wrecks {
        let Some(Destruction::Overheat { temperature, .. }) = wreck.destruction() else { panic!("not overheated") };
        assert!(temperature > 1_000.0, "{temperature}");
        assert_eq!(wreck.craft.design().parts.len(), 1, "a wreck is one part");
    }
    fly(&w, &mut frames.0, &mut frames.1, end, 1.0 / 60.0, &Controls::default());
    assert_eq!(frames.0.len(), jump.len());
    for (a, b) in frames.0.iter().zip(&jump) {
        assert_eq!(a, b, "vessel {}", a.id());
    }
    let mut debug = start(true);
    fly(&w, &mut debug, &mut VesselIds::default(), end, 70.0, &Controls::default());
    assert_eq!(debug.len(), 1, "debug mode: nothing breaks");
}

/// A 40-part craft flown sideways through dense air (10 km, 400 m/s): the
/// parts facing the flow carry more than their load limits (the panels
/// broadside to it, a booster's tank) and are torn off intact, each with
/// what hangs from it (the tank with its booster's nose and engine).
#[test]
fn an_overloaded_part_tears_off_with_what_hangs_from_it() {
    let w = world();
    let c = craft(40);
    let earth = w.find("Earth").unwrap();
    let r = DVec3::X * (earth.physical.as_ref().unwrap().radius_eq + 10e3);
    let spin = earth.physical.as_ref().unwrap().rotation.omega(t0()).raw().cross(r);
    // Nose up (+X), the body's X axis horizontal: the air comes along −X.
    let probe = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, spin, &c);
    let side = probe.attitude.q * DVec3::X;
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, spin + side * 400.0, &c);
    a.advance(&w, t0().add_seconds(1.0), &Controls::default(), usize::MAX);
    let b = a.breakup().expect("a part tore off").clone();
    assert!(b.destroyed.is_empty());
    let d = a.craft.design().clone();
    for &(p, load) in &b.torn {
        assert!(load > d.parts[p as usize].max_load, "{}", d.parts[p as usize].name);
    }
    let torn: Vec<&str> = b.torn.iter().map(|t| d.parts[t.0 as usize].name.as_str()).collect();
    assert!(torn.iter().filter(|n| n.starts_with("panel")).count() >= 2, "{torn:?}");
    let plan = a.plan_breakup().unwrap();
    let tank = named(&c, "booster 1.0 tank 0")[0];
    let nose = named(&c, "booster 1.0 nose")[0];
    assert!(torn.contains(&"booster 1.0 tank 0"), "{torn:?}");
    assert!(plan.pieces.iter().any(|p| p.contains(&tank) && p.contains(&nose)), "{:?}", plan.pieces);
    assert!(plan.destroyed.iter().all(Option::is_none));
    let before = totals(&w, &[&a]);
    let pieces = a.split(&w, &plan, &plan.crafts(), &mut VesselIds::default());
    let mut all = vec![&a];
    all.extend(pieces.iter());
    assert_conserved(before, totals(&w, &all));
    assert!(a.breakup().is_none() && pieces.iter().all(|p| p.destruction().is_none()));
}

/// The pieces torn off above, still overloaded in the next ticks: a
/// piece's root part has no parent to be torn from, so it never counts as
/// torn (review of 2026-10-26, PARTS 2: it gave a one-piece plan, the split
/// did nothing, and the vessel stopped again every tick). Every breakup
/// that follows has at least two pieces.
#[test]
fn a_pieces_root_is_never_torn() {
    let w = world();
    let c = craft(40);
    let earth = w.find("Earth").unwrap();
    let r = DVec3::X * (earth.physical.as_ref().unwrap().radius_eq + 10e3);
    let spin = earth.physical.as_ref().unwrap().rotation.omega(t0()).raw().cross(r);
    let probe = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, spin, &c);
    let side = probe.attitude.q * DVec3::X;
    let mut fleet = vec![Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, spin + side * 400.0, &c)];
    let mut ids = VesselIds::default();
    let (mut splits, mut end) = (0, t0());
    for _ in 0..20 {
        end = end.add_seconds(0.25);
        for k in 0..fleet.len() {
            fleet[k].advance(&w, end, &Controls::default(), usize::MAX);
            let Some(b) = fleet[k].breakup().cloned() else { continue };
            let d = fleet[k].craft.design().clone();
            for &(p, _) in &b.torn {
                assert!(d.parts[p as usize].parent.is_some(), "the root {} torn", d.parts[p as usize].name);
            }
            let plan = fleet[k].plan_breakup().unwrap();
            assert!(plan.pieces.len() >= 2, "a breakup of one piece: {:?}", b);
            let pieces = fleet[k].split(&w, &plan, &plan.crafts(), &mut ids);
            splits += 1;
            fleet.extend(pieces);
        }
    }
    assert!(splits >= 2 && fleet.len() > 2, "{splits} splits, {} vessels", fleet.len());
}

/// A 12-part craft dropped onto the pad at 10 m/s (its impact limit is
/// 8 m/s): a foot hits too fast and its leg is destroyed (D066 per point);
/// the craft is not.
#[test]
fn a_hard_landing_breaks_a_leg_not_the_craft() {
    let w = world();
    let c = craft(12);
    let pad = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6082, -80.66, t0(), &c);
    let (anchor, r, v) = pad.state(&w);
    let up = r.normalize();
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), anchor, r + up * 1.0, v - up * 10.0, &c);
    a.advance(&w, t0().add_seconds(1.0), &Controls::default(), usize::MAX);
    let b = a.breakup().expect("an impact broke a part").clone();
    let d = a.craft.design().clone();
    let (part, cause) = b.destroyed[0];
    assert!(d.parts[part as usize].name.starts_with("leg"), "{}", d.parts[part as usize].name);
    let Destruction::Impact { speed, .. } = cause else { panic!("{cause:?}") };
    assert!(speed > 8.0);
    assert!(a.destruction().is_none());
}

/// A light piece of a big craft (its ground springs sized for the whole
/// craft) dropped onto the pad at 10 m/s: the explicit contact substeps
/// never add energy (relative to the turning ground, with gravity's and the
/// spin's potential), and it comes to rest (review of 2026-10-26, PARTS 3:
/// a 250 kg wreck went from 4.7 to 11.0 kJ).
#[test]
fn a_light_piece_dropped_on_the_pad_loses_energy_and_rests() {
    let w = world();
    let c = craft(40);
    let earth = w.find("Earth").unwrap();
    let spin = earth.physical.as_ref().unwrap().rotation.omega(t0()).raw();
    // The single part nearest 250 kg.
    let part = (0..c.parts.len())
        .min_by(|&a, &b| (c.parts[a].dry_mass - 250.0).abs().total_cmp(&(c.parts[b].dry_mass - 250.0).abs()));
    let part = &c.parts[part.unwrap()];
    let piece = sim::craft::craft_of(&CraftId::piece(placeholder(40).base(), &[part.id]));
    let pad = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6082, -80.66, t0(), &piece);
    let (anchor, r, v) = pad.state(&w);
    assert_eq!(anchor, earth.node);
    let up = r.normalize();
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), anchor, r + up * 1.0, v - up * 10.0, &piece);
    let energy = |a: &Vessel| {
        let (_, r, v) = a.state(&w);
        let p = a.mass_props();
        let k = sim::contact::kinetic_per_mass(
            p.mass,
            &p.inertia,
            a.attitude.q,
            v - spin.cross(r),
            a.attitude.omega - spin,
        );
        p.mass * (k - earth.gm / r.length() - 0.5 * spin.cross(r).length_squared())
    };
    let e0 = energy(&a);
    let mut most = f64::NEG_INFINITY;
    let mut t = t0();
    for _ in 0..1_500 {
        t = t.add_seconds(0.02);
        a.advance(&w, t, &Controls::default(), usize::MAX);
        if !matches!(a.phase, Phase::Powered { .. }) {
            break;
        }
        most = most.max(energy(&a) - e0);
    }
    let (_, r, v) = a.state(&w);
    let (speed, turn) = ((v - spin.cross(r)).length(), (a.attitude.omega - spin).length());
    println!("{} ({:.0} kg): most energy gained {most:.1} J; {speed:.1e} m/s, {turn:.1e} rad/s", part.name, a.mass());
    assert!(most < 0.002 * 0.5 * a.mass() * 100.0, "gained {most} J");
    // At rest: frozen, or (an intact piece on its side, which no stance
    // holds) still on the ground.
    let frozen = matches!(a.phase, Phase::Landed { .. } | Phase::Crashed { .. });
    assert!(frozen || (speed < 1e-3 && turn < 1e-3), "not at rest: {speed} m/s, {turn} rad/s");
}

/// A craft of parts and the pieces of a split saved and loaded continue as
/// if never saved (their designs come back from their ids).
#[test]
fn a_split_fleet_continues_bit_for_bit_after_a_save() {
    let w = world();
    let c = craft(40);
    let (anchor, r, v) = orbit(&w, 6_778_137.0, 2.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), anchor, r, v, &c);
    a.set_attitude(Attitude { q: a.attitude.q, omega: DVec3::new(0.0, 0.01, 0.0) });
    let at = t0().add_seconds(100.0);
    a.advance(&w, at, &Controls::default(), usize::MAX);
    let plan = a.plan_detach(&named(&c, "booster 1.1 ")[..1]);
    let mut ids = VesselIds::default();
    ids.allocate();
    let pieces = a.split(&w, &plan, &plan.crafts(), &mut ids);
    let mut fleet = vec![a];
    fleet.extend(pieces);
    let text = SaveGame::capture(&w, at, &fleet, ids, 0).to_ron();
    let mut loaded = SaveGame::from_ron(&text).unwrap().vessels;
    assert_eq!(loaded, fleet);
    let end = at.add_seconds(5_000.0);
    for (x, y) in fleet.iter_mut().zip(loaded.iter_mut()) {
        x.advance(&w, end, &Controls::default(), usize::MAX);
        y.advance(&w, end, &Controls::default(), usize::MAX);
        assert_eq!(x, y);
    }
}

/// D083 over a fleet: a piece with no control part and no power is dead;
/// it stays while a controllable craft is within 10 km and goes once none
/// is; the active vessel is never deleted.
#[test]
fn debris_goes_once_far_from_every_controllable_craft() {
    let w = world();
    let c = craft(12);
    let (anchor, r, v) = orbit(&w, 6_778_137.0, 3.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), anchor, r, v, &c);
    let fin = named(&c, "fin 0")[0];
    let leg = named(&c, "leg 0")[0];
    let plan = a.plan_split(&[fin, leg]);
    let mut ids = VesselIds::default();
    ids.allocate();
    let piece = a.split(&w, &plan, &plan.crafts(), &mut ids).remove(0);
    assert!(a.controllable() && !a.dead());
    assert!(!piece.controllable() && piece.dead());
    let t = a.time;
    assert!(separation(&w, t, &a, &piece) < 100.0);
    // Close to the craft it came from: kept.
    let fleet = vec![a.clone(), piece.clone()];
    assert_eq!(debris(&w, t, &fleet, 0), Vec::<usize>::new());
    // The craft gone, another controllable craft 50 km higher: deleted.
    let (anchor, r, v) = orbit(&w, 6_828_137.0, 3.0);
    let far = Vessel::coasting(&w, VesselId(3), t0(), anchor, r, v, &c);
    let mut far = far;
    far.advance(&w, t, &Controls::default(), usize::MAX);
    assert!(separation(&w, t, &far, &piece) > 10_000.0);
    assert_eq!(debris(&w, t, &[far.clone(), piece.clone()], 0), vec![1]);
    // Never the active vessel; never a controllable one.
    assert_eq!(debris(&w, t, &[far.clone(), piece.clone()], 1), Vec::<usize>::new());
    assert_eq!(debris(&w, t, &[piece, far], 0), Vec::<usize>::new());
}

/// A split in a coast at a deterministic epoch: reached in 1× frames or in
/// one jump, then flown on for three days in 1,000,000× frames or in one
/// jump, the vessel and its piece are the same bits.
#[test]
fn a_coast_split_is_the_same_in_frames_and_in_one_jump() {
    let w = world();
    let c = craft(40);
    let (anchor, r, v) = orbit(&w, 6_778_137.0, 4.0);
    let mut a = Vessel::coasting(&w, VesselId(1), t0(), anchor, r, v, &c);
    a.set_attitude(Attitude { q: a.attitude.q, omega: DVec3::new(0.005, 0.0, 0.01) });
    let mut b = a.clone();
    let at = t0().add_seconds(20.0);
    let free = Controls::default();
    let mut t = t0();
    while t < at {
        t = t.add_seconds(1.0 / 60.0);
        a.advance(&w, if t > at { at } else { t }, &free, usize::MAX);
    }
    b.advance(&w, at, &free, usize::MAX);
    assert_eq!(a, b);
    let split = |v: &mut Vessel| {
        let plan = v.plan_detach(&named(&c, "booster 1.2 ")[..1]);
        let mut ids = VesselIds::default();
        ids.allocate();
        let mut fleet = v.split(&w, &plan, &plan.crafts(), &mut ids);
        fleet.insert(0, v.clone());
        fleet
    };
    let (mut fa, mut fb) = (split(&mut a), split(&mut b));
    assert_eq!(fa, fb);
    let end = at.add_seconds(3.0 * 86_400.0);
    for v in &mut fa {
        let mut t = at;
        while t < end {
            t = t.add_seconds(1e6 / 60.0);
            v.advance(&w, if t > end { end } else { t }, &free, usize::MAX);
        }
    }
    for v in &mut fb {
        v.advance(&w, end, &free, usize::MAX);
    }
    assert_eq!(fa, fb);
    assert!(fa.iter().all(|v| v.time == end));
}

/// A craft's skin roughness (`AeroFile::roughness`, D085) reaches the live
/// tick's flow; craft files without it are smooth.
#[test]
fn a_crafts_roughness_reaches_the_flow() {
    use sim::craft::registry::register;
    use sim::vessel::{air_at, AeroTick};
    let w = world();
    let mut def = stress::layout(12).def();
    def.spec.aero.roughness = 0.74e-3;
    let id = CraftId("rough-12".into());
    register(&id, def);
    let rough = craft_of(&id);
    assert_eq!(rough.design().roughness, 0.74e-3);
    assert_eq!(sim::craft::small::rocket().design().roughness, 0.0);
    let earth = w.find("Earth").unwrap();
    let r = DVec3::X * (earth.physical.as_ref().unwrap().radius_eq + 10e3);
    let v = DVec3::Y * 300.0;
    let air = air_at(&w, &w.snapshot(t0()), earth.node, r, v).expect("air at 10 km");
    let tick = AeroTick::new(rough.design(), &air, rough.mass.dry_com, None).expect("wind");
    assert_eq!(tick.flow(glam::DQuat::IDENTITY).roughness, 0.74e-3);
}
