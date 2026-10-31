//! The systems screen's rules (D107; ship-model.md §8; rule 8): pure
//! functions with table tests, which the screen's Bevy systems only wire
//! up. What is known about each part (`facts`: from the ship definition,
//! the ship state and the cells' and nodes' temperatures), its colour in
//! each layer, the part tree, the orbit camera's pose and the pick of a
//! part under the cursor.
//!
//! Layers show the craft by one system: the structure's joints by their
//! load (else by kind), the temperatures the thermal network has, the
//! tanks' fills, the electrical and data items as placed, each layer's
//! colours explained by [`Layer::note`] and the layer as a whole by
//! `super::summary`. Like every readout they show only what the craft
//! measures (D106): a part's temperature with its sensor, a tank's fill
//! with its level sensor.

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

    /// What the colours mean.
    pub fn note(self) -> &'static str {
        match self {
            Layer::Physical => "Parts by their outer material or paint.",
            Layer::Structural => {
                "Each part by the joint to its parent: its load over its ultimate where a strain gauge measures \
                 it (green to red; debug mode: everywhere), else its kind: integral white, bolted grey, \
                 separation amber, hold-down concrete."
            }
            Layer::Thermal => {
                "Each part's skin cell or interior node nearest its limit, blue cold, red at it, where a \
                 temperature sensor measures it (D106; debug mode: everywhere)."
            }
            Layer::Plumbing => {
                "Tanks by fluid, brighter the fuller where a level sensor measures it (else a middle shade); \
                 lines join bottles to the tanks they pressurise and tanks to the engines they feed."
            }
            Layer::Electrical => {
                "Sources green, loads orange, bus members yellow; lines join each bus's members. The buses' \
                 voltage and load, the batteries' charge and what is without power: the summary."
            }
            Layer::Data => {
                "Computers cyan, sensors violet, linked actuators blue. What the craft knows (D106) comes from \
                 the sensors a working computer reads; every readout shows only that."
            }
            Layer::Health => {
                "Integrity and damage: green sound, yellow damaged, red failed. Heat past a part's rating and \
                 vibration past its qualification wear it."
            }
        }
    }

    /// The layer a part is mostly about, for opening the screen at it (a
    /// part card's "more"): its tanks Plumbing, a battery Electrical, a
    /// computer Data, the rest Structural.
    pub fn main(f: &PartFacts) -> Layer {
        if !f.tanks.is_empty() {
            Layer::Plumbing
        } else if f.source {
            Layer::Electrical
        } else if f.computers > 0 {
            Layer::Data
        } else {
            Layer::Structural
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
    /// Line load it carries (N/m), where the craft gives one
    /// (`sim::systems::structure::rated`), and its ring's radius (m).
    pub allowable: Option<f64>,
    pub radius: f64,
    /// Its load at the last live tick over its allowable at temperature (1:
    /// the ultimate load; RUNTIME's structure), where the craft measures it
    /// (a strain gauge, D106) and gives it a capacity.
    pub ratio: Option<f64>,
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
    /// Its tanks: fluid, liquid (kg; `None`: no level sensor, D106),
    /// capacity (kg).
    pub tanks: Vec<(String, Option<f64>, f64)>,
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

/// Each part's value nearest its own limit among `values` (the skin cells
/// or the interior nodes, value `i` of part `owner[i]`, an index into the
/// craft's `parts`, against `limits[i]`), in one pass: (K, limit K), `None`
/// for a part with none.
pub fn nearest_by_part(values: &[f64], owner: &[u32], limits: &[f64], parts: usize) -> Vec<Option<(f64, f64)>> {
    let mut best: Vec<Option<(f64, f64)>> = vec![None; parts];
    for i in 0..values.len().min(owner.len()).min(limits.len()) {
        if let Some(b) = best.get_mut(owner[i] as usize) {
            *b = nearer(*b, Some((values[i], limits[i])));
        }
    }
    best
}

/// A temperature and its limit (K, limit K).
pub type Kelvin = (f64, f64);

/// A part's skin cell and interior node nearest their own limits (`None`:
/// it has none).
pub type PartTemps = (Option<Kelvin>, Option<Kelvin>);

/// Each part's skin cell and interior node nearest their own limits, in
/// the craft's parts' order: the systems screen's THERMAL rows and the part
/// cards' (one owner; the card showed the skin's mean against the
/// interior's rating, interface-map.md finding 25).
pub fn part_temperatures(t: &Temps, parts: usize) -> Vec<PartTemps> {
    let skin = nearest_by_part(t.skin, t.cell_part, t.skin_max, parts);
    skin.into_iter().zip(nearest_by_part(t.nodes, t.node_part, t.node_max, parts)).collect()
}

/// Vessel `v`'s [`part_temperatures`] now, by part id (`Part::id`).
pub fn vessel_temperatures(v: &sim::vessel::Vessel) -> Vec<(u32, PartTemps)> {
    let design = v.craft.design();
    design.parts.iter().map(|p| p.id).zip(part_temperatures(&Temps::of(v), design.parts.len())).collect()
}

impl<'a> Temps<'a> {
    /// Vessel `v`'s thermal network now: its cells' and nodes' temperatures,
    /// parts and limits.
    pub fn of(v: &'a sim::vessel::Vessel) -> Temps<'a> {
        let design = v.craft.design();
        let st = &v.thermal().state;
        Temps {
            skin: &st.skin,
            nodes: &st.nodes,
            cell_part: &design.cell_part,
            node_part: &design.node_part,
            skin_max: &design.skin_max,
            node_max: &design.node_max,
        }
    }
}

/// The nearer its limit of two temperatures (K, limit K); the second on a
/// tie.
pub fn nearer(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> Option<(f64, f64)> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if x.0 / x.1 > y.0 / y.1 { x } else { y }),
        (x, y) => x.or(y),
    }
}

/// The parts nearest their temperature limits, nearest first (a tie keeps
/// the given order), at most `n`, and how many parts nothing measures
/// (`None`): the systems screen's summary and the Landing menu's THERMAL,
/// one rule (D106: a part no sensor measures is counted, never ranked by
/// the truth; interface-map.md finding 13).
pub fn nearest_limits<T>(
    parts: impl IntoIterator<Item = (T, Option<(f64, f64)>)>,
    n: usize,
) -> (Vec<(T, Kelvin)>, usize) {
    let mut unmeasured = 0;
    let mut hot: Vec<(T, Kelvin)> = Vec::new();
    for (part, t) in parts {
        match t {
            Some(t) => hot.push((part, t)),
            None => unmeasured += 1,
        }
    }
    hot.sort_by(|a, b| (b.1 .0 / b.1 .1).total_cmp(&(a.1 .0 / a.1 .1)));
    hot.truncate(n);
    (hot, unmeasured)
}

/// Everything the layers read, per part of the craft (`parts`, in the
/// craft's order; `colours` their physical colours).
pub fn facts(
    parts: &[Part],
    colours: &[[f32; 3]],
    def: &ShipDef,
    state: &ShipState,
    t: Option<&Temps>,
    known: &sim::systems::Knowledge,
) -> Vec<PartFacts> {
    use sim::systems::Quantity;
    let temps = t.map(|t| part_temperatures(t, parts.len()));
    (parts.iter().enumerate())
        .map(|(k, p)| {
            let id = p.id;
            let info = def.parts.iter().find(|i| i.part == id);
            let joint = (def.joints.iter().enumerate()).find(|(_, j)| j.child == id).map(|(i, j)| {
                let rated = sim::systems::structure::rated(def, j);
                JointFact {
                    name: j.name.clone(),
                    class: match j.kind {
                        JointKind::Integral => JointClass::Integral,
                        JointKind::Bolted => JointClass::Bolted,
                        JointKind::Separation { .. } => JointClass::Separation,
                        JointKind::HoldDown => JointClass::HoldDown,
                    },
                    intact: state.joints.get(i).is_none_or(|s| s.intact),
                    allowable: rated.then_some(j.allowable),
                    radius: j.radius,
                    ratio: (state.joints.get(i))
                        .filter(|_| rated && known.knows(Quantity::JointLoad(i as u32)))
                        .map(|r| r.ratio),
                }
            });
            let tanks = (def.tanks.iter().zip(&state.tanks).enumerate())
                .filter(|(_, (d, _))| d.part == id)
                .map(|(i, (d, s))| {
                    let level = known.knows(Quantity::TankLevel(i as u32)).then_some(s.liquid);
                    (d.fluid.clone(), level, d.capacity)
                })
                .collect();
            // Temperatures where the craft measures them (D106).
            let measured = known.knows(Quantity::PartTemperature(id));
            let (skin, interior) = temps.as_ref().filter(|_| measured).map_or((None, None), |v| v[k]);
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
                skin,
                interior,
                tanks,
                plumbing,
                source: def.batteries.iter().any(|b| on(b.part)),
                bus: def.buses.iter().any(|b| b.members.iter().any(|m| m.1 == id && ids.contains(&def.owner_id(m.0)))),
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

/// The shade of a tank whose level the craft cannot measure.
pub const UNKNOWN_FILL: f32 = 0.35;

/// A part that does not matter in a layer (linear).
pub const GHOST: [f32; 3] = [0.06, 0.065, 0.075];

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

/// A joint's load over its ultimate: green unloaded, yellow at half, red at
/// it and beyond.
pub fn load_colour(ratio: f64) -> [f32; 3] {
    ramp(&[[0.12, 0.75, 0.25], [0.9, 0.8, 0.1], [0.9, 0.08, 0.04]], ratio)
}

/// A part's health: green sound, yellow at half its damage, red failed
/// (damage at one or integrity gone).
pub fn health_colour(integrity: f64, damage: f64) -> [f32; 3] {
    let x = damage.max(1.0 - integrity);
    ramp(&[[0.12, 0.75, 0.25], [0.9, 0.8, 0.1], [0.9, 0.08, 0.04]], x)
}

/// A part's colour in `layer` (linear). A selection is not tinted: it is
/// isolated, the other parts ghosted (`view`).
pub fn colour(layer: Layer, f: &PartFacts) -> [f32; 3] {
    match layer {
        Layer::Physical => f.material,
        Layer::Structural => match f.joint.as_ref().map(|j| (j.class, j.intact)) {
            _ if f.ground => crate::scene::parts::GROUND,
            None => [0.85, 0.85, 0.88],
            Some((_, false)) => GHOST,
            // A measured load: green unloaded, yellow at half, red at the ultimate.
            Some(_) if f.joint.as_ref().and_then(|j| j.ratio).is_some() => {
                load_colour(f.joint.as_ref().and_then(|j| j.ratio).unwrap_or(0.0))
            }
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
            Some((fluid, Some(liquid), capacity)) => {
                let fill = if *capacity > 0.0 { (liquid / capacity) as f32 } else { 0.0 };
                fluid_colour(fluid).map(|c| c * (0.2 + 0.8 * fill.clamp(0.0, 1.0)))
            }
            // Its fluid by design, its level unknown: a middle shade.
            Some((fluid, None, _)) => fluid_colour(fluid).map(|c| c * UNKNOWN_FILL),
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
    }
}

/// The networks a layer draws as lines between their ends (craft axes):
/// plumbing lines (from the owner of one port to the other's), electrical
/// buses (each member to the bus's first), data links (each computer to
/// what it commands); none in the other layers. `at(id, part)`: where an
/// item is (a component's place, else its part's centre).
pub fn networks(layer: Layer, def: &ShipDef, at: &dyn Fn(&str, u32) -> Vec3) -> Vec<(Vec3, Vec3)> {
    let owner = |end: &str| end.split(':').next().unwrap_or(end).to_string();
    match layer {
        Layer::Plumbing => {
            def.lines.iter().map(|l| (at(&owner(&l.from), l.parts.0), at(&owner(&l.to), l.parts.1))).collect()
        }
        Layer::Electrical => (def.buses.iter())
            .flat_map(|b| {
                let first = b.members.first().map(|m| at(def.owner_id(m.0), m.1));
                b.members.iter().skip(1).filter_map(move |m| Some((first?, at(def.owner_id(m.0), m.1))))
            })
            .collect(),
        Layer::Data => (def.links.iter())
            .flat_map(|l| l.to.iter().map(move |m| (at(def.owner_id(l.from.0), l.from.1), at(def.owner_id(m.0), m.1))))
            .collect(),
        _ => Vec::new(),
    }
}

/// The colour of a layer's network lines (linear).
pub fn network_colour(layer: Layer) -> [f32; 3] {
    match layer {
        Layer::Plumbing => [0.6, 0.85, 1.0],
        Layer::Electrical => [1.0, 0.9, 0.2],
        _ => [0.2, 0.95, 1.0],
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

/// The distance at which a sphere of `radius` fits the view whatever the
/// camera's angle: `half_fov` the vertical half field of view (rad),
/// `aspect` the view's width over its height; the narrower way decides,
/// with a little room around it.
pub fn frame(radius: f32, half_fov: f32, aspect: f32) -> f32 {
    let half_x = (half_fov.tan() * aspect.max(1e-3)).atan();
    radius.max(1e-3) * 1.08 / half_fov.min(half_x).sin()
}

/// The camera's look (target, distance) moved toward `to` over `dt`
/// seconds: most of the way within a few tenths of a second, never past it.
pub fn ease(from: (Vec3, f32), to: (Vec3, f32), dt: f32) -> (Vec3, f32) {
    let k = 1.0 - (-dt.max(0.0) / EASE_S).exp();
    (from.0 + (to.0 - from.0) * k, from.1 + (to.1 - from.1) * k)
}

/// The time constant of the camera's easing (s).
pub const EASE_S: f32 = 0.08;

/// The part a click picks while part `isolated` (an index into `parts`)
/// is isolated: it first, if the ray hits it, even through a ghost in
/// front; else the nearest part hit ([`pick`]).
pub fn pick_isolating(
    origin: Vec3,
    dir: Vec3,
    parts: &[(&RenderMesh, Transform)],
    isolated: Option<usize>,
) -> Option<usize> {
    let own = isolated.filter(|&k| parts.get(k).is_some_and(|p| pick(origin, dir, std::slice::from_ref(p)).is_some()));
    own.or_else(|| pick(origin, dir, parts))
}

/// The rotation from the craft's axes (+Z the nose) to the view's (Bevy
/// axes, +Y up).
pub fn craft_to_view() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

/// The part a ray from the camera first hits: each part's mesh with its
/// transform into the view's world (`parts`: (mesh, transform)); the index
/// of the nearest (`scene::pick`, the one owner of rays against craft).
pub fn pick(origin: Vec3, dir: Vec3, parts: &[(&RenderMesh, Transform)]) -> Option<usize> {
    crate::scene::pick::ray_hit(origin, dir, Transform::IDENTITY, parts).map(|(k, _)| k)
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

/// The side columns' widths (logical px): the summary or the parts list on
/// the left, the selection's details on the right.
pub const LEFT_WIDTH: f32 = 380.0;
pub const RIGHT_WIDTH: f32 = 360.0;

/// The regions of a screen: fixed sizes, the side columns narrowed on a
/// small screen so the view keeps at least a third of the width.
pub fn regions(screen: bevy_egui::egui::Rect) -> Regions {
    use bevy_egui::egui::{pos2, Rect};
    let (w, h) = (screen.width(), screen.height());
    let shrink = (w * 2.0 / 3.0 / (LEFT_WIDTH + RIGHT_WIDTH)).min(1.0);
    let (left_w, right_w) = (LEFT_WIDTH * shrink, RIGHT_WIDTH * shrink);
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
