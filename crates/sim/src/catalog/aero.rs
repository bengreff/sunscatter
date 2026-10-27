//! Fins, grid fins and landing legs: parts on a radial mount.
//!
//! Part frame: the root (or hinge) at the origin, the mount's normal −X
//! (into the part it hangs on), outward +X, the craft's axis +Z. Grid fins
//! and legs are built **stowed** (the craft is one rigid shape, D103); their
//! mechanism carries the hinge and the deployed angle (a rotation about the
//! hinge axis), and a leg its foot's contact point as built.

use super::output::{Actuator, AeroSurface, Function, Generated, Kind, Mechanism, MountKind, MountPoint};
use super::wall::{self, Wall};
use super::{settable, Ctx};
use crate::craft::Shape;
use glam::DVec3;
use serde::{Deserialize, Serialize};

fn root_mount(size: f64) -> MountPoint {
    MountPoint {
        name: "root".into(),
        pos: DVec3::ZERO,
        normal: -DVec3::X,
        roll: DVec3::Z,
        size,
        kind: MountKind::Radial,
        allowable: 0.0,
    }
}

fn part(primitives: Vec<crate::craft::Primitive>, mounts: Vec<MountPoint>, derived: Vec<(String, f64)>) -> Generated {
    Generated {
        kind: Kind::Part,
        primitives,
        envelope: None,
        mounts,
        mass: derived.iter().map(|d| d.1).sum(),
        derived,
        outputs: Vec::new(),
        material: String::new(),
        finish: None,
        tanks: Vec::new(),
        ports: Vec::new(),
        functions: Vec::new(),
        power: 0.0,
        specific_heat: 900.0,
        max_k: 400.0,
    }
}

/// A fixed planar fin: a panel of `wall` (a sandwich, or solid plate of
/// its thickness) with root and tip chords along +Z.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinParams {
    pub root_chord: f64,
    pub tip_chord: f64,
    pub span: f64,
    pub wall: Wall,
    #[serde(default)]
    pub finish: Option<String>,
}

settable!(FinParams { root_chord, tip_chord, span });

impl FinParams {
    pub fn generate(&self, ctx: &Ctx) -> Result<Generated, String> {
        let chord = 0.5 * (self.root_chord + self.tip_chord);
        let t = self.wall.thickness();
        if !(chord > 0.0 && self.span > 0.0 && t > 0.0) {
            return Err("fin: chords, span and thickness must be positive".into());
        }
        let skin = self.wall.skin(ctx.materials, wall::finish(ctx.materials, &self.finish)?)?;
        let shape =
            Shape::Box { center: DVec3::new(self.span / 2.0, 0.0, 0.0), half: DVec3::new(self.span, t, chord) / 2.0 };
        let area = self.span * chord;
        let derived = vec![("panel".to_string(), area * self.wall.areal_mass(ctx.materials)?)];
        let mut g = part(vec![wall::primitive("fin", shape, skin)], vec![root_mount(chord)], derived);
        g.functions.push(Function::AeroSurface(AeroSurface {
            lattice: false,
            area,
            span: self.span,
            chord,
            deflection_deg: 0.0,
            rate_dps: 0.0,
            pos: DVec3::ZERO,
            axis: DVec3::X,
        }));
        g.specific_heat = self.wall.specific_heat(ctx.materials)?;
        g.max_k = self.wall.service_k(ctx.materials)?;
        (g.material, g.finish) = (self.wall.outer().into(), self.finish.clone());
        Ok(g)
    }
}

/// A grid (lattice) fin: a frame of `span` × `width` and `depth` (the
/// cells' length along the flow when deployed), webs of `web` thickness
/// `cell` apart in both directions. Stowed it lies against the body, span
/// along +Z; deployed it turns `deployed_deg` about its hinge (+Y at the
/// origin) to stick out along +X.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridFinParams {
    pub span: f64,
    pub width: f64,
    pub depth: f64,
    pub cell: f64,
    pub web: f64,
    pub material: String,
    #[serde(default)]
    pub finish: Option<String>,
    pub deflection_deg: f64,
    pub rate_dps: f64,
    pub deploy_rate_dps: f64,
    #[serde(default = "ninety")]
    pub deployed_deg: f64,
    pub actuator: Actuator,
}

fn ninety() -> f64 {
    90.0
}

settable!(GridFinParams { span, width, depth, cell, web, deflection_deg, rate_dps, deploy_rate_dps, deployed_deg });

impl GridFinParams {
    pub fn generate(&self, ctx: &Ctx) -> Result<Generated, String> {
        let (s, w, d) = (self.span, self.width, self.depth);
        if !(s > 0.0 && w > 0.0 && d > 0.0 && self.cell > 0.0 && self.web > 0.0) {
            return Err("grid fin: span, width, depth, cell and web must be positive".into());
        }
        let m = ctx.materials.get(&self.material)?;
        let skin = Wall::Solid { material: self.material.clone(), thickness: self.web }
            .skin(ctx.materials, wall::finish(ctx.materials, &self.finish)?)?;
        // Stowed: depth radial (+X), width tangential (+Y), span up (+Z).
        let shape = Shape::Box { center: DVec3::new(d / 2.0, 0.0, s / 2.0), half: DVec3::new(d, w, s) / 2.0 };
        // Webs both ways (and the frame: the outermost webs).
        let webs = (w / self.cell).floor() + 1.0;
        let rungs = (s / self.cell).floor() + 1.0;
        let volume = self.web * d * (webs * s + rungs * w);
        let derived = vec![("lattice".to_string(), volume * m.density)];
        let mut g = part(vec![wall::primitive("grid fin", shape, skin)], vec![root_mount(w)], derived);
        g.functions.push(Function::AeroSurface(AeroSurface {
            lattice: true,
            area: s * w,
            span: s,
            chord: d,
            deflection_deg: self.deflection_deg,
            rate_dps: self.rate_dps,
            pos: DVec3::ZERO,
            axis: DVec3::X,
        }));
        g.functions.push(Function::Mechanism(Mechanism {
            pos: DVec3::ZERO,
            axis: DVec3::Y,
            stowed_deg: 0.0,
            deployed_deg: self.deployed_deg,
            rate_dps: self.deploy_rate_dps,
            actuator: self.actuator,
            retractable: true,
            foot: None,
            stroke: 0.0,
        }));
        g.specific_heat = m.specific_heat;
        g.max_k = m.service_k;
        (g.material, g.finish) = (self.material.clone(), self.finish.clone());
        Ok(g)
    }
}

/// A landing leg: a strut of `length` from its hinge (at the origin, axis
/// +Y), stowed along +Z against the body, `depth` radially and `width`
/// tangentially, of `wall`; a foot pad of `foot_radius` at its end. Deployed
/// it turns `deployed_deg` about the hinge (from +Z towards +X: beyond 90°
/// it points down and out).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegParams {
    pub length: f64,
    pub depth: f64,
    pub width: f64,
    pub wall: Wall,
    #[serde(default)]
    pub finish: Option<String>,
    pub foot_radius: f64,
    pub foot_mass: f64,
    pub deployed_deg: f64,
    pub deploy_rate_dps: f64,
    pub actuator: Actuator,
    /// The crush core's stroke (m).
    pub stroke: f64,
}

settable!(LegParams { length, depth, width, foot_radius, foot_mass, deployed_deg, deploy_rate_dps, stroke });

impl LegParams {
    pub fn generate(&self, ctx: &Ctx) -> Result<Generated, String> {
        let (l, d, w) = (self.length, self.depth, self.width);
        if !(l > 0.0 && d > 0.0 && w > 0.0 && self.foot_radius > 0.0 && self.foot_mass >= 0.0) {
            return Err("leg: length, depth, width and foot radius must be positive".into());
        }
        let skin = self.wall.skin(ctx.materials, wall::finish(ctx.materials, &self.finish)?)?;
        let strut = Shape::Box { center: DVec3::new(d / 2.0, 0.0, l / 2.0), half: DVec3::new(d, w, l) / 2.0 };
        let foot_at = DVec3::new(d / 2.0, 0.0, l);
        let foot = Shape::Cylinder { base: foot_at, axis: DVec3::X * (d / 2.0).max(0.1), radius: self.foot_radius };
        let area = 2.0 * (d + w) * l;
        let derived = vec![
            ("strut".to_string(), area * self.wall.areal_mass(ctx.materials)?),
            ("foot".to_string(), self.foot_mass),
        ];
        let prims = vec![wall::primitive("leg", strut, skin), wall::primitive("foot", foot, skin)];
        let mut g = part(prims, vec![root_mount(w)], derived);
        g.functions.push(Function::Mechanism(Mechanism {
            pos: DVec3::ZERO,
            axis: DVec3::Y,
            stowed_deg: 0.0,
            deployed_deg: self.deployed_deg,
            rate_dps: self.deploy_rate_dps,
            actuator: self.actuator,
            retractable: false,
            foot: Some(foot_at),
            stroke: self.stroke,
        }));
        g.specific_heat = self.wall.specific_heat(ctx.materials)?;
        g.max_k = self.wall.service_k(ctx.materials)?;
        (g.material, g.finish) = (self.wall.outer().into(), self.finish.clone());
        Ok(g)
    }
}
