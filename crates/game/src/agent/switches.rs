//! The `set_switch`, `press` and `configure_program` tools (D126, D128
//! layer 3): the ship computer's programs as the systems screen's PROGRAMS
//! tab shows them (`systems_screen::programs`), a switch, a press or a
//! program's settings by the one rule the screen and the menus use
//! (`programs::{set_switch, press, configure}`), sent like any command
//! (from anywhere but aboard after the light delay, D067).

use super::{reach, Effect};
use crate::comms::Comms;
use crate::state::SimState;
use crate::systems_screen::programs::{self, Input, Row};
use mcp::Tool;
use serde_json::{json, Value};

/// The tools' definitions.
pub fn tools(id: &Value) -> [Tool; 3] {
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
    let configure = Tool {
        name: "configure_program".into(),
        description: "Set a ship-computer program's settings (D126: \"the autopilot is a switch with a config \
            menu\"; the PROGRAMS tab's fields) on a vessel (`id`, default the active one). `settings` maps each \
            setting's key, as `read {programs: true}` lists them under a program's `settings`, to its value: the \
            parachutes' `arm_height_m` (the drogues arm below it, m above the ground) and `main_height_m` (the \
            mains open below it); a load manager's `loads` (a list of component ids that draw power), `off_below` \
            and `on_at` (shares of the charge it reads, 0 to 1: its loads off below the one, on again at the \
            other) and `period_s`; the planned burns' `align_deg` and `align_rate_deg_s` (the turn counts as \
            done within that angle of the burn direction, turning slower than that), `settle_s` (how long they \
            settle without a level sensor, and the lead before ignition), `settle_wait_s` (how long past the \
            ignition epoch they wait for settled propellant) and `steer_hold_s` (the burn's last seconds that \
            hold the steering's direction). Checked as the computer will check them and applied all or none; refused with \
            why. Sent like any command (`arrives_in_s`)."
            .into(),
        input_schema: json!({"type": "object", "properties": {"id": id, "program": program,
            "settings": {"type": "object", "description": "Each setting's key to a number in its unit, or \
                `loads` to a list of ids"}}, "required": ["program", "settings"]}),
    };
    [switch, press, configure]
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

/// `configure_program`'s answer.
pub fn configure(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let (id, arrives) = reach(args, sim, comms)?;
    let v = &sim.fleet[sim.index_of(id).ok_or("no such vessel")?];
    let x = programs::Inputs::of(v, sim.clock)
        .ok_or_else(|| format!("vessel {} is not a craft of parts (it has no ship computer)", id.0))?;
    let program = args.get("program").and_then(Value::as_str).ok_or("`program` names one")?;
    let given = args.get("settings").and_then(Value::as_object).ok_or("`settings` maps each key to its value")?;
    let changes = (given.iter())
        .map(|(k, v)| {
            let input = match v {
                Value::Number(n) => n.as_f64().map(Input::Number),
                Value::Array(a) => {
                    a.iter().map(|s| s.as_str().map(String::from)).collect::<Option<_>>().map(Input::List)
                }
                _ => None,
            };
            input.map(|i| (k.clone(), i)).ok_or_else(|| format!("`{k}` is a number, or a list of ids for `loads`"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let command = programs::configure(&x, id, program, &changes)?;
    let rows = programs::rows(&x);
    let row = programs::find(&rows, program)?;
    let was: serde_json::Map<String, Value> = (row.settings.iter())
        .filter(|p| given.contains_key(p.key))
        .map(|p| (p.key.to_string(), value_json(&p.value).0))
        .collect();
    let reply = json!({"id": id.0, "program": row.name, "settings": given, "was": was, "arrives_in_s": arrives});
    Ok((reply, vec![Effect::Command(command)]))
}

/// A setting's value as the agent reads and sets it, and its unit.
fn value_json(v: &programs::Value) -> (Value, &'static str) {
    use programs::Value as V;
    match v {
        V::Height(h) => (json!(h), "m above the ground"),
        V::Share(x) => (json!(x), "share of the charge, 0 to 1"),
        V::Seconds(s) => (json!(s), "s"),
        V::Angle(a) => (json!(super::digits(a.to_degrees(), 3)), "°"),
        V::Rate(w) => (json!(super::digits(w.to_degrees(), 3)), "°/s"),
        V::Loads { chosen, .. } => (json!(chosen), "component ids"),
    }
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
            let settings: Vec<Value> = (r.settings.iter())
                .map(|p| {
                    let (value, unit) = value_json(&p.value);
                    let mut s = json!({"key": p.key, "label": p.label, "value": value, "unit": unit,
                        "text": p.value.text(), "by": format!("configure_program {{program: \"{}\", settings: {{{}: <{unit}>}}}}", r.name, p.key)});
                    if let programs::Value::Loads { offered, .. } = &p.value {
                        s["offered"] = json!(offered);
                    }
                    s
                })
                .collect();
            json!({"program": r.name, "title": r.title, "does": r.does, "switch": programs::name(r.switch),
                   "switches": switches, "now": r.now, "tone": super::read::tone_name(r.tone), "press": press,
                   "settings": settings})
        })
        .collect();
    json!({"programs": list})
}

#[cfg(test)]
#[path = "switches_tests.rs"]
mod tests;
