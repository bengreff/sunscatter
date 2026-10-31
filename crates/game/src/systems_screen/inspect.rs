//! The systems screen's inspector (D107; rule 8): everything known about
//! a selected part or component, as titled groups of rows, and the
//! commands it takes. Pure functions of the ship definition and state (and
//! the layers' facts), table-tested.
//!
//! One layout for every part: its name and type, then STRUCTURE (mass,
//! its joint and the joint's load), then a group per function it has
//! (engines, tanks, mechanisms, aero surfaces), THERMAL, CONDITION (wear
//! against its rated life), RATINGS, FAILURE MODES and its COMPONENTS
//! (each a link to its own). A row against a
//! limit reads `value / limit` and is coloured by the one rule
//! (`format::limit_level`: amber from 80 %, red at it); a term's hover
//! explains it ([`term`]).
//!
//! Values are what the craft measures (D106, `sim::systems::data`): a dash
//! where it has no sensor, the digits its precision gives; design data
//! (ratings, masses, ranges) as built.

use super::rules::{JointClass, PartFacts};
use crate::format::{limit_level, Known, Level};
use crate::staging::parts::label;
use sim::catalog::{Hazard, Life, Ratings};
use sim::craft::command::ShipCommand;
use sim::craft::ship::ShipDef;
use sim::craft::state::{EngineStatus, ShipState};
use sim::systems::Quantity;

/// What the inspector shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Selection {
    /// A part, by its id (`Part::id`).
    Part(u32),
    /// A component, by its index in the ship definition.
    Component(usize),
}

/// One labelled value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    /// How it reads against its limit (`format::limit_level`).
    pub level: Level,
    /// What the label means (a hover; [`term`]).
    pub hover: Option<String>,
    /// What it is about: a click selects it.
    pub open: Option<Selection>,
}

/// A titled group of rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub title: String,
    pub rows: Vec<Row>,
}

impl Group {
    pub fn new(title: impl Into<String>) -> Self {
        Group { title: title.into(), rows: Vec::new() }
    }

    /// A row (its hover the term's explanation, if [`term`] has one).
    pub fn row(&mut self, label: &str, value: impl Into<String>) -> &mut Self {
        let hover = term(label).map(str::to_string);
        self.rows.push(Row { label: label.to_string(), value: value.into(), hover, ..Row::default() });
        self
    }

    /// A row against a limit: `x` of `limit` (same units) sets its level;
    /// `None` (not measured) leaves it normal.
    pub fn limit(&mut self, label: &str, value: impl Into<String>, x: Option<f64>, limit: f64) -> &mut Self {
        self.row(label, value);
        if let (Some(x), Some(r)) = (x, self.rows.last_mut()) {
            r.level = limit_level(x, Some(limit));
        }
        self
    }

    /// The last row's level, or its link.
    pub fn level(&mut self, level: Level) -> &mut Self {
        if let Some(r) = self.rows.last_mut() {
            r.level = level;
        }
        self
    }

    pub fn opens(&mut self, sel: Selection) -> &mut Self {
        if let Some(r) = self.rows.last_mut() {
            r.open = Some(sel);
        }
        self
    }

    /// The value of the row labelled `label`.
    #[cfg(test)]
    pub fn value(&self, label: &str) -> Option<&str> {
        self.rows.iter().find(|r| r.label == label).map(|r| r.value.as_str())
    }
}

/// What a term on the screen means (the rows' hovers).
pub fn term(label: &str) -> Option<&'static str> {
    Some(match label {
        "dry mass" => "The part's own mass without what its tanks and bottles hold.",
        "derived" => "The mass its catalog generator computes from its geometry and materials.",
        "unmodelled" => "Published mass the generator does not account for (fittings, wiring, margins).",
        "joint load" | "load" => {
            "The heaviest line load on the joint's ring at the last tick over what it takes at its temperature; \
             100 % is its ultimate (it fails). Measured by a strain gauge."
        }
        "allowable line load" => {
            "The line load (force per metre of ring) the joint takes at room temperature. A dash: the craft \
             gives none (a mount with no wall data, a part hung by a hinge), so its load is not checked."
        }
        "axial capacity" => "The allowable line load times the ring's circumference: the pull or push it takes.",
        "ullage pressure" | "pressure" => {
            "The gas over the liquid (or in the bottle) against its burst pressure: amber from 80 %. The relief \
             vents above the operating pressure."
        }
        "ullage temperature" => "The temperature of the gas over the liquid.",
        "liquid" | "contents" => "What it holds now against what it holds full.",
        "chamber pressure" => "The engine's flow level against its rated full thrust (the controller's reading).",
        "Isp vac · SL" => "Specific impulse: seconds of thrust per weight of propellant, in vacuum and at sea level.",
        "skin" | "interior" => "The hottest skin cell or interior node of the part against its limit.",
        "integrity" => "100 % sound; it fails at 0 %.",
        "thermal damage" => "Wear from time above its rated temperature; it fails at 1.",
        "peak interior" => {
            "The hottest its interior nodes have been, against its rated temperature (its skin cells are judged \
             against their materials' limits, THERMAL)."
        }
        "vibration damage" => "Wear from vibration past its qualification (Miner's rule); it fails at 1.",
        "run time" | "starts" | "cycles" => "Its use so far against its rated life.",
        "g-limit" => {
            "The acceleration the craft takes along its thrust before a joint reaches its ultimate, or the crew's \
             limit along that axis (sim::systems::g_limit)."
        }
        "centre of mass" => "Its height above the craft's base, along its axis.",
        _ => return None,
    })
}

fn kg(x: f64) -> String {
    if x.abs() >= 1e4 {
        format!("{:.2} t", x / 1e3)
    } else {
        format!("{x:.1} kg")
    }
}

fn kelvin(x: f64) -> String {
    format!("{x:.0} K")
}

fn bar(pa: f64) -> String {
    format!("{:.2} bar", pa / 1e5)
}

fn kilonewtons(n: f64) -> String {
    format!("{:.1} kN", n / 1e3)
}

/// The ratings a part or component carries (only those it has).
fn ratings(r: &Ratings) -> Group {
    let mut g = Group::new("RATINGS");
    if let Some(x) = r.reliability {
        g.row("reliability", format!("{x:.1} / 10"));
    }
    if let Some(l) = r.life {
        g.row(
            "rated life",
            match l {
                Life::Hours(h) => format!("{h:.0} h"),
                Life::Cycles(c) => format!("{c:.0} cycles"),
                Life::Starts(s) => format!("{s:.0} starts"),
            },
        );
    }
    let rows = [
        ("rated temperature", r.rated_k.map(kelvin)),
        ("destruction temperature", r.destruction_k.map(kelvin)),
        ("time at temperature", r.time_at_temperature.map(crate::format::duration)),
        ("rated load", r.rated_load.map(kilonewtons)),
        ("ultimate load", r.ultimate_load.map(kilonewtons)),
        ("burst pressure", r.burst_pressure.map(bar)),
        ("rated burn time", r.burn_time.map(crate::format::duration)),
    ];
    for (label, value) in rows {
        if let Some(v) = value {
            g.row(label, v);
        }
    }
    if let (Some(grms), s) = (r.vibration_grms, r.vibration_s) {
        g.row(
            "qualified vibration",
            format!("{grms:.1} g rms{}", s.map(|s| format!(" for {s:.0} s")).unwrap_or_default()),
        );
    }
    g
}

/// The failure modes of a part or component.
fn failures(modes: &[sim::catalog::FailureMode]) -> Option<Group> {
    if modes.is_empty() {
        return None;
    }
    let mut g = Group::new("FAILURE MODES");
    for m in modes {
        let hazard = match m.hazard {
            Hazard::PerHour(x) => format!("{x:.1e} /h"),
            Hazard::PerStart(x) => format!("{x:.1e} /start"),
            Hazard::PerCycle(x) => format!("{x:.1e} /cycle"),
        };
        g.row(&m.mode, format!("{hazard}: {:?}", m.outcome));
    }
    Some(g)
}

/// The ullage's pressure from the ship state (the gas law on its moles,
/// temperature and the volume the liquid leaves; Pa).
pub fn ullage_pressure(def: &sim::craft::ship::TankDef, s: &sim::craft::state::TankState) -> f64 {
    let f = sim::fluids::fluids();
    let density = f.liquid(&def.fluid).map_or(1000.0, |l| l.density(s.liquid_k));
    let volume = (def.volume - s.liquid / density).max(1e-6);
    sim::fluids::pressure(s.ullage_moles, volume, s.ullage_k)
}

/// The groups about part `id` (with its layers' facts `f`), in the one
/// layout (see the module).
fn part_groups(def: &ShipDef, state: &ShipState, id: u32, f: Option<&PartFacts>, k: &Known, out: &mut Vec<Group>) {
    let Some(info) = def.parts.iter().find(|p| p.part == id) else { return };
    let mut g = Group::new(format!("PART {}", info.id));
    g.row("type", info.entry.clone()).row("material", info.material.clone());
    if let Some(finish) = &info.finish {
        g.row("finish", finish.clone());
    }
    if info.ground {
        g.row("ground", "the pad's, not the vehicle's");
    }
    out.push(g);
    let mut g = Group::new("STRUCTURE");
    g.row("dry mass", kg(info.mass)).row("derived", kg(info.derived));
    if let Some((gap, reason)) = &info.unmodelled {
        g.row("unmodelled", format!("{} ({reason})", kg(*gap)));
    }
    if let Some(j) = f.and_then(|f| f.joint.as_ref()) {
        let kind = match j.class {
            JointClass::Integral => "integral",
            JointClass::Bolted => "bolted",
            JointClass::Separation => "separation system",
            JointClass::HoldDown => "hold-down clamps",
        };
        g.row("joint", format!("{} · {kind}", j.name)).row("state", if j.intact { "attached" } else { "separated" });
        // RUNTIME's load at the last live tick, where a strain gauge measures it.
        let load = j.ratio.map_or_else(|| crate::format::DASH.to_string(), |r| format!("{:.0} % / 100 %", r * 100.0));
        g.limit("joint load", load, j.ratio, 1.0);
        // A dash where the craft gives the joint no capacity (D106).
        let dash = || crate::format::DASH.to_string();
        g.row("allowable line load", j.allowable.map_or_else(dash, |a| format!("{:.0} kN/m", a / 1e3)));
        if j.radius > 0.0 {
            let axial = j.allowable.map(|a| a * 2.0 * std::f64::consts::PI * j.radius);
            g.row("axial capacity", axial.map_or_else(dash, kilonewtons));
        }
    }
    out.push(g);
    for (i, e) in def.engines.iter().enumerate().filter(|(_, e)| def.owner_id(e.owner) == info.id) {
        out.push(engine_group(state, i, e, k));
    }
    for (i, _) in def.tanks.iter().enumerate().filter(|(_, t)| t.id.split(':').next() == Some(info.id.as_str())) {
        out.push(tank_group(def, state, i, k));
    }
    for (i, m) in def.mechanisms.iter().enumerate().filter(|(_, m)| m.part == id) {
        let s = state.mechanisms.get(i);
        let mut g = Group::new(format!("MECHANISM {}", m.id));
        g.row("angle", format!("{:.0}°", s.map_or(m.item.stowed_deg, |s| s.angle)))
            .row("target", format!("{:.0}°", s.map_or(m.item.stowed_deg, |s| s.target)))
            .row("stowed · deployed", format!("{:.0}° · {:.0}°", m.item.stowed_deg, m.item.deployed_deg))
            .row("rate · drive", format!("{:.0} °/s · {:?}", m.item.rate_dps, m.item.actuator))
            .row("retractable", if m.item.retractable { "yes" } else { "no" });
        out.push(g);
    }
    for (i, s) in def.surfaces.iter().enumerate().filter(|(_, s)| s.part == id) {
        let mut g = Group::new(format!("AERO SURFACE {}", s.id));
        let now = state.fins.get(i).map_or(0.0, |d| d.to_degrees());
        g.row("kind", if s.item.lattice { "lattice (grid) fin" } else { "fin" })
            .row("area", format!("{:.2} m²", s.item.area))
            .limit(
                "deflection",
                format!("{now:+.1}° / ±{:.0}°", s.item.deflection_deg),
                Some(now.abs()),
                s.item.deflection_deg,
            )
            .row("rate", format!("{:.0} °/s", s.item.rate_dps));
        out.push(g);
    }
    if let Some(f) = f.filter(|f| f.skin.is_some() || f.interior.is_some()) {
        let mut g = Group::new("THERMAL");
        for (label, v) in [("skin", f.skin), ("interior", f.interior)] {
            if let Some((t, limit)) = v {
                g.limit(label, format!("{} / {}", kelvin(t), kelvin(limit)), Some(t), limit);
            }
        }
        out.push(g);
    }
    if let Some(s) = state.parts.iter().find(|s| s.part == id) {
        out.push(condition(s, &info.ratings));
    }
    out.push(ratings(&info.ratings));
    out.extend(failures(&info.failures));
    // Its components last, each a link to its own details.
    let components: Vec<usize> = (0..def.components.len()).filter(|&i| def.components[i].part == id).collect();
    let mut g = Group::new(format!("COMPONENTS · {}", components.len()));
    for &i in &components {
        let c = &def.components[i];
        g.row(&c.id, c.entry.clone()).opens(Selection::Component(i));
    }
    out.push(g);
}

/// A part's wear: integrity, damage (fails at 1), its use against its
/// rated life, its interior's peak temperature against its rating.
fn condition(s: &sim::craft::state::PartState, r: &Ratings) -> Group {
    let mut g = Group::new("CONDITION");
    g.limit("integrity", format!("{:.0} %", s.integrity * 100.0), Some(1.0 - s.integrity), 1.0)
        .limit("thermal damage", format!("{:.2} / 1", s.thermal_damage), Some(s.thermal_damage), 1.0)
        .limit("vibration damage", format!("{:.2} / 1", s.vibration_damage), Some(s.vibration_damage), 1.0);
    let h = &s.history;
    let run = crate::format::duration(h.run_time);
    match r.life {
        Some(Life::Hours(rated)) => {
            g.limit("run time", format!("{run} / {rated:.0} h"), Some(h.run_time / 3600.0), rated);
            g.row("starts · cycles", format!("{} · {}", h.starts, h.cycles));
        }
        Some(Life::Starts(rated)) => {
            g.row("run time", run).limit("starts", format!("{} / {rated:.0}", h.starts), Some(h.starts as f64), rated);
        }
        Some(Life::Cycles(rated)) => {
            g.row("run time", run).limit("cycles", format!("{} / {rated:.0}", h.cycles), Some(h.cycles as f64), rated);
        }
        None => {
            g.row("run time", run).row("starts · cycles", format!("{} · {}", h.starts, h.cycles));
        }
    }
    g.row("flights · landings", format!("{} · {}", h.flights, h.landings));
    if h.peak_k > 0.0 {
        match r.rated_k {
            Some(rated) => {
                g.limit("peak interior", format!("{} / {}", kelvin(h.peak_k), kelvin(rated)), Some(h.peak_k), rated)
            }
            None => g.row("peak interior", kelvin(h.peak_k)),
        };
    }
    g
}

fn tank_group(def: &ShipDef, state: &ShipState, i: usize, k: &Known) -> Group {
    let (t, s) = (&def.tanks[i], &state.tanks[i]);
    let q = |f: fn(u32) -> Quantity| f(i as u32);
    let mut g = Group::new(format!("TANK {}", t.id));
    // What it holds to its sensor's precision (`format::amount`, the
    // card's and the staging bar's rule), and its share of full.
    let read = k.knowledge.read(q(Quantity::TankLevel), s.liquid);
    let level = match read.filter(|_| t.capacity > 0.0) {
        Some(r) => format!("{} ({})", crate::format::amount(read, t.capacity), crate::format::share(r, t.capacity)),
        None => crate::format::amount(read, t.capacity),
    };
    let warm = k.knows(q(Quantity::TankTemperature));
    let p = ullage_pressure(t, s);
    let known_p = k.knows(q(Quantity::TankPressure));
    let pressure = format!("{} / {}", k.value(q(Quantity::TankPressure), p, 1e5, 2, "bar"), bar(t.burst));
    g.row("fluid", t.fluid.clone())
        .row("liquid", level)
        .limit("ullage pressure", pressure, known_p.then_some(p), t.burst)
        .row("operating", bar(t.pressure))
        .row("liquid temperature", k.or_dash(warm, || kelvin(s.liquid_k)))
        .row("ullage temperature", k.or_dash(warm, || kelvin(s.ullage_k)));
    g
}

/// Ignitions left, in words, for engines with `left` each (one rule: the
/// systems screen's engine, the part card's engine and engine groups): a
/// hypergolic engine's starts are unlimited (`u32::MAX`), the pad or not;
/// while the craft is clamped a start uses the pad's supply, not one of its
/// own (`ShipState::apply`, Falcon 9's TEA-TEB on the pad), said after the
/// count.
pub fn ignitions_left(left: &[u32], clamped: bool) -> String {
    let (lo, hi) = (left.iter().min().copied().unwrap_or(0), left.iter().max().copied().unwrap_or(0));
    let each = if left.len() > 1 { " each" } else { "" };
    let count = match (lo, hi) {
        (u32::MAX, _) => return "unlimited (hypergolic)".to_string(),
        (lo, hi) if lo == hi => format!("{lo}{each}"),
        (lo, u32::MAX) => format!("{lo} to unlimited"),
        (lo, hi) => format!("{lo}–{hi}"),
    };
    match (clamped, left.len()) {
        (false, _) => count,
        (true, 1) => format!("{count} (the pad lights it)"),
        (true, _) => format!("{count} (the pad lights them)"),
    }
}

fn engine_group(state: &ShipState, i: usize, e: &sim::craft::ship::EngineDef, k: &Known) -> Group {
    let mut g = Group::new(format!("ENGINE {}", e.id));
    let d = &e.data;
    if let Some(s) = state.engines.get(i) {
        let ignitions = ignitions_left(&[s.ignitions], state.clamped);
        let level = state.engines.get(i).map_or(0.0, |r| r.level);
        let chamber =
            k.or_dash(k.knows(Quantity::EngineThrust(i as u32)), || format!("{:.0} % / 100 %", level * 100.0));
        g.row("status", format!("{:?}", s.status))
            .row("chamber pressure", chamber)
            .row("ignitions left", ignitions)
            .row("own throttle", format!("{:.0} %", s.throttle * 100.0))
            .row("selected", if s.selected { "yes (the pilot's throttle)" } else { "no" });
    }
    g.row(
        "thrust vac · SL",
        format!("{} · {}", kilonewtons(d.thrust_vac), kilonewtons(d.thrust_vac * d.isp_sl / d.isp_vac)),
    )
    .row("Isp vac · SL", format!("{:.0} s · {:.0} s", d.isp_vac, d.isp_sl))
    .row("throttle range", format!("{:.0}–{:.0} %", d.throttle_min * 100.0, d.throttle_max * 100.0))
    .row("gimbal", format!("±{:.1}°", d.gimbal_deg))
    .row("propellants", format!("{} / {} at {:.2}", d.oxidizer, d.fuel, d.mixture_ratio))
    .row("igniter", format!("{:?}", d.igniter));
    g
}

/// A component's bottle, battery and valve groups (its stored energy and
/// pressure as the craft measures them).
fn stores(def: &ShipDef, state: &ShipState, id: &str, k: &Known, out: &mut Vec<Group>) {
    for (j, b) in def.bottles.iter().enumerate().filter(|(_, b)| b.id == id) {
        let s = &state.bottles[j];
        let gas = sim::fluids::fluids().gas(&b.gas).map_or(0.0, |g| g.molar_mass);
        let q = Quantity::BottlePressure(j as u32);
        let pressure = k.value(q, sim::fluids::pressure(s.moles, b.volume, s.k), 1e5, 2, "bar");
        let p = sim::fluids::pressure(s.moles, b.volume, s.k);
        let mut g = Group::new(format!("BOTTLE {}", b.id));
        g.row("gas", b.gas.clone())
            .row("contents", k.or_dash(k.knows(q), || kg(s.moles * gas)))
            .limit("pressure", format!("{pressure} / {}", bar(b.burst)), k.knows(q).then_some(p), b.burst)
            .row("rated", bar(b.pressure))
            .row("temperature", k.or_dash(k.knowledge.perfect, || kelvin(s.k)));
        out.push(g);
    }
    for (j, _) in def.tanks.iter().enumerate().filter(|(_, t)| t.id.split(':').next() == Some(id)) {
        out.push(tank_group(def, state, j, k));
    }
    for (j, b) in def.batteries.iter().enumerate().filter(|(_, b)| b.id == id) {
        let s = &state.batteries[j];
        let charge = k.knowledge.read(Quantity::BatteryCharge(j as u32), s.charge);
        let mut g = Group::new(format!("BATTERY {}", b.id));
        g.row("charge", crate::format::charge(charge, b.item.capacity))
            .row("voltage", format!("{:.0} V", b.item.voltage))
            .row("temperature", k.or_dash(k.knowledge.perfect, || kelvin(s.k)));
        // The bus it feeds, as `systems::power` runs it.
        for bus in def.buses.iter().filter(|bus| bus.members.iter().any(|m| m.0 == b.owner)) {
            if let Some(run) = state.buses.iter().find(|r| r.name == bus.name) {
                g.row("bus", format!("{} · {:.1} V · {:.0} W", run.name, run.volts, run.load));
            }
        }
        out.push(g);
    }
    for (j, v) in def.valves.iter().enumerate().filter(|(_, v)| v.id == id) {
        let s = &state.valves[j];
        let open = format!("{}{}", if s.open { "open" } else { "closed" }, if s.stuck { " · STUCK" } else { "" });
        let mut g = Group::new(format!("VALVE {}", v.id));
        g.row("kind", format!("{:?}", v.item.kind)).row("state", open).limit(
            "cycles",
            format!("{} / {:.0}", s.cycles, v.item.cycles),
            Some(s.cycles as f64),
            v.item.cycles,
        );
        out.push(g);
    }
}

/// The groups about component `i`.
fn component_groups(def: &ShipDef, state: &ShipState, i: usize, k: &Known, out: &mut Vec<Group>) {
    let Some(c) = def.components.get(i) else { return };
    let host = def.parts.iter().find(|p| p.part == c.part).map_or("?", |p| p.id.as_str());
    let mut g = Group::new(format!("COMPONENT {}", c.id));
    g.row("type", c.entry.clone()).row("in part", host).opens(Selection::Part(c.part)).row("mass", kg(c.mass));
    if c.power > 0.0 {
        let on = state.power.get(i).is_none_or(|s| s.on);
        g.row("power", format!("{:.0} W · {}", c.power, if on { "on" } else { "off" }));
    }
    out.push(g);
    out.push(ratings(&c.ratings));
    out.extend(failures(&c.failures));
    stores(def, state, &c.id, k, out);
    let mine = |id: &str| id == c.id;
    for (j, t) in def.thrusters.iter().enumerate().filter(|(_, t)| mine(&t.id)) {
        let on = state.thrusters.get(j).is_some_and(|s| s.enabled);
        let mut g = Group::new(format!("THRUSTERS {}", t.id));
        g.row("nozzles", format!("{} × {:.0} N, Isp {:.0} s", t.nozzles.len(), t.thrust, t.isp))
            .row("enabled", if on { "yes" } else { "no" });
        out.push(g);
    }
    for (j, e) in def.engines.iter().enumerate().filter(|(_, e)| def.owner_id(e.owner) == c.id) {
        out.push(engine_group(state, j, e, k));
    }
    for s in def.sensors.iter().filter(|s| mine(&s.id)) {
        let mut g = Group::new(format!("SENSOR {}", s.id));
        g.row("measures", s.item.quantity.clone())
            .row("accuracy", format!("{:.3e}", s.item.accuracy))
            .row("rate", format!("{:.0} Hz", s.item.rate_hz));
        out.push(g);
    }
    for computer in def.computers.iter().filter(|x| mine(&x.id)) {
        let mut g = Group::new(format!("COMPUTER {}", computer.id));
        g.row("channels", computer.item.channels.to_string());
        let links: Vec<&str> = (def.links.iter().filter(|l| def.owner_id(l.from.0) == c.id))
            .flat_map(|l| l.to.iter().map(|m| def.owner_id(m.0)))
            .collect();
        g.row("commands", if links.is_empty() { "—".to_string() } else { links.join(", ") });
        out.push(g);
    }
    for s in def.seats.iter().filter(|s| mine(&s.id)) {
        let mut g = Group::new(format!("SEAT {}", s.id));
        g.row("crew", s.item.to_string());
        out.push(g);
    }
}

/// Everything known about `sel` (`facts`: the parts' layer facts; `k`:
/// what the craft measures, D106: a dash for the rest).
pub fn details(def: &ShipDef, state: &ShipState, facts: &[PartFacts], sel: Selection, k: &Known) -> Vec<Group> {
    let mut out = Vec::new();
    match sel {
        Selection::Part(id) => part_groups(def, state, id, facts.iter().find(|f| f.id == id), k, &mut out),
        Selection::Component(i) => component_groups(def, state, i, k, &mut out),
    }
    out.retain(|g| !g.rows.is_empty());
    out
}

/// The commands `sel` takes now: its engines (ignite or shut down), its
/// mechanisms (deploy or retract), valves, thruster blocks, a component's
/// power, and the separation of a part's joint or the pad's clamps.
pub fn commands(def: &ShipDef, state: &ShipState, sel: Selection) -> Vec<(String, ShipCommand)> {
    let mut out: Vec<ShipCommand> = Vec::new();
    let (id, part) = match sel {
        Selection::Part(p) => match def.parts.iter().find(|i| i.part == p) {
            Some(info) => (info.id.clone(), Some(p)),
            None => return Vec::new(),
        },
        Selection::Component(i) => match def.components.get(i) {
            Some(c) => (c.id.clone(), None),
            None => return Vec::new(),
        },
    };
    let engines: Vec<usize> = (0..def.engines.len()).filter(|&i| def.owner_id(def.engines[i].owner) == id).collect();
    if !engines.is_empty() {
        let on = |s: EngineStatus| matches!(s, EngineStatus::Starting | EngineStatus::Running);
        let running = engines.iter().any(|&i| state.engines.get(i).is_some_and(|e| on(e.status)));
        out.push(if running { ShipCommand::Shutdown(id.clone()) } else { ShipCommand::Ignite(id.clone()) });
    }
    for (i, m) in def.mechanisms.iter().enumerate().filter(|(_, m)| Some(m.part) == part) {
        let deployed = state.mechanisms.get(i).is_some_and(|s| (s.angle - m.item.deployed_deg).abs() < 0.5);
        match (deployed, m.item.retractable) {
            (false, _) => out.push(ShipCommand::Deploy(m.id.clone())),
            (true, true) => out.push(ShipCommand::Retract(m.id.clone())),
            (true, false) => {}
        }
    }
    for (i, v) in def.valves.iter().enumerate().filter(|(_, v)| v.id == id) {
        let open = state.valves.get(i).is_some_and(|s| s.open);
        out.push(if open { ShipCommand::Close(v.id.clone()) } else { ShipCommand::Open(v.id.clone()) });
    }
    for (i, t) in def.thrusters.iter().enumerate().filter(|(_, t)| t.id == id) {
        let on = state.thrusters.get(i).is_some_and(|s| s.enabled);
        out.push(ShipCommand::Rcs(t.id.clone(), !on));
    }
    if let Selection::Component(i) = sel {
        if def.components[i].power > 0.0 {
            let on = state.power.get(i).is_none_or(|s| s.on);
            out.push(ShipCommand::Power(id.clone(), !on));
        }
    }
    if let Some(p) = part {
        for (i, j) in def.joints.iter().enumerate().filter(|(_, j)| j.child == p) {
            let intact = state.joints.get(i).is_none_or(|s| s.intact);
            match j.kind {
                sim::craft::assembly::JointKind::Separation { .. } if intact => {
                    out.push(ShipCommand::Separate(j.name.clone()));
                }
                sim::craft::assembly::JointKind::HoldDown if intact => out.push(ShipCommand::ReleaseClamps),
                _ => {}
            }
        }
    }
    out.dedup();
    out.into_iter().map(|c| (label(&c), c)).collect()
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
