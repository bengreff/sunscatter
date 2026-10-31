//! The player's AI agent (D044, D069): the MCP server's tools, answered from
//! the game each frame.
//!
//! The agent is at a control location like the player (D067): it sees
//! vessels as their light arrives there, and it cannot read a vessel with no
//! signal. Actions go through the same `GameCommand`s as the keyboard and
//! the UI; nothing is special-cased for the agent. Vessel-directed commands
//! (burn plans, controls, stages) travel with the path's light delay; a
//! script flies any vessel it has a signal to, active or not (D112).
//!
//! It is the only way an agent reaches the running game, the player's and
//! a developer's alike (D044, D122): the session's tools ([`session`]: shots,
//! the camera, time, flights, saves) are every agent's.

pub mod session;

use crate::commands::GameCommand;
use crate::comms::{self, Comms, Location};
use crate::interface::layout::InterfaceSettings;
use crate::state::{SimState, WARP_LEVELS};
use crate::tracking::TrackingStation;
use bevy::prelude::*;
use glam::DVec3;
use mcp::{Server, Tool, ToolResult};
use serde_json::{json, Value};
use sim::craft::command::ShipCommand;
use sim::craft::ship::ShipDef;
use sim::craft::state::ShipState;
use sim::systems::rcs::{RcsMode, RcsOnly};
use sim::systems::Quantity;
use sim::vessel::{HoldMode, Phase, SpeedReference, VesselId};

/// What answering a call does besides answering.
#[derive(Debug, PartialEq)]
pub enum Effect {
    Command(GameCommand),
    /// Go to mission control (true) or aboard the active vessel (false).
    Station(bool),
    /// A command sent over the network: it arrives after `delay` seconds.
    Send {
        delay: f64,
        command: GameCommand,
    },
    /// The rotation input held on the active vessel ([`HeldRotation`]).
    Rotate(VesselId, DVec3),
}

/// The rotation input an agent aboard holds on the active vessel, like the
/// pilot's keys (`commands::keys::read_controls` adds it): body axes, each
/// −1–1 (x pitch, y yaw, z roll), held until the agent changes it.
#[derive(Resource, Default)]
pub struct HeldRotation(pub Option<(VesselId, DVec3)>);

impl HeldRotation {
    /// The input held on vessel `id`.
    pub fn of(&self, id: VesselId) -> DVec3 {
        self.0.filter(|(v, _)| *v == id).map_or(DVec3::ZERO, |(_, r)| r)
    }
}

/// The speed references by name (the `set_controls` tool).
const SPEED_NAMES: [(&str, SpeedReference); 3] =
    [("orbit", SpeedReference::Orbit), ("surface", SpeedReference::Surface), ("target", SpeedReference::Target)];

/// The hold modes by name (the `set_controls` tool).
const HOLD_NAMES: [(&str, HoldMode); 6] = [
    ("stability", HoldMode::Stability),
    ("prograde", HoldMode::Prograde),
    ("retrograde", HoldMode::Retrograde),
    ("target", HoldMode::Target),
    ("anti_target", HoldMode::AntiTarget),
    ("maneuver", HoldMode::Maneuver),
];

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required})
}

/// The tools offered to the agent: the flight's here, the session's
/// ([`session::tools`]), the developer's with `dev`.
pub fn tools(dev: bool) -> Vec<Tool> {
    let mut list = flight_tools();
    list.extend(session::tools(dev));
    list
}

fn flight_tools() -> Vec<Tool> {
    let t = |name: &str, description: &str, input_schema: Value| Tool {
        name: name.into(),
        description: description.into(),
        input_schema,
    };
    let id = json!({"type": "integer", "description": "Vessel id"});
    vec![
        t(
            "get_state",
            "Game clock (TDB, the physics' time), the origin's clock (TT, the time the player sees) and the active \
             ship clock's offset from it, time warp, your control location, the active vessel, and every vessel's \
             status and signal at your location.",
            schema(json!({}), &[]),
        ),
        t("list_bodies", "Celestial bodies: name, GM (m³/s²), radius (m), what they orbit.", schema(json!({}), &[])),
        t(
            "get_vessel",
            "A vessel as seen from your location (light-delayed): position and velocity relative to the body it is \
             about (inertial axes, m and m/s), altitude, orbit (the next apsides and any impact along the predicted \
             trajectory; inclination, eccentricity and period osculating), signal. Fails without a signal.",
            schema(json!({"id": id}), &["id"]),
        ),
        t(
            "get_trajectory",
            "A vessel's predicted path from now, through its planned burns: `points` samples (time from now \
             in s, position relative to the body it is about now, m).",
            schema(json!({"id": id, "points": {"type": "integer", "minimum": 2, "maximum": 1000}}), &["id"]),
        ),
        t(
            "set_warp",
            &format!(
                "Time warp level 0–{} (factors {:?}). Levels above 3 are rails warp. Setting a level resumes time the \
                 player stopped (`time_stopped` in get_state).",
                WARP_LEVELS.len() - 1,
                WARP_LEVELS
            ),
            schema(json!({"level": {"type": "integer", "minimum": 0}}), &["level"]),
        ),
        t(
            "switch_vessel",
            "Fly a vessel: it becomes the active vessel, and your control location is aboard it if it is crewed, \
             mission control if it is not (as for the player).",
            schema(json!({"id": id}), &["id"]),
        ),
        t(
            "go_to_mission_control",
            "Make mission control (the tracking station) your control location.",
            schema(json!({}), &[]),
        ),
        t(
            "set_plan",
            "Replace a vessel's planned burns (those that can still change). Each burn: `in_s` seconds from \
             now, and Δv components (m/s) prograde, normal and radial-out relative to the body the vessel is \
             about now. For a craft of parts a burn is a node (D126): `in_s` is the burn's middle, and the \
             maneuver program flies it closed-loop (turn, settle, start, cut off on the target orbit; the \
             result's `nodes` give the predicted ignition, duration and warnings). For the made-up craft \
             `in_s` is the ignition. From anywhere but aboard, the plan travels with the signal's light \
             delay; without a signal it cannot be sent. Read the result with get_trajectory.",
            schema(
                json!({"id": id, "burns": {"type": "array", "items": {"type": "object", "properties": {
                    "in_s": {"type": "number", "minimum": 0},
                    "prograde": {"type": "number"}, "normal": {"type": "number"}, "radial": {"type": "number"}},
                    "required": ["in_s"]}}}),
                &["id", "burns"],
            ),
        ),
        t(
            "get_landing_prediction",
            "Where, when and how fast the active vessel meets the ground if it keeps its throttle and chute, \
             holding the attitude its controls give (surface prograde/retrograde holds follow the surface velocity, \
             other holds keep the current attitude, SAS off noses into the wind) (its own gravity, drag, lift and thrust model): seconds from now, \
             the body, latitude/longitude (deg), ground height (m), vertical and horizontal speed relative \
             to the ground (m/s); the full-thrust braking burn that stops 10 m above the ground (seconds \
             to ignition, its altitude; null if it cannot stop); and the current radar altitude, vertical \
             and horizontal surface speed and TWR (thrust at ambient pressure, local gravity).",
            schema(json!({}), &[]),
        ),
        t(
            "closest_approaches",
            "Closest approaches between two vessels over their stored trajectories (nearest first): seconds \
             from now, distance (m), relative speed (m/s).",
            schema(json!({"a": id, "b": id}), &["a", "b"]),
        ),
        t(
            "set_controls",
            "Throttle (0–1), SAS, the attitude hold and the speed reference of a vessel (`id`, default the active \
             one; each vessel keeps its own, active or not). From anywhere but aboard, the settings reach it after \
             the signal's light delay (`arrives_in_s`); without a signal they cannot be sent. Prograde and \
             retrograde follow the speed reference (orbit, surface or target, as on its navball) about its \
             reference body; a planned burn is flown by the maneuver hold on its own. `rotate` is the pilot's \
             rotation keys held (body axes, each −1–1: x pitch, y yaw, z roll; [0,0,0] lets go), aboard the active \
             vessel only: for a short manoeuvre such as a pitch kick, flown from stopped time with `step` (agents \
             fly by decisions, D044). `rcs` (off, on, translate or rotate only) and `rcs_fine` are the pilot's RCS \
             mode (R and the RCS menu), aboard the active vessel only.",
            schema(
                json!({
                    "id": id,
                    "throttle": {"type": "number", "minimum": 0, "maximum": 1},
                    "sas": {"type": "boolean"},
                    "hold": {"type": "string", "enum": HOLD_NAMES.map(|(n, _)| n)},
                    "speed_reference": {"type": "string", "enum": SPEED_NAMES.map(|(n, _)| n)},
                    "rotate": {"type": "array", "items": {"type": "number", "minimum": -1, "maximum": 1},
                               "minItems": 3, "maxItems": 3},
                    "rcs": {"type": "string", "enum": RCS_NAMES.map(|(n, _)| n)},
                    "rcs_fine": {"type": "boolean"}
                }),
                &[],
            ),
        ),
        t(
            "stage",
            "Run a craft of parts' next stage of its sequence (`id`, default the active vessel): engines lit, \
             pieces separated, as its stage list says. It travels like any command (`arrives_in_s`).",
            schema(json!({"id": id}), &[]),
        ),
        command_tool(&id),
    ]
}

/// The `command` tool's definition (`flight_tools`).
fn command_tool(id: &Value) -> Tool {
    let description = "A craft of parts' commands as the player gives them (`id`, default the active vessel; they \
        travel like any command, `arrives_in_s`). `function`: run one of its functions by name (the action bar's and \
        the systems screen's FUNCTIONS: an abort, the legs, the grid fins). `item` (a part's or a component's id, as \
        the systems screen names it) with `do`: one of its commands by its label, as its part card and the systems \
        screen show them (IGNITE, SHUT DOWN, DEPLOY, OPEN, SEPARATE, RCS … on, POWER … off, ARM PARACHUTES). `item` \
        alone lists its commands now; neither lists the functions and every item that has commands now.";
    let properties =
        json!({"id": id, "function": {"type": "string"}, "item": {"type": "string"}, "do": {"type": "string"}});
    Tool { name: "command".into(), description: description.into(), input_schema: schema(properties, &[]) }
}

fn epoch_text(t: sim::time::Epoch) -> String {
    let (y, mo, d, h, mi, s) = t.to_calendar();
    format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:06.3} TDB")
}

/// The origin's clock reading (D088): TT.
fn site_text(t: sim::time::Epoch) -> String {
    let (y, mo, d, h, mi, s) = t.to_calendar();
    format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:06.3} TT")
}

fn status(phase: &Phase) -> &'static str {
    match phase {
        Phase::Landed { .. } => "landed",
        Phase::Crashed { .. } => "crashed",
        Phase::Powered { .. } => "powered",
        Phase::Coasting { .. } => "coasting",
    }
}

fn signal_json(s: Option<&comms::Signal>) -> Value {
    s.map_or(Value::Null, |s| {
        json!({"delay_s": s.delay, "rate_bps": if s.rate.is_finite() { json!(s.rate) } else { json!("unlimited") }, "via": s.via})
    })
}

fn vessel_id(args: &Value) -> Result<VesselId, String> {
    args.get("id").and_then(Value::as_u64).map(VesselId).ok_or_else(|| "missing integer `id`".into())
}

/// The vessel a command is for (`id`, default the active one) and how long
/// it takes to reach it from the agent's location (s: 0 aboard, else the
/// signal's light delay); an error without a signal. The command itself is
/// routed by `commands::apply` (D067, D034).
fn reach(args: &Value, sim: &SimState, comms: &Comms) -> Result<(VesselId, f64), String> {
    let id = if args.get("id").is_some() { vessel_id(args)? } else { sim.ship().id() };
    sim.index_of(id).ok_or("no such vessel")?;
    if comms.location == Location::Vessel(id) {
        return Ok((id, 0.0));
    }
    let delay = comms.signal(id).map(|s| s.delay).ok_or_else(|| format!("no signal to vessel {}", id.0))?;
    Ok((id, delay))
}

/// Answers one call: the result, and what the game must do.
/// `prediction`: the active vessel's background coast while it flies live.
pub fn answer(
    tool: &str,
    args: &Value,
    sim: &SimState,
    comms: &Comms,
    prediction: &crate::state::Prediction,
) -> (ToolResult, Vec<Effect>) {
    match run(tool, args, sim, comms, prediction) {
        Ok((v, effects)) => (ToolResult::Ok(v), effects),
        Err(e) => (ToolResult::Err(e), Vec::new()),
    }
}

#[allow(clippy::too_many_lines)]
fn run(
    tool: &str,
    args: &Value,
    sim: &SimState,
    comms: &Comms,
    prediction: &crate::state::Prediction,
) -> Result<(Value, Vec<Effect>), String> {
    let eph = &sim.world.eph;
    let name = |n: sim::frame::NodeId| eph.node(n).name.clone();
    Ok(match tool {
        "get_state" => {
            let vessels: Vec<Value> = sim
                .fleet
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    json!({"id": v.id().0, "status": status(&v.phase), "about": name(sim.dominant_of(i)),
                           "signal": signal_json(comms.signal(v.id()))})
                })
                .collect();
            let v = json!({
                "clock": epoch_text(sim.clock),
                "site_clock": {"site": sim.origin, "time": site_text(sim.site_time()),
                               "active_ship_clock_offset_s": sim.ship_clock_offset(sim.active)},
                "warp_level": sim.effective_warp(),
                "warp_factor": if sim.paused { 0.0 } else { WARP_LEVELS[sim.effective_warp()] },
                "time_stopped": sim.paused,
                "location": comms.location_name(), "active_vessel": sim.ship().id().0, "vessels": vessels,
            });
            (v, Vec::new())
        }
        "list_bodies" => {
            let bodies: Vec<Value> = eph
                .bodies()
                .map(|b| {
                    let radius = sim.world.source(b).and_then(|s| s.physical.as_ref()).map(|p| p.radius_eq);
                    json!({"name": name(b), "gm": eph.node(b).gm, "radius_m": radius,
                           "orbits": crate::relations::primary(eph, b).map(name)})
                })
                .collect();
            (json!({"bodies": bodies}), Vec::new())
        }
        "get_vessel" => get_vessel(args, sim, comms, prediction)?,
        "get_trajectory" => {
            let id = vessel_id(args)?;
            let i = sim.index_of(id).ok_or("no such vessel")?;
            if comms.signal(id).is_none() {
                return Err(format!("no signal from vessel {}", id.0));
            }
            if !crate::format::Known::of(&sim.fleet[i]).orbit() {
                return Err(format!("vessel {} does not know its position and velocity", id.0));
            }
            let n = args.get("points").and_then(Value::as_u64).unwrap_or(100).clamp(2, 1000) as usize;
            // The line's segments from now: coasts and planned burns, and the
            // prediction where they do not give the path (D089).
            let segs = crate::trajectory::apsides::vessel_segments(sim, prediction, i);
            let last = segs.last().ok_or("the vessel has no predicted path")?;
            let (t0, t1) = (sim.clock, last.t0.add_seconds(last.computed_until()));
            let span = t1.seconds_since(t0);
            if span <= 0.0 {
                return Err("no prediction yet".into());
            }
            let body = sim.dominant_of(i);
            let points: Vec<Value> = (0..n)
                .filter_map(|k| {
                    let s = span * k as f64 / (n - 1) as f64;
                    let at = t0.add_seconds(s);
                    let (anchor, r, _) = crate::trajectory::eval_at(&segs, at)?;
                    let b = sim.world.snapshot(at).relative_r(body, anchor);
                    Some(json!({"t_s": s, "position_m": (r - b).to_array()}))
                })
                .collect();
            (json!({"about": name(body), "points": points}), Vec::new())
        }
        "set_warp" => {
            let level = args.get("level").and_then(Value::as_u64).ok_or("missing integer `level`")? as usize;
            if level >= WARP_LEVELS.len() {
                return Err(format!("level must be 0–{}", WARP_LEVELS.len() - 1));
            }
            (json!({"requested": level}), vec![Effect::Command(GameCommand::SetWarp(level))])
        }
        "switch_vessel" => {
            let id = vessel_id(args)?;
            let i = sim.index_of(id).ok_or("no such vessel")?;
            // Aboard a crewed vessel; a probe is flown from mission control.
            let crewed = sim.fleet[i].crew() > 0;
            let at = comms.name_of(comms::location_rule(false, id, crewed, comms.mission_control));
            (
                json!({"active_vessel": id.0, "location": at}),
                vec![Effect::Command(GameCommand::Switch(id)), Effect::Station(false)],
            )
        }
        "go_to_mission_control" => (json!({"location": "mission control"}), vec![Effect::Station(true)]),
        "set_plan" => set_plan(args, sim, comms)?,
        "get_landing_prediction" => (landing_json(sim), Vec::new()),
        "closest_approaches" => approaches(args, sim)?,
        "set_controls" => {
            let (id, arrives) = reach(args, sim, comms)?;
            let throttle = args.get("throttle").and_then(Value::as_f64);
            if throttle.is_some_and(|t| !(0.0..=1.0).contains(&t)) {
                return Err("throttle must be in 0–1".into());
            }
            let sas = args.get("sas").and_then(Value::as_bool);
            let hold = match args.get("hold").and_then(Value::as_str) {
                None => None,
                Some(name) => Some(
                    HOLD_NAMES
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, m)| *m)
                        .ok_or(format!("unknown hold `{name}`"))?,
                ),
            };
            let speed = match args.get("speed_reference").and_then(Value::as_str) {
                None => None,
                Some(name) => Some(
                    SPEED_NAMES
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, s)| *s)
                        .ok_or(format!("unknown speed reference `{name}`"))?,
                ),
            };
            let mut effects = Vec::new();
            if let Some(r) = args.get("rotate") {
                let r: Vec<f64> =
                    r.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
                let [x, y, z] = r[..] else { return Err("`rotate` is three numbers".into()) };
                if [x, y, z].iter().any(|c| !(-1.0..=1.0).contains(c)) {
                    return Err("`rotate`'s axes are each in −1–1".into());
                }
                if comms.location != Location::Vessel(id) || id != sim.ship().id() {
                    return Err("the rotation keys are the pilot's: go aboard the active vessel".into());
                }
                effects.push(Effect::Rotate(id, DVec3::new(x, y, z)));
            }
            let rcs = rcs_mode(args, sim.controls().rcs)?;
            if let Some(mode) = rcs {
                if comms.location != Location::Vessel(id) || id != sim.ship().id() {
                    return Err("RCS's mode is the pilot's: go aboard the active vessel".into());
                }
                effects.push(Effect::Command(GameCommand::SetRcs(mode)));
            }
            let command = GameCommand::SetControls { vessel: id, throttle, sas, hold, speed };
            let hold = hold.and_then(|m| HOLD_NAMES.iter().find(|(_, h)| *h == m)).map(|(n, _)| *n);
            let speed = speed.and_then(|m| SPEED_NAMES.iter().find(|(_, s)| *s == m)).map(|(n, _)| *n);
            let rcs = rcs.map(|m| json!({"mode": rcs_name(m), "fine": m.fine}));
            let reply = json!({"id": id.0, "throttle": throttle, "sas": sas, "hold": hold, "speed_reference": speed,
                               "rotate": args.get("rotate"), "rcs": rcs, "arrives_in_s": arrives});
            effects.push(Effect::Command(command));
            (reply, effects)
        }
        "stage" => {
            let (id, arrives) = reach(args, sim, comms)?;
            let i = sim.index_of(id).ok_or("no such vessel")?;
            if sim.fleet[i].ship().is_none() {
                return Err(format!("vessel {} is not a craft of parts (it has no stage sequence)", id.0));
            }
            (json!({"id": id.0, "arrives_in_s": arrives}), vec![Effect::Command(GameCommand::Stage(id))])
        }
        "command" => command(args, sim, comms)?,
        _ => return Err(format!("unknown tool {tool}")),
    })
}

/// The pilot's RCS modes by name (`set_controls`' `rcs`).
const RCS_NAMES: [(&str, Option<RcsOnly>); 4] = [
    ("off", None),
    ("on", Some(RcsOnly::Both)),
    ("translate", Some(RcsOnly::Translate)),
    ("rotate", Some(RcsOnly::Rotate)),
];

fn rcs_name(m: RcsMode) -> &'static str {
    let only = m.on.then_some(m.only);
    RCS_NAMES.iter().find(|(_, o)| *o == only).map_or("on", |(n, _)| n)
}

/// `set_controls`' `rcs` and `rcs_fine` applied to the pilot's mode now
/// (`None`: neither given).
fn rcs_mode(args: &Value, now: RcsMode) -> Result<Option<RcsMode>, String> {
    let name = args.get("rcs").and_then(Value::as_str);
    let fine = args.get("rcs_fine").and_then(Value::as_bool);
    if name.is_none() && fine.is_none() {
        return Ok(None);
    }
    let mut mode = now;
    if let Some(name) = name {
        let only = RCS_NAMES.iter().find(|(n, _)| *n == name).ok_or(format!("unknown RCS mode `{name}`"))?.1;
        mode.on = only.is_some();
        mode.only = only.unwrap_or(mode.only);
    }
    mode.fine = fine.unwrap_or(mode.fine);
    Ok(Some(mode))
}

/// Each part's and component's commands now, by id, those with any: a
/// part's as its card gives them (`part_card::content::part_commands`), a
/// component's as the systems screen does (`inspect::commands`).
fn command_items(
    v: &sim::vessel::Vessel,
    def: &ShipDef,
    state: &ShipState,
) -> Vec<(String, Vec<(String, ShipCommand)>)> {
    use crate::interface::part_card::content::{chutes, part_commands};
    use crate::systems_screen::inspect::{self, Selection};
    let (chute_parts, words) = chutes(v);
    let chute = (!chute_parts.is_empty()).then_some((chute_parts.as_slice(), words));
    let parts = def.parts.iter().map(|p| (p.id.clone(), part_commands(def, state, chute, p.part)));
    let components = (def.components.iter().enumerate())
        .map(|(i, c)| (c.id.clone(), inspect::commands(def, state, Selection::Component(i))));
    parts.chain(components).filter(|(_, list)| !list.is_empty()).collect()
}

/// The `command` tool: a craft's functions and its parts' and components'
/// commands, as the action bar, the part cards and the systems screen give
/// them (interface-map.md finding 10), sent like any command.
fn command(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let (id, arrives) = reach(args, sim, comms)?;
    let v = &sim.fleet[sim.index_of(id).ok_or("no such vessel")?];
    let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else {
        return Err(format!("vessel {} is not a craft of parts (it has no commands)", id.0));
    };
    let s = |k: &str| args.get(k).and_then(Value::as_str);
    let functions: Vec<&str> = state.functions.iter().map(|f| f.name.as_str()).collect();
    if let Some(name) = s("function") {
        let Some(name) = functions.iter().find(|f| f.eq_ignore_ascii_case(name)) else {
            return Err(format!("no function `{name}`: it has {functions:?}"));
        };
        let run = GameCommand::Function { vessel: id, name: name.to_string() };
        return Ok((json!({"id": id.0, "function": name, "arrives_in_s": arrives}), vec![Effect::Command(run)]));
    }
    let items = command_items(v, &def, state);
    let labels = |list: &[(String, ShipCommand)]| list.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>();
    match (s("item"), s("do")) {
        (Some(item), Some(label)) => {
            let list = items.iter().find(|(i, _)| i == item).map_or(&[][..], |(_, l)| l.as_slice());
            let Some((label, c)) = list.iter().find(|(l, _)| l.eq_ignore_ascii_case(label)) else {
                return Err(format!("`{label}` is not one of {item}'s commands now: {:?}", labels(list)));
            };
            let send = GameCommand::Ship { vessel: id, commands: vec![c.clone()] };
            Ok((json!({"id": id.0, "item": item, "do": label, "arrives_in_s": arrives}), vec![Effect::Command(send)]))
        }
        (Some(item), None) => {
            let known = def.parts.iter().any(|p| p.id == item) || def.components.iter().any(|c| c.id == item);
            if !known {
                return Err(format!("no part or component `{item}`"));
            }
            let list = items.iter().find(|(i, _)| i == item).map_or(Vec::new(), |(_, l)| labels(l));
            Ok((json!({"id": id.0, "item": item, "commands": list}), Vec::new()))
        }
        (None, Some(_)) => Err("`do` needs the `item` it is about".into()),
        (None, None) => {
            let items: serde_json::Map<String, Value> =
                items.iter().map(|(i, l)| (i.clone(), json!(labels(l)))).collect();
            Ok((json!({"id": id.0, "functions": functions, "items": items}), Vec::new()))
        }
    }
}

/// The `get_vessel` tool.
fn get_vessel(
    args: &Value,
    sim: &SimState,
    comms: &Comms,
    prediction: &crate::state::Prediction,
) -> Result<(Value, Vec<Effect>), String> {
    let name = |n: sim::frame::NodeId| sim.world.eph.node(n).name.clone();
    let id = vessel_id(args)?;
    let i = sim.index_of(id).ok_or("no such vessel")?;
    let signal = comms.signal(id).ok_or_else(|| {
        let heard = comms.last_contact.get(&id).map(|t| epoch_text(*t)).unwrap_or_else(|| "never".into());
        format!("no signal from vessel {} at {} (last heard {heard})", id.0, comms.location_name())
    })?;
    let v = &sim.fleet[i];
    let body = sim.dominant_of(i);
    let (anchor, r, vel) = v.state_at(&sim.world, sim.clock);
    let k = sim.world.snapshot(sim.clock).relative(body, anchor);
    let (rel, v_rel) = (comms::retarded(r - k.r, vel - k.v, signal.delay), vel - k.v);
    let orbit = crate::relations::orbit_about(&sim.world, sim.clock, anchor, r, vel, body);
    let radius = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
    // Apsides from the predicted trajectory (D076), not the conic: the
    // line's segments (D089).
    let segs = crate::trajectory::apsides::vessel_segments(sim, prediction, i);
    let aps = crate::trajectory::apsides::of_segments(sim, &segs);
    let (ap, pe) = aps.next_after(sim.clock);
    let upcoming: Vec<Value> = aps
        .list
        .iter()
        .filter(|a| a.t.seconds_since(sim.clock) > 0.0)
        .take(8)
        .map(|a| {
            json!({"kind": if a.is_apo { "apoapsis" } else { "periapsis" }, "body": name(a.body),
                   "altitude_m": a.altitude, "in_s": a.t.seconds_since(sim.clock),
                   "near_circular": a.near_circular})
        })
        .collect();
    let impact = aps.impact.filter(|m| m.t.seconds_since(sim.clock) > 0.0);
    let orbit = orbit.map(|o| {
        json!({"periapsis_altitude_m": pe.map(|a| a.altitude), "apoapsis_altitude_m": ap.map(|a| a.altitude),
               "apsides": upcoming,
               "impact": impact.map(|m| json!({"body": name(m.body), "in_s": m.t.seconds_since(sim.clock)})),
               "inclination_deg": o.elements.i.to_degrees(), "eccentricity": o.elements.e,
               "period_s": o.period()})
    });
    // What the vessel knows (D106): a dash (null) where no working sensor
    // measures it, and each value to its precision; the truth in debug mode.
    let known = crate::format::Known::of(v);
    // Above the reference ellipsoid (the equatorial radius reads −4.9 km on
    // the pad at 28.6° N).
    let physical = sim.world.source(body).and_then(|s| s.physical.as_ref());
    let altitude = physical
        .map_or(rel.length() - radius, |p| p.altitude(p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), sim.clock)));
    let pole =
        sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(DVec3::Z, |p| p.rotation.pole(sim.clock));
    let out = json!({
        "id": id.0, "status": status(&v.phase), "about": name(body), "seen_delay_s": signal.delay,
        "debug": v.debug(),
        "position_m": known_vec(&known, Quantity::Position, rel), "velocity_ms": known_vec(&known, Quantity::Velocity, v_rel),
        "altitude_m": known_value(&known, Quantity::Altitude, altitude)
            .or_else(|| known_value(&known, Quantity::Position, altitude)),
        "orbit": orbit.filter(|_| known.orbit()), "attitude": attitude_json(&known, v.attitude.q, rel, pole),
        "stages": stages_json(v, &known), "signal": signal_json(Some(signal)),
    });
    Ok((out, Vec::new()))
}

/// A value as the vessel knows it: shown to its precision, or null.
fn known_value(known: &crate::format::Known, q: Quantity, truth: f64) -> Option<f64> {
    known.knowledge.read(q, truth).map(|r| r.shown())
}

fn known_vec(known: &crate::format::Known, q: Quantity, a: DVec3) -> Value {
    match a.to_array().map(|x| known_value(known, q, x)) {
        [Some(x), Some(y), Some(z)] => json!([x, y, z]),
        _ => Value::Null,
    }
}

/// The navball's attitude (heading, pitch, roll, deg) and the body axes in
/// the local frame (east, north, up): x the pitch axis, y the yaw axis
/// (−y the top), z the nose. Null without an attitude reference.
fn attitude_json(known: &crate::format::Known, q: glam::DQuat, rel: DVec3, pole: DVec3) -> Value {
    use crate::navball::rules::{attitude_angles, local_frame, ship_axes};
    if !known.knows(Quantity::Attitude) {
        return Value::Null;
    }
    let local = local_frame(rel, pole);
    let a = attitude_angles(&ship_axes(q), &local);
    let enu = |d: DVec3| [d.dot(local.east), d.dot(local.north), d.dot(local.up)];
    json!({"heading_deg": a.heading, "pitch_deg": a.pitch, "roll_deg": a.roll,
           "axes_enu": {"x": enu(q * DVec3::X), "y": enu(q * DVec3::Y), "z": enu(q * DVec3::Z)}})
}

/// A craft of parts' sequence and propellant as it knows them: the next
/// stage, each tank's level (kg, null where unmeasured) and each engine's
/// state.
fn stages_json(v: &sim::vessel::Vessel, known: &crate::format::Known) -> Value {
    let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else { return Value::Null };
    let stack = crate::staging::stack_of(v, v.controls());
    let next = crate::staging::next(&stack);
    let next_name = stack.iter().find(|s| Some(s.number) == next).and_then(|s| s.name.clone());
    let tanks: Vec<Value> = (def.tanks.iter().zip(&state.tanks))
        .enumerate()
        .map(|(i, (t, s))| {
            json!({"id": t.id, "fluid": t.fluid, "capacity_kg": t.capacity,
                   "liquid_kg": known_value(known, Quantity::TankLevel(i as u32), s.liquid)})
        })
        .collect();
    let engines: Vec<Value> = (def.engines.iter().zip(&state.engines))
        .map(|(e, s)| json!({"id": e.id, "main": e.main, "status": format!("{:?}", s.status), "throttle": s.throttle}))
        .collect();
    json!({"next": next, "next_name": next_name, "feeding_share": session::feeding_share(v),
           "tanks": tanks, "engines": engines})
}

/// The `get_landing_prediction` tool: the landing panel's numbers.
fn landing_json(sim: &SimState) -> Value {
    let ship = sim.ship();
    let name = |n: sim::frame::NodeId| sim.world.eph.node(n).name.clone();
    let known = crate::format::Known::of(ship);
    let none = json!({"impact": null, "braking": null, "surface": null});
    // The panel's numbers need the vessel's position and velocity (D106).
    if !known.orbit() {
        return none;
    }
    let Some(start) = crate::landing::start_of(&sim.world, ship, sim.controls()) else { return none };
    let l = crate::landing::predict(&sim.world, &ship.craft, ship.id(), &start);
    let (anchor, r, v) = ship.state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let m = sim::landing::surface_motion(&sim.world, &snap, anchor, start.body, r, v);
    let impact = l.impact.map(|h| {
        json!({
            "in_s": h.t.seconds_since(sim.clock), "body": name(h.body),
            "lat_deg": h.lat.to_degrees(), "lon_deg": h.lon.to_degrees(), "ground_height_m": h.height,
            "speed_ms": h.speed, "vertical_ms": h.v_vertical, "horizontal_ms": h.v_horizontal,
        })
    });
    let braking = l.braking.map(|b| {
        json!({
            "ignite_in_s": b.t_ignite.seconds_since(sim.clock), "ignite_altitude_m": b.ignite_height,
            "stop_in_s": b.t_stop.seconds_since(sim.clock), "stop_height_m": b.stop_height,
        })
    });
    let twr = start.twr(&ship.craft, m.pressure, m.gravity);
    let surface = json!({
        "body": name(start.body), "radar_altitude_m": known_value(&known, Quantity::Altitude, m.height - ship.contact_height()),
        "vertical_ms": m.v_vertical, "horizontal_ms": m.v_horizontal,
        "twr": known.propellant.then_some(twr),
    });
    json!({"impact": impact, "braking": braking, "surface": surface})
}

/// The `closest_approaches` tool.
fn approaches(args: &Value, sim: &SimState) -> Result<(Value, Vec<Effect>), String> {
    let get = |k: &str| args.get(k).and_then(Value::as_u64).map(VesselId).ok_or(format!("missing `{k}`"));
    let (a, b) = (get("a")?, get("b")?);
    let tr = |id: VesselId| {
        sim.index_of(id)
            .and_then(|i| sim.fleet[i].trajectory())
            .ok_or(format!("vessel {} has no stored trajectory (not coasting)", id.0))
    };
    let (ta, tb) = (tr(a)?, tr(b)?);
    let t1 = std::cmp::min_by(ta.computed_until(), tb.computed_until(), |x, y| x.seconds_since(*y).total_cmp(&0.0));
    let list = sim::approach::closest_approaches(&sim.world, |t| ta.eval(t), |t| tb.eval(t), sim.clock, t1, 400);
    let out: Vec<Value> = list
        .iter()
        .take(5)
        .map(|c| json!({"in_s": c.t.seconds_since(sim.clock), "distance_m": c.distance, "speed_ms": c.speed}))
        .collect();
    Ok((json!({"approaches": out}), Vec::new()))
}

/// The `set_plan` tool.
fn set_plan(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let name = |n: sim::frame::NodeId| sim.world.eph.node(n).name.clone();
    let id = vessel_id(args)?;
    let i = sim.index_of(id).ok_or("no such vessel")?;
    let delay = if comms.location == Location::Vessel(id) {
        0.0
    } else {
        comms.signal(id).map(|s| s.delay).ok_or_else(|| format!("no signal to vessel {}", id.0))?
    };
    let burns = args.get("burns").and_then(Value::as_array).ok_or("missing array `burns`")?;
    let body = sim.dominant_of(i);
    let mut draft = Vec::new();
    for b in burns {
        let get = |k: &str| b.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let in_s = b.get("in_s").and_then(Value::as_f64).ok_or("each burn needs `in_s`")?;
        if in_s <= delay {
            return Err(format!("a burn in {in_s} s ignites before the plan arrives ({delay:.3} s)"));
        }
        let dv = glam::DVec3::new(get("prograde"), get("normal"), get("radial"));
        draft.push(crate::planner::DraftBurn { t_start: sim.clock.add_seconds(in_s), dv, reference: body });
    }
    let vessel = &sim.fleet[i];
    let command = crate::planner::command_for(vessel, &draft, sim.clock.add_seconds(delay));
    // A craft of parts: the prediction of each node (the planner's).
    let (info, available, _) = crate::planner::draft_info(&sim.world, vessel, &draft);
    let nodes: Vec<Value> = draft
        .iter()
        .zip(&info)
        .filter(|_| vessel.ship().is_some())
        .map(|(d, b)| {
            json!({"node_in_s": d.t_start.seconds_since(sim.clock), "ignite_in_s": b.ignite.seconds_since(sim.clock),
                "duration_s": b.duration, "warnings": b.warnings})
        })
        .collect();
    let effect = if delay > 0.0 { Effect::Send { delay, command } } else { Effect::Command(command) };
    let mut out = json!({"sent": draft.len(), "arrives_in_s": delay, "about": name(body)});
    if vessel.ship().is_some() {
        out["nodes"] = json!(nodes);
        out["available_dv_ms"] = json!(available);
    }
    Ok((out, vec![effect]))
}

/// The running server, if the setting is on.
#[derive(Default)]
pub struct AgentServer {
    server: Option<(Server, u16, String)>,
    error: Option<String>,
    env: Option<Option<(u16, String)>>,
}

/// A token for the agent to present (local use; not cryptographic).
pub fn new_token() -> String {
    use std::hash::{BuildHasher, RandomState};
    let a = RandomState::new().hash_one(std::time::SystemTime::now());
    let b = RandomState::new().hash_one(a);
    format!("{a:016x}{b:016x}")
}

/// Starts or stops the server with the setting, and answers its calls.
#[allow(clippy::too_many_arguments)]
pub fn serve(
    mut iface: ResMut<InterfaceSettings>,
    mut state: Local<AgentServer>,
    sim: Res<SimState>,
    comms: Res<Comms>,
    pred: Res<crate::state::Prediction>,
    mut station: ResMut<TrackingStation>,
    mut in_flight: ResMut<crate::commands::InFlight>,
    mut commands: MessageWriter<GameCommand>,
    mut session: ResMut<session::Session>,
    mut held: ResMut<HeldRotation>,
) {
    // `SUNSCATTER_AGENT=<port>:<token>` turns the server on (demos, tests).
    if state.server.is_none() && state.env.is_none() {
        state.env = Some(std::env::var("SUNSCATTER_AGENT").ok().and_then(|v| {
            let (port, token) = v.split_once(':')?;
            Some((port.parse().ok()?, token.to_string()))
        }));
        if let Some(Some((port, token))) = &state.env {
            iface.agent = crate::interface::layout::AgentSettings { enabled: true, port: *port, token: token.clone() };
        }
    }
    let want = iface.agent.enabled.then(|| iface.agent.port);
    if want.is_some() && iface.agent.token.is_empty() {
        iface.agent.token = new_token();
    }
    let running = state.server.as_ref().map(|s| (s.1, s.2.clone()));
    if want.map(|p| (p, iface.agent.token.clone())) != running {
        state.server = None;
        state.error = None;
        if let Some(port) = want {
            match Server::start(port, Some(iface.agent.token.clone()), tools(session::developer())) {
                Ok(s) => state.server = Some((s, port, iface.agent.token.clone())),
                Err(e) => {
                    iface.agent.enabled = false;
                    error!("agent server on port {port}: {e}");
                }
            }
        }
    }
    let Some((server, _, _)) = &state.server else { return };
    for call in server.poll() {
        if session::handles(&call.tool) {
            session.incoming.push(call);
            continue;
        }
        let (result, effects) = answer(&call.tool, &call.arguments, &sim, &comms, &pred);
        for e in effects {
            match e {
                Effect::Command(c) => {
                    commands.write(c);
                }
                Effect::Station(open) => station.open = open,
                Effect::Send { delay, command } => in_flight.send(sim.clock, delay, command),
                Effect::Rotate(id, r) => held.0 = (r != DVec3::ZERO).then_some((id, r)),
            }
        }
        call.reply(result);
    }
}

/// The command a player runs to connect Claude Code.
pub fn connect_command(port: u16, token: &str) -> String {
    format!(
        "claude mcp add --transport http sunscatter http://127.0.0.1:{port}/mcp --header \"Authorization: Bearer {token}\""
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (SimState, Comms) {
        let mut sim = SimState::new();
        sim.spawn_test_ships(3);
        (sim, Comms::load())
    }

    #[test]
    fn every_tool_has_an_object_schema_and_an_answer() {
        let (sim, comms) = setup();
        for t in flight_tools() {
            assert_eq!(t.input_schema["type"], "object", "{}", t.name);
            // Every tool answers something (an error for missing arguments).
            let _ = answer(&t.name, &json!({}), &sim, &comms, &Default::default());
        }
    }

    #[test]
    fn state_lists_every_vessel() {
        let (sim, comms) = setup();
        let (ToolResult::Ok(v), effects) = answer("get_state", &json!({}), &sim, &comms, &Default::default()) else {
            panic!()
        };
        assert!(effects.is_empty());
        assert_eq!(v["vessels"].as_array().unwrap().len(), sim.fleet.len());
        assert_eq!(v["active_vessel"], sim.ship().id().0);
    }

    #[test]
    fn a_vessel_without_signal_cannot_be_read() {
        let (sim, mut comms) = setup();
        let id = sim.fleet[1].id();
        comms.signals.insert(id, None);
        let (r, _) = answer("get_vessel", &json!({"id": id.0}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("no signal")));
        comms.signals.insert(id, Some(comms::Signal { delay: 0.01, rate: 1e6, via: vec![] }));
        let (r, _) = answer("get_vessel", &json!({"id": id.0}), &sim, &comms, &Default::default());
        let ToolResult::Ok(v) = r else { panic!("{r:?}") };
        assert_eq!(v["about"], "Earth");
        assert!(v["altitude_m"].as_f64().unwrap() > 300_000.0);
    }

    #[test]
    fn a_plan_from_mission_control_travels_with_the_delay() {
        let (sim, mut comms) = setup();
        let id = sim.fleet[1].id();
        comms.location = Location::Site(0);
        comms.signals.insert(id, Some(comms::Signal { delay: 0.02, rate: 1e6, via: vec![] }));
        let args = json!({"id": id.0, "burns": [{"in_s": 600.0, "prograde": 100.0}]});
        let (r, e) = answer("set_plan", &args, &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Ok(_)), "{r:?}");
        // A craft of parts' plan is its nodes (D126).
        assert!(matches!(&e[..], [Effect::Send { delay, command: GameCommand::SetNodes { .. } }] if *delay == 0.02));
        comms.signals.insert(id, None);
        let (r, _) = answer("set_plan", &args, &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("no signal")));
    }

    #[test]
    fn actions_become_commands_and_controls_reach_any_vessel() {
        let (sim, mut comms) = setup();
        let (_, e) = answer("set_warp", &json!({"level": 5}), &sim, &comms, &Default::default());
        assert_eq!(e, vec![Effect::Command(GameCommand::SetWarp(5))]);
        let (r, _) = answer("set_warp", &json!({"level": 99}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(_)));
        // Aboard the active vessel: its controls, at once.
        let active = sim.ship().id();
        comms.location = Location::Vessel(active);
        let (r, e) = answer("set_controls", &json!({"throttle": 0.5, "sas": false}), &sim, &comms, &Default::default());
        let set = |vessel, throttle, sas, hold| GameCommand::SetControls { vessel, throttle, sas, hold, speed: None };
        assert_eq!(e, vec![Effect::Command(set(active, Some(0.5), Some(false), None))]);
        assert!(matches!(r, ToolResult::Ok(v) if v["arrives_in_s"] == 0.0));
        // From mission control, an inactive probe's (D112): after the light
        // delay, and not without a signal.
        comms.location = Location::Site(0);
        let probe = sim.fleet[2].id();
        assert_eq!(sim.fleet[2].crew(), 0);
        comms.signals.insert(probe, Some(comms::Signal { delay: 0.02, rate: 1e6, via: vec![] }));
        let args = json!({"id": probe.0, "hold": "retrograde"});
        let (r, e) = answer("set_controls", &args, &sim, &comms, &Default::default());
        assert_eq!(e, vec![Effect::Command(set(probe, None, None, Some(HoldMode::Retrograde)))]);
        assert!(matches!(r, ToolResult::Ok(v) if v["arrives_in_s"] == 0.02 && v["hold"] == "retrograde"));
        let (r, _) =
            answer("set_controls", &json!({"id": probe.0, "throttle": 2.0}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("0–1")));
        let (r, _) =
            answer("set_controls", &json!({"id": probe.0, "hold": "sideways"}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("unknown hold")));
        // Its next stage (the small rocket's Ignition) after the light delay.
        let (r, e) = answer("stage", &json!({"id": probe.0}), &sim, &comms, &Default::default());
        assert_eq!(e, vec![Effect::Command(GameCommand::Stage(probe))]);
        assert!(matches!(r, ToolResult::Ok(v) if v["arrives_in_s"] == 0.02));
        comms.signals.insert(probe, None);
        let (r, _) = answer("set_controls", &args, &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("no signal")));
        let (r, _) = answer("set_controls", &json!({"id": 999}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("no such vessel")));
    }

    #[test]
    fn a_probe_is_flown_from_mission_control() {
        let (sim, comms) = setup();
        let probe = sim.fleet[2].id();
        assert_eq!(sim.fleet[2].crew(), 0);
        let (r, e) = answer("switch_vessel", &json!({"id": probe.0}), &sim, &comms, &Default::default());
        assert_eq!(e, vec![Effect::Command(GameCommand::Switch(probe)), Effect::Station(false)]);
        let site = comms.name_of(Location::Site(comms.mission_control));
        assert!(matches!(r, ToolResult::Ok(v) if v["location"] == site.as_str() && v["active_vessel"] == probe.0));
    }

    #[test]
    fn the_rotation_keys_and_the_speed_reference() {
        let (sim, mut comms) = setup();
        let active = sim.ship().id();
        comms.location = Location::Vessel(active);
        let args = json!({"rotate": [0.04, 0.0, 0.0], "speed_reference": "surface"});
        let (r, e) = answer("set_controls", &args, &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Ok(v) if v["speed_reference"] == "surface"));
        let set = GameCommand::SetControls {
            vessel: active,
            throttle: None,
            sas: None,
            hold: None,
            speed: Some(SpeedReference::Surface),
        };
        assert_eq!(e, vec![Effect::Rotate(active, DVec3::new(0.04, 0.0, 0.0)), Effect::Command(set)]);
        // (arguments, the error's words)
        let bad = [
            (json!({"rotate": [2.0, 0.0, 0.0]}), "−1–1"),
            (json!({"rotate": [0.0, 0.0]}), "three numbers"),
            (json!({"speed_reference": "galactic"}), "unknown speed reference"),
        ];
        for (args, words) in bad {
            let (r, _) = answer("set_controls", &args, &sim, &comms, &Default::default());
            assert!(matches!(&r, ToolResult::Err(e) if e.contains(words)), "{args}: {r:?}");
        }
        // Not from mission control: the keys are the pilot's.
        comms.location = Location::Site(0);
        comms.signals.insert(active, Some(comms::Signal { delay: 0.0, rate: 1e6, via: vec![] }));
        let (r, _) = answer("set_controls", &json!({"rotate": [0.0, 0.1, 0.0]}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("aboard")));
        let held = HeldRotation(Some((active, DVec3::X)));
        assert_eq!((held.of(active), held.of(VesselId(999))), (DVec3::X, DVec3::ZERO));
    }

    #[test]
    fn the_agent_sees_what_the_vessel_knows() {
        // A craft of parts with no sensors knows neither where it is nor its
        // tanks; in debug mode it knows everything (D106).
        let mut sim = SimState::with_craft(&crate::test_rocket::rocket());
        let mut comms = Comms::load();
        let id = sim.ship().id();
        comms.location = Location::Vessel(id);
        comms.signals.insert(id, Some(comms::Signal { delay: 0.0, rate: 1e6, via: vec![] }));
        let get = |sim: &SimState| match answer("get_vessel", &json!({"id": id.0}), sim, &comms, &Default::default()) {
            (ToolResult::Ok(v), _) => v,
            (r, _) => panic!("{r:?}"),
        };
        let blind = get(&sim);
        for key in ["position_m", "velocity_ms", "altitude_m", "orbit", "attitude"] {
            assert!(blind[key].is_null(), "{key}: {}", blind[key]);
        }
        assert!(blind["stages"]["tanks"][0]["liquid_kg"].is_null());
        assert!(blind["stages"]["tanks"][0]["capacity_kg"].as_f64().unwrap() > 0.0, "the design is known");
        let (ToolResult::Ok(l), _) = answer("get_landing_prediction", &json!({}), &sim, &comms, &Default::default())
        else {
            panic!()
        };
        assert!(l["surface"].is_null());
        let s = &mut sim;
        s.fleet[s.active].set_debug(&s.world, true);
        let seen = get(&sim);
        assert_eq!(seen["position_m"].as_array().map(Vec::len), Some(3));
        assert!(seen["altitude_m"].as_f64().is_some_and(|a| a.abs() < 100.0), "on the pad: {}", seen["altitude_m"]);
        assert!(seen["attitude"]["pitch_deg"].as_f64().is_some_and(|p| p > 80.0), "upright: {}", seen["attitude"]);
        assert!(seen["stages"]["tanks"][0]["liquid_kg"].as_f64().is_some());
        assert!(seen["stages"]["next"].is_u64(), "{}", seen["stages"]);
    }

    #[test]
    fn a_script_stages_a_craft_of_parts() {
        let sim = SimState::with_craft(&crate::test_rocket::rocket());
        let mut comms = Comms::load();
        comms.location = Location::Vessel(sim.ship().id());
        let (r, e) = answer("stage", &json!({}), &sim, &comms, &Default::default());
        assert_eq!(e, vec![Effect::Command(GameCommand::Stage(sim.ship().id()))]);
        assert!(matches!(r, ToolResult::Ok(v) if v["arrives_in_s"] == 0.0));
    }

    /// The `command` tool (interface-map.md finding 10): it lists the
    /// craft's functions and each item's commands, runs a function by name
    /// and an item's command by its label (any case), and refuses what the
    /// craft or the item cannot do now.
    #[test]
    fn a_script_runs_functions_and_part_commands() {
        let sim = SimState::with_craft(&crate::test_rocket::rocket());
        let me = sim.ship().id();
        let mut comms = Comms::load();
        comms.location = Location::Vessel(me);
        let call = |args: Value| answer("command", &args, &sim, &comms, &Default::default());
        let (ToolResult::Ok(v), e) = call(json!({})) else { panic!() };
        assert!(e.is_empty());
        assert!(v["functions"].as_array().unwrap().iter().any(|f| f == "Legs"), "{v}");
        assert!(v["items"]["tank"].as_array().unwrap().iter().any(|c| c == "SEPARATE separation"), "{v}");
        assert!(v["items"]["engine#1"].as_array().unwrap().iter().any(|c| c == "IGNITE engine#1"), "{v}");
        let (ToolResult::Ok(v), e) = call(json!({"item": "tank"})) else { panic!() };
        assert!(e.is_empty() && v["commands"].as_array().is_some_and(|c| !c.is_empty()), "{v}");
        let (_, e) = call(json!({"function": "legs"}));
        assert_eq!(e, vec![Effect::Command(GameCommand::Function { vessel: me, name: "Legs".into() })]);
        let (r, e) = call(json!({"item": "tank", "do": "separate separation"}));
        let separate = vec![ShipCommand::Separate("separation".into())];
        assert_eq!(e, vec![Effect::Command(GameCommand::Ship { vessel: me, commands: separate })]);
        assert!(matches!(r, ToolResult::Ok(v) if v["do"] == "SEPARATE separation" && v["arrives_in_s"] == 0.0));
        // Refused, with nothing sent.
        let refused = [
            json!({"function": "Warp drive"}),
            json!({"item": "tank", "do": "IGNITE tank"}),
            json!({"item": "nothing"}),
            json!({"do": "ARM PARACHUTES"}),
        ];
        for args in refused {
            let (r, e) = call(args.clone());
            assert!(matches!(r, ToolResult::Err(_)) && e.is_empty(), "{args}: {r:?}");
        }
    }

    /// The pilot's RCS mode through `set_controls` (table: what is given,
    /// the mode before, the mode sent), aboard the active vessel only.
    #[test]
    fn a_script_sets_the_rcs_mode() {
        let off = RcsMode::default();
        let on = |only, fine| RcsMode { on: true, only, fine };
        let table = [
            (json!({}), off, None),
            (json!({"rcs": "on"}), off, Some(on(RcsOnly::Both, false))),
            (json!({"rcs": "translate"}), off, Some(on(RcsOnly::Translate, false))),
            (json!({"rcs": "rotate", "rcs_fine": true}), off, Some(on(RcsOnly::Rotate, true))),
            (json!({"rcs_fine": true}), on(RcsOnly::Both, false), Some(on(RcsOnly::Both, true))),
            (
                json!({"rcs": "off"}),
                on(RcsOnly::Translate, true),
                Some(RcsMode { on: false, ..on(RcsOnly::Translate, true) }),
            ),
        ];
        for (args, before, want) in table {
            assert_eq!(rcs_mode(&args, before), Ok(want), "{args}");
        }
        let names = [(off, "off"), (on(RcsOnly::Both, true), "on"), (on(RcsOnly::Rotate, false), "rotate")];
        for (mode, name) in names {
            assert_eq!(rcs_name(mode), name);
        }
        assert!(rcs_mode(&json!({"rcs": "warp"}), off).is_err());
        let sim = SimState::with_craft(&crate::test_rocket::rocket());
        let mut comms = Comms::load();
        comms.location = Location::Vessel(sim.ship().id());
        let (_, e) = answer("set_controls", &json!({"rcs": "on"}), &sim, &comms, &Default::default());
        assert!(e.contains(&Effect::Command(GameCommand::SetRcs(on(RcsOnly::Both, false)))), "{e:?}");
        comms.location = Location::Site(0);
        let (r, e) = answer("set_controls", &json!({"rcs": "on"}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(_)) && e.is_empty(), "{r:?}");
    }
}
