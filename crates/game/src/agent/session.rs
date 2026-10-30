//! The agent's tools on the game session (D044, D122): what the player can
//! see and do besides flying, for every agent (a shot of the view or of a
//! screen, the camera, time, a new flight, saves, debug mode), and behind
//! the developer flag ([`DEV_VAR`]) the look's benchmark and the profiler.
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
use sim::time::Epoch;
use sim::vessel::VesselId;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The developer flag: the look's benchmark and the profiler's tools.
pub const DEV_VAR: &str = "SUNSCATTER_DEV";

/// Whether the developer flag is set (not `0`).
pub fn developer() -> bool {
    std::env::var(DEV_VAR).is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Every agent's session tools, and the developer's.
const SESSION: [&str; 9] =
    ["shoot", "set_camera", "pause", "step", "run_until", "new_flight", "save_game", "load_game", "set_debug"];
const DEVELOPER: [&str; 2] = ["bench", "profile"];

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

const MENU_NAMES: [(&str, MenuId); 7] = [
    ("rcs", MenuId::Rcs),
    ("orbital", MenuId::Orbital),
    ("landing", MenuId::Landing),
    ("maneuver", MenuId::Maneuver),
    ("crew", MenuId::Crew),
    ("power", MenuId::Power),
    ("comms", MenuId::Comms),
];
const LAYER_NAMES: [(&str, Layer); 7] = [
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

fn names<T>(list: &[(&'static str, T)]) -> Vec<&'static str> {
    list.iter().map(|(n, _)| *n).collect()
}

fn by_name<T: Copy>(list: &[(&str, T)], name: &str, what: &str) -> Result<T, String> {
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
             menus open for the shot (the others closed); the systems screen opens on `layer` with `part` (a \
             craft-file id) selected. Everything is put back after the shot.",
            schema(
                json!({
                    "path": {"type": "string"}, "name": {"type": "string"},
                    "width": {"type": "integer", "minimum": 16, "maximum": 8192},
                    "height": {"type": "integer", "minimum": 16, "maximum": 8192},
                    "ui_scale": {"type": "number", "exclusiveMinimum": 0},
                    "screen": {"type": "string", "enum": ["flight", "systems", "tracking", "pause", "settings"]},
                    "layer": {"type": "string", "enum": names(&LAYER_NAMES)},
                    "part": {"type": "string"},
                    "menus": {"type": "array", "items": {"type": "string", "enum": names(&MENU_NAMES)}},
                    "settings_tab": {"type": "string", "enum": names(&TAB_NAMES)},
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
             condition's name, `paused`, `timeout`, `vessel_gone`) and the vessel's state then. A condition on \
             something no working sensor measures is refused. Physics warp is capped while engines run.",
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
            "Load the save `name` (from the save list, e.g. quicksave): time warp back to 1×.",
            schema(json!({"name": {"type": "string"}}), &["name"]),
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
    }
    list
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
    Bench { call: Call, path: Option<PathBuf>, held: bool },
    Profile(Profile),
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
    ui_scale: Option<f32>,
    held: bool,
}

struct Run {
    call: Call,
    until: Until,
    vessel: VesselId,
    fleet: usize,
    q_max: f64,
    wall: f64,
    started: Epoch,
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
    pub settle_max: f64,
}

/// The screen a shot shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    Flight,
    Systems { layer: Option<Layer>, part: Option<String> },
    Tracking,
    Pause,
    Settings(SettingsTab),
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
            "systems" => Screen::Systems { layer, part: s("part").map(str::to_string) },
            "tracking" => Screen::Tracking,
            "pause" => Screen::Pause,
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
        let name = s("name").and_then(crate::saves::sanitize).unwrap_or_else(|| "shot".into());
        let settle_max = args.get("settle_s").and_then(Value::as_f64).unwrap_or(SETTLE_MAX).clamp(SETTLE_MIN, 120.0);
        Ok(ShotSpec {
            path: s("path").map(PathBuf::from),
            name,
            size,
            ui_scale: ui_scale.map(|u| u as f32),
            screen,
            menus,
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
            Some("its dynamic pressure (position and velocity)")
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

/// A vessel's state as it knows it (`None`: no working sensor measures it).
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
/// knows every one of their levels.
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
    if !tanks.iter().all(|&t| k.knows(Quantity::TankLevel(t as u32))) {
        return None;
    }
    let capacity: f64 = tanks.iter().map(|&t| def.tanks[t].capacity).sum();
    let liquid: f64 = tanks.iter().map(|&t| state.tanks[t].liquid).sum();
    (capacity > 0.0).then(|| liquid / capacity)
}

/// The vessel at fleet index `i` as it knows itself now.
pub fn sample(sim: &SimState, i: usize) -> Sample {
    use sim::systems::Quantity;
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
    let nav = known.orbit();
    Sample {
        altitude: altitude.filter(|_| known.knows(Quantity::Altitude) || known.knows(Quantity::Position)),
        apoapsis: apo.filter(|_| nav),
        periapsis: peri.filter(|_| nav),
        feeding: feeding_share(v),
        q: nav.then_some(q),
        speed: known.knows(Quantity::Velocity).then(|| vrel.length()),
        fleet: sim.fleet.len(),
        status: super::status(&v.phase),
    }
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
) {
    let dt = measures.real.delta_secs_f64();
    let session = &mut *session;
    for call in std::mem::take(&mut session.incoming) {
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
            Job::Shot(s) => shot(s, dt, &mut commands, &mut pause, &mut screens, &mut out),
            Job::Run(r) => run_job(r, dt, &mut sim, &mut stop),
            Job::Bench { call, path, held } => {
                if measures.bench.running() {
                    Some(Job::Bench { call, path, held })
                } else {
                    pause.0 = held;
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
        "bench" | "profile" if !dev => done(call, Err(format!("a developer tool: start the game with {DEV_VAR}=1"))),
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
                    ui_scale: out.offscreen.ui_scale,
                    held: pause.0,
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
            Started::Job(Job::Run(Run { call, until, vessel: id, fleet, q_max, wall: 0.0, started }))
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
                        json!({"loaded": path})
                    })
                    .map_err(|e| format!("{}: {e}", path.display()))
            };
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
            let held = pause.0;
            pause.0 = true;
            measures.bench.start(&label, &only);
            Started::Job(Job::Bench { call, path, held })
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
        other => done(call, Err(format!("unknown tool {other}"))),
    }
}

/// Opens what a shot shows.
fn show(spec: &ShotSpec, sim: &SimState, s: &mut Screens) -> Result<(), String> {
    if let Some(menus) = &spec.menus {
        s.iface.menus.show_only(None);
        for m in menus {
            s.iface.menus.set_open(*m, true);
        }
    }
    s.systems.open = false;
    s.station.open = false;
    s.menu.open = false;
    s.settings.open = false;
    match &spec.screen {
        Screen::Flight => {}
        Screen::Systems { layer, part } => {
            let def = sim.ship().ship_def();
            let part = match part {
                None => None,
                Some(p) => Some(
                    def.as_ref()
                        .and_then(|d| d.part_named(p))
                        .ok_or_else(|| format!("the active vessel has no part `{p}`"))?,
                ),
            };
            s.systems.selected = None;
            s.systems.open_at(*layer, part);
        }
        Screen::Tracking => s.station.open = true,
        Screen::Pause => s.menu.open = true,
        Screen::Settings(tab) => {
            s.menu.open = true;
            (s.settings.open, s.settings.tab) = (true, *tab);
        }
    }
    Ok(())
}

/// A shot's frames: settle, take, put back and answer.
fn shot(
    mut s: Shot,
    dt: f64,
    commands: &mut Commands,
    pause: &mut SimPause,
    screens: &mut Screens,
    out: &mut Output,
) -> Option<Job> {
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
            screens.systems.open = r.systems;
            screens.station.open = r.station;
            screens.menu.open = r.pause_menu;
            (screens.settings.open, screens.settings.tab) = r.settings;
            out.offscreen.ui_scale = r.ui_scale;
            pause.0 = r.held;
            let size = out.offscreen.image.as_ref().map(|_| out.offscreen.size);
            s.call.reply(ToolResult::Ok(json!({"path": s.path, "settled": settled, "size": size})));
            None
        }
    }
}

/// A run's frame: answered when it stops.
fn run_job(mut r: Run, dt: f64, sim: &mut SimState, stop: &mut StopAt) -> Option<Job> {
    r.wall += dt;
    let Some(i) = sim.index_of(r.vessel) else {
        stop.0 = None;
        sim.paused = !r.until.keep_running;
        r.call.reply(ToolResult::Ok(json!({"reached": "vessel_gone", "clock": super::epoch_text(sim.clock)})));
        return None;
    };
    let s = sample(sim, i);
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
        assert_eq!(spec.screen, Screen::Systems { layer: Some(Layer::Plumbing), part: Some("s1.tank".into()) });
        assert_eq!(spec.name, "max_q_", "a file name");
        let spec = ShotSpec::parse(&json!({"menus": ["Orbital", "landing"], "width": 3024, "height": 1964})).unwrap();
        assert_eq!(spec.menus, Some(vec![MenuId::Orbital, MenuId::Landing]));
        assert_eq!(spec.size, Some((3024, 1964)));
        assert_eq!(
            ShotSpec::parse(&json!({"screen": "settings"})).unwrap().screen,
            Screen::Settings(SettingsTab::Interface)
        );
        // (arguments, the error's words)
        let bad = [
            (json!({"width": 100}), "together"),
            (json!({"menus": ["cockpit"]}), "unknown menu"),
            (json!({"layer": "x-ray"}), "unknown layer"),
            (json!({"screen": "bridge"}), "unknown screen"),
            (json!({"ui_scale": 0.0}), "ui_scale"),
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
        assert!(names(true).iter().any(|n| n == "bench") && names(true).iter().any(|n| n == "profile"));
        for t in tools(true) {
            assert!(handles(&t.name), "{}", t.name);
            assert_eq!(t.input_schema["type"], "object", "{}", t.name);
        }
    }
}
