//! The agent's tools on the game session (D044, D122): what the player can
//! see and do besides flying, for every agent (a shot of the view or of a
//! screen, the camera, time, a new flight, saves, debug mode), and behind
//! the developer flag ([`DEV_VAR`]) the look's benchmark, the profiler and
//! a fleet in orbit for the performance check.
//!
//! Game time runs while an agent thinks, as it does for the player. `pause`
//! stops it; `step` and `run_until` run it to an epoch or an event and stop
//! it there, so a mission test is deterministic: the clock stops exactly at
//! an epoch ([`StopAt`], the frame's advance ends there), and every command
//! sent while time is stopped applies at that clock.
//!
//! Calls are answered from [`run`], after the simulation each frame; the
//! long ones over several frames: a shot once the view has settled (the
//! clock held meanwhile), the clock's run once it stops, a measurement once
//! it is done. The rules are pure functions with table tests.

use crate::bench::Bench;
use crate::camera::{self, CameraRig, Focus};
use crate::commands::GameCommand;
use crate::interface::layout::InterfaceSettings;
use crate::interface::menus::{MenuId, MenuSettings};
use crate::offscreen::{self, Offscreen};
use crate::persist::Persist;
use crate::settings_ui::{SettingsTab, SettingsUi};
use crate::state::{SimPause, SimState, WARP_LEVELS};
use crate::systems_screen::{rules::Layer, SystemsScreen};
use crate::terrain::Terrain;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot, ScreenshotCaptured};
use glam::DVec3;
use mcp::{Call, Tool, ToolResult};
use serde_json::{json, Value};
use sim::craft::{craft_of, CraftId};
use sim::time::Epoch;
use sim::vessel::VesselId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The developer flag: the look's benchmark, the profiler's and the
/// fleet's tools.
pub const DEV_VAR: &str = "SUNSCATTER_DEV";

/// Whether the developer flag is set (not `0`).
pub fn developer() -> bool {
    std::env::var(DEV_VAR).is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Every agent's session tools, and the developer's.
const SESSION: [&str; 11] = [
    "shoot",
    "set_camera",
    "pause",
    "step",
    "run_until",
    "new_flight",
    "save_game",
    "load_game",
    "list_saves",
    "revert_to_launch",
    "set_debug",
];
const DEVELOPER: [&str; 3] = ["bench", "profile", "spawn_vessels"];
/// The most vessels one `spawn_vessels` call adds.
const SPAWN_MAX: u64 = 50;

/// Whether `tool` is answered here.
pub fn handles(tool: &str) -> bool {
    SESSION.contains(&tool) || DEVELOPER.contains(&tool)
}

/// A shot waits at least this long for the view to settle, and at most
/// this long by default (s, real time): the terrain and its maps load.
const SETTLE_MIN: f64 = 1.0;
const SETTLE_MAX: f64 = 15.0;
/// How long a run waits for its stop by default, and at most (s, real time).
const RUN_TIMEOUT: f64 = 300.0;
const RUN_TIMEOUT_MAX: f64 = 840.0;
/// Max-Q: the dynamic pressure has peaked above this (Pa) and fallen to
/// this share of its peak.
const MAX_Q_FLOOR: f64 = 5_000.0;
const MAX_Q_FALL: f64 = 0.97;

pub(super) const MENU_NAMES: [(&str, MenuId); 7] = [
    ("rcs", MenuId::Rcs),
    ("orbital", MenuId::Orbital),
    ("landing", MenuId::Landing),
    ("maneuver", MenuId::Maneuver),
    ("crew", MenuId::Crew),
    ("power", MenuId::Power),
    ("comms", MenuId::Comms),
];
pub(super) const LAYER_NAMES: [(&str, Layer); 7] = [
    ("physical", Layer::Physical),
    ("structural", Layer::Structural),
    ("thermal", Layer::Thermal),
    ("plumbing", Layer::Plumbing),
    ("electrical", Layer::Electrical),
    ("data", Layer::Data),
    ("health", Layer::Health),
];
const TAB_NAMES: [(&str, SettingsTab); 4] = [
    ("orbits", SettingsTab::Orbits),
    ("controls", SettingsTab::Controls),
    ("interface", SettingsTab::Interface),
    ("visuals", SettingsTab::Visuals),
];
const STATUSES: [&str; 4] = ["landed", "crashed", "coasting", "powered"];

pub(super) fn names<T>(list: &[(&'static str, T)]) -> Vec<&'static str> {
    list.iter().map(|(n, _)| *n).collect()
}

pub(super) fn by_name<T: Copy>(list: &[(&str, T)], name: &str, what: &str) -> Result<T, String> {
    list.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| *v).ok_or_else(|| {
        format!("unknown {what} `{name}` (one of {})", list.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))
    })
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": properties, "required": required})
}

/// The session's tools; the benchmark and the profiler with `dev`.
#[allow(clippy::too_many_lines)]
pub fn tools(dev: bool) -> Vec<Tool> {
    let t = |name: &str, description: &str, input_schema: Value| Tool {
        name: name.into(),
        description: description.into(),
        input_schema,
    };
    let num = |d: &str| json!({"type": "number", "description": d});
    let until = json!({
        "seconds": num("Game seconds from now (the clock stops exactly there)"),
        "epoch": {"type": "string", "description": "TDB, YYYY-MM-DD[THH:MM[:SS]] (the clock stops exactly there)"},
        "altitude_above_m": num("The vessel's altitude above its body's ellipsoid reaches this"),
        "altitude_below_m": num("… falls to this"),
        "apoapsis_above_m": num("Its osculating apoapsis altitude reaches this"),
        "periapsis_above_m": num("Its osculating periapsis altitude reaches this"),
        "feeding_below": num("The propellant left in the tanks feeding its running engines falls to this share (0–1)"),
        "max_q": {"type": "boolean", "description": "Its dynamic pressure has peaked (above 5 kPa) and fallen 3 %"},
        "separation": {"type": "boolean", "description": "The fleet grows (a stage or anything separates)"},
        "status": {"type": "string", "enum": STATUSES},
        "id": {"type": "integer", "description": "The vessel the events are of (default the active one)"},
        "warp": {"type": "integer", "minimum": 0, "description": "Warp level meanwhile (default the current)"},
        "keep_running": {"type": "boolean", "description": "Leave time running at the stop (default: stopped)"},
        "timeout_s": num("Real seconds to wait at most (default 300, at most 840)"),
    });
    let mut list = vec![
        t(
            "shoot",
            "A screenshot (PNG) of what the player sees, once the view has settled (the clock held meanwhile): \
             the flight view, or a named screen. Returns the file's path (default under the game's home, `shots/`). \
             `width`/`height` size the image of a headless game (SUNSCATTER_OFFSCREEN=1); `ui_scale` scales the \
             interface for this shot (0.2 shrinks it to specks: a shot of the scene); `menus` are the flight view's \
             menus open for the shot (the others closed; the RCS menu shows while RCS is on); the systems screen opens on `layer` with `part` (a \
             craft-file id of a part, isolated, or of a component, selected), the thermal layer's cut-away at `cut` (0 none, 1 all); `help` shows the keys and every way in, `saves` the \
             Saves window; `cards` opens those parts' cards \
             (craft-file ids) by the staging bar. Everything is put back after the shot.",
            schema(
                json!({
                    "path": {"type": "string"}, "name": {"type": "string"},
                    "width": {"type": "integer", "minimum": 16, "maximum": 8192},
                    "height": {"type": "integer", "minimum": 16, "maximum": 8192},
                    "ui_scale": {"type": "number", "exclusiveMinimum": 0},
                    "screen": {"type": "string", "enum": ["flight", "systems", "tracking", "pause", "settings", "saves", "help"]},
                    "layer": {"type": "string", "enum": names(&LAYER_NAMES)},
                    "part": {"type": "string"},
                    "menus": {"type": "array", "items": {"type": "string", "enum": names(&MENU_NAMES)}},
                    "settings_tab": {"type": "string", "enum": names(&TAB_NAMES)},
                    "cards": {"type": "array", "items": {"type": "string"}},
                    "cut": {"type": "number", "minimum": 0, "maximum": 1},
                    "settle_s": num("Real seconds to wait at most for the terrain (default 15)"),
                }),
                &[],
            ),
        ),
        t(
            "set_camera",
            "Place the camera: `focus` \"ship\" (the active vessel) or a body's name, or `vessel` (an id); \
             `distance_m` from it; `pitch_deg`/`yaw_deg` about the focus's up (the local vertical of a vessel, a \
             body's pole); or, focused on a body, `sun_angle_deg`: the camera that far round from the subsolar \
             point (0 the lit face, 180 the night side).",
            schema(
                json!({"focus": {"type": "string"}, "vessel": {"type": "integer"},
                       "distance_m": {"type": "number", "exclusiveMinimum": 0},
                       "pitch_deg": {"type": "number"}, "yaw_deg": {"type": "number"},
                       "sun_angle_deg": {"type": "number"}}),
                &[],
            ),
        ),
        t(
            "pause",
            "Stop time (the player's `,`). set_warp, step or run_until runs it again; a run under way ends.",
            schema(json!({}), &[]),
        ),
        t(
            "step",
            "Run time for `seconds` of game time and stop it there exactly (from a stop, deterministic), at warp \
             level `warp` (default the current). Answers when the clock has stopped.",
            schema(json!({"seconds": num("Game seconds"), "warp": {"type": "integer", "minimum": 0}}), &["seconds"]),
        ),
        t(
            "run_until",
            "Run time until the first of the given stops (an epoch, or an event of the vessel as it knows it), then \
             stop it (unless `keep_running`). Answers when it stops, with what stopped it (`reached`: `time`, a \
             condition's name, `paused`, `timeout`, `vessel_gone`) and the vessel's state then, each number as its \
             sensors read it (null where none measures it; the stops compare these). A condition on something no \
             working sensor measures is refused. Physics warp is capped while engines run.",
            schema(until, &[]),
        ),
        t(
            "new_flight",
            "A new flight: the active vessel replaced by a new craft on the pad (`craft`: a directory under \
             data/craft, e.g. falcon9-crew).",
            schema(json!({"craft": {"type": "string"}}), &["craft"]),
        ),
        t(
            "save_game",
            "Save the game as `name` (the game's save list).",
            schema(json!({"name": {"type": "string"}}), &["name"]),
        ),
        t(
            "load_game",
            "Load the save `name` (from the save list, e.g. quicksave): time warp back to 1×, and time stopped \
             if it was stopped (`pause` first to replay from the save exactly).",
            schema(json!({"name": {"type": "string"}}), &["name"]),
        ),
        t(
            "list_saves",
            "The save list as the pause menu's Saves window shows it (newest first, with the time saved), and \
             whether the active vessel has a launch save to revert to (`launch_save`).",
            schema(json!({}), &[]),
        ),
        t(
            "revert_to_launch",
            "The pause menu's Revert to launch (D114): the whole game back to when the active vessel (or the vessel \
             it split from) was put on the pad, from its launch save; without one, a new craft of its kind on the \
             pad. Everything since is lost. Time warp back to 1×, and time stopped if it was stopped.",
            schema(json!({}), &[]),
        ),
        t(
            "set_debug",
            "Debug mode (D064) on or off for every vessel: infinite propellant, no overheating, infinite impact \
             tolerance, and every reading the truth (a dead computer is still dead).",
            schema(json!({"on": {"type": "boolean"}}), &["on"]),
        ),
    ];
    if dev {
        list.push(t(
            "bench",
            "Developer: the look's benchmark from the current view (the clock held): the frame time of the whole \
             look and of the look without each part, so what each costs. `only`: label fragments to run; `path`: \
             also write the table (Markdown). Takes about 4 s per configuration.",
            schema(
                json!({"label": {"type": "string"}, "only": {"type": "array", "items": {"type": "string"}},
                       "path": {"type": "string"}}),
                &[],
            ),
        ));
        list.push(t(
            "profile",
            "Developer: frame statistics over `seconds` of real time (fps, worst and 95th-percentile frame, \
             the warp achieved, compute-limited frames, the vessels) and, when the game was started with \
             SUNSCATTER_PROFILE=1, the profiler's report for that window (CPU per system and thread, GPU per \
             pass group, the counts that drive the cost).",
            schema(json!({"seconds": num("Real seconds, 1–60 (default 5)"), "label": {"type": "string"}}), &[]),
        ));
        list.push(t(
            "spawn_vessels",
            "Developer: add `count` vessels of `craft` coasting in low Earth orbit, each on its own orbit, every \
             other one uncrewed (a probe flown with light delay): the fleet of the performance check. `craft`: a \
             craft id (a directory under data/craft, or a craft built in code: small-rocket, stress-<parts>), \
             default small-rocket. Answers with their ids.",
            schema(json!({"count": num("Vessels to add, 1–50 (default 10)"), "craft": {"type": "string"}}), &[]),
        ));
    }
    list
}

/// What a `spawn_vessels` call asks for: the craft and how many.
pub fn spawn_args(args: &Value) -> Result<(CraftId, usize), String> {
    let count = match args.get("count") {
        None => 10,
        Some(c) => {
            c.as_u64().filter(|n| (1..=SPAWN_MAX).contains(n)).ok_or_else(|| format!("`count` is 1–{SPAWN_MAX}"))?
        }
    };
    let craft = match args.get("craft") {
        None => sim::craft::small::id("rocket"),
        Some(c) => CraftId(c.as_str().filter(|c| !c.is_empty()).ok_or("`craft` is a craft id")?.to_string()),
    };
    Ok((craft, count as usize))
}

/// Where a step or a run stops the clock: the frame's advance ends there
/// (`state::advance`).
#[derive(Resource, Default)]
pub struct StopAt(pub Option<Epoch>);

/// The clock a frame advances to: `target`, or the stop if it comes first.
pub fn clamp_to_stop(target: Epoch, stop: Option<Epoch>) -> Epoch {
    match stop {
        Some(s) if target.seconds_since(s) > 0.0 => s,
        _ => target,
    }
}

/// The calls waiting and under way.
#[derive(Resource, Default)]
pub struct Session {
    pub incoming: Vec<Call>,
    jobs: Vec<Job>,
    shots: u32,
}

/// A call answered over several frames.
enum Job {
    Shot(Shot),
    Run(Run),
    Bench { call: Call, path: Option<PathBuf> },
    Profile(Profile),
}

/// What a job in flight is (for the clock's hold, [`clock_held`]).
#[derive(Clone, Copy, Debug, PartialEq)]
enum JobKind {
    Shot,
    Run,
    Bench,
    Profile,
}

impl Job {
    fn kind(&self) -> JobKind {
        match self {
            Job::Shot(_) => JobKind::Shot,
            Job::Run(_) => JobKind::Run,
            Job::Bench { .. } => JobKind::Bench,
            Job::Profile(_) => JobKind::Profile,
        }
    }
}

/// Whether the clock is held: while any shot settles or the benchmark
/// measures, so overlapping ones release it only when the last ends. (Each
/// used to save the hold and put it back when done, so a shot begun under
/// the benchmark put the benchmark's hold back after it had ended: time
/// stood still while `get_state` said it ran.)
fn clock_held(jobs: impl IntoIterator<Item = JobKind>) -> bool {
    jobs.into_iter().any(|k| matches!(k, JobKind::Shot | JobKind::Bench))
}

/// How many of the waiting calls (their tools, in the order they came)
/// start this frame: all of them, up to a shot that would overlap another
/// shot in flight. The screens are one state: two shots at once showed each
/// other's screens, and the second saved the first's open screens and put
/// them back after the first had restored the player's. The calls behind a
/// waiting shot wait with it, so they still run in the order sent.
fn starting<'a>(tools: impl IntoIterator<Item = &'a str>, shot_in_flight: bool) -> usize {
    let mut shooting = shot_in_flight;
    let mut n = 0;
    for tool in tools {
        if tool == "shoot" {
            if shooting {
                break;
            }
            shooting = true;
        }
        n += 1;
    }
    n
}

struct Shot {
    call: Call,
    path: PathBuf,
    settle_max: f64,
    timer: f64,
    /// Set once the file is written.
    taken: Option<(Arc<AtomicBool>, bool)>,
    restore: Restore,
}

/// What a shot changed, to put back.
struct Restore {
    menus: MenuSettings,
    systems: bool,
    station: bool,
    pause_menu: bool,
    settings: (bool, SettingsTab),
    saves: bool,
    help: bool,
    cards: Vec<crate::interface::part_card::Open>,
    ui_scale: Option<f32>,
    /// The headless view's size (a shot's `width`/`height` stayed for every
    /// later shot).
    size: (u32, u32),
}

struct Run {
    call: Call,
    until: Until,
    vessel: VesselId,
    fleet: usize,
    q_max: f64,
    wall: f64,
    started: Epoch,
    /// Stopped: what stopped it and when (real s), its answer waiting for
    /// the predicted path from the stopped clock ([`answer_now`]).
    stopped: Option<(&'static str, f64)>,
}

/// Real seconds a stopped run waits at most for the predicted path from
/// the stopped clock (it is recomputed a few times a second of real time).
const PREDICTION_WAIT_S: f64 = 5.0;

/// Whether a run that has stopped answers now: at once while time runs on
/// (`keep_running`: no path is ever from the clock), else once the
/// predicted path is from the stopped clock (`current`), or after
/// [`PREDICTION_WAIT_S`] without it. So `get_vessel` after a `step` reads
/// the stopped state's apsides, the same on every replay (finding 55).
fn answer_now(stopped: bool, current: bool, waited: f64) -> bool {
    !stopped || current || waited >= PREDICTION_WAIT_S
}

struct Profile {
    call: Call,
    seconds: f64,
    wall: f64,
    dts: Vec<f64>,
    limited: u32,
    started: Epoch,
    /// The profiler's report was asked for (it is written at the frame's end).
    cut: bool,
}

/// A shot's view, from its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotSpec {
    pub path: Option<PathBuf>,
    pub name: String,
    pub size: Option<(u32, u32)>,
    pub ui_scale: Option<f32>,
    pub screen: Screen,
    pub menus: Option<Vec<MenuId>>,
    /// Part cards open for the shot (craft-file ids), stacked by the
    /// staging bar; `None` leaves the open ones.
    pub cards: Option<Vec<String>>,
    pub settle_max: f64,
}

/// The screen a shot shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    Flight,
    /// `cut`: the thermal layer's cut-away (0 none, 1 all).
    Systems {
        layer: Option<Layer>,
        part: Option<String>,
        cut: Option<f32>,
    },
    Tracking,
    Pause,
    Settings(SettingsTab),
    /// The pause menu's Saves window.
    Saves,
    /// The keys and every way in (`?`).
    Help,
}

impl ShotSpec {
    pub fn parse(args: &Value) -> Result<ShotSpec, String> {
        let s = |k: &str| args.get(k).and_then(Value::as_str);
        let size = match (args.get("width").and_then(Value::as_u64), args.get("height").and_then(Value::as_u64)) {
            (Some(w), Some(h)) if (16..=8192).contains(&w) && (16..=8192).contains(&h) => Some((w as u32, h as u32)),
            (None, None) => None,
            _ => return Err("`width` and `height` go together, 16–8192 px".into()),
        };
        let ui_scale = args.get("ui_scale").and_then(Value::as_f64);
        if ui_scale.is_some_and(|u| !(u > 0.0 && u <= 8.0)) {
            return Err("`ui_scale` must be in (0, 8]".into());
        }
        let layer = s("layer").map(|l| by_name(&LAYER_NAMES, l, "layer")).transpose()?;
        let screen = match s("screen").unwrap_or(if layer.is_some() { "systems" } else { "flight" }) {
            "flight" => Screen::Flight,
            "systems" => Screen::Systems {
                layer,
                part: s("part").map(str::to_string),
                cut: args.get("cut").and_then(Value::as_f64).map(|c| c.clamp(0.0, 1.0) as f32),
            },
            "tracking" => Screen::Tracking,
            "pause" => Screen::Pause,
            "help" => Screen::Help,
            "saves" => Screen::Saves,
            "settings" => {
                Screen::Settings(by_name(&TAB_NAMES, s("settings_tab").unwrap_or("interface"), "settings tab")?)
            }
            other => return Err(format!("unknown screen `{other}`")),
        };
        let menus = match args.get("menus") {
            None => None,
            Some(Value::Array(list)) => Some(
                list.iter()
                    .map(|m| {
                        m.as_str().ok_or("menus are names".to_string()).and_then(|n| by_name(&MENU_NAMES, n, "menu"))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Some(_) => return Err("`menus` is a list of names".into()),
        };
        let cards = match args.get("cards") {
            None => None,
            Some(Value::Array(list)) => Some(
                list.iter()
                    .map(|c| c.as_str().map(str::to_string).ok_or_else(|| "cards are part ids".to_string()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Some(_) => return Err("`cards` is a list of part ids".into()),
        };
        let name = s("name").and_then(crate::saves::sanitize).unwrap_or_else(|| "shot".into());
        let settle_max = args.get("settle_s").and_then(Value::as_f64).unwrap_or(SETTLE_MAX).clamp(SETTLE_MIN, 120.0);
        Ok(ShotSpec {
            path: s("path").map(PathBuf::from),
            name,
            size,
            ui_scale: ui_scale.map(|u| u as f32),
            screen,
            menus,
            cards,
            settle_max,
        })
    }
}

/// Whether a shot can be taken after `timer` seconds with the terrain
/// `busy`: `Some(true)` settled, `Some(false)` waited long enough anyway,
/// `None` wait.
pub fn settle(timer: f64, busy: bool, max: f64) -> Option<bool> {
    if timer >= SETTLE_MIN && !busy {
        Some(true)
    } else if timer >= max {
        Some(false)
    } else {
        None
    }
}

/// The stops of a step or a run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Until {
    pub stop: Option<Epoch>,
    pub altitude_above: Option<f64>,
    pub altitude_below: Option<f64>,
    pub apoapsis_above: Option<f64>,
    pub periapsis_above: Option<f64>,
    pub feeding_below: Option<f64>,
    pub max_q: bool,
    pub separation: bool,
    pub status: Option<&'static str>,
    pub warp: Option<usize>,
    pub keep_running: bool,
    pub timeout: f64,
}

impl Until {
    /// From a call's arguments at clock `now`; `step` takes only seconds.
    pub fn parse(args: &Value, now: Epoch, step: bool) -> Result<Until, String> {
        let f = |k: &str| args.get(k).and_then(Value::as_f64);
        let b = |k: &str| args.get(k).and_then(Value::as_bool).unwrap_or(false);
        let seconds = f("seconds");
        if seconds.is_some_and(|s| !(s > 0.0 && s.is_finite())) {
            return Err("`seconds` must be positive".into());
        }
        let epoch = args
            .get("epoch")
            .and_then(Value::as_str)
            .map(|e| parse_epoch(e).ok_or("`epoch` is TDB, YYYY-MM-DD[THH:MM[:SS]]"))
            .transpose()?;
        if epoch.is_some_and(|e| e.seconds_since(now) <= 0.0) {
            return Err("`epoch` is not in the future".into());
        }
        let stop = match (seconds.map(|s| now.add_seconds(s)), epoch) {
            (Some(a), Some(b)) => Some(clamp_to_stop(a, Some(b))),
            (a, b) => a.or(b),
        };
        let status = args
            .get("status")
            .and_then(Value::as_str)
            .map(|s| STATUSES.iter().find(|x| **x == s).copied().ok_or(format!("unknown status `{s}`")))
            .transpose()?;
        let warp = args.get("warp").and_then(Value::as_u64).map(|w| w as usize);
        if warp.is_some_and(|w| w >= WARP_LEVELS.len()) {
            return Err(format!("`warp` is a level 0–{}", WARP_LEVELS.len() - 1));
        }
        let until = if step {
            Until { stop, warp, ..Until::default() }
        } else {
            Until {
                stop,
                altitude_above: f("altitude_above_m"),
                altitude_below: f("altitude_below_m"),
                apoapsis_above: f("apoapsis_above_m"),
                periapsis_above: f("periapsis_above_m"),
                feeding_below: f("feeding_below"),
                max_q: b("max_q"),
                separation: b("separation"),
                status,
                warp,
                keep_running: b("keep_running"),
                timeout: 0.0,
            }
        };
        let timeout = f("timeout_s").unwrap_or(RUN_TIMEOUT).clamp(1.0, RUN_TIMEOUT_MAX);
        let until = Until { timeout, ..until };
        if until == (Until { stop: None, warp, timeout, keep_running: until.keep_running, ..Until::default() }) {
            return Err("nothing to run until: give `seconds`, an `epoch` or an event".into());
        }
        Ok(until)
    }

    /// What the vessel must know for these stops (the first it does not).
    pub fn unknown(&self, s: &Sample) -> Option<&'static str> {
        let alt = (self.altitude_above.is_some() || self.altitude_below.is_some()) && s.altitude.is_none();
        let orbit = (self.apoapsis_above.is_some() || self.periapsis_above.is_some()) && s.apoapsis.is_none();
        if alt {
            Some("its altitude")
        } else if orbit {
            Some("its orbit (position and velocity)")
        } else if self.feeding_below.is_some() && s.feeding.is_none() {
            Some("the propellant in its feeding tanks (every tank level, and an engine running)")
        } else if self.max_q && s.q.is_none() {
            Some("its dynamic pressure (its velocity)")
        } else {
            None
        }
    }
}

/// `YYYY-MM-DD`, `YYYY-MM-DDTHH:MM` or `YYYY-MM-DDTHH:MM:SS[.s]` (TDB).
pub fn parse_epoch(s: &str) -> Option<Epoch> {
    let s = s.trim().trim_end_matches(" TDB");
    let (date, time) = s.split_once('T').unwrap_or((s, "00:00"));
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(':').collect();
    let [y, mo, da] = d[..] else { return None };
    let (h, mi, sec) = match t[..] {
        [h, mi] => (h, mi, "0"),
        [h, mi, sec] => (h, mi, sec),
        _ => return None,
    };
    let (y, mo, da, h, mi) = (y.parse().ok()?, mo.parse().ok()?, da.parse().ok()?, h.parse().ok()?, mi.parse().ok()?);
    let sec: f64 = sec.parse().ok()?;
    ((1..=12).contains(&mo) && (1..=31).contains(&da) && h < 24 && mi < 60 && (0.0..60.0).contains(&sec))
        .then(|| Epoch::from_calendar(y, mo, da, h, mi, sec))
}

/// A vessel's state as it knows it (`None`: no working sensor measures it;
/// [`Sample::read`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// m above the body's ellipsoid.
    pub altitude: Option<f64>,
    /// Osculating apsides' altitudes above the equatorial radius (m); an
    /// open orbit's apoapsis is infinite.
    pub apoapsis: Option<f64>,
    pub periapsis: Option<f64>,
    /// The share of propellant left in the tanks feeding its running engines.
    pub feeding: Option<f64>,
    /// Dynamic pressure (Pa).
    pub q: Option<f64>,
    pub speed: Option<f64>,
    pub fleet: usize,
    pub status: &'static str,
}

impl Sample {
    /// `truth` (every number the physics gives) as the vessel reads it
    /// (D106), by the menus' rules, as `get_vessel` gives them: the altitude
    /// through an altimeter or the position, the apsides through the
    /// position once the orbit is known, the speed through the velocity,
    /// the dynamic pressure as the Landing menu shows it; `None` where no
    /// working sensor measures it, the truth in debug mode. The stops
    /// compare these, so an answer agrees with what stopped it.
    pub fn read(truth: Sample, known: &crate::format::Known) -> Sample {
        use sim::systems::Quantity::{Position, Velocity};
        let apsis = |x: Option<f64>| x.and_then(|x| super::known_value(known, Position, x)).filter(|_| known.orbit());
        Sample {
            altitude: truth.altitude.and_then(|a| super::known_altitude(known, a)),
            apoapsis: apsis(truth.apoapsis),
            periapsis: apsis(truth.periapsis),
            q: truth.q.and_then(|q| crate::interface::menus::landing::dynamic_pressure(known, q)),
            speed: truth.speed.and_then(|v| super::known_value(known, Velocity, v)),
            ..truth
        }
    }

    pub fn json(&self) -> Value {
        let fin = |x: Option<f64>| x.filter(|v| v.is_finite());
        json!({"altitude_m": self.altitude, "apoapsis_m": fin(self.apoapsis), "periapsis_m": self.periapsis,
               "feeding_share": self.feeding, "dynamic_pressure_pa": self.q, "speed_ms": self.speed,
               "status": self.status, "fleet": self.fleet})
    }
}

/// The first stop reached (its name), given the clock reached the stop
/// epoch (`at_stop`), the fleet's size at the start and the peak dynamic
/// pressure so far.
pub fn reached(u: &Until, s: &Sample, at_stop: bool, fleet0: usize, q_max: f64) -> Option<&'static str> {
    let ge = |x: Option<f64>, y: Option<f64>| matches!((x, y), (Some(x), Some(y)) if x >= y);
    let le = |x: Option<f64>, y: Option<f64>| matches!((x, y), (Some(x), Some(y)) if x <= y);
    if at_stop {
        Some("time")
    } else if ge(s.altitude, u.altitude_above) {
        Some("altitude_above_m")
    } else if le(s.altitude, u.altitude_below) {
        Some("altitude_below_m")
    } else if ge(s.apoapsis, u.apoapsis_above) {
        Some("apoapsis_above_m")
    } else if ge(s.periapsis, u.periapsis_above) {
        Some("periapsis_above_m")
    } else if le(s.feeding, u.feeding_below) {
        Some("feeding_below")
    } else if u.max_q && q_max > MAX_Q_FLOOR && s.q.is_some_and(|q| q < MAX_Q_FALL * q_max) {
        Some("max_q")
    } else if u.separation && s.fleet > fleet0 {
        Some("separation")
    } else if u.status.is_some_and(|st| st == s.status) {
        Some("status")
    } else {
        None
    }
}

/// The share of propellant left in the tanks feeding the starting or
/// running main engines (the first stage's, until MECO), if the vessel
/// knows every one of their levels: their readings' sum over their
/// capacity, to its precision (a sum's, `Reading`'s `Sum`).
pub fn feeding_share(v: &sim::vessel::Vessel) -> Option<f64> {
    use sim::craft::state::EngineStatus;
    use sim::systems::Quantity;
    let (def, state) = (v.ship_def()?, v.ship()?);
    let running = (def.engines.iter().zip(&state.engines))
        .filter(|(e, s)| e.main && matches!(s.status, EngineStatus::Starting | EngineStatus::Running));
    let mut tanks: Vec<usize> = running.flat_map(|(e, _)| e.feeds.iter().flatten().map(|&t| t as usize)).collect();
    tanks.sort_unstable();
    tanks.dedup();
    let k = v.knowledge();
    let capacity: f64 = tanks.iter().map(|&t| def.tanks[t].capacity).sum();
    let liquid = tanks.iter().map(|&t| k.read(Quantity::TankLevel(t as u32), state.tanks[t].liquid)).sum();
    share_of(liquid, capacity)
}

/// A share of `capacity` from the reading of what it holds, to the
/// reading's precision; `None` unknown or of nothing.
fn share_of(held: Option<sim::systems::Reading>, capacity: f64) -> Option<f64> {
    let r = held.filter(|_| capacity > 0.0)?;
    Some(sim::systems::Reading { value: r.value / capacity, precision: r.precision / capacity }.shown())
}

/// The vessel at fleet index `i` as it knows itself now ([`Sample::read`]).
pub fn sample(sim: &SimState, i: usize) -> Sample {
    let v = &sim.fleet[i];
    let known = crate::format::Known::of(v);
    let body = sim.dominant_of(i);
    let snap = sim.world.snapshot(sim.clock);
    let (anchor, r, vel) = v.state_at(&sim.world, sim.clock);
    let k = snap.relative(body, anchor);
    let (rel, vrel) = (r - k.r, vel - k.v);
    let src = sim.world.source(body);
    let physical = src.and_then(|s| s.physical.as_ref());
    let altitude = physical.map(|p| p.altitude(p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), sim.clock)));
    let (apo, peri) = match (src, physical) {
        (Some(s), Some(p)) => {
            let el = sim::kepler::Elements::from_state(rel, vrel, s.gm);
            let apo = if el.e < 1.0 { el.apoapsis() - p.radius_eq } else { f64::INFINITY };
            (Some(apo), Some(el.periapsis() - p.radius_eq))
        }
        _ => (None, None),
    };
    let q =
        sim::vessel::air_at(&sim.world, &snap, anchor, r, vel).map_or(0.0, |a| 0.5 * a.rho * a.wind.length_squared());
    let truth = Sample {
        altitude,
        apoapsis: apo,
        periapsis: peri,
        feeding: feeding_share(v),
        q: Some(q),
        speed: Some(vrel.length()),
        fleet: sim.fleet.len(),
        status: super::status(&v.phase),
    };
    Sample::read(truth, &known)
}

/// The camera's pitch and yaw about `up` (rad, `camera::basis`) for a
/// camera `angle` (rad) round from the direction `to_sun`, towards the
/// side `to_sun × up`.
pub fn sunlit(to_sun: DVec3, up: DVec3, angle: f64) -> (f64, f64) {
    let side = to_sun.cross(up).try_normalize().unwrap_or_else(|| to_sun.any_orthonormal_vector());
    let d = to_sun * angle.cos() + side * angle.sin();
    let (e1, e2) = camera::basis(up);
    (d.dot(up).clamp(-1.0, 1.0).asin(), d.dot(e2).atan2(d.dot(e1)))
}

/// Whether two body names match, ignoring case and punctuation ("hiiaka"
/// is Hi'iaka).
pub fn same_name(a: &str, b: &str) -> bool {
    let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    key(a) == key(b)
}

/// The screens a shot opens and puts back.
#[derive(SystemParam)]
pub struct Screens<'w> {
    iface: ResMut<'w, InterfaceSettings>,
    systems: ResMut<'w, SystemsScreen>,
    station: ResMut<'w, crate::tracking::TrackingStation>,
    menu: ResMut<'w, crate::interface::pause::PauseMenu>,
    settings: ResMut<'w, SettingsUi>,
    saves: ResMut<'w, crate::saves::SaveUi>,
    help: ResMut<'w, crate::interface::help::Help>,
    cards: ResMut<'w, crate::interface::part_card::PartCards>,
}

/// The developer's measures.
#[derive(SystemParam)]
pub struct Measures<'w> {
    bench: ResMut<'w, Bench>,
    profile: ResMut<'w, crate::profile::Profile>,
    real: Res<'w, Time<Real>>,
}

/// The view's target and the files.
#[derive(SystemParam)]
pub struct Output<'w> {
    offscreen: ResMut<'w, Offscreen>,
    images: ResMut<'w, Assets<Image>>,
    persist: Res<'w, Persist>,
    terrain: Res<'w, Terrain>,
}

/// Answers the session's calls (after the simulation each frame).
#[allow(clippy::too_many_arguments)]
pub fn run(
    mut session: ResMut<Session>,
    mut commands: Commands,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut pause: ResMut<SimPause>,
    mut stop: ResMut<StopAt>,
    mut screens: Screens,
    mut measures: Measures,
    mut out: Output,
    mut game: MessageWriter<GameCommand>,
    prediction: Res<crate::state::Prediction>,
) {
    let dt = measures.real.delta_secs_f64();
    let session = &mut *session;
    let in_flight = session.jobs.iter().any(|j| j.kind() == JobKind::Shot);
    let now = starting(session.incoming.iter().map(|c| c.tool.as_str()), in_flight);
    let waiting = session.incoming.split_off(now);
    for call in std::mem::replace(&mut session.incoming, waiting) {
        let n = session.shots + 1;
        let started =
            start(call, n, &mut sim, &mut rig, &mut pause, &mut stop, &mut screens, &mut measures, &mut out, &mut game);
        match started {
            Started::Job(Job::Shot(s)) => {
                session.shots = n;
                session.jobs.push(Job::Shot(s));
            }
            Started::Job(j) => session.jobs.push(j),
            Started::Pause => {
                for j in session.jobs.iter_mut() {
                    if let Job::Run(r) = j {
                        r.until.timeout = 0.0;
                    }
                }
            }
            Started::Done => {}
        }
    }
    let jobs = std::mem::take(&mut session.jobs);
    for job in jobs {
        let kept = match job {
            Job::Shot(s) => shot(s, dt, &mut commands, &mut screens, &mut out),
            Job::Run(r) => run_job(r, dt, &mut sim, &mut stop, &prediction),
            Job::Bench { call, path } => {
                if measures.bench.running() {
                    Some(Job::Bench { call, path })
                } else {
                    let b = &measures.bench;
                    let md = crate::bench::markdown(b);
                    let written = path.as_ref().map(|p| std::fs::write(p, &md).map_err(|e| e.to_string()));
                    let base = crate::bench::baseline(&b.results).unwrap_or(0.0);
                    let results: Vec<Value> = b
                        .results
                        .iter()
                        .map(|r| json!({"label": r.label, "avg_ms": r.avg_ms, "p95_ms": r.p95_ms, "cost_ms": base - r.avg_ms}))
                        .collect();
                    call.reply(ToolResult::Ok(json!({"view": b.view, "results": results, "markdown": md,
                        "written": written.map(|w| w.err().unwrap_or_else(|| "ok".into()))})));
                    None
                }
            }
            Job::Profile(p) => profile_job(p, dt, &sim, &mut measures),
        };
        session.jobs.extend(kept);
    }
    // A shot waiting for the one before keeps the clock held between them.
    let shot_waiting = session.incoming.iter().any(|c| c.tool == "shoot").then_some(JobKind::Shot);
    pause.0 = clock_held(session.jobs.iter().map(Job::kind).chain(shot_waiting));
}

#[allow(clippy::large_enum_variant)]
enum Started {
    Job(Job),
    Pause,
    Done,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn start(
    call: Call,
    n: u32,
    sim: &mut SimState,
    rig: &mut CameraRig,
    pause: &mut SimPause,
    stop: &mut StopAt,
    screens: &mut Screens,
    measures: &mut Measures,
    out: &mut Output,
    game: &mut MessageWriter<GameCommand>,
) -> Started {
    let args = call.arguments.clone();
    let done = |call: Call, r: Result<Value, String>| {
        call.reply(match r {
            Ok(v) => ToolResult::Ok(v),
            Err(e) => ToolResult::Err(e),
        });
        Started::Done
    };
    let dev = developer();
    let tool = call.tool.clone();
    match tool.as_str() {
        "bench" | "profile" | "spawn_vessels" if !dev => {
            done(call, Err(format!("a developer tool: start the game with {DEV_VAR}=1")))
        }
        "shoot" => match ShotSpec::parse(&args) {
            Err(e) => done(call, Err(e)),
            Ok(spec) => {
                let path = spec.path.clone().unwrap_or_else(|| {
                    let home = out.persist.dirs.settings_file.parent().map(PathBuf::from).unwrap_or_default();
                    home.join("shots").join(format!("{n:03}_{}.png", spec.name))
                });
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Some(size) = spec.size {
                    if !offscreen::resize(&mut out.offscreen, &mut out.images, size) {
                        return done(call, Err("a size needs a headless game (SUNSCATTER_OFFSCREEN=1)".into()));
                    }
                }
                let restore = Restore {
                    menus: screens.iface.menus.clone(),
                    systems: screens.systems.open,
                    station: screens.station.open,
                    pause_menu: screens.menu.open,
                    settings: (screens.settings.open, screens.settings.tab),
                    saves: screens.saves.open,
                    help: screens.help.open,
                    cards: screens.cards.open.clone(),
                    ui_scale: out.offscreen.ui_scale,
                    size: out.offscreen.size,
                };
                if let Err(e) = show(&spec, sim, screens) {
                    return done(call, Err(e));
                }
                if spec.ui_scale.is_some() {
                    out.offscreen.ui_scale = spec.ui_scale;
                }
                pause.0 = true;
                let settle_max = spec.settle_max;
                Started::Job(Job::Shot(Shot { call, path, settle_max, timer: 0.0, taken: None, restore }))
            }
        },
        "set_camera" => {
            let r = set_camera(&args, sim, rig);
            done(call, r)
        }
        "pause" => {
            sim.paused = true;
            stop.0 = None;
            call.reply(ToolResult::Ok(json!({"time_stopped": true})));
            Started::Pause
        }
        "step" | "run_until" => {
            let step = tool == "step";
            let until = match Until::parse(&args, sim.clock, step) {
                Ok(u) => u,
                Err(e) => return done(call, Err(e)),
            };
            let id = match args.get("id").and_then(Value::as_u64) {
                Some(id) => VesselId(id),
                None => sim.ship().id(),
            };
            let Some(i) = sim.index_of(id) else { return done(call, Err("no such vessel".into())) };
            let s = sample(sim, i);
            if let Some(what) = until.unknown(&s) {
                return done(call, Err(format!("vessel {} does not know {what}: no working sensor measures it", id.0)));
            }
            stop.0 = until.stop;
            if let Some(w) = until.warp {
                sim.warp = w;
            }
            sim.paused = false;
            let (fleet, q_max, started) = (s.fleet, s.q.unwrap_or(0.0), sim.clock);
            Started::Job(Job::Run(Run { call, until, vessel: id, fleet, q_max, wall: 0.0, started, stopped: None }))
        }
        "new_flight" => {
            let craft = args.get("craft").and_then(Value::as_str).unwrap_or("").to_string();
            if !crate::state::craft_exists(&craft) {
                let choices: Vec<&str> =
                    crate::state::craft_choices(crate::state::craft_exists).iter().map(|c| c.0).collect();
                return done(call, Err(format!("no craft `{craft}` (choices: {})", choices.join(", "))));
            }
            game.write(GameCommand::NewFlight(craft.clone()));
            done(call, Ok(json!({"craft": craft, "requested": true})))
        }
        "save_game" | "load_game" => {
            let Some(name) = args.get("name").and_then(Value::as_str).and_then(crate::saves::sanitize) else {
                return done(call, Err("missing a usable `name`".into()));
            };
            let path = crate::saves::save_path(&out.persist.dirs.saves, &name);
            let r = if tool == "save_game" {
                let screen = crate::saves::screen_of(screens.systems.open, screens.station.open);
                let _ = std::fs::create_dir_all(&out.persist.dirs.saves);
                crate::saves::save(sim, screen, &path).map(|()| json!({"saved": path})).map_err(|e| e.to_string())
            } else {
                crate::saves::read(sim, &path)
                    .map(|save| {
                        game.write(GameCommand::Restore(Box::new(save)));
                        // Stopped time stays stopped, so a test replays
                        // from the save exactly (the player's load runs it).
                        if sim.paused {
                            game.write(GameCommand::PauseTime);
                        }
                        json!({"loaded": path, "time_stopped": sim.paused})
                    })
                    .map_err(|e| format!("{}: {e}", path.display()))
            };
            done(call, r)
        }
        "list_saves" => done(call, Ok(crate::saves::listing(sim, &out.persist.dirs.saves))),
        "revert_to_launch" => {
            let r = crate::saves::revert(sim, &out.persist.dirs.saves).map(|c| {
                let from =
                    if matches!(c, GameCommand::Restore(_)) { "its launch save" } else { "a new craft on the pad" };
                game.write(c);
                // Stopped time stays stopped, as `load_game`.
                if sim.paused {
                    game.write(GameCommand::PauseTime);
                }
                json!({"reverted": from, "time_stopped": sim.paused})
            });
            done(call, r)
        }
        "set_debug" => match args.get("on").and_then(Value::as_bool) {
            Some(on) => {
                game.write(GameCommand::SetDebug(on));
                done(call, Ok(json!({"debug": on})))
            }
            None => done(call, Err("missing boolean `on`".into())),
        },
        "bench" => {
            let label = args.get("label").and_then(Value::as_str).unwrap_or("view").to_string();
            let only: Vec<String> = args
                .get("only")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default();
            let path = args.get("path").and_then(Value::as_str).map(PathBuf::from);
            pause.0 = true;
            measures.bench.start(&label, &only);
            Started::Job(Job::Bench { call, path })
        }
        "profile" => {
            let seconds = args.get("seconds").and_then(Value::as_f64).unwrap_or(5.0).clamp(1.0, 60.0);
            if crate::profile::enabled() {
                let label = args.get("label").and_then(Value::as_str).unwrap_or("agent");
                measures.profile.restart(label);
                measures.profile.last.clear();
            }
            let started = sim.clock;
            Started::Job(Job::Profile(Profile {
                call,
                seconds,
                wall: 0.0,
                dts: Vec::new(),
                limited: 0,
                started,
                cut: false,
            }))
        }
        "spawn_vessels" => {
            let r = spawn_args(&args).and_then(|(craft, count)| {
                sim::craft::registry::definition(&craft).map_err(|e| format!("craft `{}`: {e}", craft.0))?;
                let ids = sim.spawn_in_orbit(&craft_of(&craft), count);
                Ok(json!({"craft": craft.0, "ids": ids.iter().map(|id| id.0).collect::<Vec<_>>()}))
            });
            done(call, r)
        }
        other => done(call, Err(format!("unknown tool {other}"))),
    }
}

/// Opens what a shot shows.
fn show(spec: &ShotSpec, sim: &SimState, s: &mut Screens) -> Result<(), String> {
    if let Some(menus) = &spec.menus {
        if menus.contains(&MenuId::Rcs) && !sim.controls().rcs.on {
            return Err("the RCS menu shows while RCS is on: `set_controls {rcs: \"on\"}` first".into());
        }
        s.iface.menus.show_only(None);
        for m in menus {
            s.iface.menus.set_open(*m, true);
        }
    }
    s.systems.open = false;
    s.station.open = false;
    s.menu.open = false;
    s.settings.open = false;
    s.saves.open = false;
    s.help.open = false;
    if let Some(cards) = &spec.cards {
        let (vessel, def) = (sim.ship().id(), sim.ship().ship_def());
        s.cards.open.clear();
        for c in cards {
            let part = def
                .as_ref()
                .and_then(|d| d.part_named(c))
                .ok_or_else(|| format!("the active vessel has no part `{c}`"))?;
            s.cards.open(vessel, part, crate::interface::part_card::From::Bar);
        }
    }
    match &spec.screen {
        Screen::Flight => {}
        Screen::Systems { layer, part, cut } => {
            use crate::systems_screen::inspect::{named, Selection};
            let def = sim.ship().ship_def();
            let sel = match part {
                None => None,
                Some(p) => Some(
                    def.as_ref()
                        .and_then(|d| named(d, p))
                        .ok_or_else(|| format!("the active vessel has no part or component `{p}`"))?,
                ),
            };
            s.systems.close();
            match sel {
                Some(Selection::Component(_)) => {
                    s.systems.open_at(*layer, None);
                    s.systems.select(sel);
                }
                Some(Selection::Part(id)) => s.systems.open_at(*layer, Some(id)),
                None => s.systems.open_at(*layer, None),
            }
            if let Some(c) = cut {
                s.systems.cut = *c;
            }
        }
        Screen::Tracking => s.station.open = true,
        Screen::Pause => s.menu.open = true,
        Screen::Help => s.help.open = true,
        Screen::Saves => {
            s.menu.open = true;
            s.saves.show_list();
        }
        Screen::Settings(tab) => {
            s.menu.open = true;
            (s.settings.open, s.settings.tab) = (true, *tab);
        }
    }
    Ok(())
}

/// A shot's frames: settle, take, put back and answer.
fn shot(mut s: Shot, dt: f64, commands: &mut Commands, screens: &mut Screens, out: &mut Output) -> Option<Job> {
    s.timer += dt;
    match &s.taken {
        None => {
            let Some(settled) = settle(s.timer, out.terrain.busy(), s.settle_max) else { return Some(Job::Shot(s)) };
            let flag = Arc::new(AtomicBool::new(false));
            let shot = match &out.offscreen.image {
                Some(image) => Screenshot::image(image.clone()),
                None => Screenshot::primary_window(),
            };
            let (mut save, done) = (save_to_disk(s.path.clone()), flag.clone());
            commands.spawn(shot).observe(move |captured: On<ScreenshotCaptured>| {
                save(captured);
                done.store(true, Ordering::SeqCst);
            });
            s.taken = Some((flag, settled));
            Some(Job::Shot(s))
        }
        Some((flag, _)) if !flag.load(Ordering::SeqCst) => {
            if s.timer > s.settle_max + 30.0 {
                s.call.reply(ToolResult::Err("the screenshot was never written".into()));
                return None;
            }
            Some(Job::Shot(s))
        }
        Some((_, settled)) => {
            let r = &s.restore;
            screens.iface.menus = r.menus.clone();
            if !r.systems {
                screens.systems.close();
            }
            screens.station.open = r.station;
            screens.menu.open = r.pause_menu;
            (screens.settings.open, screens.settings.tab) = r.settings;
            screens.saves.open = r.saves;
            screens.help.open = r.help;
            screens.cards.open = r.cards.clone();
            out.offscreen.ui_scale = r.ui_scale;
            let size = out.offscreen.image.as_ref().map(|_| out.offscreen.size);
            if size.is_some_and(|s| s != r.size) {
                offscreen::resize(&mut out.offscreen, &mut out.images, r.size);
            }
            s.call.reply(ToolResult::Ok(json!({"path": s.path, "settled": settled, "size": size})));
            None
        }
    }
}

/// A run's frame: answered when it stops, once the predicted path is from
/// the stopped clock ([`answer_now`]).
fn run_job(
    mut r: Run,
    dt: f64,
    sim: &mut SimState,
    stop: &mut StopAt,
    prediction: &crate::state::Prediction,
) -> Option<Job> {
    r.wall += dt;
    let Some(i) = sim.index_of(r.vessel) else {
        stop.0 = None;
        sim.paused = !r.until.keep_running;
        r.call.reply(ToolResult::Ok(json!({"reached": "vessel_gone", "clock": super::epoch_text(sim.clock)})));
        return None;
    };
    let s = sample(sim, i);
    let why = match r.stopped {
        Some((why, _)) => why,
        None => {
            r.q_max = r.q_max.max(s.q.unwrap_or(0.0));
            let at_stop = r.until.stop.is_some_and(|t| sim.clock.seconds_since(t) >= 0.0);
            let why = reached(&r.until, &s, at_stop, r.fleet, r.q_max).or_else(|| {
                if r.until.timeout <= 0.0 {
                    Some("paused")
                } else {
                    (r.wall >= r.until.timeout).then_some("timeout")
                }
            });
            let Some(why) = why else { return Some(Job::Run(r)) };
            stop.0 = None;
            if why != "paused" {
                sim.paused = !r.until.keep_running;
            }
            r.stopped = Some((why, r.wall));
            why
        }
    };
    let ship = sim.ship();
    let current = prediction.current(ship.id(), sim.clock, crate::trajectory::predict::needed(ship));
    let waited = r.stopped.map_or(0.0, |(_, at)| r.wall - at);
    if !answer_now(sim.paused, current, waited) {
        return Some(Job::Run(r));
    }
    let out = json!({
        "reached": why, "clock": super::epoch_text(sim.clock), "elapsed_s": sim.clock.seconds_since(r.started),
        "wall_s": r.wall, "time_stopped": sim.paused, "vessel": r.vessel.0, "state": s.json(),
        "active_vessel": sim.ship().id().0,
    });
    r.call.reply(ToolResult::Ok(out));
    None
}

/// A profile window's frame: answered after `seconds` (and the profiler's
/// report, when it is on).
fn profile_job(mut p: Profile, dt: f64, sim: &SimState, m: &mut Measures) -> Option<Job> {
    if !p.cut {
        p.wall += dt;
        p.dts.push(dt);
        p.limited += u32::from(sim.compute_limited);
        if p.wall < p.seconds {
            return Some(Job::Profile(p));
        }
        p.cut = true;
        if crate::profile::enabled() {
            m.profile.cut();
            return Some(Job::Profile(p));
        }
    }
    let on = crate::profile::enabled();
    if on && m.profile.last.is_empty() {
        return Some(Job::Profile(p));
    }
    let stats = crate::profile::report::frame_stats(&p.dts);
    let mut sorted = p.dts.clone();
    sorted.sort_by(f64::total_cmp);
    let p95 = sorted.get(sorted.len() * 95 / 100).copied().unwrap_or(0.0);
    let worst = sorted.last().copied().unwrap_or(0.0);
    let game_s = sim.clock.seconds_since(p.started);
    let report = if on {
        json!({"text": m.profile.last, "summary": m.profile.summary})
    } else {
        json!("off: start the game with SUNSCATTER_PROFILE=1 for the per-system tables")
    };
    p.call.reply(ToolResult::Ok(json!({
        "frames": p.dts.len(), "seconds": p.wall, "fps": stats.fps(), "worst_frame_ms": worst * 1e3,
        "p95_frame_ms": p95 * 1e3, "warp_achieved": game_s / p.wall.max(1e-9), "compute_limited_frames": p.limited,
        "vessels": sim.fleet.len(), "profiler": report,
    })));
    None
}

/// The `set_camera` tool.
fn set_camera(args: &Value, sim: &SimState, rig: &mut CameraRig) -> Result<Value, String> {
    let f = |k: &str| args.get(k).and_then(Value::as_f64);
    let mut body = None;
    if let Some(id) = args.get("vessel").and_then(Value::as_u64) {
        let id = VesselId(id);
        sim.index_of(id).ok_or("no such vessel")?;
        rig.focus = if id == sim.ship().id() { Focus::Ship } else { Focus::Vessel(id) };
    } else if let Some(name) = args.get("focus").and_then(Value::as_str) {
        if name.eq_ignore_ascii_case("ship") {
            rig.focus = Focus::Ship;
        } else {
            let s = sim.world.sources.iter().find(|s| same_name(&s.name, name) && s.physical.is_some());
            let node = s.map(|s| s.node).ok_or_else(|| format!("no body `{name}`"))?;
            camera::focus_body(rig, sim, node);
            body = Some(node);
        }
    }
    if let Focus::Body(b) = rig.focus {
        body = Some(b);
    }
    if let Some(angle) = f("sun_angle_deg") {
        let b = body.ok_or("`sun_angle_deg` needs a body in focus")?;
        let sun = sim.world.find("Sun").map(|s| s.node).ok_or("no Sun")?;
        let to_sun = sim.world.snapshot(sim.clock).relative_r(sun, b).normalize();
        let up = sim.world.source(b).and_then(|s| s.physical.as_ref()).map_or(DVec3::Z, |p| p.rotation.pole(sim.clock));
        (rig.pitch, rig.yaw) = sunlit(to_sun, up, angle.to_radians());
    }
    if let Some(p) = f("pitch_deg") {
        rig.pitch = p.to_radians().clamp(-1.55, 1.55);
    }
    if let Some(y) = f("yaw_deg") {
        rig.yaw = y.to_radians();
    }
    if let Some(d) = f("distance_m") {
        if d.is_nan() || d <= 0.0 {
            return Err("`distance_m` must be positive".into());
        }
        rig.distance = d;
    }
    let focus = match rig.focus {
        Focus::Ship => json!("ship"),
        Focus::Vessel(id) => json!({"vessel": id.0}),
        Focus::Body(b) => json!(sim.world.eph.node(b).name),
    };
    Ok(json!({"focus": focus, "distance_m": rig.distance, "pitch_deg": rig.pitch.to_degrees(),
              "yaw_deg": rig.yaw.to_degrees()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch(s: &str) -> Epoch {
        parse_epoch(s).unwrap()
    }

    #[test]
    fn epochs_parse_to_the_second() {
        let start = epoch("2030-01-01");
        // (text, seconds after 2030-01-01T00:00 TDB, or none)
        let cases = [
            ("2030-01-01", Some(0.0)),
            ("2030-01-02T06:30", Some(86_400.0 + 6.5 * 3600.0)),
            ("2030-01-01T00:00:12.5", Some(12.5)),
            ("2030-01-01T00:01:00 TDB", Some(60.0)),
            ("2030-13-01", None),
            ("2030-01-01T24:00", None),
            ("2030-01-01T00:00:60", None),
            ("tomorrow", None),
        ];
        for (text, want) in cases {
            assert_eq!(parse_epoch(text).map(|e| e.seconds_since(start)), want, "{text}");
        }
    }

    #[test]
    fn the_clock_is_held_while_any_shot_or_benchmark_is_in_flight() {
        use JobKind::{Bench, Profile, Run, Shot};
        // (jobs in flight, held)
        let cases: [(&[JobKind], bool); 6] = [
            (&[], false),
            (&[Run], false),
            (&[Profile, Run], false),
            (&[Shot], true),
            (&[Bench], true),
            (&[Run, Bench, Shot], true),
        ];
        for (jobs, want) in cases {
            assert_eq!(clock_held(jobs.iter().copied()), want, "{jobs:?}");
        }
        // A shot taken while the benchmark runs, outliving it (the clock
        // stood still after both ended): held until the last ends.
        let frames: [&[JobKind]; 4] = [&[Bench], &[Bench, Shot], &[Shot], &[]];
        let held: Vec<bool> = frames.iter().map(|jobs| clock_held(jobs.iter().copied())).collect();
        assert_eq!(held, [true, true, true, false]);
    }

    #[test]
    fn a_shot_waits_for_the_shot_in_flight_and_the_calls_behind_it_wait_too() {
        // (waiting calls, a shot in flight, how many start this frame)
        let cases: [(&[&str], bool, usize); 8] = [
            (&[], false, 0),
            (&[], true, 0),
            (&["shoot"], false, 1),
            (&["shoot"], true, 0),
            (&["get_state", "set_camera"], true, 2),
            (&["shoot", "shoot"], false, 1),
            (&["set_camera", "shoot", "get_state", "shoot", "pause"], false, 3),
            (&["get_state", "shoot", "pause"], true, 1),
        ];
        for (tools, in_flight, want) in cases {
            assert_eq!(starting(tools.iter().copied(), in_flight), want, "{tools:?} {in_flight}");
        }
        // Two shots sent together are taken one after the other, each
        // putting back what it found: the second starts once the first ends.
        let frames = [(&["shoot", "shoot"][..], false), (&["shoot"][..], true), (&["shoot"][..], false)];
        let started: Vec<usize> = frames.iter().map(|(t, f)| starting(t.iter().copied(), *f)).collect();
        assert_eq!(started, [1, 0, 1]);
    }

    #[test]
    fn a_stop_ends_the_frames_advance_there() {
        let t = epoch("2030-01-01");
        // (target, stop, expected)
        let cases = [(10.0, None, 10.0), (10.0, Some(4.0), 4.0), (10.0, Some(12.0), 10.0), (4.0, Some(4.0), 4.0)];
        for (target, stop, want) in cases {
            let got = clamp_to_stop(t.add_seconds(target), stop.map(|s| t.add_seconds(s)));
            assert_eq!(got.seconds_since(t), want, "{target} {stop:?}");
        }
    }

    #[test]
    fn shots_are_specified_by_name() {
        let spec = ShotSpec::parse(&json!({})).unwrap();
        assert_eq!(
            (spec.screen.clone(), spec.menus.clone(), spec.size, spec.name.as_str()),
            (Screen::Flight, None, None, "shot")
        );
        let spec = ShotSpec::parse(&json!({"layer": "plumbing", "part": "s1.tank", "name": "max q!"})).unwrap();
        assert_eq!(
            spec.screen,
            Screen::Systems { layer: Some(Layer::Plumbing), part: Some("s1.tank".into()), cut: None }
        );
        assert_eq!(spec.name, "max_q_", "a file name");
        let spec = ShotSpec::parse(&json!({"layer": "thermal", "cut": 2.0})).unwrap();
        assert_eq!(spec.screen, Screen::Systems { layer: Some(Layer::Thermal), part: None, cut: Some(1.0) });
        let spec = ShotSpec::parse(&json!({"menus": ["Orbital", "landing"], "width": 3024, "height": 1964})).unwrap();
        assert_eq!(spec.menus, Some(vec![MenuId::Orbital, MenuId::Landing]));
        assert_eq!(spec.size, Some((3024, 1964)));
        assert_eq!(
            ShotSpec::parse(&json!({"screen": "settings"})).unwrap().screen,
            Screen::Settings(SettingsTab::Interface)
        );
        assert_eq!(ShotSpec::parse(&json!({"screen": "saves"})).unwrap().screen, Screen::Saves);
        let spec = ShotSpec::parse(&json!({"screen": "help", "cards": ["s1.tank", "m1d#1"]})).unwrap();
        assert_eq!((spec.screen, spec.cards), (Screen::Help, Some(vec!["s1.tank".into(), "m1d#1".into()])));
        // (arguments, the error's words)
        let bad = [
            (json!({"width": 100}), "together"),
            (json!({"menus": ["cockpit"]}), "unknown menu"),
            (json!({"layer": "x-ray"}), "unknown layer"),
            (json!({"screen": "bridge"}), "unknown screen"),
            (json!({"ui_scale": 0.0}), "ui_scale"),
            (json!({"cards": "s1.tank"}), "list of part ids"),
        ];
        for (args, words) in bad {
            let e = ShotSpec::parse(&args).unwrap_err();
            assert!(e.contains(words), "{args}: {e}");
        }
    }

    #[test]
    fn a_shot_waits_for_the_terrain_up_to_its_limit() {
        // (seconds waited, terrain busy, expected)
        let cases = [(0.5, false, None), (1.0, false, Some(true)), (5.0, true, None), (15.0, true, Some(false))];
        for (t, busy, want) in cases {
            assert_eq!(settle(t, busy, 15.0), want, "{t} {busy}");
        }
    }

    /// A stopped run answers once the predicted path is from the stopped
    /// clock, or after the wait's bound; at once while time runs on
    /// (finding 55: a `get_vessel` right after a `step` read a path from a
    /// moment before, its apsides missing near the end of the insertion).
    #[test]
    fn a_stopped_run_answers_once_its_prediction_is_from_the_stop() {
        // (time stopped, path current, real seconds waited) → answers now
        let cases = [
            (true, false, 0.0, false),
            (true, false, 0.5, false),
            (true, true, 0.0, true),
            (true, true, 0.3, true),
            (true, false, PREDICTION_WAIT_S, true),
            (false, false, 0.0, true),
        ];
        for (stopped, current, waited, want) in cases {
            assert_eq!(answer_now(stopped, current, waited), want, "{stopped} {current} {waited}");
        }
    }

    #[test]
    fn runs_stop_at_their_first_event() {
        let now = epoch("2030-01-01");
        let u = Until::parse(&json!({"seconds": 30.0, "epoch": "2030-01-01T00:00:20"}), now, false).unwrap();
        assert_eq!(u.stop.map(|s| s.seconds_since(now)), Some(20.0), "the first of the two");
        assert!(Until::parse(&json!({"warp": 2}), now, false).is_err(), "nothing to stop at");
        assert!(Until::parse(&json!({"epoch": "2029-12-31"}), now, false).is_err(), "in the past");
        assert!(Until::parse(&json!({"seconds": -1.0}), now, true).is_err());
        let step = Until::parse(&json!({"seconds": 2.5, "altitude_above_m": 1.0}), now, true).unwrap();
        assert_eq!(step.altitude_above, None, "a step stops only at its time");
        let u = Until::parse(
            &json!({"altitude_above_m": 10_000.0, "apoapsis_above_m": 2e5, "feeding_below": 0.08, "max_q": true,
                    "separation": true, "status": "landed"}),
            now,
            false,
        )
        .unwrap();
        let s = Sample {
            altitude: Some(5_000.0),
            apoapsis: Some(1e5),
            periapsis: Some(-6e6),
            feeding: Some(0.5),
            q: Some(20_000.0),
            fleet: 1,
            status: "powered",
            ..Sample::default()
        };
        // (case, sample, at the stop, peak q, expected)
        let cases = [
            ("climbing", s, false, 20_000.0, None),
            ("the stop's time", s, true, 20_000.0, Some("time")),
            ("high enough", Sample { altitude: Some(10_000.0), ..s }, false, 0.0, Some("altitude_above_m")),
            ("apoapsis", Sample { apoapsis: Some(2.5e5), ..s }, false, 0.0, Some("apoapsis_above_m")),
            ("MECO's reserve", Sample { feeding: Some(0.08), ..s }, false, 0.0, Some("feeding_below")),
            ("past max-Q", Sample { q: Some(19_000.0), ..s }, false, 20_000.0, Some("max_q")),
            ("not yet past it", Sample { q: Some(19_800.0), ..s }, false, 20_000.0, None),
            ("a low peak is not max-Q", Sample { q: Some(1_000.0), ..s }, false, 4_000.0, None),
            ("separated", Sample { fleet: 2, ..s }, false, 0.0, Some("separation")),
            ("landed", Sample { status: "landed", ..s }, false, 0.0, Some("status")),
            ("unknown altitude never stops it", Sample { altitude: None, ..s }, false, 0.0, None),
        ];
        for (case, sample, at_stop, q_max, want) in cases {
            assert_eq!(reached(&u, &sample, at_stop, 1, q_max), want, "{case}");
        }
        assert_eq!(u.unknown(&s), None);
        assert_eq!(u.unknown(&Sample { feeding: None, ..s }).map(|w| w.contains("propellant")), Some(true));
    }

    #[test]
    fn the_sunlit_view_is_turned_from_the_sun() {
        let (to_sun, up) = (DVec3::X, DVec3::Z);
        let dir = |(pitch, yaw): (f64, f64)| {
            let (e1, e2) = camera::basis(up);
            up * pitch.sin() + (e1 * yaw.cos() + e2 * yaw.sin()) * pitch.cos()
        };
        // (angle, the camera's direction from the body)
        let side = to_sun.cross(up);
        let cases = [(0.0, to_sun), (90.0, side), (180.0, -to_sun)];
        for (angle, want) in cases {
            let d = dir(sunlit(to_sun, up, f64::to_radians(angle)));
            assert!((d - want).length() < 1e-9, "{angle}: {d}");
        }
    }

    #[test]
    fn body_names_match_loosely() {
        // (typed, body name, matches)
        for (typed, name, want) in [("hiiaka", "Hi'iaka", true), ("TITAN", "Titan", true), ("io", "Ion", false)] {
            assert_eq!(same_name(typed, name), want, "{typed}");
        }
    }

    #[test]
    fn a_runs_state_reads_as_the_menus_show_it() {
        use crate::format::Known;
        use sim::systems::{Knowledge, Quantity::*};
        let known = |qs: &[(sim::systems::Quantity, f64)]| {
            let mut k = Knowledge::default();
            for (q, p) in qs {
                k.learn(*q, *p);
            }
            Known::from(k)
        };
        let truth = Sample {
            altitude: Some(30.078_471),
            apoapsis: Some(-4_891.77),
            periapsis: Some(-6_371_000.4),
            feeding: Some(0.5),
            q: Some(1_234.5),
            speed: Some(408.04),
            fleet: 2,
            status: "landed",
        };
        let blind = Sample { fleet: 2, status: "landed", ..Sample::default() };
        // A GNSS fix: the position to ±3 m, its velocity to ±0.1 m/s and
        // its altitude to ±4.5 m (`systems::provides`).
        let gnss = known(&[(Position, 3.0), (Velocity, 0.1), (Altitude, 4.5)]);
        let fixed = Sample {
            altitude: Some(30.0),
            apoapsis: Some(-4_892.0),
            periapsis: Some(-6_371_000.0),
            q: Some(1_230.0),
            speed: Some(408.0),
            ..truth
        };
        // (case, what it knows, what it reads)
        let cases = [
            ("blind", known(&[]), Sample { feeding: Some(0.5), ..blind }),
            (
                "an altimeter alone",
                known(&[(Altitude, 1.0)]),
                Sample { altitude: Some(30.0), feeding: Some(0.5), ..blind },
            ),
            (
                "a position without its velocity: no orbit",
                known(&[(Position, 25.0)]),
                Sample { altitude: Some(30.0), feeding: Some(0.5), ..blind },
            ),
            ("a fix", gnss, fixed),
            // (the dynamic pressure to the menu's digits, as the orbit's
            // angles and eccentricity are, `agent::known_digits`)
            ("debug mode: the truth", Known::from(Knowledge::perfect()), Sample { q: Some(1_230.0), ..truth }),
        ];
        for (case, k, want) in cases {
            assert_eq!(Sample::read(truth, &k), want, "{case}");
        }
        // A share of what tanks hold, to the sum's precision: two tanks of
        // 50 t each read to ±750 kg are ±1.5 % of their 100 t, so to the
        // per cent.
        let held = |kg: f64, p: f64| Some(sim::systems::Reading { value: kg, precision: p });
        let share = share_of(held(43_210.0, 1_500.0), 100_000.0).unwrap();
        assert!((share - 0.43).abs() < 1e-12, "{share}");
        assert_eq!(share_of(held(43_210.0, 0.0), 100_000.0), Some(0.4321), "exactly, in debug mode");
        assert_eq!(share_of(None, 100_000.0), None, "a level unknown");
        assert_eq!(share_of(held(0.0, 0.0), 0.0), None, "no tanks feed it");
    }

    #[test]
    fn the_feeding_share_follows_the_running_engines_tanks() {
        let craft = crate::test_rocket::rocket();
        let mut sim = SimState::with_craft(&craft);
        let a = sim.active;
        assert_eq!(feeding_share(&sim.fleet[a]), None, "nothing runs: no share");
        sim.controls_mut().throttle = 1.0;
        let s = &mut sim;
        s.fleet[a].next_stage(&s.world).expect("the ignition");
        assert_eq!(feeding_share(&sim.fleet[a]), None, "no tank-level sensor: unknown");
        let s = &mut sim;
        s.fleet[a].set_debug(&s.world, true);
        let share = feeding_share(&sim.fleet[a]).expect("debug mode knows everything");
        // The oxidizer full, the fuel loaded to 90 %: their liquid over their capacity.
        let def = sim.fleet[a].ship_def().unwrap();
        let full: f64 = def.tanks.iter().map(|t| t.capacity).sum();
        let loaded: f64 = def.tanks.iter().map(|t| t.capacity * t.load.fill).sum();
        assert!((share - loaded / full).abs() < 1e-12, "{share}");
    }

    #[test]
    fn developer_tools_only_with_the_flag() {
        let names = |dev| tools(dev).into_iter().map(|t| t.name).collect::<Vec<_>>();
        assert!(!names(false).iter().any(|n| DEVELOPER.contains(&n.as_str())));
        for d in DEVELOPER {
            assert!(names(true).iter().any(|n| n == d), "{d}");
        }
        for t in tools(true) {
            assert!(handles(&t.name), "{}", t.name);
            assert_eq!(t.input_schema["type"], "object", "{}", t.name);
        }
    }

    #[test]
    fn a_fleet_is_some_vessels_of_a_craft() {
        // (arguments, the craft and count asked for, or an error)
        let ok = |c: &str, n| Ok((CraftId(c.into()), n));
        let cases = [
            (json!({}), ok("small-rocket", 10)),
            (json!({"count": 1}), ok("small-rocket", 1)),
            (json!({"count": 50, "craft": "falcon9-crew"}), ok("falcon9-crew", 50)),
            (json!({"craft": "stress-300"}), ok("stress-300", 10)),
            (json!({"count": 0}), Err("`count` is 1–50".to_string())),
            (json!({"count": 51}), Err("`count` is 1–50".to_string())),
            (json!({"count": 2.5}), Err("`count` is 1–50".to_string())),
            (json!({"craft": ""}), Err("`craft` is a craft id".to_string())),
            (json!({"craft": 3}), Err("`craft` is a craft id".to_string())),
        ];
        for (args, want) in cases {
            assert_eq!(spawn_args(&args), want, "{args}");
        }
    }

    #[test]
    fn spawned_vessels_coast_on_their_own_orbits_every_other_one_a_probe() {
        let mut sim = SimState::new();
        let before = sim.fleet.len();
        let ids = sim.spawn_in_orbit(sim::craft::small::rocket_ref(), 3);
        assert_eq!(ids.len(), 3);
        assert_eq!(sim.fleet.len(), before + 3);
        let crews: Vec<u32> = ids.iter().map(|&id| sim.fleet[sim.index_of(id).unwrap()].crew()).collect();
        assert!(crews[0] > 0 && crews[1] == 0 && crews[2] > 0, "{crews:?}");
        for &id in &ids {
            let v = &sim.fleet[sim.index_of(id).unwrap()];
            assert!(v.ship().is_some(), "a craft of parts");
            assert!(matches!(v.phase, sim::vessel::Phase::Coasting { .. }), "{:?}", v.phase);
        }
    }
}
