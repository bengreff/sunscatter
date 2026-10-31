//! `set_switch`, `press`, `configure_program` and `read {programs}` on the
//! Falcon 9: the programs as the PROGRAMS tab lists them with the tool of
//! each control, a switch, a press and the settings by the screen's rule,
//! and what is refused.

use super::*;
use crate::commands::GameCommand;
use crate::comms::Location;
use crate::state::craft_or_fallback;
use crate::systems_screen::programs::Row;
use sim::systems::ship_os::Switch;

fn aboard() -> (SimState, Comms) {
    let sim = SimState::with_craft(&craft_or_fallback("falcon9-crew"));
    let mut comms = Comms::load();
    comms.location = Location::Vessel(sim.ship().id());
    (sim, comms)
}

#[test]
fn a_switch_and_a_press_go_by_the_tabs_rule() {
    let (sim, comms) = aboard();
    let id = sim.ship().id();
    let (v, effects) = set_switch(&json!({"program": "chutes", "switch": "auto"}), &sim, &comms).unwrap();
    assert_eq!((v["was"].as_str(), v["arrives_in_s"].as_f64()), (Some("press"), Some(0.0)));
    let want = GameCommand::SetSwitch { vessel: id, program: "chutes".into(), switch: Switch::Auto };
    assert_eq!(effects, vec![Effect::Command(want)]);
    let (v, effects) = press(&json!({"program": "chutes"}), &sim, &comms).unwrap();
    assert_eq!(v["does"], "arm the drogues");
    assert_eq!(effects, vec![Effect::Command(GameCommand::Press { vessel: id, program: "chutes".into() })]);
    // (tool, arguments, the refusal says)
    let refused = [
        (
            "set_switch",
            json!({"program": "warp", "switch": "off"}),
            "no program `warp` aboard (it has maneuver, chutes)",
        ),
        ("set_switch", json!({"program": "chutes", "switch": "on"}), "unknown switch `on`"),
        ("press", json!({"program": "maneuver"}), "Planned burns: nothing to press (no burn planned)"),
        ("press", json!({"program": "chutes", "id": 999}), "no such vessel"),
    ];
    for (tool, args, says) in refused {
        let e = if tool == "press" { press(&args, &sim, &comms) } else { set_switch(&args, &sim, &comms) };
        assert!(e.as_ref().unwrap_err().contains(says), "{tool} {args}: {e:?}");
    }
}

/// The page is the tab's rows; each switch position and the press say the
/// call that works them; a refused press says why.
#[test]
fn the_programs_read_as_the_tab_lists_them() {
    let rows = [
        Row {
            name: "chutes".into(),
            title: "Parachutes".into(),
            does: "arms".into(),
            switch: Switch::Press,
            now: "stowed: arms on a press".into(),
            tone: crate::interface::menus::Tone::Normal,
            press: Ok("arm the drogues".into()),
            settings: vec![programs::Param {
                key: programs::MAIN_HEIGHT,
                label: "MAINS OPEN BELOW",
                value: programs::Value::Height(Some(1_800.0)),
            }],
        },
        Row {
            name: "maneuver".into(),
            title: "Planned burns".into(),
            does: "flies".into(),
            switch: Switch::Auto,
            now: "no burn planned".into(),
            tone: crate::interface::menus::Tone::Dim,
            press: Err("no burn planned".into()),
            settings: Vec::new(),
        },
    ];
    let v = programs_json(&rows);
    let chutes = &v["programs"][0];
    assert_eq!((chutes["program"].as_str(), chutes["switch"].as_str()), (Some("chutes"), Some("press")));
    assert_eq!(chutes["tone"], Value::Null, "a normal tone is left out");
    let switches = chutes["switches"].as_array().unwrap();
    let selected: Vec<&str> =
        switches.iter().filter(|s| s["selected"] == true).map(|s| s["label"].as_str().unwrap()).collect();
    assert_eq!(selected, ["ON PRESS"]);
    assert_eq!(switches[0]["by"], r#"set_switch {program: "chutes", switch: "auto"}"#);
    assert_eq!(chutes["press"], json!({"does": "arm the drogues", "by": "press {program: \"chutes\"}"}));
    assert_eq!(v["programs"][1]["press"], json!({"refused": "no burn planned"}));
    assert_eq!(
        chutes["settings"],
        json!([{"key": "main_height_m", "label": "MAINS OPEN BELOW", "value": 1800.0, "unit": "m above the ground",
                "text": crate::format::distance(1_800.0),
                "by": "configure_program {program: \"chutes\", settings: {main_height_m: <m above the ground>}}"}])
    );
    assert_eq!(v["programs"][1]["settings"], json!([]));
    assert_eq!(v["programs"][1]["tone"], "dim");
    // `read {programs: true}` on the Falcon 9 is the tab's rows.
    let (sim, _) = aboard();
    let got = super::super::read::read(&json!({"programs": true}), &sim, &Default::default()).unwrap();
    let x = crate::systems_screen::programs::Inputs::of(sim.ship(), sim.clock).unwrap();
    assert_eq!(got, programs_json(&crate::systems_screen::programs::rows(&x)));
    let e = super::super::read::read(&json!({"programs": true, "navball": true}), &sim, &Default::default());
    assert!(e.unwrap_err().contains("alone"));
}

/// `configure_program` by the tab's rule: the settings checked and sent
/// whole, with what they were; what the computer would refuse is refused
/// at once, with why.
#[test]
fn a_programs_settings_go_by_the_tabs_rule() {
    use sim::systems::ship_os::program::Setting;
    let (sim, comms) = aboard();
    let id = sim.ship().id();
    let args = json!({"program": "chutes", "settings": {"arm_height_m": 6000, "main_height_m": 2000.5}});
    let (v, effects) = configure(&args, &sim, &comms).unwrap();
    assert_eq!(v["was"], json!({"arm_height_m": 5500.0, "main_height_m": 1800.0}), "Dragon's own");
    assert_eq!((v["program"].as_str(), v["arrives_in_s"].as_f64()), (Some("chutes"), Some(0.0)));
    let settings = vec![Setting::ArmHeight(6000.0), Setting::MainHeight(2000.5)];
    assert_eq!(
        effects,
        vec![Effect::Command(GameCommand::Configure { vessel: id, program: "chutes".into(), settings })]
    );
    // (arguments, the refusal says)
    let refused = [
        (json!({"program": "chutes", "settings": {"main_height_m": -1}}), "a height above the ground is 0 m or more"),
        (json!({"program": "chutes", "settings": {"main_height_m": "high"}}), "`main_height_m` is a number"),
        (json!({"program": "chutes", "settings": {"loads": ["x"]}}), "Parachutes has no setting `loads`"),
        (json!({"program": "maneuver", "settings": {"period_s": 9}}), "Planned burns has no settings"),
        (json!({"program": "chutes"}), "`settings` maps"),
        (json!({"program": "warp", "settings": {}}), "no program `warp` aboard"),
    ];
    for (args, says) in refused {
        let e = configure(&args, &sim, &comms);
        assert!(e.as_ref().unwrap_err().contains(says), "{args}: {e:?}");
    }
}
