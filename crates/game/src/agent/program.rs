//! The `edit_program` tool (D107, D111, D114): the systems screen's stage
//! and function editors, for the agent. A call reads one list (the stage
//! sequence or the saved functions) as the editor shows it, the piece's
//! view (`sim::craft::program::view`), and applies its edits by the
//! editors' own rules (`systems_screen::edit::{allowed, apply}`; a command
//! by its label from `edit::catalogue`, the pick list), then sends the list
//! whole as the editor's "save to the vessel" does
//! (`GameCommand::SetStages`, `SetFunctions`). The sim checks every command
//! and refuses the whole list with its reasons, in a notification.

use super::{reach, Effect};
use crate::commands::GameCommand;
use crate::comms::Comms;
use crate::staging::parts::label;
use crate::state::SimState;
use crate::systems_screen::edit::{self, Edit};
use mcp::Tool;
use serde_json::{json, Value};
use sim::craft::command::{ShipCommand, Stage};
use sim::craft::program;
use sim::craft::ship::ShipDef;
use sim::craft::state::ShipState;

/// The tool's definition.
pub fn tool(id: &Value) -> Tool {
    let description = "A craft of parts' program as the systems screen's editors edit it (`id`, default the active \
        vessel): `list` is `stages` (the sequence; the stages already run are locked, D114) or `functions` (the \
        action bar shows the pinned ones). Without `edits` it reads the list as the editor shows it (this piece's \
        entries, each with its `entry` index and its commands' indices) and the `catalogue` of every command the \
        craft takes. `edits` apply in order, by entry index: {add: name}, {remove: entry}, {swap: [entry, entry]} \
        (moves one past the other), {rename: {entry, name}}, {add_command: {entry, command: label from the \
        catalogue}}, {remove_command: {entry, command: index}}, {swap_commands: {entry, commands: [index, \
        index]}}, {pin: {entry, on}} (functions only). Then the whole list is sent like any command \
        (`arrives_in_s`), as the editor's \"save to the vessel\"; the vessel refuses a list it cannot run whole, \
        with why, in the notifications. Any edit the editor would not allow refuses the call.";
    let edit = json!({"type": "object", "description": "One edit: exactly one of add, remove, swap, rename, \
        add_command, remove_command, swap_commands, pin"});
    let properties = json!({"id": id, "list": {"type": "string", "enum": ["stages", "functions"]},
                            "edits": {"type": "array", "items": edit}});
    Tool {
        name: "edit_program".into(),
        description: description.into(),
        input_schema: json!({"type": "object", "properties": properties, "required": ["list"]}),
    }
}

/// The tool's answer.
pub fn edit_program(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let (id, arrives) = reach(args, sim, comms)?;
    let v = &sim.fleet[sim.index_of(id).ok_or("no such vessel")?];
    let (def, state) =
        v.ship_def().zip(v.ship()).ok_or_else(|| format!("vessel {} is not a craft of parts (no program)", id.0))?;
    let functions = match args.get("list").and_then(Value::as_str) {
        Some("stages") => false,
        Some("functions") => true,
        _ => return Err("`list` is `stages` or `functions`".into()),
    };
    let edits = match args.get("edits") {
        None => Vec::new(),
        Some(Value::Array(list)) => list.clone(),
        Some(_) => return Err("`edits` is a list of edits".into()),
    };
    let (list, mut page) = edited(&def, state, functions, &edits)?;
    let page = page.as_object_mut().expect("an object");
    page.insert("id".into(), json!(id.0));
    if edits.is_empty() {
        let catalogue: Vec<String> = edit::catalogue(&def).iter().map(label).collect();
        page.insert("catalogue".into(), json!(catalogue));
        return Ok((Value::Object(page.clone()), Vec::new()));
    }
    page.insert("arrives_in_s".into(), json!(arrives));
    let command = if functions {
        GameCommand::SetFunctions { vessel: id, functions: list }
    } else {
        GameCommand::SetStages { vessel: id, stages: list }
    };
    Ok((Value::Object(page.clone()), vec![Effect::Command(command)]))
}

/// The list (`functions`, else the sequence) of the piece `def` in `state`
/// after `edits`, each checked as the editor allows it, and the page that
/// shows it.
pub fn edited(
    def: &ShipDef,
    state: &ShipState,
    functions: bool,
    edits: &[Value],
) -> Result<(Vec<Stage>, Value), String> {
    let mut list = if functions { state.functions.clone() } else { state.stages.clone() };
    let locked = if functions { 0 } else { state.sequence as usize };
    let catalogue = edit::catalogue(def);
    for (k, e) in edits.iter().enumerate() {
        let edit = parse(e, &catalogue).map_err(|why| format!("edit {k}: {why}"))?;
        check(def, &list, &edit, locked, functions).map_err(|why| format!("edit {k}: {why}"))?;
        edit::apply(&mut list, edit);
    }
    let page = page(def, state, &list, locked, functions);
    Ok((list, page))
}

/// One edit from its JSON.
fn parse(e: &Value, catalogue: &[ShipCommand]) -> Result<Edit, String> {
    let obj = e.as_object().filter(|o| o.len() == 1).ok_or("each edit is an object with one key")?;
    let (key, v) = obj.iter().next().expect("one key");
    let n = |v: &Value, k: &str| -> Result<usize, String> {
        v.get(k).and_then(Value::as_u64).map(|n| n as usize).ok_or_else(|| format!("`{key}` needs integer `{k}`"))
    };
    let pair = |v: &Value| -> Result<(usize, usize), String> {
        match v.as_array().map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>()).as_deref() {
            Some([a, b]) => Ok((*a as usize, *b as usize)),
            _ => Err(format!("`{key}` needs two indices")),
        }
    };
    let text = |v: Option<&Value>, k: &str| -> Result<String, String> {
        v.and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("`{key}` needs text `{k}`"))
    };
    Ok(match key.as_str() {
        "add" => Edit::Add(text(Some(v), "name")?),
        "remove" => Edit::Remove(v.as_u64().ok_or("`remove` is an entry index")? as usize),
        "swap" => {
            let (a, b) = pair(v)?;
            Edit::Swap(a, b)
        }
        "rename" => Edit::Rename(n(v, "entry")?, text(v.get("name"), "name")?),
        "add_command" => {
            let wanted = text(v.get("command"), "command")?;
            let c = (catalogue.iter())
                .find(|c| label(c).eq_ignore_ascii_case(&wanted))
                .ok_or_else(|| format!("the craft takes no `{wanted}` (edit_program without edits lists them)"))?;
            Edit::AddAction(n(v, "entry")?, c.clone())
        }
        "remove_command" => Edit::RemoveAction(n(v, "entry")?, n(v, "command")?),
        "swap_commands" => {
            let (a, b) = pair(v.get("commands").ok_or("`swap_commands` needs `commands`")?)?;
            Edit::SwapActions(n(v, "entry")?, a, b)
        }
        "pin" => Edit::Pin(n(v, "entry")?, v.get("on").and_then(Value::as_bool).ok_or("`pin` needs `on`")?),
        other => return Err(format!("unknown edit `{other}`")),
    })
}

/// Whether the editor allows `edit` on `list` (the piece `def`'s view of
/// it, the first `locked` entries run): the locked stages take none
/// (`edit::allowed`), an entry or command must be one the editor shows,
/// and only a function is pinned.
pub fn check(def: &ShipDef, list: &[Stage], edit: &Edit, locked: usize, functions: bool) -> Result<(), String> {
    if !edit::allowed(edit, locked) {
        return Err(format!("the {locked} stages already run are locked (D114): their place and commands stay"));
    }
    let view = program::view(def, list);
    let shown = |i: usize| -> Result<&program::Shown, String> {
        if i >= list.len() {
            return Err(format!("no entry {i} (the list has {})", list.len()));
        }
        view.iter().find(|s| s.index == i).ok_or_else(|| format!("entry {i} is another piece's (not in this view)"))
    };
    let command = |i: usize, j: usize| -> Result<(), String> {
        let s = shown(i)?;
        s.actions.contains(&j).then_some(()).ok_or_else(|| format!("entry {i} shows no command {j}"))
    };
    match edit {
        Edit::Add(name) if name.trim().is_empty() => Err("a new entry needs a name".into()),
        Edit::Add(_) => Ok(()),
        Edit::Remove(i) | Edit::AddAction(i, _) => shown(*i).map(|_| ()),
        Edit::Rename(i, name) if name.trim().is_empty() => Err(format!("entry {i} needs a name")),
        Edit::Rename(i, _) => shown(*i).map(|_| ()),
        Edit::Swap(i, k) => shown(*i).and(shown(*k)).map(|_| ()),
        Edit::RemoveAction(i, j) => command(*i, *j),
        Edit::SwapActions(i, j, k) => command(*i, *j).and(command(*i, *k)),
        Edit::Pin(..) if !functions => Err("only a function is pinned (to the action bar)".into()),
        Edit::Pin(i, _) => shown(*i).map(|_| ()),
    }
}

/// The list as the editor shows it: this piece's entries in order, each
/// with its index, name, its commands shown (their indices and labels),
/// whether it has run (a stage) or is pinned (a function), an abort
/// with whether the piece offers it now.
fn page(def: &ShipDef, state: &ShipState, list: &[Stage], locked: usize, functions: bool) -> Value {
    let entries: Vec<Value> = program::view(def, list)
        .iter()
        .map(|s| {
            let e = &list[s.index];
            let commands: Vec<Value> =
                s.actions.iter().map(|&j| json!({"command": j, "label": label(&e.actions[j])})).collect();
            let mut v = json!({"entry": s.index, "name": e.name, "commands": commands});
            let m = v.as_object_mut().expect("an object");
            if functions {
                m.insert("pinned".into(), json!(e.pinned));
            } else {
                m.insert("run".into(), json!(s.index < locked));
            }
            if e.abort {
                let offered = program::offered(def, state, e);
                m.insert("abort".into(), json!(offered.err().map_or("offered now", |w| w.describe())));
            }
            v
        })
        .collect();
    json!({"list": if functions { "functions" } else { "stages" }, "entries": entries})
}

#[cfg(test)]
#[path = "program_tests.rs"]
mod tests;
