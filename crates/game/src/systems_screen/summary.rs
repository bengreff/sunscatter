//! The systems screen's layer summaries (D107; rule 8): a layer shown as
//! a whole before any part, in the inspector's groups and rows (a row
//! against a limit coloured by `format::limit_level`; a row about a part
//! selects it).
//!
//! * **Structure**: the mass now (`sim::systems::estimate::aboard`, the
//!   staging bar's rule: to the precision the tank sensors give) and the
//!   centre of mass (`estimate::centre`, the same rule), the g-limit and what
//!   sets it (`sim::systems::g_limit`), the joints nearest their ultimate.
//! * **Thermal**: the parts nearest their temperature limits.
//! * **Plumbing**: each tank's contents, the tanks' and bottles' pressures
//!   against their burst, what feeds what (the lines), the engines' flows.
//! * **Electrical**: the Power menu's own sections (`menus::power`: one
//!   owner): the buses, the batteries, what is without power and why.
//! * **Data**: the computers, the sensors by kind, what the craft measures.
//! * **Health**: the parts' wear, what is lost and why.
//!
//! Like every readout, only what the craft measures (D106): a dash, and a
//! count of what nothing measures.

use super::inspect::{Group, Selection};
use super::rules::{Layer, PartFacts};
use crate::format::{self, limit_level, Known, Level, DASH};
use crate::interface::menus::{power, Item, Tone};
use sim::craft::ship::ShipDef;
use sim::craft::state::ShipState;
use sim::systems::Quantity;

/// What the summaries read besides the ship.
pub struct Inputs<'a> {
    pub def: &'a ShipDef,
    pub state: &'a ShipState,
    pub facts: &'a [PartFacts],
    pub known: &'a Known,
    /// What is aboard now: its mass to its precision, its dry mass
    /// (`sim::systems::estimate::aboard`, the staging bar's own).
    pub aboard: sim::systems::estimate::Aboard,
    /// The centre of mass's height above the lowest point aboard, as the
    /// craft knows it (`sim::systems::estimate::centre`: the pad's mount
    /// left out, to the tank sensors' precision).
    pub com: sim::systems::Reading,
    /// The electrical system (`menus::power::gather`).
    pub power: Option<&'a power::PowerData>,
    /// The proper acceleration the craft feels now, body axes: the
    /// navball's G (`navball::felt`, one owner).
    pub accel: glam::DVec3,
}

/// How many joints or parts a "nearest their limits" list shows.
pub const TOP: usize = 6;

/// Layer `layer` as a whole.
pub fn summary(layer: Layer, x: &Inputs) -> Vec<Group> {
    let mut out = match layer {
        Layer::Physical => vec![craft(x)],
        Layer::Structural => structure(x),
        Layer::Thermal => vec![thermal(x)],
        Layer::Plumbing => fluids(x),
        Layer::Electrical => electrical(x.power),
        Layer::Data => data(x),
        Layer::Health => health(x),
    };
    out.retain(|g| !g.rows.is_empty());
    out
}

/// A mass the craft's files give exactly (its dry mass), read as every mass
/// is (`format::mass`).
fn exact(kg: f64) -> String {
    format::mass(Some(sim::systems::Reading { value: kg, precision: 0.0 }))
}

fn name(def: &ShipDef, part: u32) -> String {
    def.parts.iter().find(|p| p.part == part).map_or_else(|| format!("part {part}"), |p| p.id.clone())
}

/// The craft: its parts and components, its mass.
fn craft(x: &Inputs) -> Group {
    let mut g = Group::new("CRAFT");
    let ground = x.facts.iter().filter(|f| f.ground).count();
    g.row("parts", format!("{} (and {ground} of the pad's)", x.facts.len() - ground))
        .row("components", x.def.components.len().to_string())
        .row("mass", format::mass(Some(x.aboard.mass)))
        .row("dry mass", exact(x.aboard.dry));
    g
}

/// The mass and centre of mass, the g-limit, the joints nearest their
/// ultimate (measured by strain gauges) and how many nothing measures.
fn structure(x: &Inputs) -> Vec<Group> {
    let (def, state, k) = (x.def, x.state, x.known);
    let mut g = Group::new("STRUCTURE");
    g.row("mass", format::mass(Some(x.aboard.mass)))
        .row("dry mass", exact(x.aboard.dry))
        .row("centre of mass", format!("{} above the base", format::metres(x.com)));
    let g0 = crate::navball::rules::G0;
    let now = x.accel.length() / g0;
    use sim::systems::g_limit::{g_limit, ACCELERATING};
    match g_limit(def, state, x.accel) {
        _ if !k.knows(Quantity::Acceleration) => {
            g.row("g-limit", DASH);
        }
        Some(l) => {
            let by = crate::navball::g_meter::limiter(l.by, |j| def.joints.get(j).map(|j| j.name.clone()));
            g.limit("g-limit", format!("{now:.1} g / {:.1} g", l.accel / g0), Some(now), l.accel / g0)
                .row("set by", by);
        }
        None if x.accel.length() < ACCELERATING => {
            g.row("g-limit", "none: not accelerating");
        }
        None => {
            g.row("g-limit", "none within 100 g");
        }
    }
    let mut joints = Group::new("JOINTS NEAREST THEIR ULTIMATE");
    let intact = |i: usize| state.joints.get(i).is_none_or(|s| s.intact);
    let rated = |i: usize| sim::systems::structure::rated(def, &def.joints[i]);
    let mut loads: Vec<(usize, f64)> = (0..def.joints.len())
        .filter(|&i| intact(i) && rated(i) && k.knows(Quantity::JointLoad(i as u32)))
        .filter_map(|i| Some((i, state.joints.get(i)?.ratio)))
        .collect();
    loads.sort_by(|a, b| b.1.total_cmp(&a.1));
    for &(i, r) in loads.iter().take(TOP) {
        let j = &def.joints[i];
        joints.limit(&j.name, format!("{:.0} % / 100 %", r * 100.0), Some(r), 1.0).opens(Selection::Part(j.child));
    }
    let unrated = (0..def.joints.len()).filter(|&i| intact(i) && !rated(i)).count();
    let blind = (0..def.joints.len()).filter(|&i| intact(i) && rated(i)).count() - loads.len();
    if blind > 0 {
        joints.row("not measured", format!("{blind} joints (no strain gauge)"));
    }
    if unrated > 0 {
        joints.row("no capacity", format!("{unrated} joints (no allowable given, or hung by a hinge)"));
    }
    vec![g, joints]
}

/// The parts nearest their temperature limits (each by its skin cell or
/// interior node nearest its own, `rules::nearest_limits`), and how many
/// nothing measures.
fn thermal(x: &Inputs) -> Group {
    let mut g = Group::new("NEAREST THEIR LIMITS");
    let parts = x.facts.iter().filter(|f| !f.ground).map(|f| (f, super::rules::nearer(f.skin, f.interior)));
    let (hot, unmeasured) = super::rules::nearest_limits(parts, TOP);
    for (f, (t, limit)) in hot {
        g.limit(&f.name, format!("{t:.0} K / {limit:.0} K"), Some(t), limit).opens(Selection::Part(f.id));
    }
    if unmeasured > 0 {
        g.row("not measured", format!("{unmeasured} parts (no temperature sensor)"));
    }
    g
}

/// The tanks' contents, the pressures against their burst, the lines,
/// the engines' flows now.
fn fluids(x: &Inputs) -> Vec<Group> {
    let (def, state, k) = (x.def, x.state, x.known);
    let mut tanks = Group::new("TANKS");
    let mut pressures = Group::new("PRESSURES (burst)");
    for (i, (d, s)) in def.tanks.iter().zip(&state.tanks).enumerate() {
        let q = |f: fn(u32) -> Quantity| f(i as u32);
        let level = format::amount(k.knowledge.read(q(Quantity::TankLevel), s.liquid), d.capacity);
        tanks.row(&format!("{} · {}", d.id, d.fluid), level);
        tanks.opens(Selection::Part(d.part));
        let p = super::inspect::ullage_pressure(d, s);
        let known = k.knows(q(Quantity::TankPressure));
        let read = k.knowledge.read(q(Quantity::TankPressure), p);
        let shown = format!("{} / {}", format::pressure(read), format::bar(d.burst));
        pressures.limit(&d.id, shown, known.then_some(p), d.burst).opens(Selection::Part(d.part));
    }
    for (j, (b, s)) in def.bottles.iter().zip(&state.bottles).enumerate() {
        let q = Quantity::BottlePressure(j as u32);
        let p = sim::fluids::pressure(s.moles, b.volume, s.k);
        let shown = format!("{} / {}", format::pressure(k.knowledge.read(q, p)), format::bar(b.burst));
        pressures.limit(&format!("{} · {}", b.id, b.gas), shown, k.knows(q).then_some(p), b.burst);
        if let Some(c) = def.components.iter().position(|c| c.id == b.id) {
            pressures.opens(Selection::Component(c));
        }
    }
    vec![tanks, pressures, feeds(def), flows(x)]
}

/// What feeds what: each line's owner and what the lines from it reach
/// (by their ends' owners, each once; past two, how many more).
fn feeds(def: &ShipDef) -> Group {
    let mut g = Group::new("LINES");
    let owner = |end: &str| end.split(':').next().unwrap_or(end).to_string();
    let mut from: Vec<(String, Vec<String>)> = Vec::new();
    for l in &def.lines {
        let (a, b) = (owner(&l.from), owner(&l.to));
        match from.iter_mut().find(|f| f.0 == a) {
            Some(f) if f.1.contains(&b) => {}
            Some(f) => f.1.push(b),
            None => from.push((a, vec![b])),
        }
    }
    for (a, to) in from {
        let more = if to.len() > 2 { format!(" +{}", to.len() - 2) } else { String::new() };
        g.row(&a, format!("→ {}{more}", to[..to.len().min(2)].join(", ")));
    }
    g
}

/// Each burning engine's propellant flow now (`systems::propulsion`'s, at
/// its flow level), where its controller reports it.
fn flows(x: &Inputs) -> Group {
    let mut g = Group::new("FLOWS NOW");
    let mut total = 0.0;
    for (i, e) in x.def.engines.iter().enumerate() {
        let level = x.state.engines.get(i).map_or(0.0, |r| r.level);
        if level <= 0.0 || !x.known.knows(Quantity::EngineThrust(i as u32)) {
            continue;
        }
        let mdot = sim::systems::propulsion::steady(&e.data, level, 0.0).1;
        total += mdot;
        g.row(&e.id, format!("{mdot:.1} kg/s")).opens(Selection::Part(e.part));
    }
    if g.rows.is_empty() {
        g.row("engines", "none burning");
    } else if g.rows.len() > 1 {
        g.row("total", format!("{total:.1} kg/s"));
    }
    g
}

/// The Power menu's sections (one owner) as groups, named by id as every
/// layer names things (the menu writes them in capitals): a warning amber,
/// an alert red.
fn electrical(p: Option<&power::PowerData>) -> Vec<Group> {
    (power::sections_by_id(p).into_iter())
        .map(|s| {
            let mut g = Group::new(if s.title.is_empty() { "BUSES".to_string() } else { s.title.clone() });
            for item in &s.items {
                match item {
                    Item::Row(r) => {
                        g.row(&r.label, r.value.clone()).level(match r.tone {
                            Tone::Warn => Level::Warn,
                            Tone::Alert => Level::Over,
                            _ => Level::Normal,
                        });
                    }
                    Item::Note(text, _) => {
                        g.row("", text.clone());
                    }
                    _ => {}
                }
            }
            g
        })
        .collect()
}

/// The computers, the sensors by kind, and what the craft measures of
/// what it could (D106).
fn data(x: &Inputs) -> Vec<Group> {
    let (def, k) = (x.def, x.known);
    let mut computers = Group::new("COMPUTERS");
    for c in &def.computers {
        let commands: usize = def.links.iter().filter(|l| def.owner_id(l.from.0) == c.id).map(|l| l.to.len()).sum();
        computers.row(&c.id, format!("{} channels · commands {commands}", c.item.channels));
        if let Some(i) = def.components.iter().position(|x| x.id == c.id) {
            computers.opens(Selection::Component(i));
        }
    }
    let mut sensors = Group::new(format!("SENSORS · {}", def.sensors.len()));
    let mut kinds: Vec<(&str, usize)> = Vec::new();
    for s in &def.sensors {
        match kinds.iter_mut().find(|(q, _)| *q == s.item.quantity) {
            Some(k) => k.1 += 1,
            None => kinds.push((&s.item.quantity, 1)),
        }
    }
    for (kind, n) in kinds {
        sensors.row(kind, n.to_string());
    }
    let mut known = Group::new("WHAT THE CRAFT MEASURES");
    let craft = x.facts.iter().filter(|f| !f.ground);
    let count = |n: usize, q: &dyn Fn(u32) -> Quantity| (0..n as u32).filter(|&i| k.knows(q(i))).count();
    let rows: [(&str, usize, usize); 6] = [
        ("tank levels", count(def.tanks.len(), &Quantity::TankLevel), def.tanks.len()),
        ("tank pressures", count(def.tanks.len(), &Quantity::TankPressure), def.tanks.len()),
        ("bottle pressures", count(def.bottles.len(), &Quantity::BottlePressure), def.bottles.len()),
        ("battery charges", count(def.batteries.len(), &Quantity::BatteryCharge), def.batteries.len()),
        ("joint loads", count(def.joints.len(), &Quantity::JointLoad), def.joints.len()),
        // The craft's own parts, as the thermal and health layers count
        // them (not the pad's).
        (
            "part temperatures",
            craft.clone().filter(|f| k.knows(Quantity::PartTemperature(f.id))).count(),
            craft.count(),
        ),
    ];
    for (label, n, of) in rows.into_iter().filter(|r| r.2 > 0) {
        known.row(label, format!("{n} of {of}"));
    }
    if k.knowledge.perfect {
        known.row("", "debug mode: everything");
    }
    vec![computers, sensors, known]
}

/// The parts' wear (the worst first) and what is lost and why.
fn health(x: &Inputs) -> Vec<Group> {
    let mut g = Group::new("CONDITION");
    let parts: Vec<&PartFacts> = x.facts.iter().filter(|f| !f.ground).collect();
    let worn = |f: &PartFacts| f.damage.max(1.0 - f.integrity);
    let mut hurt: Vec<&PartFacts> = parts.iter().copied().filter(|f| worn(f) > 0.0).collect();
    hurt.sort_by(|a, b| worn(b).total_cmp(&worn(a)));
    g.row("sound", format!("{} of {} parts", parts.len() - hurt.len(), parts.len()));
    for f in hurt.iter().take(TOP) {
        let text = format!("integrity {:.0} % · damage {:.2} / 1", f.integrity * 100.0, f.damage);
        g.row(&f.name, text).level(limit_level(worn(f), Some(1.0))).opens(Selection::Part(f.id));
    }
    let mut lost = Group::new(format!("LOST · {}", x.state.lost.len()));
    for l in &x.state.lost {
        lost.row(x.def.owner_id(l.owner), format!("{} ({})", l.why, name(x.def, l.part))).level(Level::Over);
    }
    vec![g, lost]
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;
