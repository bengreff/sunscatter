//! The systems screen's rules (D107; ship-model.md §8; rule 8): pure
//! functions with table tests, which the screen's Bevy systems only wire
//! up. What is known about each part (`facts`: from the ship definition,
//! the ship state and the cells' and nodes' temperatures), its colour in
//! each layer, the part tree, the orbit camera's pose and the pick of a
//! part under the cursor.
//!
//! Layers show what exists: the structure's joints by kind (their loads
//! are RUNTIME's, not computed yet), the temperatures the thermal network
//! has, the tanks' fills, the electrical and data items as placed; each
//! layer says plainly what it cannot show yet ([`Layer::note`]).

use bevy::prelude::{Quat, Transform, Vec3};
use sim::craft::assembly::JointKind;
use sim::craft::ship::ShipDef;
use sim::craft::state::ShipState;
use sim::craft::{Part, RenderMesh};

/// The layers the craft is shown by (D107).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layer {
    #[default]
    Physical,
    Structural,
    Thermal,
    Plumbing,
    Electrical,
    Data,
    Health,
}

impl Layer {
    pub const ALL: [Layer; 7] = [
        Layer::Physical,
        Layer::Structural,
        Layer::Thermal,
        Layer::Plumbing,
        Layer::Electrical,
        Layer::Data,
        Layer::Health,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Layer::Physical => "PHYSICAL",
            Layer::Structural => "STRUCTURAL",
            Layer::Thermal => "THERMAL",
            Layer::Plumbing => "PLUMBING",
            Layer::Electrical => "ELECTRICAL",
            Layer::Data => "DATA",
            Layer::Health => "HEALTH",
        }
    }

    /// What the colours mean, and what the simulation does not give yet.
    pub fn note(self) -> &'static str {
        match self {
            Layer::Physical => "Parts by their outer material or paint.",
            Layer::Structural => {
                "Joints by kind: integral white, bolted grey, separation amber, hold-down concrete. \
                 Loads against capacity are not simulated yet (RUNTIME)."
            }
            Layer::Thermal => "Each part's hottest skin cell or interior node against its limit: blue cold, red at it.",
            Layer::Plumbing => {
                "Tanks by fluid, brighter the fuller. Pressures, flows and settling are not simulated yet \
                 (RUNTIME): pressures shown are the ullage gas law on the ship state."
            }
            Layer::Electrical => {
                "Sources green, loads orange, bus members yellow. Power flow and charge use are not \
                 simulated yet (RUNTIME)."
            }
            Layer::Data => {
                "Computers cyan, sensors violet, linked actuators blue. What the craft knows from its \
                 sensors is not simulated yet (RUNTIME)."
            }
            Layer::Health => {
                "Integrity and damage: green sound, yellow damaged, red failed. Damage does not accumulate \
                 yet (RUNTIME)."
            }
        }
    }
}

/// How a part hangs from its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointClass {
    Integral,
    Bolted,
    Separation,
    HoldDown,
}

/// The joint of a part to its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct JointFact {
    pub name: String,
    pub class: JointClass,
    pub intact: bool,
    /// Line load it carries (N/m) and its ring's radius (m).
    pub allowable: f64,
    pub radius: f64,
}

/// What the screen knows of one part.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartFacts {
    /// Its id in the craft (`Part::id`), name, physical colour (linear).
    pub id: u32,
    pub name: String,
    pub material: [f32; 3],
    pub ground: bool,
    pub joint: Option<JointFact>,
    /// The hottest skin cell and interior node with their limits (K).
    pub skin: Option<(f64, f64)>,
    pub interior: Option<(f64, f64)>,
    /// Its tanks: fluid, liquid (kg), capacity (kg).
    pub tanks: Vec<(String, f64, f64)>,
    /// Bottles, valves, engines or thrusters on it or its components.
    pub plumbing: bool,
    /// A battery or solar array; a bus member; its components' draw (W).
    pub source: bool,
    pub bus: bool,
    pub load: f64,
    /// Computers, sensors; named by a data link.
    pub computers: usize,
    pub sensors: usize,
    pub linked: bool,
    /// 1 intact, 0 destroyed; thermal and vibration damage (fail at 1).
    pub integrity: f64,
    pub damage: f64,
}

/// The cells' and nodes' temperatures and what they belong to.
pub struct Temps<'a> {
    pub skin: &'a [f64],
    pub nodes: &'a [f64],
    /// The part (an index into the craft's parts) of each cell and node.
    pub cell_part: &'a [u32],
    pub node_part: &'a [u32],
    pub skin_max: &'a [f64],
    pub node_max: &'a [f64],
}

/// The hottest of `values` against its limit among those of part `k`:
/// (temperature, limit), the one nearest its limit.
fn hottest(values: &[f64], owner: &[u32], limits: &[f64], k: usize) -> Option<(f64, f64)> {
    (0..values.len().min(owner.len()).min(limits.len()))
        .filter(|&i| owner[i] as usize == k)
        .map(|i| (values[i], limits[i]))
        .max_by(|a, b| (a.0 / a.1).total_cmp(&(b.0 / b.1)))
}

/// Everything the layers read, per part of the craft (`parts`, in the
/// craft's order; `colours` their physical colours).
pub fn facts(
    parts: &[Part],
    colours: &[[f32; 3]],
    def: &ShipDef,
    state: &ShipState,
    t: Option<&Temps>,
) -> Vec<PartFacts> {
    (parts.iter().enumerate())
        .map(|(k, p)| {
            let id = p.id;
            let info = def.parts.iter().find(|i| i.part == id);
            let joint = (def.joints.iter().enumerate()).find(|(_, j)| j.child == id).map(|(i, j)| JointFact {
                name: j.name.clone(),
                class: match j.kind {
                    JointKind::Integral => JointClass::Integral,
                    JointKind::Bolted => JointClass::Bolted,
                    JointKind::Separation { .. } => JointClass::Separation,
                    JointKind::HoldDown => JointClass::HoldDown,
                },
                intact: state.joints.get(i).is_none_or(|s| s.intact),
                allowable: j.allowable,
                radius: j.radius,
            });
            let tanks = (def.tanks.iter().zip(&state.tanks))
                .filter(|(d, _)| d.part == id)
                .map(|(d, s)| (d.fluid.clone(), s.liquid, d.capacity))
                .collect();
            let on = |part: u32| part == id;
            let plumbing = def.bottles.iter().any(|b| on(b.part))
                || def.valves.iter().any(|v| on(v.part))
                || def.engines.iter().any(|e| on(e.part))
                || def.thrusters.iter().any(|x| on(x.part));
            let comps = def.components.iter().enumerate().filter(|(_, c)| on(c.part));
            let load = comps.filter(|(i, _)| state.power.get(*i).is_none_or(|s| s.on)).map(|(_, c)| c.power).sum();
            let ids: Vec<&str> = std::iter::once(info.map_or("", |i| i.id.as_str()))
                .chain(def.components.iter().filter(|c| on(c.part)).map(|c| c.id.as_str()))
                .collect();
            let linked = def.links.iter().any(|l| l.to.iter().any(|m| m.1 == id) || l.from.1 == id);
            let part_state = state.parts.iter().find(|s| s.part == id);
            PartFacts {
                id,
                name: info.map_or_else(|| p.name.clone(), |i| i.id.clone()),
                material: colours.get(k).copied().unwrap_or(crate::scene::parts::UNKNOWN),
                ground: info.is_some_and(|i| i.ground),
                joint,
                skin: t.and_then(|t| hottest(t.skin, t.cell_part, t.skin_max, k)),
                interior: t.and_then(|t| hottest(t.nodes, t.node_part, t.node_max, k)),
                tanks,
                plumbing,
                source: def.batteries.iter().any(|b| on(b.part)),
                bus: def.buses.iter().any(|b| b.members.iter().any(|m| m.1 == id && ids.contains(&m.0.as_str()))),
                load,
                computers: def.computers.iter().filter(|c| on(c.part)).count(),
                sensors: def.sensors.iter().filter(|s| on(s.part)).count(),
                linked,
                integrity: part_state.map_or(1.0, |s| s.integrity),
                damage: part_state.map_or(0.0, |s| s.thermal_damage.max(s.vibration_damage)),
            }
        })
        .collect()
}

/// A part that does not matter in a layer (linear).
pub const GHOST: [f32; 3] = [0.06, 0.065, 0.075];
/// The selection's tint.
pub const SELECTED: [f32; 3] = [1.0, 0.55, 0.08];

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A ramp through `stops` (each at an even share of 0–1).
fn ramp(stops: &[[f32; 3]], x: f64) -> [f32; 3] {
    let x = x.clamp(0.0, 1.0) * (stops.len() - 1) as f64;
    let i = (x.floor() as usize).min(stops.len() - 2);
    mix(stops[i], stops[i + 1], (x - i as f64) as f32)
}

/// A temperature's colour against its limit: blue at 100 K, through
/// green and yellow, red at the limit.
pub fn heat_colour(k: f64, limit: f64) -> [f32; 3] {
    let x = (k - 100.0) / (limit - 100.0).max(1.0);
    ramp(&[[0.05, 0.15, 0.9], [0.05, 0.6, 0.7], [0.15, 0.7, 0.15], [0.9, 0.8, 0.1], [0.9, 0.08, 0.04]], x)
}

/// A liquid's colour (LOX pale blue, RP-1 amber, hydrazines yellow and
/// rust, water blue; anything else grey).
pub fn fluid_colour(fluid: &str) -> [f32; 3] {
    match fluid {
        "LOX" => [0.45, 0.7, 1.0],
        "RP-1" => [0.85, 0.5, 0.12],
        "MMH" => [0.85, 0.8, 0.2],
        "NTO" => [0.7, 0.25, 0.08],
        "water" | "Water" => [0.1, 0.35, 0.9],
        _ => [0.6, 0.6, 0.6],
    }
}

/// A part's health: green sound, yellow at half its damage, red failed
/// (damage at one or integrity gone).
pub fn health_colour(integrity: f64, damage: f64) -> [f32; 3] {
    let x = damage.max(1.0 - integrity);
    ramp(&[[0.12, 0.75, 0.25], [0.9, 0.8, 0.1], [0.9, 0.08, 0.04]], x)
}

/// A part's colour in `layer` (linear), the selection tinted.
pub fn colour(layer: Layer, f: &PartFacts, selected: bool) -> [f32; 3] {
    let c = match layer {
        Layer::Physical => f.material,
        Layer::Structural => match f.joint.as_ref().map(|j| (j.class, j.intact)) {
            _ if f.ground => crate::scene::parts::GROUND,
            None => [0.85, 0.85, 0.88],
            Some((_, false)) => GHOST,
            Some((JointClass::Integral, _)) => [0.8, 0.8, 0.82],
            Some((JointClass::Bolted, _)) => [0.35, 0.37, 0.4],
            Some((JointClass::Separation, _)) => [1.0, 0.6, 0.1],
            Some((JointClass::HoldDown, _)) => crate::scene::parts::GROUND,
        },
        Layer::Thermal => {
            match [f.skin, f.interior].into_iter().flatten().max_by(|a, b| (a.0 / a.1).total_cmp(&(b.0 / b.1))) {
                Some((k, limit)) => heat_colour(k, limit),
                None => GHOST,
            }
        }
        Layer::Plumbing => match f.tanks.iter().max_by(|a, b| a.2.total_cmp(&b.2)) {
            Some((fluid, liquid, capacity)) => {
                let fill = if *capacity > 0.0 { (liquid / capacity) as f32 } else { 0.0 };
                fluid_colour(fluid).map(|c| c * (0.2 + 0.8 * fill.clamp(0.0, 1.0)))
            }
            None if f.plumbing => [0.55, 0.57, 0.6],
            None => GHOST,
        },
        Layer::Electrical => match (f.source, f.load > 0.0, f.bus) {
            (true, _, _) => [0.15, 0.8, 0.3],
            (_, true, _) => [1.0, 0.5, 0.1],
            (_, _, true) => [0.95, 0.85, 0.15],
            _ => GHOST,
        },
        Layer::Data => match (f.computers > 0, f.sensors > 0, f.linked) {
            (true, _, _) => [0.1, 0.85, 0.9],
            (_, true, _) => [0.6, 0.3, 0.95],
            (_, _, true) => [0.2, 0.4, 0.95],
            _ => GHOST,
        },
        Layer::Health if f.ground => GHOST,
        Layer::Health => health_colour(f.integrity, f.damage),
    };
    if selected {
        mix(c, SELECTED, 0.55)
    } else {
        c
    }
}

/// The parts as a tree (attachments, roots first, children after their
/// parent in the craft's order): (depth, index into `parts`).
pub fn tree(parts: &[Part]) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(parts.len());
    fn visit(parts: &[Part], k: usize, depth: usize, out: &mut Vec<(usize, usize)>) {
        out.push((depth, k));
        for c in (0..parts.len()).filter(|&c| parts[c].parent == Some(k as u32)) {
            visit(parts, c, depth + 1, out);
        }
    }
    for root in (0..parts.len()).filter(|&k| parts[k].parent.is_none_or(|p| p as usize >= parts.len())) {
        visit(parts, root, 0, &mut out);
    }
    out
}

/// The orbit camera around `target` (Bevy axes, Y up): `yaw` about the
/// vertical, `pitch` above the horizon (rad), `distance` (m), looking at it.
pub fn orbit_pose(target: Vec3, yaw: f32, pitch: f32, distance: f32) -> Transform {
    let dir = Vec3::new(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos());
    Transform::from_translation(target + dir * distance).looking_at(target, Vec3::Y)
}

/// The rotation from the craft's axes (+Z the nose) to the view's (Bevy
/// axes, +Y up).
pub fn craft_to_view() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

/// The nearest hit of a ray (origin, unit direction) with a triangle
/// (Möller–Trumbore): the distance along the ray.
fn hit(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    let t = e2.dot(q) * inv;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0).then_some(t)
}

/// The part a ray from the camera first hits: each part's mesh with its
/// transform into the view's world (`parts`: (mesh, transform)); the index
/// of the nearest.
pub fn pick(origin: Vec3, dir: Vec3, parts: &[(&RenderMesh, Transform)]) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (k, (mesh, t)) in parts.iter().enumerate() {
        let m = t.to_matrix();
        let at = |i: u32| m.transform_point3(Vec3::from_array(mesh.positions[i as usize]));
        for tri in mesh.indices.as_chunks::<3>().0 {
            if let Some(d) = hit(origin, dir, at(tri[0]), at(tri[1]), at(tri[2])) {
                if best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, k));
                }
            }
        }
    }
    best.map(|(_, k)| k)
}

/// Where the screen's regions go (logical px): the title and layer bar
/// on top, the parts on the left, the inspector on the right, the editors
/// at the bottom, the view in the middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regions {
    pub top: bevy_egui::egui::Rect,
    pub left: bevy_egui::egui::Rect,
    pub right: bevy_egui::egui::Rect,
    pub bottom: bevy_egui::egui::Rect,
    pub view: bevy_egui::egui::Rect,
}

/// The regions of a screen: fixed sizes, the side columns narrowed on a
/// small screen so the view keeps at least a third of the width.
pub fn regions(screen: bevy_egui::egui::Rect) -> Regions {
    use bevy_egui::egui::{pos2, Rect};
    let (w, h) = (screen.width(), screen.height());
    let shrink = (w * 2.0 / 3.0 / (280.0 + 380.0)).min(1.0);
    let (left_w, right_w) = (280.0 * shrink, 380.0 * shrink);
    let top_h = 70.0_f32.min(h * 0.2);
    let bottom_h = 250.0_f32.min(h * 0.35);
    let (x0, y0, x1, y1) = (screen.min.x, screen.min.y, screen.max.x, screen.max.y);
    let (mid_top, mid_bottom) = (y0 + top_h, y1 - bottom_h);
    Regions {
        top: Rect::from_min_max(pos2(x0, y0), pos2(x1, mid_top)),
        left: Rect::from_min_max(pos2(x0, mid_top), pos2(x0 + left_w, y1)),
        right: Rect::from_min_max(pos2(x1 - right_w, mid_top), pos2(x1, y1)),
        bottom: Rect::from_min_max(pos2(x0 + left_w, mid_bottom), pos2(x1 - right_w, y1)),
        view: Rect::from_min_max(pos2(x0 + left_w, mid_top), pos2(x1 - right_w, mid_bottom)),
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
