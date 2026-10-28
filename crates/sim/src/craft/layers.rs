//! The layers of a craft of parts (D100; ship-model.md §2, §4): plumbing
//! lines and what feeds each engine and thruster block, joints from the
//! attachments (bolted by default; declared ones integral or separation
//! systems), electrical buses, data links, engine groups, and the check that
//! every command of the sequence and the functions names something the
//! craft has. Every error names what is wrong.

use super::assembly::{expand, port_ref, AssemblyFile, JointKind};
use super::build::{Placed, PlacedComponent};
use super::command::{matches, ShipCommand};
use super::ship::{BusDef, Feed, JointDef, LineDef, LinkDef, ShipDef};
use crate::catalog::{Port, PortKind, Propellant};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A part's or component's id, its part and its ports.
struct Owner<'a> {
    id: &'a str,
    part: u32,
    ports: &'a [Port],
}

fn owners<'a>(parts: &'a [Placed], components: &'a [PlacedComponent]) -> Vec<Owner<'a>> {
    let p = parts.iter().enumerate().map(|(i, p)| Owner { id: &p.id, part: i as u32, ports: &p.built.generated.ports });
    let c = components.iter().map(|c| Owner { id: &c.id, part: c.part, ports: &c.built.generated.ports });
    p.chain(c).collect()
}

/// Connects the layers into `ship`.
pub(super) fn connect(
    file: &AssemblyFile,
    parts: &[Placed],
    components: &[PlacedComponent],
    ship: &mut ShipDef,
    errors: &mut Vec<String>,
) {
    let owners = owners(parts, components);
    let ids = || owners.iter().map(|o| o.id);
    let find = |id: &str| owners.iter().find(|o| o.id == id);
    // Plumbing: lines between ports, and through valves.
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in &file.plumbing {
        let (from, to) = (expand(&line.from, ids()), expand(&line.to, ids()));
        let pairs: Vec<(String, String)> = match (from.len(), to.len()) {
            (1, _) => to.iter().map(|t| (from[0].clone(), t.clone())).collect(),
            (_, 1) => from.iter().map(|f| (f.clone(), to[0].clone())).collect(),
            (a, b) if a == b => from.into_iter().zip(to).collect(),
            _ => {
                errors.push(format!("line {} → {}: the patterns' counts differ", line.from, line.to));
                continue;
            }
        };
        if !(line.diameter > 0.0 && line.length > 0.0) {
            errors.push(format!("line {} → {}: diameter and length must be positive", line.from, line.to));
        }
        for (a, b) in pairs {
            let ends: Vec<Option<(&Owner, &Port)>> = [&a, &b]
                .iter()
                .map(|r| {
                    let (o, p) = port_ref(r).ok()?;
                    let owner = find(o)?;
                    owner.ports.iter().find(|q| q.name == p).map(|q| (owner, q))
                })
                .collect();
            let (Some((oa, pa)), Some((ob, pb))) = (ends[0], ends[1]) else {
                errors.push(format!("line {a} → {b}: no such port (ports are `part:port`)"));
                continue;
            };
            if let (Some(fa), Some(fb)) = (&pa.fluid, &pb.fluid) {
                if fa != fb {
                    errors.push(format!("line {a} → {b}: {fa} cannot flow into a port for {fb}"));
                    continue;
                }
            }
            edges.entry(a.clone()).or_default().push(b.clone());
            edges.entry(b.clone()).or_default().push(a.clone());
            ship.lines.push(LineDef {
                from: a,
                to: b,
                diameter: line.diameter,
                length: line.length,
                parts: (oa.part, ob.part),
            });
        }
    }
    for o in &owners {
        if o.ports.iter().any(|p| p.kind == PortKind::Through) {
            let (i, x) = (format!("{}:in", o.id), format!("{}:out", o.id));
            edges.entry(i.clone()).or_default().push(x.clone());
            edges.entry(x).or_default().push(i);
        }
    }
    feeds(ship, &edges, errors);
    joints(file, parts, ship, errors);
    for bus in &file.electrical {
        let members = members(&bus.members, &owners, &format!("bus {:?}", bus.name), errors);
        ship.buses.push(BusDef { name: bus.name.clone(), voltage: bus.voltage, members });
    }
    for link in &file.data {
        let what = format!("data link from {:?}", link.from);
        let Some(from) = members(std::slice::from_ref(&link.from), &owners, &what, errors).pop() else { continue };
        ship.links.push(LinkDef { from, to: members(&link.to, &owners, &what, errors) });
    }
    ship.groups = file.groups.clone();
    for (name, selectors) in &file.groups {
        for s in selectors {
            if !ship.engines.iter().any(|e| matches(s, &e.owner)) {
                errors.push(format!("group {name:?}: {s:?} names no engine"));
            }
        }
    }
    ship.sequence = file.sequence.clone();
    ship.functions = file.functions.clone();
    for stage in file.sequence.iter().chain(&file.functions) {
        for command in &stage.actions {
            if let Err(e) = ship.check(command) {
                errors.push(format!("{:?}: {e}", stage.name));
            }
        }
    }
}

/// Every port reached from `start` through lines and valves.
fn reach(edges: &BTreeMap<String, Vec<String>>, start: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::from([start.to_string()]);
    let mut queue = VecDeque::from([start.to_string()]);
    while let Some(at) = queue.pop_front() {
        for next in edges.get(&at).into_iter().flatten() {
            if seen.insert(next.clone()) {
                queue.push_back(next.clone());
            }
        }
    }
    seen
}

/// The tanks of `fluid` whose outlets `inlet` reaches.
fn tanks_feeding(ship: &ShipDef, edges: &BTreeMap<String, Vec<String>>, inlet: &str, fluid: &str) -> Vec<u32> {
    let reached = reach(edges, inlet);
    (0..ship.tanks.len() as u32)
        .filter(|&t| {
            let tank = &ship.tanks[t as usize];
            tank.fluid == fluid && reached.contains(&format!("{}.outlet", tank.id))
        })
        .collect()
}

fn feeds(ship: &mut ShipDef, edges: &BTreeMap<String, Vec<String>>, errors: &mut Vec<String>) {
    for i in 0..ship.engines.len() {
        let e = &ship.engines[i];
        let ox = tanks_feeding(ship, edges, &format!("{}:ox_in", e.owner), &e.data.oxidizer);
        let fuel = tanks_feeding(ship, edges, &format!("{}:fuel_in", e.owner), &e.data.fuel);
        if ox.is_empty() || fuel.is_empty() {
            errors.push(format!("engine {:?}: no feed of {} and {} reaches it", e.id, e.data.oxidizer, e.data.fuel));
        }
        ship.engines[i].feeds = [ox, fuel];
    }
    for i in 0..ship.thrusters.len() {
        let t = &ship.thrusters[i];
        let feed = match &t.propellant {
            Propellant::ColdGas(gas) => {
                let reached = reach(edges, &format!("{}:in", t.id));
                let bottles: Vec<u32> = (0..ship.bottles.len() as u32)
                    .filter(|&b| {
                        let bottle = &ship.bottles[b as usize];
                        bottle.gas == *gas && reached.contains(&format!("{}:out", bottle.id))
                    })
                    .collect();
                if bottles.is_empty() {
                    errors.push(format!("thrusters {:?}: no {gas} bottle reaches them", t.id));
                }
                Feed::Bottles(bottles)
            }
            Propellant::Biprop { oxidizer, fuel, .. } => {
                let ox = tanks_feeding(ship, edges, &format!("{}:ox_in", t.id), oxidizer);
                let f = tanks_feeding(ship, edges, &format!("{}:fuel_in", t.id), fuel);
                if ox.is_empty() || f.is_empty() {
                    errors.push(format!("thrusters {:?}: no feed of {oxidizer} and {fuel} reaches them", t.id));
                }
                Feed::Tanks([ox, f])
            }
        };
        ship.thrusters[i].feed = feed;
    }
}

/// A joint for every attachment: bolted by default, the declared kind where
/// one is declared.
fn joints(file: &AssemblyFile, parts: &[Placed], ship: &mut ShipDef, errors: &mut Vec<String>) {
    for (i, p) in parts.iter().enumerate() {
        let Some(parent) = p.parent else { continue };
        let (pa, ca, size) = p.joint;
        let allowable = match (pa > 0.0, ca > 0.0) {
            (true, true) => pa.min(ca),
            _ => pa.max(ca),
        };
        // The sides that carry the load: the weaker's buckling, where given.
        let (pb, cb) = p.buckling;
        let buckling = [(pa, pb), (ca, cb)]
            .iter()
            .filter(|(a, b)| *a > 0.0 && *b > 0.0)
            .map(|x| x.1)
            .fold(0.0, |m: f64, b| if m > 0.0 { m.min(b) } else { b });
        ship.joints.push(JointDef {
            name: format!("{}|{}", parts[parent as usize].id, p.id),
            parent,
            child: i as u32,
            kind: JointKind::Bolted,
            radius: size / 2.0,
            allowable,
            buckling,
            normal: p.normal.normalize_or(glam::DVec3::Z),
            pos: p.at,
        });
    }
    for decl in &file.joints {
        let (a, b) = &decl.between;
        let id = |s: &str| parts.iter().position(|p| p.id == s).map(|i| i as u32);
        let found = (id(a).zip(id(b))).and_then(|(a, b)| {
            ship.joints.iter().position(|j| (j.parent, j.child) == (a, b) || (j.parent, j.child) == (b, a))
        });
        let Some(j) = found else {
            errors.push(format!("joint {:?}: {a:?} and {b:?} are not attached to each other", decl.name));
            continue;
        };
        if ship.joints.iter().any(|x| x.name == decl.name) {
            errors.push(format!("joint {:?}: the name is taken", decl.name));
            continue;
        }
        let joint = &mut ship.joints[j];
        joint.name = decl.name.clone();
        joint.kind = decl.kind;
        if let JointKind::Separation { strength, impulse, .. } = decl.kind {
            if !(strength > 0.0 && strength <= 1.0 && impulse >= 0.0) {
                errors.push(format!("joint {:?}: a separation's strength in (0, 1], impulse ≥ 0", decl.name));
            }
            joint.allowable *= strength;
            joint.buckling *= strength;
        }
    }
}

/// The owners `selectors` name (with `#*`), each with its part.
fn members(selectors: &[String], owners: &[Owner], what: &str, errors: &mut Vec<String>) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for s in selectors {
        let found: Vec<(String, u32)> =
            owners.iter().filter(|o| matches(s, o.id)).map(|o| (o.id.to_string(), o.part)).collect();
        if found.is_empty() {
            errors.push(format!("{what}: {s:?} names nothing in the craft"));
        }
        out.extend(found);
    }
    out
}

/// Whether a command names something the craft has (`ShipDef::check`).
pub(super) fn check(ship: &ShipDef, command: &ShipCommand) -> Result<(), String> {
    let some = |found: bool, what: &str| if found { Ok(()) } else { Err(format!("{command:?} names no {what}")) };
    fn named<'a>(s: &str, mut ids: impl Iterator<Item = &'a String>) -> bool {
        ids.any(|id| matches(s, id))
    }
    match command {
        ShipCommand::Ignite(s) | ShipCommand::Shutdown(s) | ShipCommand::Select(s) => {
            some(!ship.engines_of(s).is_empty(), "engine")
        }
        ShipCommand::Throttle(s, x) => {
            some(!ship.engines_of(s).is_empty() && (0.0..=1.0).contains(x), "engine (or the throttle is not in [0, 1])")
        }
        ShipCommand::Open(s) | ShipCommand::Close(s) => some(named(s, ship.valves.iter().map(|v| &v.id)), "valve"),
        ShipCommand::Deploy(s) | ShipCommand::Retract(s) | ShipCommand::Angle(s, _) => {
            some(named(s, ship.mechanisms.iter().map(|v| &v.id)), "mechanism")
        }
        ShipCommand::Separate(s) => some(ship.joints.iter().any(|j| j.name == *s), "joint"),
        ShipCommand::Rcs(s, _) => some(named(s, ship.thrusters.iter().map(|v| &v.id)), "thruster block"),
        ShipCommand::Power(s, _) => some(named(s, ship.components.iter().map(|v| &v.id)), "component"),
        ShipCommand::ReleaseClamps | ShipCommand::ArmChutes => Ok(()),
        ShipCommand::Sequence(_) => Err(format!("{command:?} edits the sequence: not a stage's command")),
    }
}
