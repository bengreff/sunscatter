//! Surfaces of revolution from a profile: capsules, nosecones, heat
//! shields, fairings. The profile is (z, r) points along +Z (z rising); each
//! segment is a frustum (a closed solid in the craft's surface), the wall
//! covers the profile's lateral area. Stack mounts sit at the first and
//! last points where the radius is not zero.

use super::output::{Function, Generated, Kind};
use super::structure::{radial_mounts, stack_mount, RadialSet};
use super::wall::{self, Wall, ROOM_K};
use super::{settable, Ctx};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevolutionParams {
    /// (z, r) points, z rising (m).
    pub profile: Vec<(f64, f64)>,
    pub wall: Wall,
    #[serde(default)]
    pub finish: Option<String>,
    /// Mass inside the shell that is part of it (frames, the pressure
    /// vessel's floor, hatches), kg.
    #[serde(default)]
    pub inner_mass: f64,
    #[serde(default)]
    pub radial: Vec<RadialSet>,
    /// Makes the craft commandable (a crew cabin).
    #[serde(default)]
    pub control: bool,
    /// Stretches the profile along z.
    #[serde(default = "one")]
    pub scale_z: f64,
}

fn one() -> f64 {
    1.0
}

settable!(RevolutionParams { inner_mass, scale_z });

impl RevolutionParams {
    pub fn generate(&self, ctx: &Ctx) -> Result<Generated, String> {
        let profile: Vec<(f64, f64)> = self.profile.iter().map(|&(z, r)| (z * self.scale_z, r)).collect();
        let rising = profile.windows(2).all(|w| w[1].0 > w[0].0);
        if profile.len() < 2 || !rising || profile.iter().any(|p| p.1 < 0.0) || self.scale_z <= 0.0 {
            return Err("revolution: two or more points, z rising, radii non-negative".into());
        }
        let (shapes, area) = wall::profile_shapes(&profile);
        if shapes.is_empty() {
            return Err("revolution: the profile encloses nothing".into());
        }
        let m = ctx.materials;
        let skin = self.wall.skin(m, wall::finish(m, &self.finish)?)?;
        let primitives = shapes.into_iter().map(|s| wall::primitive("shell", s, skin)).collect();
        let mut derived = vec![("wall".to_string(), area * self.wall.areal_mass(m)?)];
        if self.inner_mass > 0.0 {
            derived.push(("inner structure (estimate)".into(), self.inner_mass));
        }
        let allowable = self.wall.allowable(m, ROOM_K)?;
        let (first, last) = (profile[0], profile[profile.len() - 1]);
        let mut mounts = Vec::new();
        if first.1 > 0.0 {
            mounts.push(stack_mount("bottom", first.0, -1.0, 2.0 * first.1, allowable));
        }
        if last.1 > 0.0 {
            mounts.push(stack_mount("top", last.0, 1.0, 2.0 * last.1, allowable));
        }
        let radius = |z: f64| {
            let i = profile.partition_point(|p| p.0 <= z).clamp(1, profile.len() - 1);
            let (a, b) = (profile[i - 1], profile[i]);
            a.1 + (b.1 - a.1) * ((z - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
        };
        mounts.extend(radial_mounts(&self.radial, radius, allowable));
        Ok(Generated {
            kind: Kind::Part,
            primitives,
            envelope: None,
            mounts,
            mass: derived.iter().map(|d| d.1).sum(),
            derived,
            outputs: Vec::new(),
            tanks: Vec::new(),
            ports: Vec::new(),
            functions: if self.control { vec![Function::Control] } else { Vec::new() },
            power: 0.0,
            specific_heat: self.wall.specific_heat(m)?,
            max_k: self.wall.service_k(m)?,
        })
    }
}
