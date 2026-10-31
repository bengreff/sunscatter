//! The `read` tool reads each page from the model its screen draws: a
//! menu's sections from the dock's snapshot, a part's card, a layer's
//! summary and an item's inspector, the navball's rows; what the craft
//! does not measure carries the sensor that would (D106), and debug mode
//! shows the truth.

use super::*;
use crate::interface::menus::FlightData;
use crate::state::craft_or_fallback;

fn falcon9() -> SimState {
    SimState::with_craft(&craft_or_fallback("falcon9-crew"))
}

fn seen<'a>(data: &'a FlightData, nav: Option<&'a crate::navball::NavState>) -> Seen<'a> {
    Seen { flight: Some(data), nav, ..Seen::default() }
}

#[test]
fn a_menu_read_waits_for_the_stopped_clocks_snapshot() {
    let t = |s: f64| sim::sol::sol_epoch().add_seconds(s);
    // (time stopped, the snapshot's clock, the clock, answers now)
    let cases = [
        (false, Some(t(1.0)), t(2.0), true), // time runs: the page moves on
        (true, Some(t(1.0)), t(2.0), false), // stopped: from before the stop
        (true, Some(t(2.0)), t(2.0), true),  // the stopped clock's
        (true, None, t(2.0), false),         // not gathered yet
    ];
    for (stopped, gathered, clock, want) in cases {
        assert_eq!(fresh(stopped, gathered, clock), want, "{stopped} {gathered:?}");
    }
}

#[test]
fn a_menus_page_is_its_sections_with_the_tool_of_each_control() {
    let mut s = Section::summary();
    s.row("Ap", "200 km").toned("Pe", "—", Tone::Dim).note("no target", Tone::Normal).button(
        "The systems screen: THERMAL",
        MenuAction::OpenSystems(Some(crate::systems_screen::rules::Layer::Thermal), None),
    );
    s.toggle("S1.RCS", true, "on", Tone::Good, MenuAction::Ship(sim::craft::command::ShipCommand::ReleaseClamps));
    let v = menu_json(MenuId::Landing, &[s]);
    assert_eq!(v["menu"], "LANDING / REENTRY");
    let items = &v["sections"][0]["items"];
    assert_eq!(items[0], json!({"label": "Ap", "value": "200 km"}), "a normal tone is left out");
    assert_eq!(items[1], json!({"label": "Pe", "value": "—", "tone": "dim"}));
    assert_eq!(items[2], json!({"note": "no target"}));
    assert_eq!(items[3], json!({"button": "The systems screen: THERMAL", "by": "read {layer: thermal}"}));
    assert_eq!(
        items[4],
        json!({"switch": "S1.RCS", "on": true, "value": "on", "tone": "good", "by": "command {item, do}"})
    );
}

#[test]
fn a_menu_is_read_from_the_snapshot_the_dock_draws() {
    let sim = falcon9();
    let data = FlightData { body: "Earth".into(), ..Default::default() };
    for (name, menu) in [("orbital", MenuId::Orbital), ("Landing", MenuId::Landing), ("power", MenuId::Power)] {
        let got = read(&json!({"menu": name}), &sim, &seen(&data, None)).unwrap();
        assert_eq!(got, menu_json(menu, &menu.sections(&data)), "{name}");
    }
    // The RCS menu shows only while RCS is on; nothing in the tracking
    // station; one page a call.
    let refused = |args: Value, seen: Seen| read(&args, &sim, &seen).unwrap_err();
    assert!(refused(json!({"menu": "rcs"}), seen(&data, None)).contains("while RCS is on"));
    assert!(refused(json!({"menu": "orbital"}), Seen { station: true, ..seen(&data, None) }).contains("tracking"));
    assert!(refused(json!({"menu": "cockpit"}), seen(&data, None)).contains("unknown menu"));
    assert!(refused(json!({"menu": "orbital", "card": "s1.tank"}), seen(&data, None)).contains("one page a call"));
}

/// Every part's card of the active vessel, as `read` gives it.
fn cards(sim: &SimState) -> Vec<Value> {
    let data = FlightData::default();
    let def = sim.ship().ship_def().unwrap();
    (def.parts.iter()).map(|p| read(&json!({"card": p.id}), sim, &seen(&data, None)).unwrap()).collect()
}

/// The sensors the cards name for what they do not measure.
fn unmeasured(cards: &[Value]) -> Vec<String> {
    let lines =
        cards.iter().flat_map(|c| c["sections"].as_array().unwrap()).flat_map(|s| s["lines"].as_array().unwrap());
    lines.filter_map(|l| l["measured_by"].as_str().map(str::to_string)).collect()
}

#[test]
fn a_card_reads_as_the_card_draws_with_the_sensor_of_what_is_unmeasured() {
    let mut sim = falcon9();
    let data = FlightData::default();
    let (def, state) = (sim.ship().ship_def().unwrap(), sim.ship().ship().unwrap().clone());
    let gathered = part_card::Gathered::of(&sim);
    let tank = def.part_named("s1.tank").unwrap();
    let want = card_json("s1.tank", &content::card(&gathered.live(&def, &state), tank).unwrap());
    assert_eq!(read(&json!({"card": "s1.tank"}), &sim, &seen(&data, None)).unwrap(), want);
    assert_eq!(want["card"], "s1.tank");
    // The pad's clamps cannot be released again: the card says so.
    let commands = cards(&sim).into_iter().flat_map(|c| c["commands"].as_array().unwrap().clone());
    assert!(commands.into_iter().any(|c| c["cannot_be_undone"] == true), "the clamps' release is confirmed");
    // What the craft does not measure names its sensor; debug mode knows
    // everything (D106).
    assert!(!unmeasured(&cards(&sim)).is_empty(), "Dragon's tanks are not measured on the pad");
    let s = &mut sim;
    s.fleet[s.active].set_debug(&s.world, true);
    assert_eq!(unmeasured(&cards(&sim)), Vec::<String>::new());
    let err = read(&json!({"card": "s9.nothing"}), &sim, &seen(&data, None)).unwrap_err();
    assert!(err.contains("no part `s9.nothing`"), "{err}");
}

#[test]
fn a_layer_and_an_item_read_as_the_systems_screen_shows_them() {
    let sim = falcon9();
    let data = FlightData::default();
    let v = sim.ship();
    let (def, state) = (v.ship_def().unwrap(), v.ship().unwrap());
    let facts = crate::systems_screen::view::facts_of(v).unwrap();
    let known = crate::format::Known::of(v);
    for (name, layer) in LAYER_NAMES {
        let got = read(&json!({"layer": name}), &sim, &seen(&data, None)).unwrap();
        let want = crate::systems_screen::summary::with_inputs(&sim, (&def, state), &facts, &known, |x| {
            crate::systems_screen::summary::summary(layer, x)
        });
        assert_eq!(got["layer"]["groups"], groups_json(&def, &want)["groups"], "{name}");
        assert_eq!(got["layer"]["name"], layer.name());
        assert!(got.get("item").is_none());
    }
    // A part's inspector, as the right panel: its groups and commands.
    let got = read(&json!({"item": "s1.tank"}), &sim, &seen(&data, None)).unwrap();
    let sel = inspect::named(&def, "s1.tank").unwrap();
    assert_eq!(got["item"]["groups"], groups_json(&def, &inspect::details(&def, state, &facts, sel, &known))["groups"]);
    assert_eq!(got["item"]["name"], "s1.tank");
    // Both at once; a row about a part names it, for `read {item}`.
    let got = read(&json!({"layer": "thermal", "item": "s1.tank"}), &sim, &seen(&data, None)).unwrap();
    assert!(got["layer"].is_object() && got["item"].is_object());
    let rows = got["layer"]["groups"].as_array().unwrap().iter().flat_map(|g| g["rows"].as_array().unwrap().clone());
    let about: Vec<Value> = rows.filter_map(|r| r.get("about").cloned()).collect();
    assert!(about
        .iter()
        .all(|a| a.as_str().is_some_and(|n| def.part_named(n).is_some() || inspect::named(&def, n).is_some())));
    let err = read(&json!({"layer": "x-ray"}), &sim, &seen(&data, None)).unwrap_err();
    assert!(err.contains("unknown layer"), "{err}");
}

#[test]
fn the_navball_reads_its_rows_as_the_cluster_draws_them() {
    let mut sim = falcon9();
    let data = FlightData::default();
    let state = |sim: &SimState| {
        let mut nav = crate::navball::Navball::default();
        crate::navball::nav_state(sim, &Default::default(), &Default::default(), &mut nav).unwrap()
    };
    let now = state(&sim);
    let got = read(&json!({"navball": true}), &sim, &seen(&data, Some(&now))).unwrap();
    let want =
        navball_json(&nav::readouts(&now), &crate::navball::control_rows(&sim, &now), crate::hud::state_line(&sim));
    assert_eq!(got, want);
    // Altitude, G, Ap and its time, Pe and its time; SAS, RCS, six holds.
    assert_eq!(got["readouts"].as_array().unwrap().len(), 6, "{got}");
    assert_eq!(got["readouts"][1]["label"], "G");
    assert_eq!(got["controls"]["holds"].as_array().unwrap().len(), 6);
    assert_eq!(got["controls"]["rcs"]["caption"], "RCS OFF");
    // In debug mode the speed is known.
    let s = &mut sim;
    s.fleet[s.active].set_debug(&s.world, true);
    let now = state(&sim);
    let got = read(&json!({"navball": true}), &sim, &seen(&data, Some(&now))).unwrap();
    assert!(got["speed"].as_str().is_some_and(|t| t != crate::format::DASH), "{got}");
    assert!(read(&json!({"navball": true}), &sim, &seen(&data, None)).is_err(), "nothing drawn yet");
}
