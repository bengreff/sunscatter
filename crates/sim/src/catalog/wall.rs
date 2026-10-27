//! Walls and the geometry generators share: a wall's areal mass, skin and
//! allowable line load from its materials; areas and volumes of the solids
//! of revolution parts are made of.

use crate::craft::{Primitive, Shape, Skin};
use crate::materials::{Finish, Material, Materials};
use crate::math;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Room temperature, where allowables are quoted (K).
pub const ROOM_K: f64 = 295.0;

/// A wall: one material, or a sandwich of two faces on a core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Wall {
    Solid { material: String, thickness: f64 },
    Sandwich { face: String, face_thickness: f64, core: String, core_thickness: f64 },
}

impl Wall {
    fn materials<'a>(&self, m: &'a Materials) -> Result<(&'a Material, Option<&'a Material>), String> {
        match self {
            Wall::Solid { material, .. } => Ok((m.get(material)?, None)),
            Wall::Sandwich { face, core, .. } => Ok((m.get(face)?, Some(m.get(core)?))),
        }
    }

    /// Total thickness (m).
    pub fn thickness(&self) -> f64 {
        match *self {
            Wall::Solid { thickness, .. } => thickness,
            Wall::Sandwich { face_thickness, core_thickness, .. } => 2.0 * face_thickness + core_thickness,
        }
    }

    /// kg/m².
    pub fn areal_mass(&self, m: &Materials) -> Result<f64, String> {
        let (outer, core) = self.materials(m)?;
        Ok(match *self {
            Wall::Solid { thickness, .. } => outer.density * thickness,
            Wall::Sandwich { face_thickness, core_thickness, .. } => {
                2.0 * outer.density * face_thickness + core.map_or(0.0, |c| c.density * core_thickness)
            }
        })
    }

    /// The skin cells take: the outer face's thermal data over the whole
    /// wall's areal mass, with `finish`.
    pub fn skin(&self, m: &Materials, finish: Option<&Finish>) -> Result<Skin, String> {
        let (outer, _) = self.materials(m)?;
        let face = match *self {
            Wall::Solid { thickness, .. } => thickness,
            Wall::Sandwich { face_thickness, .. } => face_thickness,
        };
        let mut skin = outer.skin(face, finish);
        skin.areal_mass = self.areal_mass(m)? + finish.map_or(0.0, |f| f.areal_mass);
        Ok(skin)
    }

    /// The line load (N/m) the wall carries at its ultimate strength at
    /// temperature `t` (a sandwich's two faces).
    pub fn allowable(&self, m: &Materials, t: f64) -> Result<f64, String> {
        let (outer, _) = self.materials(m)?;
        let carrying = match *self {
            Wall::Solid { thickness, .. } => thickness,
            Wall::Sandwich { face_thickness, .. } => 2.0 * face_thickness,
        };
        Ok(outer.strength_at(t).1 * carrying)
    }

    /// The outer material's service temperature (K): a part's interior limit.
    pub fn service_k(&self, m: &Materials) -> Result<f64, String> {
        Ok(self.materials(m)?.0.service_k)
    }

    /// The outer material's specific heat (J/(kg·K)).
    pub fn specific_heat(&self, m: &Materials) -> Result<f64, String> {
        Ok(self.materials(m)?.0.specific_heat)
    }
}

/// The finish `id`, if any.
pub fn finish<'a>(m: &'a Materials, id: &Option<String>) -> Result<Option<&'a Finish>, String> {
    id.as_ref().map(|f| m.finish(f)).transpose()
}

/// Lateral area of a frustum of height `h` between radii `r0` and `r1`.
pub fn frustum_side(h: f64, r0: f64, r1: f64) -> f64 {
    math::PI * (r0 + r1) * ((r1 - r0) * (r1 - r0) + h * h).sqrt()
}

/// Surface area of half a spheroid of equatorial radius `r` and polar
/// height `h` (an ellipsoidal tank head; `h = r` a hemisphere).
pub fn head_area(r: f64, h: f64) -> f64 {
    if (h - r).abs() < 1e-12 * r {
        return math::TAU * r * r;
    }
    if h < r {
        // Oblate: e² = 1 − h²/r².
        let e = (1.0 - h * h / (r * r)).sqrt();
        math::PI * r * r + math::PI * h * h / e * math::atanh(e)
    } else {
        // Prolate: e² = 1 − r²/h².
        let e = (1.0 - r * r / (h * h)).sqrt();
        math::PI * r * r + math::PI * r * h / e * math::asin(e)
    }
}

/// Volume of half a spheroid (m³).
pub fn head_volume(r: f64, h: f64) -> f64 {
    2.0 / 3.0 * math::PI * r * r * h
}

/// A solid primitive named `name` with `skin`.
pub fn primitive(name: &str, shape: Shape, skin: Skin) -> Primitive {
    Primitive { name: name.into(), shape, foot: false, skin: Some(skin), nozzle: false }
}

/// A cylinder or frustum along +Z from `z0` to `z1` between radii.
pub fn along_z(z0: f64, z1: f64, r0: f64, r1: f64) -> Shape {
    let (base, axis) = (DVec3::Z * z0, DVec3::Z * (z1 - z0));
    if r0 == r1 {
        Shape::Cylinder { base, axis, radius: r0 }
    } else {
        Shape::Frustum { base, axis, r_base: r0, r_top: r1 }
    }
}

/// The frustums of a profile of (z, r) points along +Z (z rising): one per
/// segment, zero radii allowed at the ends; with the profile's lateral area.
pub fn profile_shapes(profile: &[(f64, f64)]) -> (Vec<Shape>, f64) {
    let mut shapes = Vec::new();
    let mut area = 0.0;
    for w in profile.windows(2) {
        let ((z0, r0), (z1, r1)) = (w[0], w[1]);
        if z1 - z0 <= 0.0 || (r0 <= 0.0 && r1 <= 0.0) {
            continue;
        }
        shapes.push(along_z(z0, z1, r0, r1));
        area += frustum_side(z1 - z0, r0, r1);
    }
    (shapes, area)
}

/// A spheroidal head as frustums: `n` bands from its equator at `z0`
/// (radius `r`) to its pole `h` away along `sign` (±1).
pub fn head_profile(z0: f64, r: f64, h: f64, sign: f64, n: usize) -> Vec<(f64, f64)> {
    let mut points: Vec<(f64, f64)> = (0..=n)
        .map(|k| {
            let a = math::PI / 2.0 * k as f64 / n as f64;
            (z0 + sign * h * math::sin(a), r * math::cos(a))
        })
        .collect();
    if sign < 0.0 {
        points.reverse();
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Heads against the closed forms: a hemisphere 2πr², a 2:1 ellipsoidal
    /// head (h = r/2) 1.3765·πr²·… (Σ checked by the frustum profile).
    #[test]
    fn head_areas_and_volumes() {
        let r = 1.83;
        assert!((head_area(r, r) - math::TAU * r * r).abs() < 1e-9);
        for h in [0.5 * r, 0.707 * r, 1.2 * r] {
            let (_, area) = profile_shapes(&head_profile(0.0, r, h, 1.0, 400));
            // The profile's lateral area converges to the head's (less the
            // base disc, which a head does not have).
            assert!((area / head_area(r, h) - 1.0).abs() < 1e-4, "{h}: {area} vs {}", head_area(r, h));
        }
        assert!((head_volume(r, r) - 2.0 / 3.0 * math::PI * r * r * r).abs() < 1e-12);
    }
}
