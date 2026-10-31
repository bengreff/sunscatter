//! Stress craft (D081, D121): **for benchmarks and tests only.** Not a part
//! catalog, a craft builder or a design the game offers: craft creation is
//! the owner's to guide (interview 2026-10-26). It measures the craft
//! pipeline at scale (`examples/bench_scale`) and gives tests many-part craft.
//!
//! **`stress-<n>`** is a craft of catalog parts on the real path ([`file`],
//! [`parts_plan`]): [`super::small`]'s test parts of the real generators
//! (capsule, tanks, thrust structures, engines, legs, nose cones, the
//! capsule's avionics, cells and thrusters) assembled by rule into a core
//! stack and rings of radial boosters of exactly n parts, built like any
//! assembly ([`super::build`]). Not a count of arbitrary cubes: a plausible
//! many-part craft (the owner, 2026-10-30). 300 parts: a core of 2 tanks
//! and 4 rings (40 boosters of 1–2 tanks, 164 engines); 1,000: 3 tanks, 7
//! rings (112 boosters, 452 engines); 5,000: 10 tanks, 12 rings (312
//! boosters of 9–10 tanks, 1,252 engines). A stress vessel reloads from a
//! save: its craft id `stress-<n>` names the craft ([`super::registry`]).

use super::assembly::AssemblyFile;
use super::file::Shape;
use super::mesh::analytic_volume;
use super::parts::CraftDef;
use super::{CellOptions, Craft, CraftId};
use glam::DVec3;

/// The cell options of stress craft: every craft of parts' (so a design
/// rebuilt from its id after a load is the same bits).
pub fn cell_options(parts: usize) -> CellOptions {
    CellOptions::for_parts(parts)
}

/// The craft id of the stress craft of `parts` parts.
pub fn id(parts: usize) -> CraftId {
    CraftId(format!("stress-{parts}"))
}

/// The parts' dry centre of mass with each part's mass at its primitives'
/// volume centroid (body axes): against the shells' (`Craft::mass`).
pub fn volume_com(def: &CraftDef) -> DVec3 {
    let (m, moment) = def.parts.iter().fold((0.0, DVec3::ZERO), |(m, s), p| {
        let shapes = p.primitives.iter().map(|&q| &def.geometry.primitives[q as usize].shape);
        let (v, c) =
            shapes.fold((0.0, DVec3::ZERO), |(v, c), x| (v + analytic_volume(x), c + centroid(x) * analytic_volume(x)));
        (m + p.dry_mass, s + c / v * p.dry_mass)
    });
    moment / m
}

/// Volume centroid of a primitive (body axes).
fn centroid(shape: &Shape) -> DVec3 {
    // Radii never both zero (validated).
    let frustum = |base: DVec3, axis: DVec3, a: f64, b: f64| {
        base + axis * ((a * a + 2.0 * a * b + 3.0 * b * b) / (4.0 * (a * a + a * b + b * b)))
    };
    match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => frustum(base, axis, r_base, r_top),
        Shape::Cylinder { base, axis, .. } => base + axis * 0.5,
        Shape::Strut { from, to, .. } => (from + to) * 0.5,
        Shape::SphereCap { center, axis, radius, height } => {
            let d = 3.0 * (2.0 * radius - height) * (2.0 * radius - height) / (4.0 * (3.0 * radius - height));
            center + axis.normalize() * d
        }
        Shape::Box { center, .. } => center,
    }
}

#[cfg(test)]
#[path = "stress_tests.rs"]
mod tests;

// The stress craft of catalog parts (D121): what `stress-<n>` names.

/// The core stack besides its tanks: the capsule, a thrust structure, four
/// engines and four legs.
const CORE_PARTS: usize = 10;
/// A booster besides its tanks: a nose cone, a thrust structure and four
/// engines.
const BOOSTER_PARTS: usize = 6;
/// The radius of booster ring k is k times this (m): a booster's tank is
/// 2 m across.
const RING_SPACING: f64 = 2.3;
/// The height of the boosters' nose cones' bases (m; the capsule's base is
/// at 0, the core's tanks below it).
const BOOSTER_TOP: f64 = -1.0;

/// How [`file`] spends `n` parts.
#[derive(Clone, Debug, PartialEq)]
pub struct PartsPlan {
    /// Tanks in the core stack.
    pub core_tanks: usize,
    /// The core's thrust structure and four engines.
    pub engines: bool,
    /// The core's four legs.
    pub legs: bool,
    /// Rings of boosters round the core (ring k: 4k boosters).
    pub rings: usize,
    /// Each booster's tanks, ring by ring.
    pub boosters: Vec<usize>,
}

/// The plan of `n` parts (n ≥ 1): the capsule alone, then tanks under it,
/// then a thrust structure and engines, then legs, then rings of boosters
/// of `1 + n/600` tanks while they fit; the rest go to the core's tanks (at
/// most `2 + n/600`) and then to the boosters, one more tank each in turn.
pub fn parts_plan(n: usize) -> PartsPlan {
    let n = n.max(1);
    let small = |core_tanks, engines| PartsPlan { core_tanks, engines, legs: false, rings: 0, boosters: Vec::new() };
    if n < 7 {
        return small(n - 1, false);
    }
    if n <= CORE_PARTS {
        return small(n - 6, true);
    }
    let tanks = 1 + n / 600;
    let per = BOOSTER_PARTS + tanks;
    let boosters_in = |rings: usize| 2 * rings * (rings + 1);
    let mut rings = 0;
    while CORE_PARTS + 1 + boosters_in(rings + 1) * per <= n {
        rings += 1;
    }
    let count = boosters_in(rings);
    let left = n - CORE_PARTS - count * per;
    let core_tanks = if count == 0 { left } else { left.min(2 + n / 600) };
    let extra = left - core_tanks;
    let boosters = (0..count).map(|j| tanks + extra / count + usize::from(j < extra % count)).collect();
    PartsPlan { core_tanks, engines: true, legs: true, rings, boosters }
}

/// The lists of a stress craft's assembly file, as RON text.
#[derive(Default)]
struct Text {
    parts: String,
    extra: String,
    joints: String,
    plumbing: String,
    loading: String,
}

impl Text {
    /// A tank mounted under `on`'s mount `at`, loaded full of LOX and RP-1.
    fn tank(&mut self, id: &str, on: &str, at: &str) {
        use std::fmt::Write;
        let _ = writeln!(self.parts, r#"(id: "{id}", entry: "t-tank", mount: (on: "{on}", at: "{at}", via: "top")),"#);
        let ox = r#"temperature_k: 90.0, pressure: 3.0e5, pressurant: "helium""#;
        let fuel = r#"temperature_k: 293.0, pressure: 3.0e5, pressurant: "helium""#;
        let _ = writeln!(self.loading, r#"(tank: "{id}:ox", {ox}), (tank: "{id}:fuel", {fuel}),"#);
    }

    /// Lines from `tank`'s outlets to the engines of `stack`.
    fn feed(&mut self, tank: &str, stack: &str) {
        use std::fmt::Write;
        for (c, length) in [("ox", 2.0), ("fuel", 1.0)] {
            let _ = writeln!(
                self.plumbing,
                r#"(from: "{tank}:{c}.outlet", to: "{stack}.engine#*:{c}_in", diameter: 0.05, length: {length:?}),"#
            );
        }
    }

    /// Four engines on `stack`'s thrust structure.
    fn engines_on(&mut self, stack: &str) {
        use std::fmt::Write;
        let _ = writeln!(
            self.parts,
            r#"(id: "{stack}.engine", entry: "t-engine", pattern: List(4), mount: (on: "{stack}.thrust", at: "engine_#", via: "mount")),"#
        );
    }

    /// The rings of boosters of plan `p`; returns the boosters' names.
    fn rings(&mut self, p: &PartsPlan) -> Vec<String> {
        use std::fmt::Write;
        let mut boosters = vec![];
        let mut b = 0;
        for k in 1..=p.rings {
            let count = 4 * k;
            for j in 0..count {
                let a = 2.0 * crate::math::PI * j as f64 / count as f64;
                let r = RING_SPACING * k as f64;
                let (x, y) = (r * crate::math::cos(a), r * crate::math::sin(a));
                let parent = if k == 1 {
                    "core.tank1".to_string()
                } else {
                    let inner = 4 * (k - 1);
                    let first = 2 * (k - 1) * (k - 2);
                    format!("b{}.nose", first + (j * (k - 1) + k / 2) / k % inner + 1)
                };
                let tanks = p.boosters[b];
                b += 1;
                let stack = format!("b{b}");
                let _ = writeln!(
                    self.parts,
                    r#"(id: "{stack}.nose", entry: "t-nose", at: (on: Some("{parent}"), pos: ({x:?}, {y:?}, {BOOSTER_TOP:?}))),"#
                );
                let mut above = format!("{stack}.nose");
                for i in 1..=tanks {
                    let id = format!("{stack}.tank{i}");
                    self.tank(&id, &above, "bottom");
                    above = id;
                }
                let _ = writeln!(
                    self.parts,
                    r#"(id: "{stack}.thrust", entry: "t-thrust", mount: (on: "{above}", at: "bottom", via: "top")),"#
                );
                self.engines_on(&stack);
                self.feed(&above, &stack);
                let _ = writeln!(
                    self.joints,
                    r#"(name: "sep.{stack}", between: ("{parent}", "{stack}.nose"), kind: Separation(system: Clamp, impulse: 500.0, strength: 0.8)),"#
                );
                boosters.push(stack);
            }
        }
        boosters
    }
}

/// The assembly file of the stress craft of `n` parts (see [`parts_plan`]): a
/// craft of [`super::small`]'s catalog parts. The capsule carries the
/// small rocket's computer, battery, cells, IMU, GPS receiver, seat and
/// four thruster blocks (their bottle in the core's top tank). Each stack's
/// lowest tank feeds its engines; each booster hangs on a separation joint
/// from the core's top tank (ring 1) or the nearest booster of the ring
/// inside. The sequence: Ignition (every engine), Boosters (shut down and
/// separated), Separation (the capsule). Deterministic: positions from
/// `sim::math`, written exactly.
pub fn file(n: usize) -> AssemblyFile {
    use std::fmt::Write;
    let p = parts_plan(n);
    let mut t = Text::default();
    let mut engines = vec![];
    // The core.
    let mut above = ("capsule".to_string(), "bottom");
    for i in 1..=p.core_tanks {
        let id = format!("core.tank{i}");
        t.tank(&id, &above.0, above.1);
        above = (id, "bottom");
    }
    if p.engines {
        let _ = writeln!(
            t.parts,
            r#"(id: "core.thrust", entry: "t-thrust", mount: (on: "{}", at: "bottom", via: "top")),"#,
            above.0
        );
        t.engines_on("core");
        t.feed(&above.0, "core");
        engines.push("core.engine#*".to_string());
    }
    if p.legs {
        t.parts.push_str(r#"(id: "core.leg", entry: "t-leg", pattern: List(4), mount: (on: "core.thrust", at: "leg_#", via: "root")),"#);
    }
    let boosters = t.rings(&p);
    // The capsule's equipment (its thrusters' bottle in the core's top tank).
    let mut members = vec!["battery", "computer", "imu", "gps", "solar"];
    let mut data: Vec<String> = engines.clone();
    data.extend(boosters.iter().map(|s| format!("{s}.engine#*")));
    data.extend(["imu", "gps"].map(String::from));
    if p.core_tanks > 0 {
        t.extra.push_str(r#"(id: "bottle", entry: "t-bottle", in: "core.tank1", pos: (0.0, 0.0, 3.0)),
            (id: "rcs", entry: "t-rcs", pattern: Radial(count: 4, start_deg: 0.0), in: "capsule", pos: (0.7, 0.0, 0.3)),"#);
        t.plumbing.push_str(r#"(from: "bottle:out", to: "rcs#*:in", diameter: 0.01, length: 3.0),"#);
        members.push("rcs#*");
        data.push("rcs#*".into());
        t.joints.push_str(r#"(name: "separation", between: ("capsule", "core.tank1"), kind: Separation(system: Clamp, impulse: 500.0, strength: 0.8)),"#);
    }
    let list = |v: &[String]| v.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", ");
    let members: Vec<String> = members.iter().map(|s| s.to_string()).collect();
    let mut groups = format!(r#""main": [{}]"#, list(&engines));
    let booster_engines: Vec<String> = boosters.iter().map(|s| format!("{s}.engine#*")).collect();
    let mut sequence = String::new();
    if p.engines {
        let ignite = if boosters.is_empty() { r#"Ignite("main")"# } else { r#"Ignite("main"), Ignite("boosters")"# };
        let _ = writeln!(sequence, r#"(name: "Ignition", actions: [{ignite}]),"#);
    }
    if !boosters.is_empty() {
        let _ = write!(groups, r#", "boosters": [{}]"#, list(&booster_engines));
        let separate: Vec<String> = boosters.iter().map(|s| format!(r#"Separate("sep.{s}")"#)).collect();
        let _ = writeln!(sequence, r#"(name: "Boosters", actions: [Shutdown("boosters"), {}]),"#, separate.join(", "));
    }
    if p.engines {
        let _ = writeln!(sequence, r#"(name: "Separation", actions: [Shutdown("main"), Separate("separation")]),"#);
    }
    let functions = if p.legs { r#"(name: "Legs", key: "L", actions: [Deploy("core.leg#*")])"# } else { "" };
    let Text { parts, extra, joints, plumbing, loading } = t;
    let text = format!(
        r#"(
    name: "Stress craft of {n} parts", crew: 1,
    parts: [(id: "capsule", entry: "t-capsule"),
{parts}],
    components: [
        (id: "computer", entry: "t-computer", in: "capsule", pos: (0.0, 0.0, 0.6)),
        (id: "battery", entry: "t-battery", in: "capsule", pos: (0.0, 0.4, 0.6)),
        (id: "seat", entry: "t-seat", in: "capsule", pos: (0.0, 0.0, 0.3)),
        (id: "imu", entry: "t-imu", in: "capsule", pos: (0.3, 0.0, 0.6)),
        (id: "gps", entry: "t-gps", in: "capsule", pos: (-0.3, 0.0, 0.6)),
        (id: "solar", entry: "t-solar", in: "capsule", pos: (0.45, 0.45, 0.3)),
{extra}],
    joints: [{joints}],
    plumbing: [{plumbing}],
    electrical: [(name: "bus", voltage: 28.0, members: [{}])],
    data: [(from: "computer", to: [{}])],
    loading: [{loading}],
    groups: {{ {groups} }},
    sequence: [{sequence}],
    functions: [{functions}],
    aero: (cd0: 0.4),
    thermal: (skin_max_k: 1100.0, internal_max_k: 400.0, node_size: 0.5, interior_specific_heat: 900.0,
              propellant_specific_heat: 2000.0, interior_conductivity: 5.0, internal_coupling: 2.0),
    impact: (max_speed: 8.0),
    contact: (foot: (stiffness: 1.0e5, damping: 1.0e4, stroke: 0.3, friction: 0.8),
              hull: (stiffness: 1.0e6, damping: 1.0e5, friction: 0.5), stick_speed: 0.05),
    antenna: (gain_dbi: 3.0, power_w: 5.0),
)"#,
        list(&members),
        list(&data),
    );
    crate::catalog::ron_options().from_str(&text).unwrap_or_else(|e| panic!("stress craft of {n} parts: {e}"))
}

/// The definition a stress craft's base id (`stress-<n>`) names (`None`:
/// not a stress craft), built from [`super::small::catalog`].
pub fn definition(base: &str) -> Option<Result<CraftDef, String>> {
    let n = base.strip_prefix("stress-")?.parse::<usize>().ok().filter(|&n| n > 0)?;
    let ctx = crate::catalog::Ctx::data();
    Some(super::build::build(&file(n), super::small::catalog(), &ctx).map(|a| a.def).map_err(|e| e.join("\n")))
}

/// The definition of the stress craft of `n` parts (its primitives and
/// parts, without the cells and mass a [`craft`] builds). Panics if it
/// does not build.
pub fn def(n: usize) -> CraftDef {
    definition(&id(n).0).expect("a stress craft's id").unwrap_or_else(|e| panic!("{e}"))
}

/// The stress craft of `n` parts (`stress-<n>`), built once per process.
pub fn craft(n: usize) -> std::sync::Arc<Craft> {
    super::craft_of(&id(n))
}
