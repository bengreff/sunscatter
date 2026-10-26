//! What attitude-dependent forces on rails need of a craft's surface
//! (`docs/features/limbo.md` §1): per direction in body axes, the projected
//! area, the hypersonic continuum drag area and the two radiation-pressure
//! moments of the exposed cells. Built once per design from the aero bake
//! ([`crate::aero::AeroBake`]: the exposures self-shadow the craft), and
//! read at every integrator stage of a perturbed coast, so the lookup is a
//! cube map with bilinear interpolation (a few tens of flops), not a search
//! of the geodesic grid.
//!
//! Exact at the bake's 642 grid directions (the exposure rows themselves);
//! the cube map's vertices are barycentric interpolations of those, and
//! vertices on a shared cube edge are computed from the same direction on
//! both faces, so the lookup is continuous across the seams.

use crate::aero::{self, AeroBake, Flow};
use glam::DVec3;

/// Cube-map cells per face edge.
const CUBE: usize = 16;
/// Mach number of the continuum drag area (hypersonic), as the design's
/// orientation-averaged drag area ([`crate::craft::CraftDesign::mean_drag_area`]).
const CONTINUUM_MACH: f64 = 10.0;

/// The face axes and the two in-face axes of the cube map (+X, −X, +Y,
/// −Y, +Z, −Z). A shared edge uses the same parameter on both faces.
const FACES: [(DVec3, DVec3, DVec3); 6] = [
    (DVec3::X, DVec3::Y, DVec3::Z),
    (DVec3::NEG_X, DVec3::Y, DVec3::Z),
    (DVec3::Y, DVec3::X, DVec3::Z),
    (DVec3::NEG_Y, DVec3::X, DVec3::Z),
    (DVec3::Z, DVec3::X, DVec3::Y),
    (DVec3::NEG_Z, DVec3::X, DVec3::Y),
];

/// The surface seen from one direction `d` (body axes), for a stream (air
/// or light) travelling along `d`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    /// Projected area Σ A·cosθ·f (m²): θ the incidence (cosθ = −n·d), f
    /// the cell's exposure.
    pub area: f64,
    /// Hypersonic continuum drag area per unit q along `d` (m²).
    pub cont: f64,
    /// Σ A·cosθ·f·(−n) (m²).
    pub n1: DVec3,
    /// Σ A·cos²θ·f·(−n) (m²).
    pub n2: DVec3,
}

/// Optical properties of the skin for radiation pressure: the fractions
/// of the incident light reflected specularly and diffusely (the rest is
/// absorbed). Placeholders per vessel until skins carry them.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Optics {
    pub specular: f64,
    pub diffuse: f64,
}

impl Default for Optics {
    /// Painted aluminium, roughly: mostly absorbed or diffusely reflected
    /// (a placeholder: a cannonball C_R of about 1.1).
    fn default() -> Self {
        Optics { specular: 0.05, diffuse: 0.2 }
    }
}

impl Surface {
    /// Drag area C_D·A (m²) along the flow at the rarefied weight `bridge`
    /// ([`crate::aero::bridge`]): the continuum area, towards the free
    /// molecular 2·A (fully accommodating, cold wall; the live model's).
    pub fn drag_area(&self, bridge: f64) -> f64 {
        self.cont + (2.0 * self.area - self.cont) * bridge
    }

    /// Radiation force per unit pressure (m², body axes) for light
    /// travelling along unit `s`: Σ A·cosθ·f·[(1−ε)·s − 2(ε·cosθ + δ/3)·n]
    /// (Montenbruck & Gill, *Satellite Orbits*, eq. 3.73).
    pub fn radiation(&self, s: DVec3, optics: Optics) -> DVec3 {
        s * ((1.0 - optics.specular) * self.area)
            + self.n2 * (2.0 * optics.specular)
            + self.n1 * (2.0 / 3.0 * optics.diffuse)
    }
}

/// Orientation averages (over the bake's directions): what a craft whose
/// attitude is unknown presents.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeanSurface {
    /// Mean projected area (m²).
    pub area: f64,
    /// Mean continuum drag area (m²): the design's mean drag area.
    pub cont: f64,
    /// Mean of `n1·d` and `n2·d` (m²): the radiation moments along the light.
    pub n1_along: f64,
    pub n2_along: f64,
}

impl MeanSurface {
    pub fn drag_area(&self, bridge: f64) -> f64 {
        self.cont + (2.0 * self.area - self.cont) * bridge
    }

    /// Radiation force per unit pressure along the light (m², a scalar):
    /// the orientation average of [`Surface::radiation`] along `s`.
    pub fn radiation(&self, optics: Optics) -> f64 {
        (1.0 - optics.specular) * self.area
            + 2.0 * optics.specular * self.n2_along
            + 2.0 / 3.0 * optics.diffuse * self.n1_along
    }
}

/// The per-design table ([`Surface`] at any direction).
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTable {
    /// Per face, per vertex ((CUBE + 1)² row-major): area, cont, n1, n2.
    values: Vec<[f64; 8]>,
    /// Reference length for the Knudsen number (m): the bake's.
    pub length: f64,
    pub mean: MeanSurface,
}

fn pack(s: &Surface) -> [f64; 8] {
    [s.area, s.cont, s.n1.x, s.n1.y, s.n1.z, s.n2.x, s.n2.y, s.n2.z]
}

/// The surface at grid direction `k` of the bake, from its exposure row.
fn at_grid(bake: &AeroBake, k: usize, cd0: f64) -> Surface {
    let d = bake.grid.dirs[k];
    let row = &bake.exposure[k * bake.cells..][..bake.cells];
    let mut s = Surface::default();
    for (c, &e) in bake.geometry.iter().zip(row) {
        let cos = -c.normal.dot(d);
        if cos <= 0.0 || e == 0 {
            continue;
        }
        let w = c.area * f64::from(e) / 255.0 * cos;
        s.area += w;
        s.n1 -= c.normal * w;
        s.n2 -= c.normal * (w * cos);
    }
    let flow = Flow { dir: d, q: 1.0, mach: CONTINUUM_MACH, gamma: aero::air::EARTH_AIR_GAMMA, ..Flow::default() };
    s.cont = aero::aero_forces(bake, &flow, cd0).0.dot(d);
    s
}

impl SurfaceTable {
    /// Builds the table from the bake (`cd0`: the craft's subsonic drag
    /// coefficient, which the continuum force model takes).
    pub fn new(bake: &AeroBake, cd0: f64) -> Self {
        let grid: Vec<[f64; 8]> = (0..bake.grid.dirs.len()).map(|k| pack(&at_grid(bake, k, cd0))).collect();
        let n = grid.len() as f64;
        let mut mean = MeanSurface::default();
        for (row, d) in grid.iter().zip(&bake.grid.dirs) {
            mean.area += row[0];
            mean.cont += row[1];
            mean.n1_along += DVec3::new(row[2], row[3], row[4]).dot(*d);
            mean.n2_along += DVec3::new(row[5], row[6], row[7]).dot(*d);
        }
        mean = MeanSurface {
            area: mean.area / n,
            cont: mean.cont / n,
            n1_along: mean.n1_along / n,
            n2_along: mean.n2_along / n,
        };
        let mut values = Vec::with_capacity(6 * (CUBE + 1) * (CUBE + 1));
        for &(axis, e1, e2) in &FACES {
            for j in 0..=CUBE {
                for i in 0..=CUBE {
                    let (u, v) = (cube_param(i), cube_param(j));
                    let w = bake.grid.locate((axis + e1 * u + e2 * v).normalize());
                    let mut out = [0.0; 8];
                    for k in 0..3 {
                        let row = &grid[w.dirs[k] as usize];
                        for (o, x) in out.iter_mut().zip(row) {
                            *o += w.w[k] * x;
                        }
                    }
                    values.push(out);
                }
            }
        }
        SurfaceTable { values, length: bake.length, mean }
    }

    /// The surface seen from unit direction `d` (body axes).
    pub fn at(&self, d: DVec3) -> Surface {
        let a = d.abs();
        let face = if a.x >= a.y && a.x >= a.z {
            usize::from(d.x < 0.0)
        } else if a.y >= a.z {
            2 + usize::from(d.y < 0.0)
        } else {
            4 + usize::from(d.z < 0.0)
        };
        let (axis, e1, e2) = FACES[face];
        let m = d.dot(axis);
        if m.is_nan() || m <= 0.0 {
            return Surface::default();
        }
        let (u, v) = (d.dot(e1) / m, d.dot(e2) / m);
        let cell = |p: f64| {
            let x = ((p + 1.0) * 0.5 * CUBE as f64).clamp(0.0, CUBE as f64);
            let i = (x as usize).min(CUBE - 1);
            (i, x - i as f64)
        };
        let ((i, fx), (j, fy)) = (cell(u), cell(v));
        let base = face * (CUBE + 1) * (CUBE + 1);
        let at = |i: usize, j: usize| &self.values[base + j * (CUBE + 1) + i];
        let (c00, c10, c01, c11) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
        let mut out = [0.0; 8];
        for k in 0..8 {
            let lo = c00[k] + (c10[k] - c00[k]) * fx;
            let hi = c01[k] + (c11[k] - c01[k]) * fx;
            out[k] = lo + (hi - lo) * fy;
        }
        Surface {
            area: out[0],
            cont: out[1],
            n1: DVec3::new(out[2], out[3], out[4]),
            n2: DVec3::new(out[5], out[6], out[7]),
        }
    }
}

/// The in-face coordinate of cube vertex `i` (−1 to 1).
fn cube_param(i: usize) -> f64 {
    -1.0 + 2.0 * i as f64 / CUBE as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::test_craft;

    #[test]
    fn the_mean_continuum_area_is_the_designs_mean_drag_area() {
        let d = test_craft().design();
        assert_eq!(d.surfaces.mean.cont, d.mean_drag_area);
        // Cauchy: a convex body's mean projected area is a quarter of its
        // surface; the craft is not convex (legs, fins shade each other),
        // so less.
        let surface: f64 = d.cells.iter().map(|c| c.area).sum();
        assert!(d.surfaces.mean.area < 0.25 * surface && d.surfaces.mean.area > 0.15 * surface, "{}", surface);
    }

    #[test]
    fn the_lookup_matches_the_bake_at_its_directions_and_is_continuous() {
        let d = test_craft().design();
        let t = &d.surfaces;
        // At grid directions the cube map is an interpolation of the
        // exact rows: within a few per cent of the bake's own sums.
        for k in (0..d.bake.grid.dirs.len()).step_by(37) {
            let dir = d.bake.grid.dirs[k];
            let exact = d.bake.sums_at(dir).area;
            let got = t.at(dir).area;
            assert!((got - exact).abs() < 0.06 * exact + 0.05, "dir {k}: {got} vs {exact}");
        }
        // Across a cube seam (x = y): no jump.
        for z in [-0.6, 0.0, 0.3] {
            let a = t.at(DVec3::new(1.0, 1.0 - 1e-12, z).normalize());
            let b = t.at(DVec3::new(1.0 - 1e-12, 1.0, z).normalize());
            assert!((a.area - b.area).abs() < 1e-9 * a.area.max(1.0), "{z}: {} vs {}", a.area, b.area);
        }
        // Flying nose first, the air travels along −Z (body axes): far
        // less area than broadside.
        let end_on = t.at(DVec3::NEG_Z).area;
        let side = t.at(DVec3::X).area;
        assert!(side > 3.0 * end_on, "side {side} end-on {end_on}");
    }

    #[test]
    fn radiation_on_a_flat_face_is_the_textbook_value() {
        // One face, area A, facing the light: F/P = (1−ε)A + 2εA + 2δA/3.
        let s = Surface { area: 2.0, cont: 0.0, n1: DVec3::new(0.0, 0.0, 2.0), n2: DVec3::new(0.0, 0.0, 2.0) };
        let light = DVec3::Z;
        for (eps, delta) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.3, 0.4)] {
            let f = s.radiation(light, Optics { specular: eps, diffuse: delta });
            let expected = 2.0 * (1.0 - eps + 2.0 * eps + 2.0 * delta / 3.0);
            assert!((f - DVec3::Z * expected).length() < 1e-12, "{eps} {delta}: {f}");
        }
    }
}
