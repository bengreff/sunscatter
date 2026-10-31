//! The ship computer's scheduled programs (D129, D126; `ship_os::program`):
//! a program runs at its own epochs, where the advance stops as at an
//! order, so it acts at the same epochs at every warp.

use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::systems::ship_os::power_guard::PowerGuard;
use sim::systems::ship_os::program::{Builtin, Installed, Schedule, Slot};
use sim::systems::ship_os::Switch;
use sim::time::Epoch;
use sim::vessel::{Controls, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

/// The small rocket in a 400 km orbit with a load manager on its GPS
/// receiver: off below 99.5 % of the battery, on again at 99.9 % (lines set
/// to cross within an orbit: its battery carries its loads in eclipse).
fn guarded(w: &World, t: Epoch) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let deg = std::f64::consts::PI / 180.0;
    let (r, v) = Elements { a: 6_778_137.0, e: 0.0005, i: 51.6 * deg, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }
        .to_state(earth.gm);
    let mut vessel = Vessel::coasting(w, VesselId(1), t, earth.node, r, v, &sim::craft::small::rocket());
    let guard = PowerGuard { loads: vec!["gps".into()], low: 0.995, high: 0.999, shed: false };
    vessel.install(Installed {
        name: "power".into(),
        slot: Slot::auto(),
        schedule: Schedule::Every { from: t.add_seconds(7.0), period: 60.0 },
        last: None,
        program: Builtin::PowerGuard(guard),
    });
    vessel
}

/// Whether the GPS receiver is switched on, and the battery's charge.
fn power(v: &Vessel) -> (bool, f64) {
    let (def, state) = (v.ship_def().unwrap(), v.ship().unwrap());
    let gps = def.components.iter().position(|c| c.id == "gps").unwrap();
    (state.power[gps].on, state.batteries[0].charge)
}

fn fly(w: &World, v: &mut Vessel, end: Epoch, frame: f64) -> Vec<(Epoch, bool)> {
    let mut switches = Vec::new();
    let controls = Controls::default();
    while v.time < end {
        let target = v.time.add_seconds(frame);
        let target = if target > end { end } else { target };
        let before = power(v).0;
        v.advance(w, target, &controls, usize::MAX);
        let last = v.programs()[0].last;
        if power(v).0 != before {
            switches.push((last.unwrap(), power(v).0));
        }
    }
    switches
}

/// Over two orbits the guard switches the receiver off and on at the same
/// epochs at 1× (one-second frames) and at 10⁵× (1,667-second frames),
/// each one of its own epochs, and the battery ends with the same bits.
#[test]
fn a_program_acts_at_its_own_epochs_at_every_warp() {
    let w = world();
    let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
    let end = t.add_seconds(2.0 * 5_550.0);
    let (mut slow, mut fast) = (guarded(&w, t), guarded(&w, t));
    let a = fly(&w, &mut slow, end, 1.0);
    fly(&w, &mut fast, end, 1.0e5 / 60.0);
    assert!(a.iter().any(|s| !s.1) && a.iter().any(|s| s.1), "it switched both ways: {a:?}");
    for (e, _) in &a {
        let k = e.seconds_since(t.add_seconds(7.0)) / 60.0;
        assert!((k - k.round()).abs() < 1e-9, "at one of its epochs: {k}");
    }
    // At 10⁵× a frame holds several switches: compare the end state.
    assert_eq!(power(&slow), power(&fast), "the same bits at every warp");
    assert_eq!(slow.programs(), fast.programs());
    assert_eq!(slow.time, fast.time);
}

/// D126's switch: off, it never runs; on a press, it runs once at its next
/// epoch.
#[test]
fn a_program_switched_off_never_runs_and_a_press_runs_it_once() {
    let w = world();
    let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
    let mut v = guarded(&w, t);
    v.set_switch("power", Switch::Off).unwrap();
    v.advance(&w, t.add_seconds(600.0), &Controls::default(), usize::MAX);
    assert_eq!(v.programs()[0].last, None);
    v.set_switch("power", Switch::Press).unwrap();
    v.advance(&w, t.add_seconds(1_200.0), &Controls::default(), usize::MAX);
    assert_eq!(v.programs()[0].last, Some(t.add_seconds(600.0)), "not pressed: only its restart");
    v.press("power").unwrap();
    v.advance(&w, t.add_seconds(1_800.0), &Controls::default(), usize::MAX);
    assert_eq!(v.programs()[0].last, Some(t.add_seconds(1_207.0)), "once, at its next epoch");
    assert!(v.set_switch("nothing", Switch::Auto).is_err() && v.press("nothing").is_err());
    // The parachutes' deploy takes the same names: a press arms them.
    v.set_switch("chutes", Switch::Auto).unwrap();
    v.press("chutes").unwrap();
    assert!(v.chute.armed);
}

/// The maneuver program's switch (D126): off, a planned burn's program
/// never wakes; on a press given after its wake epoch, it wakes at once.
#[test]
fn the_maneuver_program_waits_for_its_switch() {
    use sim::systems::maneuver::{Node, Step};
    let w = world();
    let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
    let mut v = guarded(&w, t);
    let earth = w.find("Earth").unwrap().node;
    v.set_nodes(vec![Node { t: t.add_seconds(900.0), dv: glam::DVec3::new(5.0, 0.0, 0.0), reference: earth }]).unwrap();
    v.set_switch("maneuver", Switch::Off).unwrap();
    v.advance(&w, t.add_seconds(1_000.0), &Controls::default(), usize::MAX);
    assert_eq!(v.program().unwrap().step, Step::Wait, "off: never woken");
    v.set_switch("maneuver", Switch::Press).unwrap();
    v.advance(&w, t.add_seconds(1_010.0), &Controls::default(), usize::MAX);
    assert_eq!(v.program().unwrap().step, Step::Wait, "not pressed");
    v.press("maneuver").unwrap();
    v.advance(&w, t.add_seconds(1_020.0), &Controls::default(), usize::MAX);
    assert_ne!(v.program().unwrap().step, Step::Wait, "pressed: it wakes");
}

/// The planned burns' settings (D126's config menu): a longer settling
/// time wakes a waiting burn earlier by as much (its schedule again), its
/// ignition the same; a setting it refuses changes nothing.
#[test]
fn the_maneuver_settings_reschedule_a_waiting_burn() {
    use sim::systems::maneuver::{Node, Step};
    use sim::systems::ship_os::program::Setting;
    let w = world();
    let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
    let mut v = guarded(&w, t);
    let earth = w.find("Earth").unwrap().node;
    v.set_nodes(vec![Node { t: t.add_seconds(900.0), dv: glam::DVec3::new(5.0, 0.0, 0.0), reference: earth }]).unwrap();
    let (wake, ignite) = {
        let p = v.program().unwrap();
        (p.wake, p.ignite)
    };
    let lead = v.software().maneuver_settings.settle_lead;
    v.configure("maneuver", &[Setting::SettleLead(lead + 30.0)]).unwrap();
    let p = v.program().unwrap();
    assert_eq!(p.step, Step::Wait);
    assert!((wake.seconds_since(p.wake) - 30.0).abs() < 1e-6, "woken 30 s earlier");
    assert!(p.ignite.seconds_since(ignite).abs() < 1e-9, "the same ignition");
    let before = v.software().clone();
    assert!(v.configure("maneuver", &[Setting::SettleLead(-1.0)]).is_err());
    assert_eq!(*v.software(), before);
}
