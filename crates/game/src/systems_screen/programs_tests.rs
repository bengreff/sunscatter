//! The programs' rows: the planned burns by switch and step, the
//! parachutes by switch, state and altitude reading, an installed program,
//! the one rule for a switch and a press, and the Falcon 9's programs.

use super::*;
use sim::systems::ship_os::power_guard::PowerGuard;

fn t0() -> Epoch {
    Epoch::new(100_000_000, 0.0)
}

fn slot(switch: Switch, pressed: bool) -> Slot {
    Slot { switch, pressed }
}

fn burn(step: Step, wake_in: f64) -> Option<Burn> {
    Some(Burn { number: 1, step, wake_in })
}

/// By switch: AUTO waits and flies by itself, ON PRESS waits for a press
/// (and says so once pressed), OFF does not fly it; flying, its step.
#[test]
fn the_planned_burns_by_switch_and_step() {
    // (switch, pressed, burn, now, tone, press offered)
    #[rustfmt::skip]
    let table = [
        (Switch::Auto, false, None, "no burn planned", Tone::Dim, false),
        (Switch::Auto, false, burn(Step::Wait, 90.0), "burn 1 wakes in 1m 30s", Tone::Normal, false),
        (Switch::Press, false, burn(Step::Wait, 90.0), "burn 1 wakes in 1m 30s: waits for a press", Tone::Warn, true),
        (Switch::Press, false, burn(Step::Wait, -12.0), "burn 1 was due 12s ago: waits for a press", Tone::Warn, true),
        (Switch::Press, true, burn(Step::Wait, 90.0), "pressed: flies burn 1 when it wakes (wakes in 1m 30s)", Tone::Good, false),
        (Switch::Off, false, burn(Step::Wait, 90.0), "off: burn 1 is not flown (wakes in 1m 30s); fly it by hand", Tone::Dim, false),
        (Switch::Auto, false, burn(Step::Settle, -3.0), "burn 1: settling the propellant", Tone::Good, false),
        (Switch::Press, false, burn(Step::Burn, -9.0), "burn 1: burning", Tone::Good, false),
        (Switch::Press, false, burn(Step::Done, -99.0), "burn 1: on target", Tone::Good, false),
        (Switch::Auto, false, burn(Step::Short, -99.0), "burn 1: ended short", Tone::Warn, false),
    ];
    for (switch, pressed, b, now, tone, offered) in table {
        let r = maneuver(slot(switch, pressed), b);
        assert_eq!((r.name.as_str(), r.switch), ("maneuver", switch));
        assert_eq!((r.now.as_str(), r.tone), (now, tone), "{switch:?} {pressed} {b:?}");
        assert_eq!(r.press.is_ok(), offered, "{switch:?} {pressed} {b:?}: {:?}", r.press);
    }
    assert_eq!(maneuver(slot(Switch::Press, false), burn(Step::Wait, 9.0)).press, Ok("fly burn 1".into()));
    // Ended: nothing waits for a press (it said "it is flying a burn").
    assert_eq!(maneuver(slot(Switch::Press, false), burn(Step::Done, -9.0)).press, Err("no burn waiting".into()));
}

/// Crew Dragon's parachutes (drogues armed below 5.5 km, mains at 1.8 km).
fn dragon() -> ChuteSpec {
    crate::state::craft_or_fallback("falcon9-crew").spec.chute.clone()
}

fn chutes(switch: Switch, armed: bool, main: CanopyState, called: bool, blind: bool) -> Chutes {
    let state = ChuteState { armed, main, ..ChuteState::default() };
    Chutes { spec: dragon(), deploy: chutes::Chutes { switch, main_called: called }, state, blind }
}

/// Stowed by switch (AUTO blind cannot arm by itself), armed by what opens
/// the mains (their height, a second press switched off or blind, a press
/// already called), open; a press arms, then calls the main, then has
/// nothing left.
#[test]
fn the_parachutes_by_switch_state_and_altitude() {
    let spec = dragon();
    let (main, arm) = (distance(spec.main_height), distance(chutes::arm_height(&spec).expect("Dragon's drogues")));
    let (stowed, open) = (CanopyState::Stowed, CanopyState::Open { age: 30.0, diameter: 10.0 });
    let (arm_it, call) = (Some("arm the drogues"), Some("open the mains once the drogues let them"));
    // (switch, armed, main, called, blind, now, tone, press)
    #[rustfmt::skip]
    let table = [
        (Switch::Auto, false, stowed, false, false, format!("stowed: arms by itself below {arm} while descending"), Tone::Normal, arm_it),
        (Switch::Auto, false, stowed, false, true, "stowed: no altitude reading, so it cannot arm by itself: press to arm".into(), Tone::Warn, arm_it),
        (Switch::Press, false, stowed, false, false, "stowed: arms on a press".into(), Tone::Normal, arm_it),
        (Switch::Off, false, stowed, false, false, "stowed: nothing opens by itself; each press opens the next canopy".into(), Tone::Dim, arm_it),
        (Switch::Press, true, stowed, false, false, format!("armed: the mains open below {main}"), Tone::Good, call),
        (Switch::Press, true, stowed, false, true, "armed: no altitude reading, so the mains open on a second press".into(), Tone::Warn, call),
        (Switch::Off, true, stowed, false, false, "armed: the mains open on a second press".into(), Tone::Warn, call),
        (Switch::Off, true, stowed, true, false, "armed: the mains open once the drogues let them".into(), Tone::Good, None),
        (Switch::Auto, true, open, false, false, "mains open".into(), Tone::Good, None),
        (Switch::Auto, true, CanopyState::Failed, false, false, "mains torn".into(), Tone::Alert, None),
    ];
    for (switch, armed, m, called, blind, now, tone, press) in table {
        let r = parachutes(&chutes(switch, armed, m, called, blind));
        assert_eq!((r.name.as_str(), r.switch), ("chutes", switch));
        assert_eq!((r.now.as_str(), r.tone), (now.as_str(), tone), "{switch:?} {armed} {m:?} {called} {blind}");
        assert_eq!(r.press.as_deref().ok(), press, "{switch:?} {armed} {m:?} {called}");
    }
    let r = parachutes(&chutes(Switch::Press, false, stowed, false, false));
    assert_eq!(
        r.does,
        format!(
            "AUTO arms the drogues below {arm} while descending; the mains open below {main} by the altitude reading, \
             or on a second press"
        )
    );
}

/// A load manager every minute: its loads and lines, the next run, a
/// press only ON PRESS, nothing without a computer.
#[test]
fn an_installed_program_by_switch_and_computer() {
    let guard = PowerGuard { loads: vec!["s2.gps#1".into()], low: 0.2, high: 0.5, shed: false };
    let p = |switch, pressed, shed| Installed {
        name: "power".into(),
        slot: slot(switch, pressed),
        schedule: Schedule::Every { from: t0(), period: 60.0 },
        last: Some(t0().add_seconds(60.0)),
        program: Builtin::PowerGuard(PowerGuard { shed, ..guard.clone() }),
    };
    let now = t0().add_seconds(75.0);
    // (switch, pressed, shed, computing, now, tone, press offered)
    #[rustfmt::skip]
    let table = [
        (Switch::Auto, false, false, true, "loads on; next run in 45s", Tone::Normal, false),
        (Switch::Auto, false, true, true, "1 off until 50 %; next run in 45s", Tone::Warn, false),
        (Switch::Press, false, false, true, "loads on; waits for a press", Tone::Normal, true),
        (Switch::Press, true, false, true, "loads on; pressed: runs at its next epoch (next run in 45s)", Tone::Good, false),
        (Switch::Off, false, false, true, "loads on; off", Tone::Dim, false),
        (Switch::Auto, false, false, false, "loads on; no working flight computer: it misses its epochs", Tone::Alert, false),
    ];
    for (switch, pressed, shed, computing, want, tone, offered) in table {
        let r = installed(&p(switch, pressed, shed), now, computing);
        assert_eq!((r.name.as_str(), r.title.as_str()), ("power", "Load manager"));
        assert_eq!((r.now.as_str(), r.tone), (want, tone), "{switch:?} {pressed} {shed} {computing}");
        assert_eq!(r.press.is_ok(), offered, "{switch:?} {pressed}");
    }
    let r = installed(&p(Switch::Auto, false, false), now, true);
    assert_eq!(
        r.does,
        "switches s2.gps#1 off below 20 % of the charge it reads in the batteries, on again at 50 %, every 1m 00s"
    );
}

/// A switch for a program aboard becomes the command; an unknown one is
/// refused naming those aboard; a press only where the row offers one.
#[test]
fn one_rule_for_a_switch_and_a_press() {
    let v = VesselId(4);
    let rows = vec![
        maneuver(slot(Switch::Press, false), burn(Step::Wait, 30.0)),
        parachutes(&chutes(Switch::Press, true, CanopyState::Open { age: 1.0, diameter: 1.0 }, false, false)),
    ];
    assert_eq!(
        set_switch(&rows, v, "chutes", Switch::Auto),
        Ok(GameCommand::SetSwitch { vessel: v, program: "chutes".into(), switch: Switch::Auto })
    );
    assert_eq!(press(&rows, v, "maneuver"), Ok(GameCommand::Press { vessel: v, program: "maneuver".into() }));
    assert_eq!(press(&rows, v, "chutes"), Err("Parachutes: nothing to press (nothing left to open)".into()));
    assert_eq!(
        set_switch(&rows, v, "warp", Switch::Off),
        Err("no program `warp` aboard (it has maneuver, chutes)".into())
    );
    for (s, label, name) in SWITCHES {
        assert_eq!((super::label(s), super::name(s), by_name(name)), (label, name, Some(s)));
    }
    assert_eq!(by_name("on"), None);
}

/// The Falcon 9 on the pad: the planned burns by themselves with nothing
/// planned, the parachutes on a press (`ship_os`'s defaults), no installed
/// program.
#[test]
fn the_falcon_9s_programs() {
    let sim = crate::state::SimState::with_craft(&crate::state::craft_or_fallback("falcon9-crew"));
    let x = Inputs::of(sim.ship(), sim.clock).expect("a craft of parts");
    let rows = rows(&x);
    let names: Vec<(&str, Switch)> = rows.iter().map(|r| (r.name.as_str(), r.switch)).collect();
    assert_eq!(names, [("maneuver", Switch::Auto), ("chutes", Switch::Press)]);
    assert_eq!(rows[0].now, "no burn planned");
    assert_eq!(rows[1].now, "stowed: arms on a press");
}
