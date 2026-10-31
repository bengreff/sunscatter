//! A part card's content (D111) as pure data with table tests (rule 8):
//! the live essentials of one part on this ship now — its engines (state,
//! thrust, chamber pressure, ignitions), tanks (kg/kg, pressure, liquid
//! temperature), bottles, batteries, mechanisms, separation joint or the
//! pad's clamps, thruster blocks, valves, the cabin, the parachutes, its
//! temperature against its rating — and its commands, the irreversible
//! ones confirmed. What its components carry is summed by kind
//! (`inside`), never listed one by one; their commands are on the systems
//! screen. What the craft cannot measure is unknown, with the
//! sensor that would measure it (D106: the card shows a dash, its tooltip
//! names the sensor); debug mode knows everything.

use super::inside::Inside;
use crate::format;
use crate::systems_screen::inspect::{self, Selection};
use sim::craft::assembly::JointKind;
use sim::craft::command::{matches, ShipCommand};
use sim::craft::ship::ShipDef;
use sim::craft::state::{EngineStatus, ShipState};
use sim::systems::{Knowledge, Quantity};

/// A card: the part's id and catalog entry, its sections, its commands.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub title: String,
    pub entry: String,
    pub sections: Vec<Section>,
    pub commands: Vec<Command>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub heading: String,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub label: String,
    pub value: Value,
    /// How it reads against its limit (`format::limit_level`; normal when
    /// it has none or is not measured).
    pub level: format::Level,
}

/// A value as shown, or unknown with the sensor that would measure it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Shown(String),
    Unknown(String),
}

/// A command on the card; `confirm`: it cannot be undone.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    pub label: String,
    pub command: ShipCommand,
    pub confirm: bool,
}

/// What a card reads besides the ship.
#[derive(Clone, Copy)]
pub struct Live<'a> {
    pub def: &'a ShipDef,
    pub state: &'a ShipState,
    pub knowledge: &'a Knowledge,
    /// Each engine's thrust now (N; aligned with `ShipDef::engines`).
    pub thrusts: &'a [f64],
    /// Each part's skin cell and interior node nearest their limits, by
    /// part id (`systems_screen::rules::vessel_temperatures`).
    pub temps: &'a [(u32, crate::systems_screen::rules::PartTemps)],
    /// The parachutes' parts and their state's words, if the craft has them.
    pub chute: Option<(&'a [u32], &'a str)>,
}

impl Live<'_> {
    /// A value of `q` (true value `truth`) as `show` writes its reading,
    /// else unknown with its sensor.
    pub(super) fn value(&self, q: Quantity, truth: f64, show: impl Fn(sim::systems::Reading) -> String) -> Value {
        match self.knowledge.read(q, truth) {
            Some(r) => Value::Shown(show(r)),
            None => Value::Unknown(sensor_for(self.def, q)),
        }
    }
}

pub(super) fn line(label: &str, value: Value) -> Line {
    Line { label: label.to_string(), value, level: format::Level::Normal }
}

/// A line against a limit: `x` (when measured) of `limit` sets its level.
pub(super) fn against(label: &str, value: Value, x: f64, limit: f64) -> Line {
    let level = match value {
        Value::Shown(_) => format::limit_level(x, Some(limit)),
        Value::Unknown(_) => format::Level::Normal,
    };
    Line { level, ..line(label, value) }
}

pub(super) fn shown(label: &str, text: impl Into<String>) -> Line {
    line(label, Value::Shown(text.into()))
}

/// A reading in a unit of `scale` SI units with at most `max` decimals.
pub(super) fn scaled(r: sim::systems::Reading, scale: f64, max: usize, unit: &str) -> String {
    format!("{:.*} {unit}", format::decimals(r.precision, scale, max), r.shown() / scale)
}

/// The card of part `part` (`Part::id`), if the craft has it.
pub fn card(live: &Live, part: u32) -> Option<Card> {
    let def = live.def;
    let info = def.parts.iter().find(|p| p.part == part)?;
    let inside = Inside::of(def, part, &info.id);
    let mut sections = Vec::new();
    for i in (0..def.engines.len()).filter(|&i| def.engines[i].part == part && !inside.engines.contains(&i)) {
        sections.push(engine(live, i));
    }
    for t in (0..def.tanks.len()).filter(|&t| def.tanks[t].part == part && !inside.tanks.contains(&t)) {
        sections.push(tank(live, t));
    }
    stores(live, part, &inside, &mut sections);
    joints_and_mechanisms(live, part, &inside, &mut sections);
    sections.extend(super::inside::sections(live, &inside));
    let mut own = vec![];
    if let Some(c) = live.state.cabin.as_ref().filter(|c| c.part == part) {
        let kpa = |r: sim::systems::Reading| scaled(r, 1e3, 1, "kPa");
        let total = c.o2 + c.n2 + c.co2 + c.h2o;
        own.push(line("cabin pressure", live.value(Quantity::CabinPressure, total, kpa)));
        own.push(line("O₂", live.value(Quantity::CabinO2, c.o2, kpa)));
        own.push(line("CO₂", live.value(Quantity::CabinCo2, c.co2, kpa)));
    }
    if let Some((_, words)) = live.chute.filter(|(parts, _)| parts.contains(&part)) {
        own.push(shown("parachutes", words));
    }
    // As the systems screen's THERMAL: the skin cell and the interior node
    // nearest their own limits, where the part has them.
    let (skin, interior) = live.temps.iter().find(|(p, _)| *p == part).map_or((None, None), |x| x.1);
    for (label, t) in [("skin", skin), ("interior", interior)] {
        let Some((k, limit)) = t else { continue };
        let value =
            live.value(Quantity::PartTemperature(part), k, |r| format!("{} / {limit:.0} K", scaled(r, 1.0, 0, "K")));
        own.push(against(label, value, k, limit));
    }
    sections.push(Section { heading: format!("PART {}", info.id), lines: own });
    Some(Card { title: info.id.clone(), entry: info.entry.clone(), sections, commands: commands(live, part) })
}

/// An engine's state, thrust, chamber pressure (its flow level: the
/// controller's reading) and ignitions left.
fn engine(live: &Live, i: usize) -> Section {
    let (d, s) = (&live.def.engines[i], &live.state.engines[i]);
    let run = live.state.engines.get(i);
    let state = engine_words(live, i);
    let thrust = live.thrusts.get(i).copied().unwrap_or(0.0);
    let rated = format::force(Some(sim::systems::Reading { value: d.data.thrust_vac, precision: 0.0 }));
    let q = Quantity::EngineThrust(i as u32);
    let level = run.map_or(0.0, |r| r.level);
    let ignitions = match (s.ignitions, live.state.clamped) {
        (_, true) => "the pad's supply".to_string(),
        (u32::MAX, _) => "unlimited".to_string(),
        (n, _) => n.to_string(),
    };
    let mut lines = vec![
        shown("state", state),
        line("thrust", live.value(q, thrust, |r| format!("{} of {rated} vac", format::force(Some(r))))),
        line("chamber pressure", live.value(q, level, |_| format!("{:.0} % of rated", level * 100.0))),
        shown("ignitions left", ignitions),
    ];
    if let Some(why) = run.and_then(|r| r.reason.as_ref()).filter(|_| s.status != EngineStatus::Running) {
        lines.push(shown("stopped", why.clone()));
    }
    Section { heading: format!("ENGINE {}", d.id), lines }
}

/// Engine `i`'s state in words.
pub(super) fn engine_words(live: &Live, i: usize) -> &'static str {
    let run = live.state.engines.get(i);
    match live.state.engines[i].status {
        EngineStatus::Off if run.is_some_and(|r| r.settling.is_some()) => "waiting for settled propellant",
        EngineStatus::Off => "off",
        EngineStatus::Starting => "starting",
        EngineStatus::Running => "running",
        EngineStatus::ShuttingDown => "shutting down",
        EngineStatus::Failed => "FAILED",
    }
}

/// A tank's liquid against its capacity, ullage pressure against its
/// rating, the liquid's temperature.
fn tank(live: &Live, t: usize) -> Section {
    let (d, s) = (&live.def.tanks[t], &live.state.tanks[t]);
    let i = t as u32;
    let cap = d.capacity;
    let level =
        live.value(Quantity::TankLevel(i), s.liquid, |r| crate::interface::stage_bar::rows::amount(Some(r), cap));
    let rated = d.pressure / 1e5;
    let p = inspect::ullage_pressure(d, s);
    let pressure =
        live.value(Quantity::TankPressure(i), p, |r| format!("{} of {rated:.1} bar", scaled(r, 1e5, 2, "bar")));
    let temperature = live.value(Quantity::TankTemperature(i), s.liquid_k, |r| scaled(r, 1.0, 1, "K"));
    let pressure = against("pressure", pressure, p, d.burst);
    let lines = vec![line("contents", level), pressure, line("temperature", temperature)];
    Section { heading: format!("TANK {} · {}", d.id, d.fluid), lines }
}

/// The part's own bottles, batteries, thruster blocks and valves (not
/// those `inside` it).
fn stores(live: &Live, part: u32, inside: &Inside, out: &mut Vec<Section>) {
    let (def, state) = (live.def, live.state);
    for (j, b) in def.bottles.iter().enumerate().filter(|(j, b)| b.part == part && !inside.bottles.contains(j)) {
        let s = &state.bottles[j];
        let p = sim::fluids::pressure(s.moles, b.volume, s.k);
        let rated = b.pressure / 1e5;
        let v = live.value(Quantity::BottlePressure(j as u32), p, |r| {
            format!("{} of {rated:.0} bar", scaled(r, 1e5, 0, "bar"))
        });
        let lines = vec![against("pressure", v, p, b.burst)];
        out.push(Section { heading: format!("BOTTLE {} · {}", b.id, b.gas), lines });
    }
    for (j, b) in def.batteries.iter().enumerate().filter(|(j, b)| b.part == part && !inside.batteries.contains(j)) {
        let cap = b.item.capacity.max(1.0);
        let charge = state.batteries[j].charge;
        let v = live.value(Quantity::BatteryCharge(j as u32), charge, |r| format!("{:.0} %", r.shown() / cap * 100.0));
        let wh = format!("{:.0} Wh", cap / 3600.0);
        out.push(Section {
            heading: format!("BATTERY {}", b.id),
            lines: vec![line("charge", v), shown("capacity", wh)],
        });
    }
    for (j, t) in def.thrusters.iter().enumerate().filter(|(j, t)| t.part == part && !inside.thrusters.contains(j)) {
        let lost = state.out(t.owner);
        let on = state.thrusters.get(j).is_some_and(|s| s.enabled);
        let words = if lost {
            "LOST (failed or no power)"
        } else if on {
            "enabled"
        } else {
            "disabled"
        };
        out.push(Section { heading: format!("RCS {}", t.id), lines: vec![shown("state", words)] });
    }
    for (j, v) in def.valves.iter().enumerate().filter(|(j, v)| v.part == part && !inside.valves.contains(j)) {
        let s = &state.valves[j];
        let words = format!("{}{}", if s.open { "open" } else { "closed" }, if s.stuck { " · STUCK" } else { "" });
        out.push(Section { heading: format!("VALVE {}", v.id), lines: vec![shown("state", words)] });
    }
}

/// The part's own mechanisms (angle and where it is going) and the joint
/// that holds it to its parent when it is a separation or the pad's clamps.
fn joints_and_mechanisms(live: &Live, part: u32, inside: &Inside, out: &mut Vec<Section>) {
    let (def, state) = (live.def, live.state);
    for (j, m) in def.mechanisms.iter().enumerate().filter(|(j, m)| m.part == part && !inside.mechanisms.contains(j)) {
        let s = &state.mechanisms[j];
        let words = if (s.angle - s.target).abs() > 0.5 {
            format!("moving: {:.0}° to {:.0}°", s.angle, s.target)
        } else if (s.angle - m.item.deployed_deg).abs() < 0.5 {
            "deployed".to_string()
        } else if (s.angle - m.item.stowed_deg).abs() < 0.5 {
            "stowed".to_string()
        } else {
            format!("{:.0}°", s.angle)
        };
        out.push(Section { heading: format!("MECHANISM {}", m.id), lines: vec![shown("state", words)] });
    }
    for (j, d) in def.joints.iter().enumerate().filter(|(_, d)| d.child == part) {
        let intact = state.joints.get(j).is_some_and(|s| s.intact);
        let words = match (&d.kind, intact) {
            (JointKind::HoldDown, true) if state.release_armed => "armed: lets go at full thrust",
            (JointKind::HoldDown, true) => "holding",
            (JointKind::HoldDown, false) => "released",
            (JointKind::Separation { .. }, true) => "attached",
            (JointKind::Separation { .. }, false) => "separated",
            _ => continue,
        };
        let heading = if d.kind == JointKind::HoldDown { "CLAMPS" } else { "SEPARATION" };
        out.push(Section { heading: format!("{heading} {}", d.name), lines: vec![shown("state", words)] });
    }
}

/// The part's commands on its card, each with whether it asks first.
fn commands(live: &Live, part: u32) -> Vec<Command> {
    (part_commands(live.def, live.state, live.chute, part).into_iter())
        .map(|(label, command)| Command { confirm: irreversible(live.def, &command), label, command })
        .collect()
}

/// A part's own commands (its components' are on the systems screen,
/// `inspect::commands`) and arming the parachutes it carries while they
/// are stowed (`chute`: the parachutes' parts and their state's words,
/// [`chutes`]): the one list its card and the agent's `command` tool give.
pub fn part_commands(
    def: &ShipDef,
    state: &ShipState,
    chute: Option<(&[u32], &str)>,
    part: u32,
) -> Vec<(String, ShipCommand)> {
    let mut list = inspect::commands(def, state, Selection::Part(part));
    if chute.is_some_and(|(parts, words)| parts.contains(&part) && words == "stowed") {
        list.push((crate::staging::parts::label(&ShipCommand::ArmChutes), ShipCommand::ArmChutes));
    }
    list.dedup_by(|a, b| a.1 == b.1);
    list
}

/// A vessel's parachutes: the parts that carry them and their state in
/// words (empty and "stowed" without any).
pub fn chutes(v: &sim::vessel::Vessel) -> (Vec<u32>, &'static str) {
    let parts = v.craft.design().parts.iter().filter(|p| p.chute).map(|p| p.id).collect();
    (parts, chute_words(crate::interface::stage_bar::icons::chute_look(&v.chute)))
}

/// The parachutes' state in words (from their look, `stage_bar::icons`).
pub fn chute_words(look: crate::interface::stage_bar::icons::Look) -> &'static str {
    use crate::interface::stage_bar::icons::Look;
    match look {
        Look::Idle => "stowed",
        Look::Armed => "armed",
        Look::Running => "open",
        Look::Failed => "torn",
        Look::Spent => "cut away",
    }
}

/// Whether a command cannot be undone: a separation, the clamps' release,
/// arming the parachutes, deploying what does not retract.
pub fn irreversible(def: &ShipDef, c: &ShipCommand) -> bool {
    match c {
        ShipCommand::Separate(_) | ShipCommand::ReleaseClamps | ShipCommand::ArmChutes => true,
        ShipCommand::Deploy(s) => def.mechanisms.iter().filter(|m| matches(s, &m.id)).any(|m| !m.item.retractable),
        _ => false,
    }
}

/// What would measure `q` (D106's tooltip): the sensor the craft has for
/// it that is not working, else the kind of sensor it needs.
pub fn sensor_for(def: &ShipDef, q: Quantity) -> String {
    let (kind, words, part) = match q {
        Quantity::TankLevel(t) => {
            ("tank-level", "a level sensor in the tank", def.tanks.get(t as usize).map(|t| t.part))
        }
        Quantity::TankPressure(t) => {
            ("pressure", "a pressure transducer on the tank", def.tanks.get(t as usize).map(|t| t.part))
        }
        Quantity::TankTemperature(t) => {
            ("temperature", "a thermocouple in the tank", def.tanks.get(t as usize).map(|t| t.part))
        }
        Quantity::PartTemperature(p) => ("temperature", "a thermocouple on the part", Some(p)),
        Quantity::BottlePressure(b) => {
            ("bottle-pressure", "a bottle pressure transducer", def.bottles.get(b as usize).map(|b| b.part))
        }
        Quantity::EngineThrust(_) => return "the engine's own controller (lost with its part)".into(),
        Quantity::BatteryCharge(_) => return "the battery's management electronics (lost with its part)".into(),
        Quantity::CabinPressure | Quantity::CabinO2 | Quantity::CabinCo2 => {
            return "a cabin air sensor (none modelled yet)".into()
        }
        _ => return "a sensor the craft does not carry".into(),
    };
    let found = def.sensors.iter().find(|s| s.item.quantity == kind && Some(s.part) == part);
    match found {
        Some(s) => format!("{} (not working: switched off, failed, unpowered or not linked)", s.id),
        None => format!("{words}: none fitted"),
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
