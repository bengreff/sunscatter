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
    Chutes {
        spec: dragon(),
        deploy: chutes::Chutes { switch, main_called: called, ..Default::default() },
        state,
        blind,
    }
}

/// Stowed by switch (AUTO blind cannot arm by itself), armed by what opens
/// the mains (their height, a second press switched off or blind, a press
/// already called), open; a press arms, then calls the main, then has
/// nothing left.
#[test]
fn the_parachutes_by_switch_state_and_altitude() {
    let spec = dragon();
    let (main, arm) = (
        distance(spec.main_height),
        distance(chutes::arm_height(&spec, &Default::default()).expect("Dragon's drogues")),
    );
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
        let r = installed(&p(switch, pressed, shed), now, computing, &[]);
        assert_eq!((r.name.as_str(), r.title.as_str()), ("power", "Load manager"));
        assert_eq!((r.now.as_str(), r.tone), (want, tone), "{switch:?} {pressed} {shed} {computing}");
        assert_eq!(r.press.is_ok(), offered, "{switch:?} {pressed}");
    }
    let r = installed(&p(Switch::Auto, false, false), now, true, &[]);
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

/// The config menus: Dragon's parachutes by their two heights (the craft's
/// until the pilot sets them), a chute without drogues by one, a load
/// manager by its loads, lines and period; the planned burns have none.
#[test]
fn each_programs_settings() {
    let row = |r: &Row| -> Vec<(&str, &str, String)> {
        r.settings.iter().map(|p| (p.key, p.label, p.value.text())).collect()
    };
    let dragon = chutes(Switch::Press, false, CanopyState::Stowed, false, false);
    assert_eq!(
        row(&parachutes(&dragon)),
        [(ARM_HEIGHT, "DROGUES ARM BELOW", distance(5_500.0)), (MAIN_HEIGHT, "MAINS OPEN BELOW", distance(1_800.0))]
    );
    let set = chutes::Chutes { arm_height: Some(7_000.0), main_height: Some(2_500.0), ..dragon.deploy };
    let r = parachutes(&Chutes { deploy: set, ..dragon.clone() });
    assert_eq!(row(&r)[0].2, distance(7_000.0), "the pilot's");
    assert!(r.does.contains(&distance(7_000.0)) && r.does.contains(&distance(2_500.0)), "{}", r.does);
    let lone = Chutes { spec: ChuteSpec { drogue: None, ..dragon.spec.clone() }, ..dragon.clone() };
    assert_eq!(row(&parachutes(&lone)), [(MAIN_HEIGHT, "OPEN BELOW", distance(1_800.0))]);
    let blind = Chutes { spec: ChuteSpec { drogue_height: None, ..dragon.spec.clone() }, ..dragon };
    assert_eq!(row(&parachutes(&blind))[0].2, "none set", "no arming height: it never arms by itself");

    let guard = PowerGuard { loads: vec!["a".into(), "b".into()], low: 0.25, high: 0.6, shed: false };
    let p = Installed {
        name: "power".into(),
        slot: Slot::auto(),
        schedule: Schedule::Every { from: t0(), period: 600.0 },
        last: None,
        program: Builtin::PowerGuard(guard),
    };
    let offered = ["a".to_string(), "b".into(), "c".into()];
    let r = installed(&p, t0(), true, &offered);
    assert_eq!(
        row(&r),
        [
            (LOADS, "LOADS", "a, b".to_string()),
            (OFF_BELOW, "OFF BELOW", "25 %".into()),
            (ON_AT, "ON AGAIN AT", "60 %".into()),
            (PERIOD, "EVERY", duration(600.0)),
        ]
    );
    assert_eq!(r.settings[0].value, Value::Loads { chosen: vec!["a".into(), "b".into()], offered: offered.to_vec() });
    let woken = Installed { schedule: Schedule::At(t0()), ..p };
    assert_eq!(installed(&woken, t0(), true, &offered).settings.len(), 3, "a wake epoch has no period");
    let burns = burn_settings(&Default::default());
    assert_eq!(
        burns.iter().map(|p| (p.key, p.label, p.value.text())).collect::<Vec<_>>(),
        [
            (ALIGN, "ALIGNED WITHIN", "2.0°".to_string()),
            (ALIGN_RATE, "TURNING UNDER", "0.52°/s".into()),
            (SETTLE, "SETTLES FOR", duration(20.0)),
            (SETTLE_WAIT, "IGNITES ANYWAY AFTER", duration(60.0)),
            (STEER_HOLD, "HOLDS ITS AIM FOR THE LAST", duration(2.0)),
        ]
    );
}

/// One edit of a setting: its key and value.
type Change = (String, Input);
/// A program, its changes, and the settings sent or the refusal.
type Case = (&'static str, Vec<Change>, Result<Vec<Setting>, &'static str>);

/// The config menu's one rule (the tab's and MCP's): a program's settings
/// by their keys, checked as the computer will check them, refused with
/// why; the Falcon 9 with a load manager installed.
#[test]
fn one_rule_for_the_settings() {
    let mut sim = crate::state::SimState::with_craft(&crate::state::craft_or_fallback("falcon9-crew"));
    let guard = PowerGuard { loads: vec!["dragon.docking".into()], low: 0.3, high: 0.8, shed: false };
    let clock = sim.clock;
    sim.fleet[sim.active].install(Installed {
        name: "power".into(),
        slot: Slot::auto(),
        schedule: Schedule::Every { from: clock, period: 60.0 },
        last: None,
        program: Builtin::PowerGuard(guard),
    });
    let x = Inputs::of(sim.ship(), sim.clock).expect("a craft of parts");
    assert!(x.switchable.iter().any(|l| l == "dragon.docking"));
    let n = Input::Number;
    let list = |l: &[&str]| Input::List(l.iter().map(|s| s.to_string()).collect());
    let c = |k: &str, v: Input| (k.to_string(), v);
    // (program, changes, the settings sent or the refusal)
    #[rustfmt::skip]
    let table: Vec<Case> = vec![
        ("chutes", vec![c(ARM_HEIGHT, n(6_000.0)), c(MAIN_HEIGHT, n(2_000.0))], Ok(vec![Setting::ArmHeight(6_000.0), Setting::MainHeight(2_000.0)])),
        ("chutes", vec![c(MAIN_HEIGHT, n(-5.0))], Err("Parachutes: a height above the ground is 0 m or more, not -5")),
        ("chutes", vec![c(OFF_BELOW, n(0.3))], Err("Parachutes has no setting `off_below` (it has arm_height_m, main_height_m)")),
        ("chutes", vec![c(MAIN_HEIGHT, list(&[]))], Err("`main_height_m` is a number")),
        ("chutes", vec![], Err("Parachutes: no setting given (it has arm_height_m, main_height_m)")),
        ("maneuver", vec![c(PERIOD, n(1.0))], Err("Planned burns has no setting `period_s` (it has align_deg, align_rate_deg_s, settle_s, settle_wait_s, steer_hold_s)")),
        ("maneuver", vec![c(ALIGN, n(3.0)), c(SETTLE, n(10.0))], Ok(vec![Setting::Align(3f64.to_radians()), Setting::SettleLead(10.0)])),
        ("maneuver", vec![c(ALIGN, n(0.0))], Err("Planned burns: an alignment is more than 0° and at most 90°, not 0 rad")),
        ("maneuver", vec![c(STEER_HOLD, n(-1.0))], Err("Planned burns: a time is 0 s or more, not -1")),
        ("power", vec![c(OFF_BELOW, n(0.5)), c(ON_AT, n(0.9))], Ok(vec![Setting::Low(0.5), Setting::High(0.9)])),
        ("power", vec![c(OFF_BELOW, n(0.9))], Err("Load manager: the on line (80 %) is below the off line (90 %): it would switch at every run")),
        ("power", vec![c(PERIOD, n(600.0))], Ok(vec![Setting::Period(600.0)])),
        ("power", vec![c(LOADS, list(&["dragon.docking", "dragon.docking"]))], Err("Load manager: dragon.docking is listed twice (at 1)")),
        ("power", vec![c(LOADS, n(1.0))], Err("`loads` is a list of component ids")),
        ("warp", vec![c(PERIOD, n(1.0))], Err("no program `warp` aboard (it has maneuver, chutes, power)")),
    ];
    for (program, changes, want) in table {
        let got = settings_of(&x, program, &changes);
        assert_eq!(got, want.map(|s| (program.to_string(), s)).map_err(String::from), "{program} {changes:?}");
    }
    let bad = settings_of(&x, "power", &[c(LOADS, list(&["dragon.nowhere"]))]).unwrap_err();
    assert!(bad.starts_with("Load manager: dragon.nowhere is not a component that draws power"), "{bad}");
}
