//! The player's AI agent (D044, D069): the MCP server's tools, answered from
//! the game each frame.
//!
//! The agent is at a control location like the player (D067): it sees
//! vessels as their light arrives there, and it cannot read a vessel with no
//! signal. Actions go through the same `GameCommand`s as the keyboard and
//! the UI; nothing is special-cased for the agent. Vessel-directed commands
//! (burn plans) will travel with the path's light delay.

use crate::commands::GameCommand;
use crate::comms::{self, Comms, Location};
use crate::interface::layout::InterfaceSettings;
use crate::state::{SimState, WARP_LEVELS};
use crate::tracking::TrackingStation;
use bevy::prelude::*;
use mcp::{Server, Tool, ToolResult};
use serde_json::{json, Value};
use sim::vessel::{Phase, VesselId};

/// What answering a call does besides answering.
#[derive(Debug, PartialEq)]
pub enum Effect {
    Command(GameCommand),
    /// Go to mission control (true) or aboard the active vessel (false).
    Station(bool),
    /// Throttle (0–1) and SAS of the active vessel, when aboard.
    Controls {
        throttle: Option<f64>,
        sas: Option<bool>,
    },
    /// A command sent over the network: it arrives after `delay` seconds.
    Send {
        delay: f64,
        command: GameCommand,
    },
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required})
}

/// The tools offered to the agent.
pub fn tools() -> Vec<Tool> {
    let t = |name: &str, description: &str, input_schema: Value| Tool {
        name: name.into(),
        description: description.into(),
        input_schema,
    };
    let id = json!({"type": "integer", "description": "Vessel id"});
    vec![
        t(
            "get_state",
            "Game clock (TDB), time warp, your control location, the active vessel, and every vessel's status and \
             signal at your location.",
            schema(json!({}), &[]),
        ),
        t("list_bodies", "Celestial bodies: name, GM (m³/s²), radius (m), what they orbit.", schema(json!({}), &[])),
        t(
            "get_vessel",
            "A vessel as seen from your location (light-delayed): position and velocity relative to the body it is \
             about (inertial axes, m and m/s), altitude, osculating orbit, signal. Fails without a signal.",
            schema(json!({"id": id}), &["id"]),
        ),
        t(
            "get_trajectory",
            "A vessel's predicted path from now: `points` samples (time from now in s, position relative to the \
             body it is about now, m).",
            schema(json!({"id": id, "points": {"type": "integer", "minimum": 2, "maximum": 1000}}), &["id"]),
        ),
        t(
            "set_warp",
            &format!(
                "Time warp level 0–{} (factors {:?}). Levels above 3 are rails warp.",
                WARP_LEVELS.len() - 1,
                WARP_LEVELS
            ),
            schema(json!({"level": {"type": "integer", "minimum": 0}}), &["level"]),
        ),
        t(
            "switch_vessel",
            "Go aboard a vessel: it becomes the active vessel and your control location.",
            schema(json!({"id": id}), &["id"]),
        ),
        t(
            "go_to_mission_control",
            "Make mission control (the tracking station) your control location.",
            schema(json!({}), &[]),
        ),
        t(
            "set_plan",
            "Replace a vessel's planned burns (not yet ignited). Each burn: `in_s` seconds from now, and Δv \
             components (m/s) prograde, normal and radial-out relative to the body the vessel is about now. \
             From anywhere but aboard, the plan travels with the signal's light delay; without a signal it \
             cannot be sent. Read the result with get_trajectory.",
            schema(
                json!({"id": id, "burns": {"type": "array", "items": {"type": "object", "properties": {
                    "in_s": {"type": "number", "minimum": 0},
                    "prograde": {"type": "number"}, "normal": {"type": "number"}, "radial": {"type": "number"}},
                    "required": ["in_s"]}}}),
                &["id", "burns"],
            ),
        ),
        t(
            "set_controls",
            "Throttle (0–1) and SAS of the active vessel. Only aboard it (as its crew).",
            schema(
                json!({"throttle": {"type": "number", "minimum": 0, "maximum": 1}, "sas": {"type": "boolean"}}),
                &[],
            ),
        ),
    ]
}

fn epoch_text(t: sim::time::Epoch) -> String {
    let (y, mo, d, h, mi, s) = t.to_calendar();
    format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:06.3} TDB")
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

/// Answers one call: the result, and what the game must do.
pub fn answer(tool: &str, args: &Value, sim: &SimState, comms: &Comms) -> (ToolResult, Vec<Effect>) {
    match run(tool, args, sim, comms) {
        Ok((v, effects)) => (ToolResult::Ok(v), effects),
        Err(e) => (ToolResult::Err(e), Vec::new()),
    }
}

fn run(tool: &str, args: &Value, sim: &SimState, comms: &Comms) -> Result<(Value, Vec<Effect>), String> {
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
                "warp_level": sim.effective_warp(), "warp_factor": WARP_LEVELS[sim.effective_warp()],
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
        "get_vessel" => {
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
            let orbit = orbit.map(|o| {
                json!({"periapsis_altitude_m": o.elements.periapsis() - radius,
                       "apoapsis_altitude_m": o.elements.apoapsis() - radius,
                       "inclination_deg": o.elements.i.to_degrees(), "eccentricity": o.elements.e,
                       "period_s": o.period()})
            });
            let out = json!({
                "id": id.0, "status": status(&v.phase), "about": name(body), "seen_delay_s": signal.delay,
                "position_m": rel.to_array(), "velocity_ms": v_rel.to_array(),
                "altitude_m": rel.length() - radius, "orbit": orbit, "signal": signal_json(Some(signal)),
            });
            (out, Vec::new())
        }
        "get_trajectory" => {
            let id = vessel_id(args)?;
            let i = sim.index_of(id).ok_or("no such vessel")?;
            if comms.signal(id).is_none() {
                return Err(format!("no signal from vessel {}", id.0));
            }
            let n = args.get("points").and_then(Value::as_u64).unwrap_or(100).clamp(2, 1000) as usize;
            let seg = sim.fleet[i].segment().ok_or("the vessel is not coasting")?;
            let (t0, t1) = crate::trajectory::future_span(seg, sim.clock).ok_or("no prediction yet")?;
            let body = sim.dominant_of(i);
            let now = sim.clock.seconds_since(seg.t0);
            let points: Vec<Value> = (0..n)
                .filter_map(|k| {
                    let t = t0 + (t1 - t0) * k as f64 / (n - 1) as f64;
                    let (anchor, r, _) = seg.eval(t)?;
                    let at = seg.t0.add_seconds(t);
                    let b = sim.world.snapshot(at).relative_r(body, anchor);
                    Some(json!({"t_s": t - now, "position_m": (r - b).to_array()}))
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
            sim.index_of(id).ok_or("no such vessel")?;
            (json!({"aboard": id.0}), vec![Effect::Command(GameCommand::Switch(id)), Effect::Station(false)])
        }
        "go_to_mission_control" => (json!({"location": "mission control"}), vec![Effect::Station(true)]),
        "set_plan" => set_plan(args, sim, comms)?,
        "set_controls" => {
            if comms.location != Location::Vessel(sim.ship().id()) {
                return Err("you are not aboard the active vessel (use switch_vessel)".into());
            }
            let throttle = args.get("throttle").and_then(Value::as_f64);
            if throttle.is_some_and(|t| !(0.0..=1.0).contains(&t)) {
                return Err("throttle must be in 0–1".into());
            }
            let sas = args.get("sas").and_then(Value::as_bool);
            (json!({"throttle": throttle, "sas": sas}), vec![Effect::Controls { throttle, sas }])
        }
        _ => return Err(format!("unknown tool {tool}")),
    })
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
    let plan = crate::planner::plan_from(&sim.fleet[i], &draft, sim.clock.add_seconds(delay));
    let command = GameCommand::SetPlan { vessel: id, plan };
    let effect = if delay > 0.0 { Effect::Send { delay, command } } else { Effect::Command(command) };
    Ok((json!({"sent": draft.len(), "arrives_in_s": delay, "about": name(body)}), vec![effect]))
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
    mut sim: ResMut<SimState>,
    comms: Res<Comms>,
    mut station: ResMut<TrackingStation>,
    mut in_flight: ResMut<crate::commands::InFlight>,
    mut commands: MessageWriter<GameCommand>,
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
            match Server::start(port, Some(iface.agent.token.clone()), tools()) {
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
        let (result, effects) = answer(&call.tool, &call.arguments, &sim, &comms);
        for e in effects {
            match e {
                Effect::Command(c) => {
                    commands.write(c);
                }
                Effect::Station(open) => station.open = open,
                Effect::Send { delay, command } => in_flight.send(sim.clock, delay, command),
                Effect::Controls { throttle, sas } => {
                    if let Some(t) = throttle {
                        sim.controls.throttle = t;
                    }
                    if let Some(s) = sas {
                        sim.controls.sas = s;
                    }
                }
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
        for t in tools() {
            assert_eq!(t.input_schema["type"], "object", "{}", t.name);
            // Every tool answers something (an error for missing arguments).
            let _ = answer(&t.name, &json!({}), &sim, &comms);
        }
    }

    #[test]
    fn state_lists_every_vessel() {
        let (sim, comms) = setup();
        let (ToolResult::Ok(v), effects) = answer("get_state", &json!({}), &sim, &comms) else { panic!() };
        assert!(effects.is_empty());
        assert_eq!(v["vessels"].as_array().unwrap().len(), sim.fleet.len());
        assert_eq!(v["active_vessel"], sim.ship().id().0);
    }

    #[test]
    fn a_vessel_without_signal_cannot_be_read() {
        let (sim, mut comms) = setup();
        let id = sim.fleet[1].id();
        comms.signals.insert(id, None);
        let (r, _) = answer("get_vessel", &json!({"id": id.0}), &sim, &comms);
        assert!(matches!(r, ToolResult::Err(e) if e.contains("no signal")));
        comms.signals.insert(id, Some(comms::Signal { delay: 0.01, rate: 1e6, via: vec![] }));
        let (r, _) = answer("get_vessel", &json!({"id": id.0}), &sim, &comms);
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
        let (r, e) = answer("set_plan", &args, &sim, &comms);
        assert!(matches!(r, ToolResult::Ok(_)), "{r:?}");
        assert!(matches!(&e[..], [Effect::Send { delay, command: GameCommand::SetPlan { .. } }] if *delay == 0.02));
        comms.signals.insert(id, None);
        let (r, _) = answer("set_plan", &args, &sim, &comms);
        assert!(matches!(r, ToolResult::Err(e) if e.contains("no signal")));
    }

    #[test]
    fn actions_become_commands_and_controls_need_the_crew() {
        let (sim, mut comms) = setup();
        let (_, e) = answer("set_warp", &json!({"level": 5}), &sim, &comms);
        assert_eq!(e, vec![Effect::Command(GameCommand::SetWarp(5))]);
        let (r, _) = answer("set_warp", &json!({"level": 99}), &sim, &comms);
        assert!(matches!(r, ToolResult::Err(_)));
        comms.location = Location::Site(0);
        let (r, _) = answer("set_controls", &json!({"throttle": 1.0}), &sim, &comms);
        assert!(matches!(r, ToolResult::Err(e) if e.contains("not aboard")));
        comms.location = Location::Vessel(sim.ship().id());
        let (_, e) = answer("set_controls", &json!({"throttle": 0.5, "sas": false}), &sim, &comms);
        assert_eq!(e, vec![Effect::Controls { throttle: Some(0.5), sas: Some(false) }]);
    }
}
