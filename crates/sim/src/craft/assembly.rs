//! Craft files v2 (D102, `docs/design/ship-model.md` §3.4): a craft of
//! catalog parts, `data/craft/<craft>/craft.ron` without a `geometry.ron`
//! (the single-part craft file of [`super::file`] still loads as before).
//!
//! * **Parts** name a catalog entry, change its parameters by name (`set`),
//!   and attach **only** through a mounting point of theirs (`via`) to a
//!   compatible one of a part listed earlier (`mount: (on, at, via,
//!   roll_deg)`), or are placed by coordinates on a part (`at: (on, pos,
//!   axis, roll_deg)`); the first part is the root. Touching is not
//!   attaching.
//! * **Patterns** make copies `<id>#1 … #n`: `List(n)` replaces `#` in the
//!   mount names (and in the parent's id); `Radial(count, start_deg)`
//!   turns coordinate placements about the craft's axis.
//! * **Components** sit in a part (`in`) at a position in its frame.
//! * **Joints** default from the mounts (bolted); declared ones name their
//!   kind (integral, bolted, a separation system with its impulse).
//! * **Layers**: plumbing lines between ports (`part:port`), electrical
//!   buses, data links; `#*` stands for every copy of a pattern.
//! * **Loading**: each tank's fill, temperature, ullage pressure and pressurant.
//! * **Groups** of engines, the stage **sequence** and saved **functions**
//!   as lists of [`ShipCommand`](super::command::ShipCommand).
//!
//! Newtype variants are unwrapped and `Some` is implicit
//! ([`crate::catalog::ron_options`]).

use super::command::Stage;
use super::file::{AeroFile, AntennaFile, ContactFile, ImpactLimits, ThermalLimits};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A craft of catalog parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyFile {
    pub name: String,
    /// People aboard at the start.
    pub crew: u32,
    pub parts: Vec<PartPlace>,
    #[serde(default)]
    pub components: Vec<ComponentPlace>,
    #[serde(default)]
    pub joints: Vec<JointDecl>,
    #[serde(default)]
    pub plumbing: Vec<LineDecl>,
    #[serde(default)]
    pub electrical: Vec<BusDecl>,
    #[serde(default)]
    pub data: Vec<LinkDecl>,
    #[serde(default)]
    pub loading: Vec<Loading>,
    /// Engine groups: name → selectors of engine parts or pods.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub sequence: Vec<Stage>,
    #[serde(default)]
    pub functions: Vec<Stage>,
    /// Published dry masses of groups of parts (a stage, a spacecraft):
    /// the build reports each against the derived total and carries the
    /// gap as one part's unmodelled mass.
    #[serde(default)]
    pub checks: Vec<MassCheck>,
    /// Whole-craft data the cells and the contact still take per craft.
    pub aero: AeroFile,
    pub thermal: ThermalLimits,
    pub impact: ImpactLimits,
    pub contact: ContactFile,
    pub antenna: AntennaFile,
}

/// A part: its entry, changed parameters and where it goes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartPlace {
    pub id: String,
    pub entry: String,
    #[serde(default)]
    pub set: BTreeMap<String, f64>,
    #[serde(default)]
    pub pattern: Option<Pattern>,
    #[serde(default)]
    pub mount: Option<Attach>,
    #[serde(default)]
    pub at: Option<Place>,
}

/// Attached through its mount `via` to mount `at` of part `on`, turned
/// `roll_deg` about the mount's normal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attach {
    pub on: String,
    pub at: String,
    pub via: String,
    #[serde(default)]
    pub roll_deg: f64,
}

/// Placed by coordinates: its origin at `pos`, its +Z along `axis`, turned
/// `roll_deg` about it; attached to part `on` (none: the root), in the
/// craft's axes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
    #[serde(default)]
    pub on: Option<String>,
    pub pos: DVec3,
    #[serde(default = "z_axis")]
    pub axis: DVec3,
    #[serde(default)]
    pub roll_deg: f64,
}

fn z_axis() -> DVec3 {
    DVec3::Z
}

/// Copies of a part or component, ids `<id>#1 … #n`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// `#` in mount names (and in a parent's or host's id) becomes k.
    List(u32),
    /// Coordinates turned by `start_deg + (k − 1)·360/count` about the
    /// craft's (a component: its host's) Z axis.
    Radial { count: u32, start_deg: f64 },
}

impl Pattern {
    pub fn count(&self) -> u32 {
        match *self {
            Pattern::List(n) => n,
            Pattern::Radial { count, .. } => count,
        }
    }

    /// The turn of copy `k` (1-based) about Z (deg).
    pub fn angle_deg(&self, k: u32) -> f64 {
        match *self {
            Pattern::List(_) => 0.0,
            Pattern::Radial { count, start_deg } => start_deg + 360.0 * f64::from(k - 1) / f64::from(count),
        }
    }
}

/// A component in part `in` at `pos` (the part's frame), its +Z along
/// `axis`, turned `roll_deg`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentPlace {
    pub id: String,
    pub entry: String,
    #[serde(default)]
    pub set: BTreeMap<String, f64>,
    #[serde(default)]
    pub pattern: Option<Pattern>,
    #[serde(rename = "in")]
    pub host: String,
    pub pos: DVec3,
    #[serde(default = "z_axis")]
    pub axis: DVec3,
    #[serde(default)]
    pub roll_deg: f64,
}

/// How a separation system pushes the pieces apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeparationSystem {
    /// Pneumatic pushers (Falcon 9's stage separation).
    Pneumatic,
    Pyrotechnic,
    /// A clamp band released, springs pushing.
    Clamp,
}

/// A joint's kind.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum JointKind {
    /// Welded or one piece within a stage.
    Integral,
    /// A bolted flange (the default between parts).
    Bolted,
    /// A separation system: its kind, the impulse it gives (N·s) and its
    /// capacity as a share of the weaker side's wall (≤ 1).
    Separation { system: SeparationSystem, impulse: f64, strength: f64 },
}

/// A declared joint between two attached parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointDecl {
    pub name: String,
    pub between: (String, String),
    pub kind: JointKind,
}

/// A plumbing line between two ports (`part-or-component:port`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineDecl {
    pub from: String,
    pub to: String,
    /// m.
    pub diameter: f64,
    pub length: f64,
}

/// An electrical bus and the components (and parts) on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusDecl {
    pub name: String,
    pub voltage: f64,
    pub members: Vec<String>,
}

/// A data link from a computer to sensors and actuators.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkDecl {
    pub from: String,
    pub to: Vec<String>,
}

/// A published dry mass of the parts `parts` names (selectors; with their
/// components), against what they derive: the gap is `carry`'s unmodelled
/// mass, for `reason` (D101: reported, never hidden).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MassCheck {
    pub name: String,
    pub parts: Vec<String>,
    /// kg and its source.
    pub dry_mass: (f64, String),
    pub carry: String,
    pub reason: String,
}

/// How a tank is loaded: `tank` is `part:compartment` (a component tank's
/// compartment is `main`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Loading {
    pub tank: String,
    /// Share of the tank's capacity (its volume less the ullage).
    #[serde(default = "full")]
    pub fill: f64,
    pub temperature_k: f64,
    /// Ullage pressure (Pa) and the gas that holds it.
    pub pressure: f64,
    pub pressurant: String,
}

fn full() -> f64 {
    1.0
}

/// The copies of an id under a pattern: `(id, k)` with k from 1, or the id
/// itself (k = 0).
pub fn copies(id: &str, pattern: Option<Pattern>) -> Vec<(String, u32)> {
    match pattern {
        None => vec![(id.to_string(), 0)],
        Some(p) => (1..=p.count()).map(|k| (format!("{id}#{k}"), k)).collect(),
    }
}

/// `name` with `#` replaced by copy `k` (unchanged for k = 0).
pub fn nth(name: &str, k: u32) -> String {
    if k == 0 {
        name.to_string()
    } else {
        name.replace('#', &k.to_string())
    }
}

/// Splits `owner:port`.
pub fn port_ref(s: &str) -> Result<(&str, &str), String> {
    s.rsplit_once(':').ok_or_else(|| format!("{s:?} is not a port reference (`part:port`)"))
}

/// Expands a `#*` or `.*` reference over the `ids` it names (in order);
/// anything else stays itself.
pub fn expand<'a>(reference: &str, ids: impl Iterator<Item = &'a str> + Clone) -> Vec<String> {
    let (owner, port) = match reference.rsplit_once(':') {
        Some((o, p)) => (o, Some(p)),
        None => (reference, None),
    };
    if !(owner.ends_with("#*") || owner.ends_with(".*")) {
        return vec![reference.to_string()];
    }
    let with = |id: &str| port.map_or(id.to_string(), |p| format!("{id}:{p}"));
    ids.filter(|id| super::command::matches(owner, id)).map(with).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_and_references() {
        assert_eq!(copies("s1.leg", Some(Pattern::List(2))), vec![("s1.leg#1".into(), 1), ("s1.leg#2".into(), 2)]);
        assert_eq!(copies("s1.tank", None), vec![("s1.tank".into(), 0)]);
        assert_eq!(nth("leg_#", 3), "leg_3");
        assert_eq!(port_ref("s1.merlin#2:ox_in").unwrap(), ("s1.merlin#2", "ox_in"));
        let ids = ["s1.merlin#1", "s1.merlin#2", "s1.tank", "s1.merlin#10"];
        let out = expand("s1.merlin#*:ox_in", ids.iter().copied());
        assert_eq!(out, ["s1.merlin#1:ox_in", "s1.merlin#2:ox_in", "s1.merlin#10:ox_in"]);
        assert_eq!(expand("s1.tank:lox.outlet", ids.iter().copied()), ["s1.tank:lox.outlet"]);
        let dotted = ["s1.merlin.1", "s1.merlin.2", "s1.tank"];
        assert_eq!(
            expand("s1.merlin.*:fuel_in", dotted.iter().copied()),
            ["s1.merlin.1:fuel_in", "s1.merlin.2:fuel_in"]
        );
        let radial = Pattern::Radial { count: 4, start_deg: 45.0 };
        assert_eq!((radial.angle_deg(1), radial.angle_deg(4)), (45.0, 315.0));
    }
}
