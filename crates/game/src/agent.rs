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

pub mod program;
pub mod read;
pub mod session;
pub mod switches;

use crate::commands::GameCommand;
use crate::comms::{self, Comms, Location};
use crate::interface::layout::InterfaceSettings;
use crate::interface::stage_bar::{self, rows};
use crate::interface::toasts::{Link, Toasts};
use crate::navball::NavTarget;
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
    /// The flight keys held on the active vessel ([`HeldKeys`]): the
    /// rotation and the translation, each `None` where the call leaves it.
    Hold {
        vessel: VesselId,
        rotate: Option<DVec3>,
        translate: Option<DVec3>,
    },
    /// The navigation target (the Orbital menu's picker; `None` clears it).
    Target(Option<NavTarget>),
}

/// What the player's interface holds that the agent reads as the player
/// does (D044): the navigation target, the notifications, the menus'
/// snapshot and the navball's state (the `read` tool's pages), and whether
/// the tracking station is open.
#[derive(Clone, Copy, Default)]
pub struct Seen<'a> {
    pub target: Option<NavTarget>,
    pub toasts: Option<&'a Toasts>,
    pub flight: Option<&'a crate::interface::menus::FlightData>,
    pub nav: Option<&'a crate::navball::NavState>,
    pub station: bool,
}

/// The flight keys an agent aboard holds on the active vessel, like the
/// pilot's (`commands::keys::read_controls` adds them), held until the
/// agent changes them: body axes, each −1–1.
#[derive(Resource, Default)]
pub struct HeldKeys(pub Option<(VesselId, Keys)>);

/// The keys held: `rotate` (x pitch, y yaw, z roll) and `translate` (+X
/// right, +Y down, +Z the nose: the I K, J L, H N keys, with RCS on).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keys {
    pub rotate: DVec3,
    pub translate: DVec3,
}

impl HeldKeys {
    /// The keys held on vessel `id`.
    pub fn of(&self, id: VesselId) -> Keys {
        self.0.filter(|(v, _)| *v == id).map_or(Keys::default(), |(_, k)| k)
    }

    /// Holds `rotate` and `translate` on vessel `id` (`None` keeps what it
    /// holds there; another vessel's are let go).
    pub fn set(&mut self, id: VesselId, rotate: Option<DVec3>, translate: Option<DVec3>) {
        let now = self.of(id);
        let k = Keys { rotate: rotate.unwrap_or(now.rotate), translate: translate.unwrap_or(now.translate) };
        self.0 = (k != Keys::default()).then_some((id, k));
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
    let id = json!({"type": "integer", "description": "Vessel id (default: the active vessel)"});
    vec![
        t(
            "get_state",
            "Game clock (TDB, the physics' time), the origin's clock (TT, the time the player sees) and the active \
             ship clock's offset from it, time warp, your control location, the active vessel, and every vessel's \
             status and signal at your location.",
            schema(json!({}), &[]),
        ),
        t(
            "list_bodies",
            "Celestial bodies: name, GM (m³/s²), radius (m), what they orbit, and the distance from the active \
             vessel to the centre (m; its body menu's, as the vessel knows its position: null where it cannot).",
            schema(json!({}), &[]),
        ),
        t(
            "get_vessel",
            "A vessel as seen from your location (light-delayed): position and velocity relative to the body it is \
             about (inertial axes, m and m/s), altitude, orbit (the next apsides and any impact along the predicted \
             trajectory; inclination, eccentricity and period osculating; while the active vessel flies live, \
             `prediction_age_s`, how old the predicted path is (it is recomputed a few times a second of real time, \
             so just after a `step` under thrust it lags), and always `now_two_body`, the apsides from the body's \
             centre if only its gravity acted from now, the Orbital menu's ORBIT NOW), signal, and its stages: tanks, engines, \
             and `sequence`, the staging bar's rows top to bottom (each stage's `stage` number for edit_stages, KSP \
             number, name, place, commands, and the figures it shows: Δv, burnout, propellant, the active stage's \
             thrust, TWR, mass, Isp; in vacuum, or the current air with `air: \"current\"`, D120). Dashes (null \
             or \"—\") where the vessel measures nothing (D106). Fails without a signal.",
            schema(json!({"id": id, "air": {"type": "string", "enum": ["vacuum", "current"]}}), &[]),
        ),
        t(
            "get_trajectory",
            "A vessel's predicted path from now, through its planned burns: `points` samples (time from now \
             in s, position relative to the body it is about now, m, to the position's precision).",
            schema(json!({"id": id, "points": {"type": "integer", "minimum": 2, "maximum": 1000}}), &[]),
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
            "set_notify",
            "A craft's notifications (D128): `on` (the default), a risk to it (its crew's air or life support, a \
             part destroyed, no flight computer, an essential load shed) drops time warp at the risk's epoch, as \
             the player is told; off, nothing on it stops warp (its events are still logged). A craft's \
             warp-stop program, where it has one, decides instead of the built-in line while it runs.",
            schema(json!({"id": id, "on": {"type": "boolean"}}), &["on"]),
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
             about now. A burn is a node (D126): `in_s` is the burn's middle, and the maneuver program flies \
             it closed-loop (turn, settle, start, cut off on the target orbit; the result's `nodes` give the \
             predicted ignition, duration and warnings). From anywhere but aboard, the plan travels with the \
             signal's light delay; without a signal it cannot be sent. Read the result with get_trajectory.",
            schema(
                json!({"id": id, "burns": {"type": "array", "items": {"type": "object", "properties": {
                    "in_s": {"type": "number", "minimum": 0},
                    "prograde": {"type": "number"}, "normal": {"type": "number"}, "radial": {"type": "number"}},
                    "required": ["in_s"]}}}),
                &["burns"],
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
             and horizontal surface speed and TWR (thrust at ambient pressure, local gravity). Each as the Landing \
             menu shows it (D106): null where the craft cannot measure it, to its sensors' precision.",
            schema(json!({}), &[]),
        ),
        t(
            "closest_approaches",
            "Vessel `a`'s closest approaches to `b` over their stored trajectories (nearest first), as the \
             Orbital menu shows the active vessel's to its target: seconds from now, distance (m), relative \
             speed (m/s), each as `a` measures it (null where it cannot; D106). Both need a signal.",
            schema(json!({"a": id, "b": id}), &["a", "b"]),
        ),
        controls_tool(&id),
        command_tool(&id),
        read::tool(),
    ]
    .into_iter()
    .chain(interface_tools(&id))
    .collect()
}

/// The tools that reach what the player's interface holds: the staging
/// bar (the next stage, its edits), the target, the notifications
/// (interface-map.md finding 10).
fn interface_tools(id: &Value) -> Vec<Tool> {
    let t = |name: &str, description: &str, input_schema: Value| Tool {
        name: name.into(),
        description: description.into(),
        input_schema,
    };
    vec![
        t(
            "stage",
            "Run a craft of parts' next stage of its sequence (`id`, default the active vessel): engines lit, \
             pieces separated, as its stage list says. It travels like any command (`arrives_in_s`).",
            schema(json!({"id": id}), &[]),
        ),
        t(
            "edit_stages",
            "Edit a craft of parts' stage sequence as the player does on the staging bar (`id`, default the active \
             vessel), one edit a call, by the stages' `stage` numbers in get_vessel's `stages.sequence`: `move` a \
             stage still to run to another's place ({stage, to}), `move_command` one of its commands to the end \
             of another ({stage, command, to}), `delete` a stage still to run, or run a stage already run `again` \
             (D114). It travels like any command (`arrives_in_s`).",
            schema(
                json!({"id": id,
                       "move": {"type": "object", "properties": {"stage": {"type": "integer"}, "to": {"type": "integer"}},
                                "required": ["stage", "to"]},
                       "move_command": {"type": "object", "properties": {"stage": {"type": "integer"},
                                "command": {"type": "integer"}, "to": {"type": "integer"}},
                                "required": ["stage", "command", "to"]},
                       "delete": {"type": "integer"}, "again": {"type": "integer"}}),
                &[],
            ),
        ),
        t(
            "set_target",
            "The navigation target, as the Orbital menu's TARGET picks it: a `vessel` (id, not the active one) or a \
             `body` with a surface (name), or `none: true`. The navball's TARGET speed and holds and the Orbital \
             menu's distance and closest approach follow it; get_state gives it.",
            schema(json!({"vessel": id, "body": {"type": "string"}, "none": {"type": "boolean"}}), &[]),
        ),
        t(
            "notifications",
            "The notifications' messages, as the player reads them bottom left (saves, events, commands' replies), \
             oldest first, each with its number `n`, whether it is an error and the vessel it is about: those \
             after number `after` (default 0: all still kept, the last 200). `count` is the newest's number.",
            schema(json!({"after": {"type": "integer", "minimum": 0}}), &[]),
        ),
        program::tool(id),
    ]
    .into_iter()
    .chain(switches::tools(id))
    .collect()
}

/// The `set_controls` tool's definition (`flight_tools`).
fn controls_tool(id: &Value) -> Tool {
    let t = |name: &str, description: &str, input_schema: Value| Tool {
        name: name.into(),
        description: description.into(),
        input_schema,
    };
    t(
        "set_controls",
        "Throttle (0–1), SAS, the attitude hold and the speed reference of a vessel (`id`, default the active \
             one; each vessel keeps its own, active or not). From anywhere but aboard, the settings reach it after \
             the signal's light delay (`arrives_in_s`); without a signal they cannot be sent. Prograde and \
             retrograde follow the speed reference (orbit, surface or target, as on its navball) about its \
             reference body; a planned burn is flown by the maneuver hold on its own. `rotate` is the pilot's \
             rotation keys held (body axes, each −1–1: x pitch, y yaw, z roll; [0,0,0] lets go), aboard the active \
             vessel only: for a short manoeuvre such as a pitch kick, flown from stopped time with `step` (agents \
             fly by decisions, D044). `translate` is the pilot's translation keys held the same way (body axes, \
             each −1–1: x right, y down, z forward along the nose; I K, J L, H N), with RCS on. `rcs` (off, on, \
             translate or rotate only) and `rcs_fine` are the pilot's RCS mode (R and the RCS menu), aboard the \
             active vessel only.",
        schema(
            json!({
                "id": id,
                "throttle": {"type": "number", "minimum": 0, "maximum": 1},
                "sas": {"type": "boolean"},
                "hold": {"type": "string", "enum": HOLD_NAMES.map(|(n, _)| n)},
                "speed_reference": {"type": "string", "enum": SPEED_NAMES.map(|(n, _)| n)},
                "rotate": {"type": "array", "items": {"type": "number", "minimum": -1, "maximum": 1},
                           "minItems": 3, "maxItems": 3},
                "translate": {"type": "array", "items": {"type": "number", "minimum": -1, "maximum": 1},
                           "minItems": 3, "maxItems": 3},
                "rcs": {"type": "string", "enum": RCS_NAMES.map(|(n, _)| n)},
                "rcs_fine": {"type": "boolean"}
            }),
            &[],
        ),
    )
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

/// The vessel a call is about: `id`, default the active one (one rule for
/// every tool that takes a vessel; `get_vessel` required it, finding 65).
fn vessel_or_active(args: &Value, sim: &SimState) -> Result<VesselId, String> {
    if args.get("id").is_some() {
        vessel_id(args)
    } else {
        Ok(sim.ship().id())
    }
}

/// The vessel a command is for (`id`, default the active one) and how long
/// it takes to reach it from the agent's location (s: 0 aboard, else the
/// signal's light delay); an error without a signal. The command itself is
/// routed by `commands::apply` (D067, D034).
fn reach(args: &Value, sim: &SimState, comms: &Comms) -> Result<(VesselId, f64), String> {
    let id = vessel_or_active(args, sim)?;
    sim.index_of(id).ok_or("no such vessel")?;
    if comms.location == Location::Vessel(id) {
        return Ok((id, 0.0));
    }
    let delay = comms.signal(id).map(|s| s.delay).ok_or_else(|| format!("no signal to vessel {}", id.0))?;
    // Shown only (the command's route takes its own): to the millisecond
    // (it read 4.583767688959715e-08 to a craft 14 m away).
    Ok((id, seconds(delay)))
}

/// Answers one call: the result, and what the game must do.
/// `prediction`: the active vessel's background coast while it flies live.
#[cfg(test)]
pub fn answer(
    tool: &str,
    args: &Value,
    sim: &SimState,
    comms: &Comms,
    prediction: &crate::state::Prediction,
) -> (ToolResult, Vec<Effect>) {
    answer_seen(tool, args, sim, comms, prediction, Seen::default())
}

/// [`answer`], with what the player's interface holds (`seen`).
pub fn answer_seen(
    tool: &str,
    args: &Value,
    sim: &SimState,
    comms: &Comms,
    prediction: &crate::state::Prediction,
    seen: Seen,
) -> (ToolResult, Vec<Effect>) {
    match run(tool, args, sim, comms, prediction, seen) {
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
    seen: Seen,
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
                "target": target_json(sim, seen.target),
            });
            (v, Vec::new())
        }
        "list_bodies" => {
            // The body menu's distance, as the active vessel knows it (D106).
            let known = crate::format::Known::of(sim.ship());
            let bodies: Vec<Value> = eph
                .bodies()
                .map(|b| {
                    let radius = sim.world.source(b).and_then(|s| s.physical.as_ref()).map(|p| p.radius_eq);
                    let distance = known_value(&known, Quantity::Position, crate::hud::body_distance(sim, b));
                    json!({"name": name(b), "gm": eph.node(b).gm, "radius_m": radius,
                           "orbits": crate::relations::primary(eph, b).map(name), "distance_m": distance})
                })
                .collect();
            (json!({"bodies": bodies}), Vec::new())
        }
        "get_vessel" => get_vessel(args, sim, comms, prediction)?,
        "get_trajectory" => {
            let id = vessel_or_active(args, sim)?;
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
            // Each point to the position's precision (D106).
            let known = crate::format::Known::of(&sim.fleet[i]);
            let points: Vec<Value> = (0..n)
                .filter_map(|k| {
                    let s = span * k as f64 / (n - 1) as f64;
                    let at = t0.add_seconds(s);
                    let (anchor, r, _) = crate::trajectory::eval_at(&segs, at)?;
                    let b = sim.world.snapshot(at).relative_r(body, anchor);
                    Some(json!({"t_s": s, "position_m": known_vec(&known, Quantity::Position, r - b)}))
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
        "closest_approaches" => approaches(args, sim, comms)?,
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
            let (rotate, translate) = (axes(args, "rotate")?, axes(args, "translate")?);
            if rotate.is_some() || translate.is_some() {
                if comms.location != Location::Vessel(id) || id != sim.ship().id() {
                    return Err("the rotation and translation keys are the pilot's: go aboard the active vessel".into());
                }
                effects.push(Effect::Hold { vessel: id, rotate, translate });
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
            // The thrusters translate only with RCS on (the keys' rule).
            let rcs_on = rcs.map_or(sim.controls().rcs.on, |m| m.on);
            let warning = (translate.is_some_and(|t| t != DVec3::ZERO) && !rcs_on)
                .then_some("RCS is off: the thrusters translate only with RCS on (`rcs: \"on\"`)");
            let rcs = rcs.map(|m| json!({"mode": rcs_name(m), "fine": m.fine}));
            let reply = json!({"id": id.0, "throttle": throttle, "sas": sas, "hold": hold, "speed_reference": speed,
                               "rotate": args.get("rotate"), "translate": args.get("translate"), "rcs": rcs,
                               "arrives_in_s": arrives, "warning": warning});
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
        "edit_stages" => edit_stages(args, sim, comms)?,
        "edit_program" => program::edit_program(args, sim, comms)?,
        "set_notify" => {
            let id = vessel_or_active(args, sim)?;
            let i = sim.index_of(id).ok_or("no such vessel")?;
            let on = args.get("on").and_then(Value::as_bool).ok_or("`on` is true or false")?;
            let was = sim.fleet[i].notify();
            (
                json!({"id": id.0, "notify": on, "was": was}),
                vec![Effect::Command(GameCommand::SetNotify { vessel: id, on })],
            )
        }
        "set_switch" => switches::set_switch(args, sim, comms)?,
        "press" => switches::press(args, sim, comms)?,
        "configure_program" => switches::configure(args, sim, comms)?,
        "set_target" => {
            let t = target_arg(args, sim)?;
            (json!({"target": target_json(sim, t)}), vec![Effect::Target(t)])
        }
        "notifications" => {
            let after = args.get("after").and_then(Value::as_u64).unwrap_or(0);
            (notifications_json(seen.toasts, after), Vec::new())
        }
        "read" => (read::read(args, sim, &seen)?, Vec::new()),
        _ => return Err(format!("unknown tool {tool}")),
    })
}

/// `set_controls`' `rotate` or `translate` (`key`): three body-axis
/// values each in −1–1, or `None` when not given.
fn axes(args: &Value, key: &str) -> Result<Option<DVec3>, String> {
    let Some(v) = args.get(key) else { return Ok(None) };
    let r: Vec<f64> = v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    let [x, y, z] = r[..] else { return Err(format!("`{key}` is three numbers")) };
    if [x, y, z].iter().any(|c| !(-1.0..=1.0).contains(c)) {
        return Err(format!("`{key}`'s axes are each in −1–1"));
    }
    Ok(Some(DVec3::new(x, y, z)))
}

/// The pilot's RCS modes by name (`set_controls`' `rcs`).
pub(crate) const RCS_NAMES: [(&str, Option<RcsOnly>); 4] = [
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

/// The functions a piece offers now, as the action bar and the systems
/// screen offer them: those its parts still aboard take (`program::view`),
/// an abort only while it would save the crew (`program::offered`, D120).
fn functions_now<'a>(def: &ShipDef, state: &'a ShipState) -> Vec<&'a str> {
    let list = &state.functions;
    (sim::craft::program::view(def, list).into_iter())
        .filter(|v| sim::craft::program::offered(def, state, &list[v.index]).is_ok())
        .map(|v| list[v.index].name.as_str())
        .collect()
}

/// The function `name` (any case) if the piece offers it now; else why not.
fn function_named<'a>(def: &ShipDef, state: &'a ShipState, name: &str) -> Result<&'a str, String> {
    if let Some(f) = functions_now(def, state).into_iter().find(|f| f.eq_ignore_ascii_case(name)) {
        return Ok(f);
    }
    let now = functions_now(def, state);
    let Some(f) = state.functions.iter().find(|f| f.name.eq_ignore_ascii_case(name)) else {
        return Err(format!("no function `{name}`: it has {now:?}"));
    };
    match sim::craft::program::offered(def, state, f) {
        Err(why) => Err(format!("`{}` would save no one here: {}", f.name, why.describe())),
        Ok(()) => Err(format!("`{}` acts on nothing aboard this piece: it has {now:?}", f.name)),
    }
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
    let functions = functions_now(&def, state);
    if let Some(name) = s("function") {
        let name = function_named(&def, state, name)?;
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
    let id = vessel_or_active(args, sim)?;
    let i = sim.index_of(id).ok_or("no such vessel")?;
    let signal = comms.signal(id).ok_or_else(|| {
        let heard = comms.last_contact.get(&id).map(|t| epoch_text(*t)).unwrap_or_else(|| "never".into());
        format!("no signal from vessel {} at {} (last heard {heard})", id.0, comms.location_name())
    })?;
    let v = &sim.fleet[i];
    let air = match args.get("air").and_then(Value::as_str) {
        None | Some("vacuum") => stage_bar::Air::Vacuum,
        Some("current") => stage_bar::Air::Current,
        Some(other) => return Err(format!("`air` is vacuum or current, not `{other}`")),
    };
    // What the vessel knows (D106): a dash (null) where no working sensor
    // measures it, and each value to its precision, by the menus' rules;
    // the truth in debug mode.
    let known = crate::format::Known::of(v);
    let body = sim.dominant_of(i);
    let (anchor, r, vel) = v.state_at(&sim.world, sim.clock);
    let k = sim.world.snapshot(sim.clock).relative(body, anchor);
    let (rel, v_rel) = (comms::retarded(r - k.r, vel - k.v, signal.delay), vel - k.v);
    let orbit = crate::relations::orbit_about(&sim.world, sim.clock, anchor, r, vel, body);
    let radius = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
    // Apsides from the predicted trajectory (D076), not the conic: the
    // line's segments (D089).
    let segs = crate::trajectory::apsides::vessel_segments(sim, prediction, i);
    let aps = crate::trajectory::apsides::of_segments(sim, &segs, v.nodes());
    let (ap, pe) = aps.next_after(sim.clock);
    let upcoming: Vec<Value> = aps
        .list
        .iter()
        .filter(|a| a.t.seconds_since(sim.clock) > 0.0)
        .take(8)
        .map(|a| {
            json!({"kind": if a.is_apo { "apoapsis" } else { "periapsis" }, "body": name(a.body),
                   "altitude_m": known_value(&known, Quantity::Position, a.altitude),
                   "in_s": known_time(&known, a.t.seconds_since(sim.clock)), "near_circular": a.near_circular})
        })
        .collect();
    let impact = aps.impact.filter(|m| m.t.seconds_since(sim.clock) > 0.0);
    let live = i == sim.active && matches!(v.phase, Phase::Powered { .. });
    let age = prediction_age(sim.clock, live, prediction.segments.first().map(|s| s.t0));
    let at = |x: f64| known_value(&known, Quantity::Position, x);
    let orbit = orbit.map(|o| {
        let (peri, apo) = two_body_apsides(o.elements.a, o.elements.e);
        json!({"periapsis_altitude_m": pe.and_then(|a| at(a.altitude)),
               "apoapsis_altitude_m": ap.and_then(|a| at(a.altitude)),
               "prediction_age_s": age,
               "now_two_body": {"periapsis_from_centre_m": at(peri), "apoapsis_from_centre_m": apo.and_then(at),
                                "semi_major_axis_m": at(o.elements.a)},
               "apsides": upcoming,
               "impact": impact.map(|m| json!({"body": name(m.body),
                                                "in_s": known_time(&known, m.t.seconds_since(sim.clock))})),
               "inclination_deg": known_digits(&known, o.elements.i.to_degrees(), crate::format::DEGREE_DECIMALS),
               "eccentricity": known_digits(&known, o.elements.e, crate::format::ECCENTRICITY_DECIMALS),
               "period_s": o.period().and_then(|p| known_time(&known, p))})
    });
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
        "altitude_m": known_altitude(&known, altitude),
        "orbit": orbit.filter(|_| known.orbit()), "attitude": attitude_json(&known, v.attitude.q, rel, pole),
        "stages": stages_json(sim, i, &known, air)?, "signal": signal_json(Some(signal)),
    });
    Ok((out, Vec::new()))
}

/// How old the predicted path is (s) while the active vessel flies live
/// (`live`): it starts from the state when it was computed (`start`), a
/// few times a second of real time, so a reading just after a `step` under
/// thrust lags the clock. None while the stored trajectory gives the path.
fn prediction_age(clock: sim::time::Epoch, live: bool, start: Option<sim::time::Epoch>) -> Option<f64> {
    start.filter(|_| live).map(|t0| clock.seconds_since(t0).max(0.0))
}

/// The two-body apsides from the centre (m) of an orbit of semi-major axis
/// `a` and eccentricity `e`: the periapsis, and the apoapsis if it is
/// bound.
fn two_body_apsides(a: f64, e: f64) -> (f64, Option<f64>) {
    if e < 1.0 {
        (a * (1.0 - e), Some(a * (1.0 + e)))
    } else {
        (a * (1.0 - e), None)
    }
}

/// A value as the vessel knows it: shown to its precision, or null.
fn known_value(known: &crate::format::Known, q: Quantity, truth: f64) -> Option<f64> {
    known.knowledge.read(q, truth).map(|r| r.shown())
}

/// An altitude above the ellipsoid (m) as the vessel knows it: through an
/// altimeter's reading, else the position's (`get_vessel`, and `step`'s
/// and `run_until`'s stop and answer).
fn known_altitude(known: &crate::format::Known, altitude: f64) -> Option<f64> {
    known_value(known, Quantity::Altitude, altitude).or_else(|| known_value(known, Quantity::Position, altitude))
}

/// A time from the orbit (an apsis's, an impact's, a period, a closest
/// approach's): known with the position and the velocity, as the menus
/// show it (`Known::orbit`).
fn known_time(known: &crate::format::Known, t: f64) -> Option<f64> {
    known.orbit().then(|| seconds(t))
}

/// The decimals the agent's times are given to: the clock's millisecond
/// (`get_state`'s clock, `epoch_text`).
pub(crate) const SECOND_DECIMALS: usize = 3;

/// A time in seconds to the clock's millisecond (it read
/// 118.44442996130809).
pub(crate) fn seconds(t: f64) -> f64 {
    digits(t, SECOND_DECIMALS)
}

/// A number of the orbit the menus show to `decimals` once it is known
/// (its angles, `format::DEGREE_DECIMALS`; its eccentricity).
fn known_digits(known: &crate::format::Known, x: f64, decimals: usize) -> Option<f64> {
    known.orbit().then(|| digits(x, decimals))
}

/// `x` to `decimals`, as a menu shows it.
fn digits(x: f64, decimals: usize) -> f64 {
    let scale = 10f64.powi(decimals as i32);
    (x * scale).round() / scale
}

/// A place on the ground (a latitude or longitude, rad) of a body of
/// `radius` (m), in degrees to the position's precision along the ground
/// and at most the Landing menu's AT's decimals (`format::GROUND_DECIMALS`;
/// it read 15 digits); known with the orbit, as AT.
fn known_ground(known: &crate::format::Known, angle: f64, radius: f64) -> Option<f64> {
    let r = known.knowledge.read(Quantity::Position, angle * radius).filter(|_| known.orbit())?;
    Some(digits((r.shown() / radius).to_degrees(), crate::format::GROUND_DECIMALS))
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
    // The angles to the Landing menu's decimals; an axis's components (of a
    // unit vector, so each off by about the angle's error) to the attitude's
    // precision. They read raw digits (`heading_deg` 9.340000000000003).
    let deg = |x: f64| digits(x, crate::format::ATTITUDE_DECIMALS);
    let enu = |d: DVec3| {
        [d.dot(local.east), d.dot(local.north), d.dot(local.up)].map(|c| known_value(known, Quantity::Attitude, c))
    };
    json!({"heading_deg": deg(a.heading), "pitch_deg": deg(a.pitch), "roll_deg": deg(a.roll),
           "axes_enu": {"x": enu(q * DVec3::X), "y": enu(q * DVec3::Y), "z": enu(q * DVec3::Z)}})
}

/// A craft of parts' sequence and propellant as it knows them: the next
/// stage, each tank's level (kg, null where unmeasured) and each engine's
/// state.
fn stages_json(sim: &SimState, i: usize, known: &crate::format::Known, air: stage_bar::Air) -> Result<Value, String> {
    let v = &sim.fleet[i];
    let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else { return Ok(Value::Null) };
    let stack = crate::staging::parts::stack(&def, state, v.controls().chute);
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
    let estimates = stage_bar::estimates_of(sim, i, air).unwrap_or_default();
    let sequence: Vec<Value> = stage_bar::rows_of(v, &estimates).unwrap_or_default().iter().map(row_json).collect();
    Ok(json!({"next": next, "next_name": next_name, "feeding_share": session::feeding_share(v),
           "tanks": tanks, "engines": engines, "sequence": sequence}))
}

/// A staging bar row as the agent reads it (`get_vessel`'s
/// `stages.sequence`): what the player sees on it.
fn row_json(r: &rows::Row) -> Value {
    let place = match r.place {
        rows::Place::Active => "active",
        rows::Place::Next => "next",
        rows::Place::Later => "later",
        rows::Place::Done => "done",
    };
    let commands: Vec<Value> = r.items.iter().map(|i| json!({"command": i.action, "label": i.label})).collect();
    let figures = r.figures.as_ref().map(|f| {
        let propellant: Vec<Value> = f.bars.iter().map(|b| json!({"fluid": b.fluid, "held": b.text})).collect();
        let active = f.active.as_ref().map(|a| json!({"thrust": a.thrust, "twr": a.twr, "mass": a.mass, "isp": a.isp}));
        json!({"dv": f.dv, "burnout": f.burnout, "propellant": propellant, "active": active})
    });
    json!({"stage": r.stage, "number": r.number, "name": r.name, "place": place, "commands": commands,
           "figures": figures})
}

/// The `edit_stages` tool: one of the staging bar's edits (`rows::check`,
/// the bar's own rules), sent like any command.
fn edit_stages(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let (id, arrives) = reach(args, sim, comms)?;
    let v = &sim.fleet[sim.index_of(id).ok_or("no such vessel")?];
    let shown = stage_bar::rows_of(v, &Default::default())
        .ok_or_else(|| format!("vessel {} is not a craft of parts (it has no stage sequence)", id.0))?;
    let op = stage_op(args)?;
    let command = match rows::check(op, &shown)? {
        rows::Sent::Edit(e) => GameCommand::Ship { vessel: id, commands: vec![ShipCommand::Sequence(e)] },
        rows::Sent::Again(stage) => GameCommand::StageAgain { vessel: id, stage },
    };
    Ok((json!({"id": id.0, "edit": format!("{op:?}"), "arrives_in_s": arrives}), vec![Effect::Command(command)]))
}

/// `edit_stages`' one edit from its arguments.
fn stage_op(args: &Value) -> Result<rows::Op, String> {
    let n = |v: &Value, k: &str| -> Result<usize, String> {
        v.get(k).and_then(Value::as_u64).map(|n| n as usize).ok_or_else(|| format!("missing integer `{k}`"))
    };
    let given: Vec<&str> =
        ["move", "move_command", "delete", "again"].into_iter().filter(|k| args.get(*k).is_some()).collect();
    match given[..] {
        ["move"] => Ok(rows::Op::Move { stage: n(&args["move"], "stage")?, to: n(&args["move"], "to")? }),
        ["move_command"] => {
            let m = &args["move_command"];
            Ok(rows::Op::MoveCommand { stage: n(m, "stage")?, command: n(m, "command")?, to: n(m, "to")? })
        }
        ["delete"] => Ok(rows::Op::Delete(n(args, "delete")?)),
        ["again"] => Ok(rows::Op::Again(n(args, "again")?)),
        _ => Err("one edit a call: `move`, `move_command`, `delete` or `again`".into()),
    }
}

/// `set_target`'s target, one of the picker's choices
/// (`navball::target_choices`); `None` for `none`.
fn target_arg(args: &Value, sim: &SimState) -> Result<Option<NavTarget>, String> {
    let choices = crate::navball::target_choices(sim);
    let body = args.get("body").and_then(Value::as_str);
    let vessel = args.get("vessel").and_then(Value::as_u64);
    let none = args.get("none").and_then(Value::as_bool) == Some(true);
    let wanted = match (body, vessel, none) {
        (Some(name), None, false) => (choices.iter())
            .find(|t| matches!(t, NavTarget::Body(n) if session::same_name(&sim.world.eph.node(*n).name, name)))
            .copied()
            .ok_or_else(|| format!("no body with a surface named `{name}`"))?,
        (None, Some(id), false) => (choices.iter())
            .find(|t| **t == NavTarget::Vessel(VesselId(id)))
            .copied()
            .ok_or_else(|| format!("vessel {id} cannot be targeted: no such vessel, or it is the active one"))?,
        (None, None, true) => return Ok(None),
        _ => return Err("one of `vessel`, `body` or `none: true`".into()),
    };
    Ok(Some(wanted))
}

/// A target as the agent reads it.
fn target_json(sim: &SimState, t: Option<NavTarget>) -> Value {
    match t {
        None => Value::Null,
        Some(NavTarget::Body(n)) => json!({"body": sim.world.eph.node(n).name}),
        Some(NavTarget::Vessel(id)) => json!({"vessel": id.0}),
    }
}

/// The `notifications` tool: the messages after number `after`.
fn notifications_json(toasts: Option<&Toasts>, after: u64) -> Value {
    let Some(toasts) = toasts else { return json!({"count": 0, "messages": []}) };
    let messages: Vec<Value> = toasts
        .since(after)
        .map(|(n, t)| {
            let vessel = t.link.map(|Link::Vessel(id)| id.0);
            json!({"n": n, "text": t.text, "error": t.error, "vessel": vessel})
        })
        .collect();
    json!({"count": toasts.count(), "messages": messages})
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
    // Each number as the Landing menu shows it (D106): times and the place
    // with the orbit, heights through the position's reading, speeds
    // through the velocity's, the braking burn with the propellant too.
    let (at, speed) =
        (|x: f64| known_value(&known, Quantity::Position, x), |v: f64| known_value(&known, Quantity::Velocity, v));
    let impact = l.impact.map(|h| {
        let radius = sim.world.source(h.body).and_then(|s| s.physical.as_ref()).map_or(1.0, |p| p.radius_eq);
        json!({
            "in_s": known_time(&known, h.t.seconds_since(sim.clock)), "body": name(h.body),
            "lat_deg": known_ground(&known, h.lat, radius), "lon_deg": known_ground(&known, h.lon, radius),
            "ground_height_m": at(h.height),
            "speed_ms": speed(h.speed), "vertical_ms": speed(h.v_vertical), "horizontal_ms": speed(h.v_horizontal),
        })
    });
    let burn = |x: Option<f64>| x.filter(|_| known.propellant);
    let braking = l.braking.map(|b| {
        json!({
            "ignite_in_s": burn(known_time(&known, b.t_ignite.seconds_since(sim.clock))),
            "ignite_altitude_m": burn(at(b.ignite_height)),
            "stop_in_s": burn(known_time(&known, b.t_stop.seconds_since(sim.clock))),
            "stop_height_m": burn(at(b.stop_height)),
        })
    });
    let twr = crate::interface::menus::data::local_twr(ship, m.pressure, m.gravity);
    let surface = json!({
        "body": name(start.body), "radar_altitude_m": known_value(&known, Quantity::Altitude, m.height - ship.contact_height()),
        "vertical_ms": speed(m.v_vertical), "horizontal_ms": speed(m.v_horizontal),
        "twr": twr.shown(),
    });
    json!({"impact": impact, "braking": braking, "surface": surface})
}

/// The `closest_approaches` tool: vessel `a`'s approaches to `b`, as the
/// Orbital menu shows the active vessel's to its target (D106): both seen
/// from the agent's location, each number through `a`'s knowledge.
fn approaches(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let get = |k: &str| args.get(k).and_then(Value::as_u64).map(VesselId).ok_or(format!("missing `{k}`"));
    let (a, b) = (get("a")?, get("b")?);
    for id in [a, b] {
        sim.index_of(id).ok_or(format!("no vessel {}", id.0))?;
        if comms.location != Location::Vessel(id) && comms.signal(id).is_none() {
            return Err(format!("no signal from vessel {} at {}", id.0, comms.location_name()));
        }
    }
    let known = crate::format::Known::of(&sim.fleet[sim.index_of(a).expect("checked")]);
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
        .map(|c| {
            json!({"in_s": known_time(&known, c.t.seconds_since(sim.clock)),
                   "distance_m": known_value(&known, Quantity::Position, c.distance),
                   "speed_ms": known_value(&known, Quantity::Velocity, c.speed)})
        })
        .collect();
    Ok((json!({"approaches": out}), Vec::new()))
}

/// The `set_plan` tool.
fn set_plan(args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
    let name = |n: sim::frame::NodeId| sim.world.eph.node(n).name.clone();
    let id = vessel_or_active(args, sim)?;
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
    let command = crate::planner::command_for(vessel, &draft);
    // The prediction of each node (the planner's).
    let (info, available) = crate::planner::draft_info(&sim.world, vessel, &draft);
    let nodes: Vec<Value> = draft
        .iter()
        .zip(&info)
        .map(|(d, b)| {
            json!({"node_in_s": seconds(d.t_start.seconds_since(sim.clock)),
                "ignite_in_s": seconds(b.ignite.seconds_since(sim.clock)), "duration_s": b.duration.map(seconds),
                "warnings": b.warnings})
        })
        .collect();
    let effect = if delay > 0.0 { Effect::Send { delay, command } } else { Effect::Command(command) };
    // The Δv available as the planner shows it, to the m/s.
    let out = json!({"sent": draft.len(), "arrives_in_s": seconds(delay), "about": name(body), "nodes": nodes,
        "available_dv_ms": available.map(|a| digits(a, 0))});
    Ok((out, vec![effect]))
}

/// The running server, if the setting is on.
#[derive(Default)]
pub struct AgentServer {
    server: Option<(Server, u16, String)>,
    error: Option<String>,
    env: Option<Option<(u16, String)>>,
    /// Menu reads waiting for the stopped clock's snapshot
    /// ([`read::fresh`]), with the frames waited.
    waiting: Vec<(mcp::Call, u32)>,
}

/// The most frames a `read {menu}` waits for the stopped clock's snapshot
/// (it is refreshed every 0.1 s of real time).
const READ_WAIT_FRAMES: u32 = 60;

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
    mut held: ResMut<HeldKeys>,
    (mut nav, toasts, data): (ResMut<crate::navball::Navball>, Res<Toasts>, Res<crate::interface::menus::FlightData>),
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
    let AgentServer { server, waiting, .. } = &mut *state;
    let Some((server, _, _)) = server.as_ref() else { return };
    let mut calls = std::mem::take(waiting);
    calls.extend(server.poll().into_iter().map(|c| (c, 0)));
    for (call, frames) in calls {
        if session::handles(&call.tool) {
            session.incoming.push(call);
            continue;
        }
        // A menu's page from the stopped clock's snapshot, the same on
        // every replay.
        let menu = call.tool == "read" && call.arguments.get("menu").is_some();
        if menu && frames < READ_WAIT_FRAMES && !read::fresh(sim.paused, data.clock, sim.clock) {
            waiting.push((call, frames + 1));
            continue;
        }
        let seen = Seen {
            target: nav.target,
            toasts: Some(&toasts),
            flight: Some(&data),
            nav: nav.now.as_ref(),
            station: station.open,
        };
        let (result, effects) = answer_seen(&call.tool, &call.arguments, &sim, &comms, &pred, seen);
        for e in effects {
            match e {
                Effect::Command(c) => {
                    commands.write(c);
                }
                Effect::Station(open) => station.open = open,
                Effect::Send { delay, command } => in_flight.send(sim.clock, delay, command),
                Effect::Hold { vessel, rotate, translate } => held.set(vessel, rotate, translate),
                Effect::Target(t) => nav.set_target(t),
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
    use sim::craft::command::SequenceEdit;

    fn setup() -> (SimState, Comms) {
        let mut sim = SimState::new();
        sim.spawn_test_ships(3);
        (sim, Comms::load())
    }

    #[test]
    fn under_thrust_the_vessel_says_how_old_its_predicted_apsides_are() {
        let t = |s: f64| sim::sol::sol_epoch().add_seconds(s);
        // (flies live, the predicted path's start, the clock, age)
        let cases = [
            (true, Some(t(100.0)), t(103.5), Some(3.5)), // a `step` under thrust just ended
            (true, Some(t(103.5)), t(103.5), Some(0.0)), // caught up
            (false, Some(t(100.0)), t(103.5), None),     // coasting: the stored trajectory
            (true, None, t(103.5), None),                // no prediction yet
        ];
        for (live, start, clock, want) in cases {
            assert_eq!(prediction_age(clock, live, start), want, "{live} {start:?}");
        }
        // The two-body apsides from the centre: bound, circular, escaping.
        // (a, e, periapsis, apoapsis)
        let orbits = [(7.0e6, 0.1, 6.3e6, Some(7.7e6)), (7.0e6, 0.0, 7.0e6, Some(7.0e6)), (-2.0e7, 1.5, 1.0e7, None)];
        for (a, e, peri, apo) in orbits {
            let (p, q) = two_body_apsides(a, e);
            assert!((p - peri).abs() < 1e-6, "{a} {e}: {p}");
            assert_eq!(q.is_some(), apo.is_some(), "{a} {e}");
            if let (Some(q), Some(apo)) = (q, apo) {
                assert!((q - apo).abs() < 1e-6, "{a} {e}: {q}");
            }
        }
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
        let rotate = Effect::Hold { vessel: active, rotate: Some(DVec3::new(0.04, 0.0, 0.0)), translate: None };
        assert_eq!(e, vec![rotate, Effect::Command(set)]);
        // (arguments, the error's words)
        let bad = [
            (json!({"rotate": [2.0, 0.0, 0.0]}), "−1–1"),
            (json!({"rotate": [0.0, 0.0]}), "three numbers"),
            (json!({"translate": [0.0, 1.5, 0.0]}), "`translate`'s axes"),
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
        let (r, _) = answer("set_controls", &json!({"translate": [0.0, 0.0, 1.0]}), &sim, &comms, &Default::default());
        assert!(matches!(r, ToolResult::Err(e) if e.contains("aboard")));
    }

    #[test]
    fn the_translation_keys_are_held_beside_the_rotation_keys() {
        let (sim, mut comms) = setup();
        let active = sim.ship().id();
        comms.location = Location::Vessel(active);
        // RCS off: held, with a warning that nothing translates yet; with
        // `rcs: on` in the same call, none.
        let args = json!({"translate": [0.0, 0.0, 1.0]});
        let (ToolResult::Ok(v), e) = answer("set_controls", &args, &sim, &comms, &Default::default()) else { panic!() };
        assert!(v["warning"].as_str().is_some_and(|w| w.contains("RCS is off")), "{v}");
        assert_eq!(e[0], Effect::Hold { vessel: active, rotate: None, translate: Some(DVec3::Z) });
        let args = json!({"translate": [0.0, 0.0, 1.0], "rcs": "on"});
        let (ToolResult::Ok(v), _) = answer("set_controls", &args, &sim, &comms, &Default::default()) else { panic!() };
        assert!(v["warning"].is_null(), "{v}");
        // (held before, set: rotate, translate) → held after; `None` keeps
        // what is held, another vessel's keys are let go, all zero is none.
        let (a, b) = (VesselId(1), VesselId(2));
        let k = |r: DVec3, t: DVec3| Keys { rotate: r, translate: t };
        let cases = [
            (None, (a, Some(DVec3::X), None), Some((a, k(DVec3::X, DVec3::ZERO)))),
            (Some((a, k(DVec3::X, DVec3::ZERO))), (a, None, Some(DVec3::Z)), Some((a, k(DVec3::X, DVec3::Z)))),
            (Some((a, k(DVec3::X, DVec3::Z))), (a, Some(DVec3::ZERO), None), Some((a, k(DVec3::ZERO, DVec3::Z)))),
            (Some((a, k(DVec3::X, DVec3::Z))), (b, None, Some(DVec3::Y)), Some((b, k(DVec3::ZERO, DVec3::Y)))),
            (Some((a, k(DVec3::X, DVec3::ZERO))), (a, Some(DVec3::ZERO), Some(DVec3::ZERO)), None),
        ];
        for (before, (id, r, t), after) in cases {
            let mut held = HeldKeys(before);
            held.set(id, r, t);
            assert_eq!(held.0, after, "{before:?} {r:?} {t:?}");
        }
        let held = HeldKeys(Some((a, k(DVec3::X, DVec3::Y))));
        assert_eq!((held.of(a).translate, held.of(VesselId(999))), (DVec3::Y, Keys::default()));
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
        let bodies = |sim: &SimState| match answer("list_bodies", &json!({}), sim, &comms, &Default::default()) {
            (ToolResult::Ok(v), _) => v["bodies"].as_array().unwrap().clone(),
            (r, _) => panic!("{r:?}"),
        };
        let earth = |list: Vec<Value>| list.into_iter().find(|b| b["name"] == "Earth").unwrap();
        assert!(earth(bodies(&sim))["distance_m"].is_null(), "a blind craft knows no distance");
        let s = &mut sim;
        s.fleet[s.active].set_debug(&s.world, true);
        let d = earth(bodies(&sim))["distance_m"].as_f64().unwrap();
        assert!((6.3e6..6.4e6).contains(&d), "the pad from Earth's centre: {d}");
        let seen = get(&sim);
        assert_eq!(seen["position_m"].as_array().map(Vec::len), Some(3));
        assert!(seen["altitude_m"].as_f64().is_some_and(|a| a.abs() < 100.0), "on the pad: {}", seen["altitude_m"]);
        assert!(seen["attitude"]["pitch_deg"].as_f64().is_some_and(|p| p > 80.0), "upright: {}", seen["attitude"]);
        assert!(seen["stages"]["tanks"][0]["liquid_kg"].as_f64().is_some());
        assert!(seen["stages"]["next"].is_u64(), "{}", seen["stages"]);
    }

    #[test]
    fn the_attitude_reads_to_the_menus_digits_and_its_precision() {
        use crate::format::Known;
        use sim::systems::Knowledge;
        let mut k = Knowledge::default();
        k.learn(Quantity::Attitude, 2e-4);
        let known = Known::from(k);
        let q = glam::DQuat::from_rotation_y(0.3) * glam::DQuat::from_rotation_x(0.163);
        let (rel, pole) = (DVec3::X * 6.4e6, DVec3::Z);
        let v = attitude_json(&known, q, rel, pole);
        // The angles to the Landing menu's tenth of a degree; the axes'
        // components to the attitude's 1e-4 (the decade of its 2e-4).
        let to = |x: &Value, scale: f64| {
            let x = x.as_f64().unwrap() * scale;
            (x - x.round()).abs() < 1e-6
        };
        for key in ["heading_deg", "pitch_deg", "roll_deg"] {
            assert!(to(&v[key], 10.0), "{key}: {v}");
        }
        for axis in ["x", "y", "z"] {
            assert!(v["axes_enu"][axis].as_array().unwrap().iter().all(|c| to(c, 1e4)), "{axis}: {v}");
        }
        assert_eq!(attitude_json(&Known::from(Knowledge::default()), q, rel, pole), Value::Null, "no attitude sensor");
    }

    #[test]
    fn get_vessel_and_the_plan_default_to_the_active_vessel() {
        let (sim, mut comms) = setup();
        let active = sim.ship().id();
        comms.signals.insert(active, Some(comms::Signal { delay: 0.0, rate: 1e6, via: vec![] }));
        assert_eq!(vessel_or_active(&json!({}), &sim), Ok(active));
        assert_eq!(vessel_or_active(&json!({"id": 7}), &sim), Ok(VesselId(7)));
        assert!(vessel_or_active(&json!({"id": "seven"}), &sim).is_err());
        let read = |args: Value| match answer("get_vessel", &args, &sim, &comms, &Default::default()).0 {
            ToolResult::Ok(v) => v,
            _ => panic!("get_vessel {args}"),
        };
        assert_eq!(read(json!({})), read(json!({"id": active.0})));
    }

    #[test]
    fn the_orbits_and_the_landings_numbers_read_as_the_menus_show_them() {
        use crate::format::Known;
        use sim::systems::Knowledge;
        let known = |qs: &[(Quantity, f64)]| {
            let mut k = Knowledge::default();
            for (q, p) in qs {
                k.learn(*q, *p);
            }
            Known::from(k)
        };
        let blind = known(&[]);
        let position = known(&[(Quantity::Position, 25.0)]);
        let fixed = known(&[(Quantity::Position, 25.0), (Quantity::Velocity, 0.05)]);
        let truth = Known::from(Knowledge::perfect());
        // Times and the orbit's numbers: with the position and the velocity.
        // (known, a time, the eccentricity 0.123456789 to 5 decimals)
        let cases = [(&blind, None, None), (&position, None, None), (&fixed, Some(12.5), Some(0.12346))];
        for (k, time, e) in cases {
            assert_eq!(known_time(k, 12.5), time);
            assert_eq!(known_digits(k, 0.123_456_789, crate::format::ECCENTRICITY_DECIMALS), e);
        }
        // Times to the clock's millisecond, without the binary noise they
        // read (finding 76).
        // (seconds, as the agent reads them)
        let times =
            [(118.444_429_961_308_09, 118.444), (0.099_999_999_999_999_98, 0.1), (4.58e-8, 0.0), (-12.0005, -12.001)];
        for (t, want) in times {
            assert_eq!(seconds(t), want, "{t}");
        }
        assert_eq!(known_time(&fixed, 118.444_429_961_308_09), Some(118.444));
        // A place on the ground: to the position's precision along it and
        // the Landing menu's AT's decimals, in debug mode too.
        let r = 6.378e6;
        let lat = 0.5 + 3.0 / r; // 3 m north of 0.5 rad (28.64789°)
        assert_eq!(known_ground(&fixed, lat, r), Some(28.648));
        assert_eq!(known_ground(&truth, lat, r), Some(28.648));
        // A position known to ±25 km: its 10 km decade, 0.09° on Earth.
        let coarse = known(&[(Quantity::Position, 25_000.0), (Quantity::Velocity, 1.0)]);
        assert_eq!(known_ground(&coarse, lat, r), Some(28.657));
        assert_eq!(known_ground(&position, lat, r), None, "an impact needs the orbit");
        // A distance and a speed through their readings.
        assert_eq!(known_value(&fixed, Quantity::Position, 1234.0), Some(1230.0));
        assert_eq!(known_value(&fixed, Quantity::Velocity, 7.6543), Some(7.65));
    }

    #[test]
    fn closest_approaches_need_a_signal_to_both_vessels() {
        let (sim, mut comms) = setup();
        let (a, b) = (sim.fleet[0].id(), sim.fleet[1].id());
        let args = json!({"a": a.0, "b": b.0});
        comms.location = Location::Site(0);
        comms.signals.insert(a, Some(comms::Signal { delay: 0.0, rate: 1e6, via: vec![] }));
        comms.signals.insert(b, None);
        let (r, _) = answer("closest_approaches", &args, &sim, &comms, &Default::default());
        assert!(matches!(&r, ToolResult::Err(e) if e.contains(&format!("no signal from vessel {}", b.0))), "{r:?}");
        comms.signals.insert(b, Some(comms::Signal { delay: 0.0, rate: 1e6, via: vec![] }));
        let (r, _) = answer("closest_approaches", &args, &sim, &comms, &Default::default());
        assert!(!matches!(&r, ToolResult::Err(e) if e.contains("no signal")), "{r:?}");
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

    /// The agent's last ways in (interface-map.md finding 10): the target
    /// the Orbital menu's picker sets, read back by get_state.
    #[test]
    fn a_script_sets_the_target_from_the_pickers_choices() {
        let (sim, comms) = setup();
        let (me, other) = (sim.ship().id(), sim.fleet[1].id());
        let moon = sim.world.find("Moon").expect("the Moon").node;
        // (arguments, the target set or the refusal's words)
        let table: [(Value, Result<Option<NavTarget>, &str>); 7] = [
            (json!({"vessel": other.0}), Ok(Some(NavTarget::Vessel(other)))),
            (json!({"body": "moon"}), Ok(Some(NavTarget::Body(moon)))),
            (json!({"none": true}), Ok(None)),
            (json!({"vessel": me.0}), Err("cannot be targeted")),
            (json!({"body": "Vulcan"}), Err("no body")),
            (json!({}), Err("one of")),
            (json!({"vessel": other.0, "none": true}), Err("one of")),
        ];
        for (args, want) in table {
            match (answer("set_target", &args, &sim, &comms, &Default::default()), want) {
                ((ToolResult::Ok(_), e), Ok(t)) => assert_eq!(e, vec![Effect::Target(t)], "{args}"),
                ((ToolResult::Err(e), effects), Err(words)) => {
                    assert!(e.contains(words) && effects.is_empty(), "{args}: {e}")
                }
                (got, want) => panic!("{args}: {got:?}, want {want:?}"),
            }
        }
        let seen = Seen { target: Some(NavTarget::Body(moon)), ..Seen::default() };
        let (ToolResult::Ok(v), _) = answer_seen("get_state", &json!({}), &sim, &comms, &Default::default(), seen)
        else {
            panic!()
        };
        assert_eq!(v["target"], json!({"body": "Moon"}));
    }

    /// The staging bar's rows in get_vessel, and its edits by the bar's
    /// rules (`rows::check`), on the small rocket at the pad (Ignition
    /// next, Separation later).
    #[test]
    fn a_script_reads_and_edits_the_staging_bar() {
        let (sim, mut comms) = setup();
        let me = sim.ship().id();
        comms.location = Location::Vessel(me);
        comms.signals.insert(me, Some(comms::Signal { delay: 0.0, rate: 1e6, via: vec![] }));
        let v = match answer("get_vessel", &json!({"id": me.0}), &sim, &comms, &Default::default()) {
            (ToolResult::Ok(v), _) => v,
            (r, _) => panic!("{r:?}"),
        };
        let rows = v["stages"]["sequence"].as_array().expect("the bar's rows");
        let line = |r: &Value| (r["stage"].as_u64(), r["place"].as_str().unwrap().to_string());
        assert_eq!(rows.iter().map(line).collect::<Vec<_>>(), [(Some(1), "later".into()), (Some(0), "next".into())]);
        assert!(rows[1]["figures"]["dv"].is_string() && rows[1]["commands"][0]["label"].is_string(), "{}", rows[1]);
        let sequence =
            |e| vec![Effect::Command(GameCommand::Ship { vessel: me, commands: vec![ShipCommand::Sequence(e)] })];
        // (arguments, what is sent; empty: refused)
        let table = [
            (json!({"move": {"stage": 1, "to": 0}}), sequence(SequenceEdit::Move { from: 1, to: 0 })),
            (json!({"delete": 1}), sequence(SequenceEdit::Delete(1))),
            (json!({"again": 0}), vec![]),
            (json!({"delete": 1, "again": 0}), vec![]),
            (json!({"move": {"stage": 1}}), vec![]),
        ];
        for (args, want) in table {
            let (r, e) = answer("edit_stages", &args, &sim, &comms, &Default::default());
            assert_eq!(e, want, "{args}: {r:?}");
        }
    }

    /// The notifications as the player reads them, after the last seen.
    #[test]
    fn a_script_reads_the_notifications() {
        let (sim, comms) = setup();
        let mut toasts = Toasts::default();
        toasts.push(0.0, "Saved", false);
        toasts.push_linked(1.0, "Stage separation", false, Some(Link::Vessel(VesselId(2))));
        toasts.push(2.0, "Nothing to load", true);
        let seen = Seen { toasts: Some(&toasts), ..Seen::default() };
        let (ToolResult::Ok(v), e) =
            answer_seen("notifications", &json!({"after": 1}), &sim, &comms, &Default::default(), seen)
        else {
            panic!()
        };
        assert!(e.is_empty());
        assert_eq!(v["count"], 3);
        assert_eq!(
            v["messages"],
            json!([{"n": 2, "text": "Stage separation", "error": false, "vessel": 2},
                   {"n": 3, "text": "Nothing to load", "error": true, "vessel": null}])
        );
    }

    /// `command` offers the functions the action bar and the systems screen
    /// offer (interface-map.md finding 23): those the piece's parts still
    /// take, an abort only while it would save the crew (D120); the rest are
    /// refused with why.
    #[test]
    fn a_script_runs_only_the_functions_the_piece_offers() {
        let f9 = sim::craft::craft_of(&sim::craft::CraftId("falcon9-crew".into()));
        let def = SimState::with_craft(&f9).ship().ship_def().unwrap();
        let piece = |keep: &dyn Fn(&str) -> bool| {
            def.subset(&def.parts.iter().filter(|p| keep(&p.id)).map(|p| p.part).collect::<Vec<_>>())
        };
        let upper = piece(&|id| id.starts_with("s2.") || id.starts_with("dragon."));
        let dragon = piece(&|id| id.starts_with("dragon."));
        // (case, piece, grid fins listed, the abort listed)
        let table = [
            ("the stack", &*def, true, true),
            ("the upper stack", &upper, false, true),
            ("Dragon", &dragon, false, false),
        ];
        for (case, d, fins, abort) in table {
            let state = ShipState::initial(d);
            let now = functions_now(d, &state);
            assert_eq!((now.contains(&"Grid fins"), now.contains(&"Launch abort")), (fins, abort), "{case}: {now:?}");
            assert_eq!(function_named(d, &state, "launch abort").is_ok(), abort, "{case}");
        }
        // (piece, function, the refusal says)
        let refused = [
            (&upper, "grid fins", "acts on nothing aboard"),
            (&dragon, "Launch abort", "would save no one here"),
            (&dragon, "Warp drive", "no function"),
        ];
        for (d, f, says) in refused {
            let e = function_named(d, &ShipState::initial(d), f).unwrap_err();
            assert!(e.contains(says), "{f}: {e}");
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
