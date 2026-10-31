//! `edit_program` edits the Falcon 9's program by the editors' rules: it
//! reads the list as the editor shows it, defines and pins a function,
//! moves and renames stages, and refuses what the editor would not allow
//! (the stages already run, another piece's entries, a pin on a stage, a
//! command the craft does not take).

use super::*;
use crate::comms::Location;
use crate::state::craft_or_fallback;

fn falcon9() -> (ShipDef, ShipState) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = (**sim::craft::load_definition(&path).unwrap().ship.as_ref().unwrap()).clone();
    let state = ShipState::initial(&def);
    (def, state)
}

fn names(list: &[Stage]) -> Vec<&str> {
    list.iter().map(|s| s.name.as_str()).collect()
}

#[test]
fn the_list_reads_as_the_editor_shows_it() {
    let (def, mut state) = falcon9();
    state.sequence = 1;
    let (list, page) = edited(&def, &state, false, &[]).unwrap();
    assert_eq!(list, state.stages, "no edits: the vessel's own list");
    let entries = page["entries"].as_array().unwrap();
    assert_eq!(entries.len(), program::view(&def, &state.stages).len());
    assert_eq!(entries[0]["run"], true, "the first stage has run");
    assert_eq!(entries[1]["run"], false);
    let first = &state.stages[0];
    assert_eq!(entries[0]["name"], first.name.as_str());
    assert_eq!(entries[0]["commands"][0]["label"], label(&first.actions[0]));
    let (_, page) = edited(&def, &state, true, &[]).unwrap();
    assert_eq!(page["list"], "functions");
    assert!(page["entries"].as_array().unwrap().iter().all(|e| e["pinned"].is_boolean()));
}

#[test]
fn a_function_is_defined_given_commands_and_pinned() {
    let (def, state) = falcon9();
    let n = state.functions.len();
    let edits = [
        json!({"add": "Chutes"}),
        json!({"add_command": {"entry": n, "command": "arm parachutes"}}),
        json!({"add_command": {"entry": n, "command": "RELEASE CLAMPS"}}),
        json!({"swap_commands": {"entry": n, "commands": [0, 1]}}),
        json!({"pin": {"entry": n, "on": true}}),
        json!({"rename": {"entry": n, "name": "Pad and chutes"}}),
    ];
    let (list, page) = edited(&def, &state, true, &edits).unwrap();
    let f = &list[n];
    assert_eq!((f.name.as_str(), f.pinned), ("Pad and chutes", true));
    assert_eq!(f.actions, [ShipCommand::ReleaseClamps, ShipCommand::ArmChutes]);
    let shown = page["entries"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(shown["entry"], n);
    assert_eq!(shown["commands"][1]["label"], "ARM PARACHUTES");
    // The stages: the second and third swap places, the first is run.
    let mut state = state;
    state.sequence = 1;
    let (list, _) = edited(&def, &state, false, &[json!({"swap": [1, 2]}), json!({"remove": 3})]).unwrap();
    let before = names(&state.stages);
    assert_eq!(names(&list)[..3], [before[0], before[2], before[1]]);
    assert_eq!(list.len(), state.stages.len() - 1);
}

#[test]
fn what_the_editor_would_not_allow_is_refused_with_why() {
    let (def, mut state) = falcon9();
    state.sequence = 2;
    // Another piece's stage: every command names what this piece lacks.
    let theirs = Stage {
        name: "theirs".into(),
        key: None,
        actions: vec![ShipCommand::Ignite("no-such-engine".into())],
        pinned: false,
        abort: false,
    };
    state.stages.push(theirs);
    let last = state.stages.len() - 1;
    // (list is functions, edits, the error's words)
    let table = [
        (false, json!([{"remove": 0}]), "edit 0: the 2 stages already run are locked"),
        (false, json!([{"swap": [1, 3]}]), "locked"),
        (false, json!([{"add_command": {"entry": 1, "command": "ARM PARACHUTES"}}]), "locked"),
        (false, json!([{"rename": {"entry": 99, "name": "x"}}]), "no entry 99"),
        (false, json!([{"rename": {"entry": last, "name": "mine"}}]), "another piece's"),
        (false, json!([{"pin": {"entry": 3, "on": true}}]), "only a function is pinned"),
        (false, json!([{"add_command": {"entry": 3, "command": "IGNITE warp-drive"}}]), "takes no `IGNITE warp-drive`"),
        (false, json!([{"remove_command": {"entry": 3, "command": 99}}]), "shows no command 99"),
        (true, json!([{"add": " "}]), "needs a name"),
        (true, json!([{"add": "A"}, {"remove": 99}]), "edit 1: no entry 99"),
        (true, json!([{"add": "A", "remove": 0}]), "one key"),
        (true, json!([{"fly": 1}]), "unknown edit `fly`"),
        (true, json!([{"swap": [0]}]), "two indices"),
    ];
    for (functions, edits, words) in table {
        let edits = edits.as_array().unwrap().clone();
        let err = edited(&def, &state, functions, &edits).unwrap_err();
        assert!(err.contains(words), "{edits:?}: {err}");
    }
}

#[test]
fn the_tool_reads_without_edits_and_sends_the_list_whole() {
    let sim = SimState::with_craft(&craft_or_fallback("falcon9-crew"));
    let mut comms = Comms::load();
    let id = sim.ship().id();
    comms.location = Location::Vessel(id);
    let (v, effects) = edit_program(&json!({"list": "functions"}), &sim, &comms).unwrap();
    assert!(effects.is_empty(), "a read sends nothing");
    let catalogue = v["catalogue"].as_array().unwrap();
    assert!(catalogue.iter().any(|c| c == "ARM PARACHUTES"), "{catalogue:?}");
    let args = json!({"list": "functions", "edits": [{"add": "Test"}]});
    let (v, effects) = edit_program(&args, &sim, &comms).unwrap();
    assert_eq!(v["arrives_in_s"], 0.0);
    let functions = sim.ship().ship().unwrap().functions.clone();
    let [Effect::Command(GameCommand::SetFunctions { vessel, functions: sent })] = &effects[..] else {
        panic!("{effects:?}")
    };
    assert_eq!((*vessel, sent.len(), sent.last().unwrap().name.as_str()), (id, functions.len() + 1, "Test"));
    assert!(edit_program(&json!({"list": "plans"}), &sim, &comms).unwrap_err().contains("`stages` or `functions`"));
}
