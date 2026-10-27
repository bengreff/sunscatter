//! Craft of catalog parts as the scene draws them (D073: flat colours,
//! cartoon-simple; D100): each part its own closed surface, a flat colour
//! from its outer finish or material, a part on a mechanism turned about
//! its hinge by the ship state's angle, and a flame per running engine
//! sized by its share of full thrust (`Vessel::engine_thrusts`). Pure rules with table tests (rule 8);
//! `super::ships` wires them up.
//!
//! A part's surface is its own primitives' union, not a cut of the craft's
//! union surface: it stays whole when its part moves (a leg deploying) and
//! is the same in a piece after a split (pieces keep the craft's body axes),
//! so meshes are kept per (base craft, part id).

use bevy::prelude::{Quat, Transform, Vec3};
use glam::DVec3;
use sim::craft::mesh::{union_surface, Resolution};
use sim::craft::ship::ShipDef;
use sim::craft::{Craft, RenderMesh};
use sim::materials::Materials;

/// Drawing resolution: a part's straight barrels need no rings along their
/// length (the cells' 0.15 m edges are for heat), its silhouettes 6° steps.
pub const DRAW_RES: Resolution = Resolution { max_edge: 1.0, max_angle: 6.0 * std::f64::consts::PI / 180.0 };

/// Hues for the few surfaces that are not grey (multiplied into the albedo).
const TINTS: &[(&str, [f32; 3])] =
    &[("copper-narloy-z", [1.0, 0.62, 0.42]), ("kevlar-49", [1.0, 0.85, 0.45]), ("solar-cells", [0.35, 0.45, 1.0])];

/// What a part with no known look is drawn in (linear).
pub const UNKNOWN: [f32; 3] = [0.5, 0.5, 0.52];
/// Ground, not vehicle (the pad's launch mount, `PartInfo::ground`):
/// concrete grey (linear).
pub const GROUND: [f32; 3] = [0.32, 0.32, 0.3];

/// The flat colour (linear RGB) of a part's outer surface, its `material`
/// and `finish` (`PartInfo`): its visible albedo taken as one minus the
/// solar absorptivity of the finish, else of the bare material (white
/// paint ~0.8, black paint and carbon fibre ~0.05–0.1, bare aluminium
/// ~0.85), tinted for the few coloured surfaces. Data-driven from
/// `data/materials.ron`, so a new material or finish needs no code.
pub fn colour(material: &str, finish: Option<&str>, m: &Materials) -> [f32; 3] {
    let finish = finish.and_then(|f| m.finish(f).ok());
    let (absorptivity, id) = match finish {
        Some(f) => (f.absorptivity, f.id.as_str()),
        None => match m.get(material) {
            Ok(mat) => (mat.absorptivity, mat.id.as_str()),
            Err(_) => return UNKNOWN,
        },
    };
    let albedo = (1.0 - absorptivity).clamp(0.02, 0.9) as f32;
    let tint = TINTS.iter().find(|(t, _)| *t == id).map_or([1.0; 3], |(_, c)| *c);
    tint.map(|c| c * albedo)
}

/// A part's own closed surface in the craft's axes (part `k` of `craft`).
pub fn part_mesh(craft: &Craft, k: usize) -> RenderMesh {
    let own: Vec<_> =
        craft.parts[k].primitives.iter().map(|&q| craft.geometry.primitives[q as usize].clone()).collect();
    union_surface(&own, &DRAW_RES).render_mesh()
}

/// Meshes appended into one (a vessel's static parts of one colour).
pub fn merge<'a>(meshes: impl IntoIterator<Item = &'a RenderMesh>) -> RenderMesh {
    let mut out = RenderMesh::default();
    for m in meshes {
        let base = out.positions.len() as u32;
        out.positions.extend_from_slice(&m.positions);
        out.normals.extend_from_slice(&m.normals);
        out.indices.extend(m.indices.iter().map(|i| i + base));
    }
    out
}

/// How a craft's parts are drawn: static parts merged by colour, each part
/// on a mechanism alone (it turns).
#[derive(Clone, Debug, PartialEq)]
pub enum Group {
    /// Indices into the craft's parts.
    Static { colour: [f32; 3], parts: Vec<usize> },
    /// A part and its mechanism (an index into the ship definition's).
    Moving { colour: [f32; 3], part: usize, mechanism: usize },
}

/// The groups of parts with `colours`, where `mechanism[k]` is part `k`'s
/// mechanism if it has one; static colours in order of first appearance.
pub fn groups(colours: &[[f32; 3]], mechanism: &[Option<usize>]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for (k, (&colour, m)) in colours.iter().zip(mechanism).enumerate() {
        if let Some(&mechanism) = m.as_ref() {
            out.push(Group::Moving { colour, part: k, mechanism });
            continue;
        }
        let found = out.iter_mut().find_map(|g| match g {
            Group::Static { colour: c, parts } if *c == colour => Some(parts),
            _ => None,
        });
        match found {
            Some(parts) => parts.push(k),
            None => out.push(Group::Static { colour, parts: vec![k] }),
        }
    }
    out
}

/// Each part's mechanism (the first one of its own) in `def`, by the
/// craft's part order (`part_ids`: each part's id).
pub fn mechanisms(def: &ShipDef, part_ids: &[u32]) -> Vec<Option<usize>> {
    part_ids.iter().map(|&id| def.mechanisms.iter().position(|m| m.part == id)).collect()
}

/// The angle a part's shape was built at: deployed in a configuration that
/// deploys it (`CraftId::deployed`, `CraftDef::deploy`), else stowed.
pub fn built_angle(m: &sim::catalog::Mechanism, deployed: bool) -> f64 {
    if deployed {
        m.deployed_deg
    } else {
        m.stowed_deg
    }
}

/// Where a part on a hinge is drawn (craft axes): turned about the axis
/// through `pos` by its angle from the one its shape was built at (deg,
/// right-handed; [`built_angle`]).
pub fn hinge(pos: DVec3, axis: DVec3, built_deg: f64, angle_deg: f64) -> Transform {
    let axis = axis.try_normalize().unwrap_or(DVec3::Z).as_vec3();
    let rotation = Quat::from_axis_angle(axis, (angle_deg - built_deg).to_radians() as f32);
    let p = pos.as_vec3();
    Transform { translation: p - rotation * p, rotation, scale: Vec3::ONE }
}

/// One engine's flame mark: its nozzle exit (craft axes), the exhaust's
/// direction and the exit's radius (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nozzle {
    pub exit: DVec3,
    pub exhaust: DVec3,
    pub radius: f64,
}

/// An engine's nozzle from its gimbal point `pos`, thrust direction `dir`
/// (the force on the craft), exit area and its part's mesh: the exit lies
/// where the mesh reaches farthest down the exhaust.
pub fn nozzle(pos: DVec3, dir: DVec3, exit_area: f64, mesh: &RenderMesh) -> Nozzle {
    let exhaust = -dir.try_normalize().unwrap_or(DVec3::Z);
    let reach =
        mesh.positions.iter().map(|p| (DVec3::from_array(p.map(f64::from)) - pos).dot(exhaust)).fold(0.0, f64::max);
    Nozzle { exit: pos + exhaust * reach, exhaust, radius: (exit_area.max(0.0) / std::f64::consts::PI).sqrt() }
}

/// Flame length per exit diameter at full thrust (a mark, not a plume model).
pub const FLAME_DIAMETERS: f64 = 8.0;

/// The flame of a nozzle at `fraction` of full thrust, as a transform of
/// the unit cone (axis +Y, apex up, height and radius 1): its base at the
/// exit, its apex down the exhaust, as long as the thrust.
pub fn flame(n: &Nozzle, fraction: f64) -> Transform {
    let length = FLAME_DIAMETERS * 2.0 * n.radius * fraction.clamp(0.0, 1.0);
    let centre = n.exit + n.exhaust * (length / 2.0);
    Transform {
        translation: centre.as_vec3(),
        rotation: Quat::from_rotation_arc(Vec3::Y, n.exhaust.as_vec3()),
        scale: Vec3::new(n.radius as f32, length as f32, n.radius as f32),
    }
}

/// Each engine's share of its full (vacuum) thrust from its thrust now
/// (`Vessel::engine_thrusts`, the sim's one rule), what its flame shows.
pub fn engine_fractions(def: &ShipDef, thrusts: &[f64]) -> Vec<f64> {
    (def.engines.iter().zip(thrusts))
        .map(|(e, &f)| if e.data.thrust_vac > 0.0 { (f / e.data.thrust_vac).clamp(0.0, 1.0) } else { 0.0 })
        .collect()
}

#[cfg(test)]
#[path = "parts_tests.rs"]
mod tests;
