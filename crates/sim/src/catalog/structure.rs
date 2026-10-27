//! Structural generators: tanks (a barrel with domes, optionally a common
//! bulkhead), shells (interstages, skirts, trunks, adapters) and thrust
//! structures (an octaweb with engine and leg mounts).
//!
//! A tank's wall is sized, not given: the barrel carries its pressure
//! (hoop stress p·R/t) and an axial load (cylinder buckling with the NASA
//! SP-8007 knockdown, γ = 0.33 for R/t near 500), the domes their pressure
//! at the crown (p·R²/(2·h·t)), each at the material's ultimate strength
//! over the safety factor and never below the minimum gauge. What the
//! walls do not account for (stringers, rings, fittings, welds, insulation)
//! is the entry's unmodelled mass against its published one.

use super::output::{Generated, Kind, MountKind, MountPoint, Port, PortKind, TankSpec};
use super::wall::{self, Wall, ROOM_K};
use super::{settable, Ctx};
use crate::math;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// The cylinder-buckling knockdown γ (NASA SP-8007, unpressurised, R/t ≈ 500).
pub const KNOCKDOWN: f64 = 0.33;
/// Bands of a dome drawn as frustums.
const DOME_BANDS: usize = 6;

/// A tank's end shape.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Dome {
    Hemispherical,
    /// Half a spheroid of height `ratio` × radius (√2/2 is the common
    /// "elliptical" head).
    Elliptical(f64),
}

impl Dome {
    fn height(&self, r: f64) -> f64 {
        match *self {
            Dome::Hemispherical => r,
            Dome::Elliptical(k) => k * r,
        }
    }
}

/// A set of radial mounts `<name>_1 … <name>_n` around the axis at height
/// `z`, the first at `start_deg` from +X towards +Y.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadialSet {
    pub name: String,
    pub z: f64,
    pub count: u32,
    pub start_deg: f64,
}

/// The radial mounts of `sets` on a body of radius `radius(z)`.
pub fn radial_mounts(sets: &[RadialSet], radius: impl Fn(f64) -> f64, allowable: f64) -> Vec<MountPoint> {
    let mut out = Vec::new();
    for s in sets {
        for k in 0..s.count {
            let a = (s.start_deg + 360.0 * k as f64 / s.count as f64) * math::PI / 180.0;
            let n = DVec3::new(math::cos(a), math::sin(a), 0.0);
            out.push(MountPoint {
                name: format!("{}_{}", s.name, k + 1),
                pos: n * radius(s.z) + DVec3::Z * s.z,
                normal: n,
                roll: DVec3::Z,
                size: 0.0,
                kind: MountKind::Radial,
                allowable,
            });
        }
    }
    out
}

/// A stack mount at height `z` facing `normal` (±Z), diameter `size`.
pub fn stack_mount(name: &str, z: f64, normal: f64, size: f64, allowable: f64) -> MountPoint {
    MountPoint {
        name: name.into(),
        pos: DVec3::Z * z,
        normal: DVec3::Z * normal,
        roll: DVec3::X,
        size,
        kind: MountKind::Stack,
        allowable,
    }
}

/// A cylindrical tank.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TankParams {
    /// Outer radius (m).
    pub radius: f64,
    /// Cylinder between the domes' equators (m).
    pub barrel: f64,
    pub domes: Dome,
    /// Compartments bottom first: (name, the fluid it is designed for).
    /// Two share a common bulkhead at `split` of the barrel.
    pub compartments: Vec<(String, String)>,
    #[serde(default = "half")]
    pub split: f64,
    /// Cylinder beyond the lower and upper domes' equators (m): the skirts
    /// that join the next parts (the domes sit inside them).
    #[serde(default)]
    pub skirt_low: f64,
    #[serde(default)]
    pub skirt_high: f64,
    pub material: String,
    #[serde(default)]
    pub finish: Option<String>,
    /// Maximum expected operating pressure (Pa).
    pub pressure: f64,
    /// Ultimate over operating loads.
    pub safety_factor: f64,
    /// Axial compression the barrel carries (N).
    #[serde(default)]
    pub axial_load: f64,
    /// Thinnest wall that can be made and handled (m).
    pub min_gauge: f64,
    /// Temperature the walls are sized at (K).
    #[serde(default = "room")]
    pub design_k: f64,
    /// Share of each compartment left as ullage when loaded full.
    #[serde(default = "ullage")]
    pub ullage: f64,
    /// Insulation of a common bulkhead (kg/m²).
    #[serde(default)]
    pub bulkhead_insulation: f64,
}

fn half() -> f64 {
    0.5
}

fn room() -> f64 {
    ROOM_K
}

fn ullage() -> f64 {
    0.03
}

settable!(TankParams {
    radius,
    barrel,
    split,
    skirt_low,
    skirt_high,
    pressure,
    safety_factor,
    axial_load,
    min_gauge,
    design_k,
    ullage,
    bulkhead_insulation
});

/// What a tank's walls are sized with: the material's ultimate strength
/// and modulus at the design temperature, the safety factor, the minimum gauge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizing {
    pub sigma: f64,
    pub modulus: f64,
    pub safety_factor: f64,
    pub min_gauge: f64,
}

/// Barrel and dome thickness (m) of a cylinder of radius `r` with heads of
/// height `h` at pressure `p` carrying `axial` compression (N): hoop,
/// buckling and minimum gauge; crown and minimum gauge.
pub fn tank_walls(r: f64, h: f64, p: f64, axial: f64, s: &Sizing) -> (f64, f64) {
    let sf = s.safety_factor;
    let hoop = sf * p * r / s.sigma;
    // N·SF ≤ γ·0.605·E·t²/R per unit circumference, N = axial/(2πR).
    let buckling = (sf * axial / (math::TAU * KNOCKDOWN * 0.605 * s.modulus)).max(0.0).sqrt();
    let crown = sf * p * r * r / (2.0 * h * s.sigma);
    (hoop.max(buckling).max(s.min_gauge), crown.max(s.min_gauge))
}

impl TankParams {
    pub fn generate(&self, ctx: &Ctx) -> Result<Generated, String> {
        let (r, b, n) = (self.radius, self.barrel, self.compartments.len());
        if !(r > 0.0 && b >= 0.0 && self.pressure >= 0.0 && self.safety_factor >= 1.0 && self.min_gauge > 0.0) {
            return Err("tank: needs radius > 0, barrel ≥ 0, pressure ≥ 0, safety_factor ≥ 1, min_gauge > 0".into());
        }
        if !(n == 1 || n == 2) || !(self.split > 0.0 && self.split < 1.0) || !(0.0..0.5).contains(&self.ullage) {
            return Err("tank: one or two compartments, split in (0, 1), ullage in [0, 0.5)".into());
        }
        for (_, fluid) in &self.compartments {
            ctx.fluids.liquid(fluid)?;
        }
        let m = ctx.materials.get(&self.material)?;
        let finish = wall::finish(ctx.materials, &self.finish)?;
        let h = self.domes.height(r);
        let sizing = Sizing {
            sigma: m.strength_at(self.design_k).1,
            modulus: m.modulus,
            safety_factor: self.safety_factor,
            min_gauge: self.min_gauge,
        };
        let sf = self.safety_factor;
        let (t, t_dome) = tank_walls(r, h, self.pressure, self.axial_load, &sizing);
        let skirt_t = tank_walls(r, h, 0.0, self.axial_load, &sizing).0;
        let (z_low, z_high) = (self.skirt_low, self.skirt_low + b);
        let top = z_high + self.skirt_high;
        let skin = Wall::Solid { material: self.material.clone(), thickness: t }.skin(ctx.materials, finish)?;
        let mut primitives = vec![wall::primitive("barrel", wall::along_z(0.0, top, r, r), skin)];
        // Domes beyond the skirts are part of the outer shape.
        for (z0, sign, name, beyond) in
            [(z_low, -1.0, "lower dome", h > z_low), (z_high, 1.0, "upper dome", h > self.skirt_high)]
        {
            if beyond {
                let (shapes, _) = wall::profile_shapes(&wall::head_profile(z0, r, h, sign, DOME_BANDS));
                primitives.extend(shapes.into_iter().map(|s| wall::primitive(name, s, skin)));
            }
        }
        let head_area = wall::head_area(r, h);
        let rho = m.density;
        let mut derived = vec![
            ("barrel".to_string(), math::TAU * r * b * t * rho),
            ("skirts".to_string(), math::TAU * r * (self.skirt_low + self.skirt_high) * skirt_t * rho),
            ("domes".to_string(), 2.0 * head_area * t_dome * rho),
        ];
        let head = wall::head_volume(r, h);
        let disc = math::PI * r * r;
        // Volumes and liquid regions (the liquid fills from each
        // compartment's lowest point: a cylinder of its volume).
        let mut regions = Vec::new();
        if n == 1 {
            regions.push((head * 2.0 + disc * b, z_low - h));
        } else {
            let z_split = z_low + self.split * b;
            derived.push(("common bulkhead".into(), head_area * (t_dome * rho + self.bulkhead_insulation)));
            // The common dome bulges into the lower compartment.
            regions.push((disc * self.split * b, z_low - h));
            regions.push((disc * (1.0 - self.split) * b + 2.0 * head, z_split - h));
        }
        let burst = self.pressure * sf;
        let mut tanks = Vec::new();
        let mut ports = Vec::new();
        for ((name, fluid), &(volume, base)) in self.compartments.iter().zip(&regions) {
            tanks.push(TankSpec {
                name: name.clone(),
                fluid: fluid.clone(),
                volume,
                base: DVec3::Z * base,
                axis: DVec3::Z,
                radius: r,
                pressure: self.pressure,
                burst,
                ullage: self.ullage,
            });
            let top_of = base + volume / disc;
            let port = |suffix: &str, kind, z: f64, fluid: Option<String>| Port {
                name: format!("{name}.{suffix}"),
                kind,
                fluid,
                pos: DVec3::Z * z,
            };
            ports.push(port("outlet", PortKind::Outlet, base, Some(fluid.clone())));
            ports.push(port("press", PortKind::Pressurant, top_of, None));
            ports.push(port("vent", PortKind::Vent, top_of, None));
        }
        let allowable =
            Wall::Solid { material: self.material.clone(), thickness: skirt_t }.allowable(ctx.materials, ROOM_K)?;
        let mounts = vec![
            stack_mount("bottom", 0.0, -1.0, 2.0 * r, allowable),
            stack_mount("top", top, 1.0, 2.0 * r, allowable),
        ];
        Ok(Generated {
            kind: Kind::Part,
            primitives,
            envelope: None,
            mounts,
            mass: derived.iter().map(|d| d.1).sum(),
            derived,
            outputs: Vec::new(),
            material: self.material.clone(),
            finish: self.finish.clone(),
            tanks,
            ports,
            functions: Vec::new(),
            power: 0.0,
            specific_heat: m.specific_heat,
            max_k: m.service_k,
        })
    }
}

/// A wall of revolution between two radii: an interstage, skirt, trunk or
/// adapter (closed as a solid in the craft's surface).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellParams {
    pub radius_low: f64,
    pub radius_high: f64,
    pub length: f64,
    pub wall: Wall,
    #[serde(default)]
    pub finish: Option<String>,
    #[serde(default)]
    pub radial: Vec<RadialSet>,
}

settable!(ShellParams { radius_low, radius_high, length });

impl ShellParams {
    pub fn generate(&self, ctx: &Ctx) -> Result<Generated, String> {
        let (r0, r1, l) = (self.radius_low, self.radius_high, self.length);
        if !(r0 > 0.0 && r1 > 0.0 && l > 0.0 && self.wall.thickness() > 0.0) {
            return Err("shell: radii, length and wall thickness must be positive".into());
        }
        let finish = wall::finish(ctx.materials, &self.finish)?;
        let skin = self.wall.skin(ctx.materials, finish)?;
        let allowable = self.wall.allowable(ctx.materials, ROOM_K)?;
        let derived = vec![("wall".to_string(), wall::frustum_side(l, r0, r1) * self.wall.areal_mass(ctx.materials)?)];
        let mut mounts = vec![
            stack_mount("bottom", 0.0, -1.0, 2.0 * r0, allowable),
            stack_mount("top", l, 1.0, 2.0 * r1, allowable),
        ];
        mounts.extend(radial_mounts(&self.radial, |z| r0 + (r1 - r0) * (z / l).clamp(0.0, 1.0), allowable));
        Ok(Generated {
            kind: Kind::Part,
            primitives: vec![wall::primitive("shell", wall::along_z(0.0, l, r0, r1), skin)],
            envelope: None,
            mounts,
            mass: derived[0].1,
            derived,
            outputs: Vec::new(),
            material: self.wall.outer().into(),
            finish: self.finish.clone(),
            tanks: Vec::new(),
            ports: Vec::new(),
            functions: Vec::new(),
            power: 0.0,
            specific_heat: self.wall.specific_heat(ctx.materials)?,
            max_k: self.wall.service_k(ctx.materials)?,
        })
    }
}

/// A thrust structure: a cylinder of `radius` and `length` whose bottom
/// carries engine mounts (`engine_1` the centre one if `centre`, then the
/// ring), leg and other radial mounts on its side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThrustParams {
    pub radius: f64,
    pub length: f64,
    /// Plates, ring and beams as one wall.
    pub wall: Wall,
    #[serde(default)]
    pub finish: Option<String>,
    /// Engines on the ring, the ring's radius and the first one's angle.
    pub ring: u32,
    pub ring_radius: f64,
    #[serde(default)]
    pub ring_start_deg: f64,
    pub centre: bool,
    /// Height of the engines' gimbal points (m above the bottom face).
    #[serde(default)]
    pub engine_z: f64,
    #[serde(default)]
    pub radial: Vec<RadialSet>,
}

settable!(ThrustParams { radius, length, ring, ring_radius, ring_start_deg, engine_z });

impl ThrustParams {
    pub fn generate(&self, ctx: &Ctx) -> Result<Generated, String> {
        let (r, l) = (self.radius, self.length);
        if !(r > 0.0 && l > 0.0 && self.ring_radius >= 0.0 && self.ring_radius < r) {
            return Err("thrust structure: radius and length positive, ring_radius in [0, radius)".into());
        }
        let finish = wall::finish(ctx.materials, &self.finish)?;
        let skin = self.wall.skin(ctx.materials, finish)?;
        let areal = self.wall.areal_mass(ctx.materials)?;
        let allowable = self.wall.allowable(ctx.materials, ROOM_K)?;
        let derived = vec![
            ("plates".to_string(), 2.0 * math::PI * r * r * areal),
            ("ring".to_string(), math::TAU * r * l * areal),
        ];
        let mut mounts =
            vec![stack_mount("bottom", 0.0, -1.0, 2.0 * r, allowable), stack_mount("top", l, 1.0, 2.0 * r, allowable)];
        let engine = |k: usize, at: DVec3| MountPoint {
            name: format!("engine_{k}"),
            pos: at + DVec3::Z * self.engine_z,
            normal: -DVec3::Z,
            roll: DVec3::X,
            size: 0.0,
            kind: MountKind::Engine,
            allowable: 0.0,
        };
        if self.centre {
            mounts.push(engine(1, DVec3::ZERO));
        }
        let first = usize::from(self.centre) + 1;
        for k in 0..self.ring as usize {
            let a = (self.ring_start_deg + 360.0 * k as f64 / self.ring as f64) * math::PI / 180.0;
            mounts.push(engine(first + k, DVec3::new(math::cos(a), math::sin(a), 0.0) * self.ring_radius));
        }
        mounts.extend(radial_mounts(&self.radial, |_| r, allowable));
        Ok(Generated {
            kind: Kind::Part,
            primitives: vec![wall::primitive("thrust structure", wall::along_z(0.0, l, r, r), skin)],
            envelope: None,
            mounts,
            mass: derived.iter().map(|d| d.1).sum(),
            derived,
            outputs: Vec::new(),
            material: self.wall.outer().into(),
            finish: self.finish.clone(),
            tanks: Vec::new(),
            ports: Vec::new(),
            functions: Vec::new(),
            power: 0.0,
            specific_heat: self.wall.specific_heat(ctx.materials)?,
            max_k: self.wall.service_k(ctx.materials)?,
        })
    }
}
