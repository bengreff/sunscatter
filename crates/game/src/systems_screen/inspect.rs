//! The systems screen's inspector (D107; rule 8): everything known about
//! a selected part or component, as titled groups of labelled values, and
//! the commands it takes. Pure functions of the ship definition and state
//! (and the layers' facts), table-tested.
//!
//! Values are what the simulation holds now (perfect information until the
//! knowledge model, D106, gives each its precision or a dash). What the
//! simulation does not compute yet says so: joint loads, power flow.

use super::rules::{JointClass, PartFacts};
use crate::staging::parts::label;
use sim::catalog::{Hazard, Life, Ratings};
use sim::craft::command::ShipCommand;
use sim::craft::ship::ShipDef;
use sim::craft::state::{EngineStatus, ShipState};

/// What the inspector shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    /// A part, by its id (`Part::id`).
    Part(u32),
    /// A component, by its index in the ship definition.
    Component(usize),
}

/// A titled group of labelled values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Group {
    pub title: String,
    pub rows: Vec<(String, String)>,
}

impl Group {
    fn new(title: impl Into<String>) -> Self {
        Group { title: title.into(), rows: Vec::new() }
    }

    fn row(&mut self, label: &str, value: impl Into<String>) -> &mut Self {
        self.rows.push((label.to_string(), value.into()));
        self
    }
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

/// The groups about part `id` (with its layers' facts `f`).
fn part_groups(def: &ShipDef, state: &ShipState, id: u32, f: Option<&PartFacts>, out: &mut Vec<Group>) {
    let Some(info) = def.parts.iter().find(|p| p.part == id) else { return };
    let mut g = Group::new(format!("PART {}", info.id));
    g.row("catalog entry", info.entry.clone()).row("dry mass", kg(info.mass)).row("derived", kg(info.derived));
    if let Some((gap, reason)) = &info.unmodelled {
        g.row("unmodelled", format!("{} ({reason})", kg(*gap)));
    }
    g.row("material", info.material.clone());
    if let Some(finish) = &info.finish {
        g.row("finish", finish.clone());
    }
    if info.ground {
        g.row("ground", "the pad's, not the vehicle's");
    }
    out.push(g);
    if let Some(j) = f.and_then(|f| f.joint.as_ref()) {
        let mut g = Group::new(format!("JOINT {}", j.name));
        let kind = match j.class {
            JointClass::Integral => "integral",
            JointClass::Bolted => "bolted",
            JointClass::Separation => "separation system",
            JointClass::HoldDown => "hold-down clamps",
        };
        g.row("kind", kind).row("state", if j.intact { "attached" } else { "separated" });
        g.row("allowable line load", format!("{:.0} kN/m", j.allowable / 1e3));
        if j.radius > 0.0 {
            g.row("axial capacity", kilonewtons(j.allowable * 2.0 * std::f64::consts::PI * j.radius));
        }
        g.row("load", "not simulated yet (RUNTIME)");
        out.push(g);
    }
    out.push(ratings(&info.ratings));
    out.extend(failures(&info.failures));
    if let Some(s) = state.parts.iter().find(|s| s.part == id) {
        let mut g = Group::new("CONDITION");
        g.row("integrity", format!("{:.0} %", s.integrity * 100.0))
            .row("thermal damage", format!("{:.2}", s.thermal_damage))
            .row("vibration damage", format!("{:.2}", s.vibration_damage));
        let h = &s.history;
        g.row("run time", crate::format::duration(h.run_time))
            .row("starts · cycles", format!("{} · {}", h.starts, h.cycles))
            .row("flights · landings", format!("{} · {}", h.flights, h.landings));
        if h.peak_k > 0.0 {
            g.row("peak temperature", kelvin(h.peak_k));
        }
        out.push(g);
    }
    if let Some(f) = f.filter(|f| f.skin.is_some() || f.interior.is_some()) {
        let mut g = Group::new("THERMAL");
        for (label, v) in [("skin (nearest its limit)", f.skin), ("interior (nearest its limit)", f.interior)] {
            if let Some((k, limit)) = v {
                g.row(label, format!("{} / {}", kelvin(k), kelvin(limit)));
            }
        }
        out.push(g);
    }
    for (i, _) in def.tanks.iter().enumerate().filter(|(_, t)| t.id.split(':').next() == Some(info.id.as_str())) {
        out.push(tank_group(def, state, i));
    }
    for (i, e) in def.engines.iter().enumerate().filter(|(_, e)| e.owner == info.id) {
        out.push(engine_group(state, i, e));
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
    for s in def.surfaces.iter().filter(|s| s.part == id) {
        let mut g = Group::new(format!("AERO SURFACE {}", s.id));
        g.row("kind", if s.item.lattice { "lattice (grid) fin" } else { "fin" })
            .row("area", format!("{:.2} m²", s.item.area))
            .row("deflection", format!("±{:.0}° at {:.0} °/s", s.item.deflection_deg, s.item.rate_dps))
            .row("aerodynamics", "the lattice-fin model is RUNTIME's (not simulated yet)");
        out.push(g);
    }
}

fn tank_group(def: &ShipDef, state: &ShipState, i: usize) -> Group {
    let (t, s) = (&def.tanks[i], &state.tanks[i]);
    let mut g = Group::new(format!("TANK {}", t.id));
    let fill = if t.capacity > 0.0 { s.liquid / t.capacity } else { 0.0 };
    g.row("fluid", t.fluid.clone())
        .row("liquid", format!("{} / {} ({:.1} %)", kg(s.liquid), kg(t.capacity), fill * 100.0))
        .row("liquid temperature", kelvin(s.liquid_k))
        .row("ullage pressure", bar(ullage_pressure(t, s)))
        .row("ullage temperature", kelvin(s.ullage_k))
        .row("operating · burst", format!("{} · {}", bar(t.pressure), bar(t.burst)));
    g
}

fn engine_group(state: &ShipState, i: usize, e: &sim::craft::ship::EngineDef) -> Group {
    let mut g = Group::new(format!("ENGINE {}", e.id));
    let d = &e.data;
    if let Some(s) = state.engines.get(i) {
        let ignitions =
            if s.ignitions == u32::MAX { "unlimited (hypergolic)".to_string() } else { s.ignitions.to_string() };
        g.row("status", format!("{:?}", s.status))
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

/// The groups about component `i`.
fn component_groups(def: &ShipDef, state: &ShipState, i: usize, out: &mut Vec<Group>) {
    let Some(c) = def.components.get(i) else { return };
    let host = def.parts.iter().find(|p| p.part == c.part).map_or("?", |p| p.id.as_str());
    let mut g = Group::new(format!("COMPONENT {}", c.id));
    g.row("catalog entry", c.entry.clone()).row("in part", host).row("mass", kg(c.mass));
    if c.power > 0.0 {
        let on = state.power.get(i).is_none_or(|s| s.on);
        g.row("power", format!("{:.0} W · {}", c.power, if on { "on" } else { "off" }));
    }
    out.push(g);
    out.push(ratings(&c.ratings));
    out.extend(failures(&c.failures));
    let mine = |id: &str| id == c.id;
    for (k, b) in def.bottles.iter().enumerate().filter(|(_, b)| mine(&b.id)) {
        let s = &state.bottles[k];
        let gas = sim::fluids::fluids().gas(&b.gas).map_or(0.0, |g| g.molar_mass);
        let mut g = Group::new(format!("BOTTLE {}", b.id));
        g.row("gas", b.gas.clone())
            .row("contents", kg(s.moles * gas))
            .row(
                "pressure",
                format!("{} (rated {})", bar(sim::fluids::pressure(s.moles, b.volume, s.k)), bar(b.pressure)),
            )
            .row("temperature", kelvin(s.k))
            .row("burst", bar(b.burst));
        out.push(g);
    }
    for (k, _) in def.tanks.iter().enumerate().filter(|(_, t)| t.id.split(':').next() == Some(c.id.as_str())) {
        out.push(tank_group(def, state, k));
    }
    for (k, b) in def.batteries.iter().enumerate().filter(|(_, b)| mine(&b.id)) {
        let s = &state.batteries[k];
        let mut g = Group::new(format!("BATTERY {}", b.id));
        g.row("charge", format!("{:.0} / {:.0} Wh", s.charge / 3600.0, b.item.capacity / 3600.0))
            .row("voltage", format!("{:.0} V", b.item.voltage))
            .row("temperature", kelvin(s.k))
            .row("use", "power flow is not simulated yet (RUNTIME)");
        out.push(g);
    }
    for (k, v) in def.valves.iter().enumerate().filter(|(_, v)| mine(&v.id)) {
        let s = &state.valves[k];
        let mut g = Group::new(format!("VALVE {}", v.id));
        g.row("kind", format!("{:?}", v.item.kind))
            .row(
                "state",
                format!("{}{}", if s.open { "open" } else { "closed" }, if s.stuck { " · STUCK" } else { "" }),
            )
            .row("cycles", format!("{} of {:.0} rated", s.cycles, v.item.cycles));
        out.push(g);
    }
    for (k, t) in def.thrusters.iter().enumerate().filter(|(_, t)| mine(&t.id)) {
        let on = state.thrusters.get(k).is_some_and(|s| s.enabled);
        let mut g = Group::new(format!("THRUSTERS {}", t.id));
        g.row("nozzles", format!("{} × {:.0} N, Isp {:.0} s", t.nozzles.len(), t.thrust, t.isp))
            .row("enabled", if on { "yes" } else { "no" });
        out.push(g);
    }
    for (k, e) in def.engines.iter().enumerate().filter(|(_, e)| e.owner == c.id) {
        out.push(engine_group(state, k, e));
    }
    for s in def.sensors.iter().filter(|s| mine(&s.id)) {
        let mut g = Group::new(format!("SENSOR {}", s.id));
        g.row("measures", s.item.quantity.clone())
            .row("accuracy", format!("{:.3e}", s.item.accuracy))
            .row("rate", format!("{:.0} Hz", s.item.rate_hz));
        out.push(g);
    }
    for k in def.computers.iter().filter(|k| mine(&k.id)) {
        let mut g = Group::new(format!("COMPUTER {}", k.id));
        g.row("channels", k.item.channels.to_string());
        let links: Vec<&str> =
            (def.links.iter().filter(|l| l.from.0 == c.id)).flat_map(|l| l.to.iter().map(|m| m.0.as_str())).collect();
        g.row("commands", if links.is_empty() { "—".to_string() } else { links.join(", ") });
        out.push(g);
    }
    for s in def.seats.iter().filter(|s| mine(&s.id)) {
        out.push(Group { title: format!("SEAT {}", s.id), rows: vec![("crew".into(), s.item.to_string())] });
    }
}

/// Everything known about `sel` (`facts`: the parts' layer facts).
pub fn details(def: &ShipDef, state: &ShipState, facts: &[PartFacts], sel: Selection) -> Vec<Group> {
    let mut out = Vec::new();
    match sel {
        Selection::Part(id) => part_groups(def, state, id, facts.iter().find(|f| f.id == id), &mut out),
        Selection::Component(i) => component_groups(def, state, i, &mut out),
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
    let engines: Vec<usize> = (0..def.engines.len()).filter(|&i| def.engines[i].owner == id).collect();
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
