//! The `read` tool (D044, D106): what the player reads on a page, as data,
//! from the same models the screens draw (one model, two views; never a
//! second rule, so it cannot drift and shows no more than the player
//! sees): a task menu (`MenuId::sections` of the menus' snapshot), a
//! part's card (`part_card::content::card`, its inputs by
//! `part_card::Gathered`), a systems-screen layer's summary
//! (`systems_screen::summary::summary`, its inputs by `with_inputs` and
//! `view::facts_of`), an item's inspector (`inspect::details`, its
//! commands `inspect::commands`), and the navball cluster
//! (`navball::rules::readouts`, `navball::control_rows`).
//!
//! Each control on a page says which tool works it (`by`): the agent
//! clicks through its own tools, sent like any command.

use super::session::{by_name, names, LAYER_NAMES, MENU_NAMES};
use super::Seen;
use crate::format::Level;
use crate::interface::menus::{Item, MenuAction, MenuId, Section, Tone};
use crate::interface::part_card::{self, content};
use crate::navball::rules as nav;
use crate::state::SimState;
use crate::systems_screen::inspect::{self, Group, Selection};
use mcp::Tool;
use serde_json::{json, Map, Value};
use sim::craft::ship::ShipDef;

/// The tool's definition.
pub fn tool() -> Tool {
    let description = "What the player reads on a page of the active vessel's flight view, as data, from the same \
        model the page draws (D106: dashes where the craft measures nothing; debug mode shows the truth). One of: \
        `menu` (a task menu by name: its sections, rows, notes, choices, buttons and switches, each control with \
        the tool that works it, `by`; the RCS menu while RCS is on), `card` (a part's card by its id: its \
        sections, lines with the sensor that would measure an unmeasured one, and commands for `command`), \
        `layer` (a systems-screen layer's summary of the craft as a whole) and/or `item` (a part's or component's \
        inspector, as the screen's right panel shows it, with its commands), or `navball: true` (the cluster: \
        speed, altitude, G, Ap and Pe, SAS, RCS, the hold buttons, the throttle and the flight state). Not in the \
        tracking station, whose flight view is not drawn.";
    let properties = json!({
        "menu": {"type": "string", "enum": names(&MENU_NAMES)},
        "card": {"type": "string", "description": "A part's id, as get_vessel and the systems screen name it"},
        "layer": {"type": "string", "enum": names(&LAYER_NAMES)},
        "item": {"type": "string", "description": "A part's or component's id"},
        "navball": {"type": "boolean"}
    });
    Tool {
        name: "read".into(),
        description: description.into(),
        input_schema: json!({"type": "object", "properties": properties, "required": []}),
    }
}

/// Whether a `read {menu}` answers now from the menus' snapshot gathered
/// at `gathered`: while time runs at once (the page moves on as the
/// player's does), while it is stopped once the snapshot is the stopped
/// clock's, so a read after a `step` is the same on every replay (the
/// snapshot is refreshed every 0.1 s of real time; the wait is bounded by
/// the caller).
pub fn fresh(stopped: bool, gathered: Option<sim::time::Epoch>, clock: sim::time::Epoch) -> bool {
    !stopped || gathered == Some(clock)
}

/// The tool's answer.
pub fn read(args: &Value, sim: &SimState, seen: &Seen) -> Result<Value, String> {
    if seen.station {
        return Err("the pages are the flight view's: the tracking station is open (switch_vessel to fly one)".into());
    }
    let s = |k: &str| args.get(k).and_then(Value::as_str);
    let navball = args.get("navball").and_then(Value::as_bool) == Some(true);
    match (s("menu"), s("card"), s("layer").is_some() || s("item").is_some(), navball) {
        (Some(name), None, false, false) => {
            let menu = by_name(&MENU_NAMES, name, "menu")?;
            if menu == MenuId::Rcs && !sim.controls().rcs.on {
                return Err("the RCS menu shows while RCS is on: `set_controls {rcs: \"on\"}` first".into());
            }
            let data = seen.flight.ok_or("the menus have not been gathered yet")?;
            Ok(menu_json(menu, &menu.sections(data)))
        }
        (None, Some(part), false, false) => {
            let v = sim.ship();
            let (def, state) = v.ship_def().zip(v.ship()).ok_or("the active vessel is not a craft of parts")?;
            let id = def.part_named(part).ok_or_else(|| format!("the active vessel has no part `{part}`"))?;
            let gathered = part_card::Gathered::of(sim);
            let card = content::card(&gathered.live(&def, state), id).ok_or("no card for that part")?;
            Ok(card_json(part, &card))
        }
        (None, None, true, false) => systems(s("layer"), s("item"), sim),
        (None, None, false, true) => {
            let now = seen.nav.ok_or("the navball has nothing to show yet")?;
            let line = crate::hud::state_line(sim);
            Ok(navball_json(&nav::readouts(now), &crate::navball::control_rows(sim, now), line))
        }
        _ => Err("one page a call: `menu`, `card`, `layer` and/or `item`, or `navball: true`".into()),
    }
}

/// The systems screen's `layer` summary and/or `item`'s inspector.
fn systems(layer: Option<&str>, item: Option<&str>, sim: &SimState) -> Result<Value, String> {
    let v = sim.ship();
    let (def, state) = v.ship_def().zip(v.ship()).ok_or("the active vessel is not a craft of parts")?;
    let facts = crate::systems_screen::view::facts_of(v).unwrap_or_default();
    let known = crate::format::Known::of(v);
    let mut out = Map::new();
    if let Some(name) = layer {
        let layer = by_name(&LAYER_NAMES, name, "layer")?;
        let groups = crate::systems_screen::summary::with_inputs(sim, (&def, state), &facts, &known, |x| {
            crate::systems_screen::summary::summary(layer, x)
        });
        let mut page = groups_json(&def, &groups);
        page.insert("name".into(), json!(layer.name()));
        page.insert("note".into(), json!(layer.note()));
        out.insert("layer".into(), Value::Object(page));
    }
    if let Some(name) = item {
        let sel =
            inspect::named(&def, name).ok_or_else(|| format!("the active vessel has no part or component `{name}`"))?;
        let commands: Vec<Value> = inspect::commands(&def, state, sel)
            .into_iter()
            .map(|(label, _)| json!({"label": label, "by": "command {item, do}"}))
            .collect();
        let mut page = groups_json(&def, &inspect::details(&def, state, &facts, sel, &known));
        page.insert("name".into(), json!(selection_name(&def, sel)));
        page.insert("commands".into(), json!(commands));
        out.insert("item".into(), Value::Object(page));
    }
    Ok(Value::Object(out))
}

fn tone_name(t: Tone) -> Option<&'static str> {
    match t {
        Tone::Normal => None,
        Tone::Dim => Some("dim"),
        Tone::Good => Some("good"),
        Tone::Warn => Some("warn"),
        Tone::Alert => Some("alert"),
    }
}

fn level_name(l: Level) -> Option<&'static str> {
    match l {
        Level::Normal => None,
        Level::Warn => Some("near its limit"),
        Level::Over => Some("at or over its limit"),
    }
}

/// `v` with `key` set to `value` when there is one (a normal tone or level
/// is left out).
fn with(mut v: Value, key: &str, value: Option<&str>) -> Value {
    if let (Some(x), Value::Object(m)) = (value, &mut v) {
        m.insert(key.into(), json!(x));
    }
    v
}

/// The tool that works a menu's control.
fn by(action: &MenuAction) -> String {
    match action {
        MenuAction::SetTarget(_) => "set_target".into(),
        MenuAction::OpenPlanner => "set_plan".into(),
        MenuAction::OpenSystems(Some(layer), _) => format!("read {{layer: {}}}", layer.name().to_lowercase()),
        MenuAction::OpenSystems(None, _) => "read {layer}".into(),
        MenuAction::Ship(_) => "command {item, do}".into(),
        MenuAction::SetRcs(_) => "set_controls {rcs, rcs_fine}".into(),
    }
}

/// One line of a menu's section.
fn item_json(i: &Item) -> Value {
    match i {
        Item::Row(r) => with(json!({"label": r.label, "value": r.value}), "tone", tone_name(r.tone)),
        Item::Note(text, tone) => with(json!({"note": text}), "tone", tone_name(*tone)),
        Item::Choice { label, current, options } => {
            let options: Vec<Value> =
                options.iter().map(|(name, selected, _)| json!({"name": name, "selected": selected})).collect();
            let by = options_by(i);
            json!({"choice": label, "current": current, "options": options, "by": by})
        }
        Item::Button { label, action } => json!({"button": label, "by": by(action)}),
        Item::Toggle { label, on, value, tone, action } => {
            with(json!({"switch": label, "on": on, "value": value, "by": by(action)}), "tone", tone_name(*tone))
        }
    }
}

/// The tool a choice's options work (its first option's).
fn options_by(i: &Item) -> Option<String> {
    match i {
        Item::Choice { options, .. } => options.first().map(|(_, _, a)| by(a)),
        _ => None,
    }
}

/// A menu as the agent reads it: its title and its sections (the summary
/// first, untitled), as `MenuId::sections` gives them.
pub fn menu_json(menu: MenuId, sections: &[Section]) -> Value {
    let sections: Vec<Value> = sections
        .iter()
        .map(|s| json!({"id": s.id, "title": s.title, "items": s.items.iter().map(item_json).collect::<Vec<_>>()}))
        .collect();
    json!({"menu": menu.title(), "sections": sections})
}

/// A part's card as the agent reads it: an unmeasured line's text (a dash,
/// a store's "—/793 kg") with the sensor that would measure it (the card's
/// hover), and its commands for `command {item, do}`.
pub fn card_json(part: &str, card: &content::Card) -> Value {
    let sections: Vec<Value> = card
        .sections
        .iter()
        .map(|s| {
            let lines: Vec<Value> = s
                .lines
                .iter()
                .map(|l| {
                    let v = match &l.value {
                        content::Value::Shown(text) => json!({"label": l.label, "value": text}),
                        content::Value::Unknown { text, sensor } => {
                            json!({"label": l.label, "value": text, "measured_by": sensor})
                        }
                    };
                    with(v, "level", level_name(l.level))
                })
                .collect();
            json!({"heading": s.heading, "lines": lines})
        })
        .collect();
    let commands: Vec<Value> = card
        .commands
        .iter()
        .map(
            |c| json!({"label": c.label, "cannot_be_undone": c.confirm, "by": format!("command {{item: {part}, do}}")}),
        )
        .collect();
    json!({"card": card.title, "entry": card.entry, "sections": sections, "commands": commands})
}

/// A part's or component's id, as the screen names it.
fn selection_name(def: &ShipDef, sel: Selection) -> String {
    match sel {
        Selection::Part(id) => def.parts.iter().find(|p| p.part == id).map_or_else(|| id.to_string(), |p| p.id.clone()),
        Selection::Component(i) => def.components.get(i).map_or_else(|| i.to_string(), |c| c.id.clone()),
    }
}

/// The systems screen's groups as the agent reads them: each row's level
/// against its limit, the item it is about (`about`, for `read {item}`),
/// and the terms' explanations (the labels' hovers) once at the end.
pub fn groups_json(def: &ShipDef, groups: &[Group]) -> Map<String, Value> {
    let mut terms = Map::new();
    let groups: Vec<Value> = groups
        .iter()
        .map(|g| {
            let rows: Vec<Value> = g
                .rows
                .iter()
                .map(|r| {
                    if let Some(h) = &r.hover {
                        terms.insert(r.label.clone(), json!(h));
                    }
                    let about = r.open.map(|s| selection_name(def, s));
                    with(
                        with(json!({"label": r.label, "value": r.value}), "level", level_name(r.level)),
                        "about",
                        about.as_deref(),
                    )
                })
                .collect();
            json!({"title": g.title, "rows": rows})
        })
        .collect();
    Map::from_iter([("groups".to_string(), json!(groups)), ("terms".to_string(), Value::Object(terms))])
}

/// A cluster button as the agent reads it.
fn button_json(b: &nav::Button) -> Value {
    json!({"caption": b.caption, "lit": b.lit, "enabled": b.enabled, "hover": b.hover})
}

/// The navball cluster as the agent reads it: the readouts and the
/// controls as drawn, and the flight state's line under the ball.
pub fn navball_json(r: &nav::Readouts, c: &nav::ControlRows, (line, alert): (String, bool)) -> Value {
    let readout = |x: &nav::Readout| {
        with(
            with(json!({"label": x.label.trim(), "text": x.text}), "level", level_name(x.level)),
            "hover",
            x.hover.as_deref(),
        )
    };
    let readouts: Vec<Value> = std::iter::once(&r.altitude).chain([&r.g]).chain(&r.apsides).map(readout).collect();
    let holds: Vec<Value> = c.holds.iter().map(|(_, b)| button_json(b)).collect();
    json!({
        "speed": r.speed, "caption": r.caption, "readouts": readouts,
        "controls": {"sas": button_json(&c.sas), "rcs": button_json(&c.rcs), "holds": holds, "throttle": c.throttle,
                     "by": "set_controls"},
        "state": line, "state_alert": alert,
    })
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
