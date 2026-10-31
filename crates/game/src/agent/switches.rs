//! The `set_switch` and `press` tools (D126, D128 layer 3): the ship
//! computer's programs as the systems screen's PROGRAMS tab shows them
//! (`systems_screen::programs`), a switch or a press by the one rule the
//! screen and the menus use (`programs::{set_switch, press}`), sent like
//! any command (from anywhere but aboard after the light delay, D067).

use super::{reach, Effect};
use crate::comms::Comms;
use crate::state::SimState;
use crate::systems_screen::programs::{self, Row};
use mcp::Tool;
use serde_json::{json, Value};

/// The tools' definitions.
pub fn tools(id: &Value) -> [Tool; 2] {
    let names: Vec<&str> = programs::SWITCHES.iter().map(|s| s.2).collect();
    let program = json!({"type": "string", "description": "The program's name, as `read {programs: true}` lists \
        them: `maneuver` (the planned burns), `chutes` (the parachutes' deploy), or an installed program's"});
    let switch = Tool {
        name: "set_switch".into(),
        description: "A ship-computer program's switch (D126; the systems screen's PROGRAMS tab, the Maneuver \
            menu's PROGRAM, Landing ▸ PARACHUTE's DEPLOY) on a vessel (`id`, default the active one): `auto` (it \
            runs by itself), `press` (it runs when pressed for, once a press) or `off` (it does not run; the \
            controls stay the pilot's). Sent like any command (`arrives_in_s`). `read {programs: true}` lists \
            the programs with what each is doing."
            .into(),
        input_schema: json!({"type": "object", "properties": {"id": id, "program": program,
            "switch": {"type": "string", "enum": names}}, "required": ["program", "switch"]}),
    };
    let press = Tool {
        name: "press".into(),
        description: "Press for a ship-computer program (D126; the PROGRAMS tab's press) on a vessel (`id`, \
            default the active one): a planned burn switched to `press` flies when it wakes; the parachutes arm, \
            or, armed, the mains open once the drogues let them (the manual deploy, the P key's function); an \
            installed program runs once at its next epoch. Refused, with why, when a press does nothing now (as \
            the tab greys it out). Sent like any command (`arrives_in_s`)."
            .into(),
        input_schema: json!({"type": "object", "properties": {"id": id, "program": program},
            "required": ["program"]}),
    };
    [switch, press]
}

/// The programs of the vessel `args` names (default the active one) and
/// its id and the command's delay.
fn rows_of(args: &Value, sim: &SimState, comms: &Comms) -> Result<(sim::vessel::VesselId, f64, Vec<Row>), String> {
    let (id, arrives) = reach(args, sim, comms)?;
    let v = &sim.fleet[sim.index_of(id).ok_or("no such vessel")?];
    let x = programs::Inputs::of(v, sim.clock)
        .ok_or_else(|| format!("vessel {} is not a craft of parts (it has no ship computer)", id.0))?;
    Ok((id, arrives, programs::rows(&x)))
}

/// `set_switch`'s answer.
pub fn set_switch(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let (id, arrives, rows) = rows_of(args, sim, comms)?;
    let program = args.get("program").and_then(Value::as_str).ok_or("`program` names one")?;
    let name = args.get("switch").and_then(Value::as_str).ok_or("`switch` is auto, press or off")?;
    let switch = programs::by_name(name).ok_or_else(|| format!("unknown switch `{name}`: auto, press or off"))?;
    let command = programs::set_switch(&rows, id, program, switch)?;
    let was = programs::find(&rows, program).map(|r| programs::name(r.switch))?;
    let reply = json!({"id": id.0, "program": program, "switch": name, "was": was, "arrives_in_s": arrives});
    Ok((reply, vec![Effect::Command(command)]))
}

/// `press`'s answer.
pub fn press(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let (id, arrives, rows) = rows_of(args, sim, comms)?;
    let program = args.get("program").and_then(Value::as_str).ok_or("`program` names one")?;
    let command = programs::press(&rows, id, program)?;
    let does = programs::find(&rows, program)?.press.clone().unwrap_or_default();
    Ok((json!({"id": id.0, "program": program, "does": does, "arrives_in_s": arrives}), vec![Effect::Command(command)]))
}

/// The programs as the agent reads them (`read {programs: true}`): each
/// one's switch, what it does and is doing, its press or why not, and the
/// tool that works each control.
pub fn programs_json(rows: &[Row]) -> Value {
    let list: Vec<Value> = rows
        .iter()
        .map(|r| {
            let switches: Vec<Value> = programs::SWITCHES
                .iter()
                .map(|(s, label, name)| {
                    json!({"name": name, "label": label, "selected": *s == r.switch,
                           "by": format!("set_switch {{program: \"{}\", switch: \"{name}\"}}", r.name)})
                })
                .collect();
            let press = match &r.press {
                Ok(does) => json!({"does": does, "by": format!("press {{program: \"{}\"}}", r.name)}),
                Err(why) => json!({"refused": why}),
            };
            json!({"program": r.name, "title": r.title, "does": r.does, "switch": programs::name(r.switch),
                   "switches": switches, "now": r.now, "tone": super::read::tone_name(r.tone), "press": press})
        })
        .collect();
    json!({"programs": list})
}

#[cfg(test)]
#[path = "switches_tests.rs"]
mod tests;
