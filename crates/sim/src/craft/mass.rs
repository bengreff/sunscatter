//! Mass properties (realism-1 §3a): centre of mass and inertia tensor of the
//! dry craft plus its propellant, recomputed as the propellant drains.
//!
//! The dry mass is spread over the surface as a thin shell, in proportion
//! to each surface's skin areal mass (scaled so the total is `dry_mass`:
//! the shell stands in for structure and equipment too). The propellant is
//! a solid cylinder filling the tank from its base (it settles against the
//! thrust), so its centre of mass and inertia change with the fill level.
//! Everything is in body axes; inertia tensors are about the centre of mass.

use super::file::{Primitive, Skin, Tank};
use super::mesh::Surface;
use glam::{DMat3, DVec3};

/// Mass, centre of mass (body axes) and inertia tensor about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassProps {
    pub mass: f64,
    pub com: DVec3,
    pub inertia: DMat3,
}

/// What mass properties are computed from (saved with a vessel).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MassModel {
    pub dry_mass: f64,
    pub dry_com: DVec3,
    /// Dry inertia about `dry_com` (kg·m²).
    pub dry_inertia: DMat3,
    pub tank_base: DVec3,
    /// Unit direction in which the tank fills.
    pub tank_dir: DVec3,
    pub tank_height: f64,
    pub tank_radius: f64,
    /// Propellant mass of a full tank (kg).
    pub capacity: f64,
}

/// `m (|d|² 𝟙 − d dᵀ)`: what moving a mass `m` by `d` from the reference
/// point adds to an inertia tensor (parallel-axis theorem).
pub fn parallel_axis(m: f64, d: DVec3) -> DMat3 {
    (DMat3::from_diagonal(DVec3::splat(d.length_squared())) - outer(d, d)) * m
}

/// `a bᵀ`.
pub fn outer(a: DVec3, b: DVec3) -> DMat3 {
    DMat3::from_cols(a * b.x, a * b.y, a * b.z)
}

/// Inertia of a solid cylinder of mass `m` about its centre, axis `dir` (unit).
pub fn solid_cylinder(m: f64, radius: f64, height: f64, dir: DVec3) -> DMat3 {
    let axial = 0.5 * m * radius * radius;
    let transverse = m * (3.0 * radius * radius + height * height) / 12.0;
    DMat3::from_diagonal(DVec3::splat(transverse)) + outer(dir, dir) * (axial - transverse)
}

/// Mass, first moment and second moment `Σ m x xᵀ` about the origin of a
/// thin shell over `surface`, with areal mass per primitive.
fn shell_moments(surface: &Surface, areal_mass: impl Fn(u32) -> f64) -> (f64, DVec3, DMat3) {
    let (mut m, mut first, mut second) = (0.0, DVec3::ZERO, DMat3::ZERO);
    for (t, tri) in surface.triangles.iter().enumerate() {
        let [a, b, c] = tri.map(|i| surface.positions[i as usize]);
        let area = 0.5 * (b - a).cross(c - a).length();
        let mt = areal_mass(surface.patches[surface.tri_patch[t] as usize].primitive) * area;
        let s = a + b + c;
        m += mt;
        first += s * (mt / 3.0);
        // ∫ x xᵀ dA / A over a triangle = (a aᵀ + b bᵀ + c cᵀ + s sᵀ) / 12.
        second += (outer(a, a) + outer(b, b) + outer(c, c) + outer(s, s)) * (mt / 12.0);
    }
    (m, first, second)
}

/// Inertia about the origin from a second moment `Σ m x xᵀ`.
fn inertia_from_second(second: DMat3) -> DMat3 {
    let trace = second.x_axis.x + second.y_axis.y + second.z_axis.z;
    DMat3::from_diagonal(DVec3::splat(trace)) - second
}

impl MassModel {
    /// The model of a craft: `dry_mass` spread over `surface` by skin areal
    /// mass, and the tank.
    pub fn new(
        surface: &Surface,
        primitives: &[Primitive],
        default_skin: &Skin,
        dry_mass: f64,
        tank: &Tank,
        capacity: f64,
    ) -> Self {
        let (m, first, second) =
            shell_moments(surface, |p| primitives[p as usize].skin.unwrap_or(*default_skin).areal_mass);
        let scale = dry_mass / m;
        let com = first / m;
        let about_origin = inertia_from_second(second * scale);
        MassModel {
            dry_mass,
            dry_com: com,
            dry_inertia: about_origin - parallel_axis(dry_mass, com),
            tank_base: tank.base,
            tank_dir: tank.axis.normalize(),
            tank_height: tank.axis.length(),
            tank_radius: tank.radius,
            capacity,
        }
    }

    /// Mass properties with `propellant` kg in the tank (clamped to
    /// [0, capacity]).
    pub fn at(&self, propellant: f64) -> MassProps {
        let p = propellant.clamp(0.0, self.capacity);
        let mass = self.dry_mass + p;
        if p == 0.0 {
            return MassProps { mass, com: self.dry_com, inertia: self.dry_inertia };
        }
        let h = self.tank_height * p / self.capacity;
        let p_com = self.tank_base + self.tank_dir * (0.5 * h);
        let com = (self.dry_com * self.dry_mass + p_com * p) / mass;
        let inertia = self.dry_inertia
            + parallel_axis(self.dry_mass, self.dry_com - com)
            + solid_cylinder(p, self.tank_radius, h, self.tank_dir)
            + parallel_axis(p, p_com - com);
        MassProps { mass, com, inertia }
    }
}

#[cfg(test)]
mod tests {
    use super::super::mesh::{union_surface, Resolution};
    use super::super::{test_craft, Shape};
    use super::*;

    fn close(a: DMat3, b: DMat3, rel: f64) -> bool {
        let scale = b.x_axis.length().max(b.y_axis.length()).max(b.z_axis.length());
        (a - b).x_axis.length().max((a - b).y_axis.length()).max((a - b).z_axis.length()) <= rel * scale
    }

    fn cylinder_model(radius: f64, h: f64, base: DVec3) -> (MassModel, f64) {
        let skin = test_craft().geometry.skin;
        let prim = Primitive {
            name: String::new(),
            shape: Shape::Cylinder { base, axis: DVec3::Z * h, radius },
            foot: false,
            skin: None,
        };
        let s = union_surface(std::slice::from_ref(&prim), &Resolution::default());
        let tank = Tank { base, axis: DVec3::Z * h, radius };
        let area = 2.0 * std::f64::consts::PI * radius * (radius + h);
        (MassModel::new(&s, &[prim], &skin, 1000.0, &tank, 5000.0), area)
    }

    #[test]
    fn closed_cylindrical_shell_matches_the_analytic_inertia() {
        let (r, h) = (1.0, 2.0);
        let (m, area) = cylinder_model(r, h, DVec3::new(0.5, -0.3, 1.0));
        let sigma = 1000.0 / area;
        let (side, cap) = (sigma * 2.0 * std::f64::consts::PI * r * h, sigma * std::f64::consts::PI * r * r);
        let izz = side * r * r + 2.0 * cap * r * r / 2.0;
        let ixx = side * (r * r / 2.0 + h * h / 12.0) + 2.0 * cap * (r * r / 4.0 + h * h / 4.0);
        let exact = DMat3::from_diagonal(DVec3::new(ixx, ixx, izz));
        assert!(close(m.dry_inertia, exact, 5e-3), "{} vs {exact}", m.dry_inertia);
        assert!((m.dry_com - DVec3::new(0.5, -0.3, 2.0)).length() < 1e-9);
    }

    #[test]
    fn propellant_is_a_solid_cylinder_filling_from_the_base() {
        let (r, h) = (1.0, 2.0);
        let (m, _) = cylinder_model(r, h, DVec3::ZERO);
        let dry = m.at(0.0);
        // Half full: a 1 m slug at the bottom.
        let half = m.at(2500.0);
        let p_com = DVec3::Z * 0.5;
        let com = (dry.com * 1000.0 + p_com * 2500.0) / 3500.0;
        assert!((half.com - com).length() < 1e-12);
        let slug = DMat3::from_diagonal(DVec3::new(
            2500.0 * (3.0 * r * r + 1.0) / 12.0,
            2500.0 * (3.0 * r * r + 1.0) / 12.0,
            2500.0 * r * r / 2.0,
        ));
        let exact = dry.inertia + parallel_axis(1000.0, dry.com - com) + slug + parallel_axis(2500.0, p_com - com);
        assert!(close(half.inertia, exact, 1e-12));
        assert_eq!(m.at(1e9), m.at(5000.0), "clamped to capacity");
    }

    /// The parallel-axis theorem against direct integration: a solid
    /// cylinder away from the origin, integrated on a grid.
    #[test]
    fn parallel_axis_matches_a_direct_integration() {
        let (r, h, c) = (0.7, 1.5, DVec3::new(2.0, -1.0, 0.5));
        let dir = DVec3::Z;
        let n = 120;
        let mut second = DMat3::ZERO;
        let mut count = 0.0;
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    let local = DVec3::new(
                        (2.0 * (i as f64 + 0.5) / n as f64 - 1.0) * r,
                        (2.0 * (j as f64 + 0.5) / n as f64 - 1.0) * r,
                        ((k as f64 + 0.5) / n as f64 - 0.5) * h,
                    );
                    if local.truncate().length() < r {
                        let x = c + local;
                        second += outer(x, x);
                        count += 1.0;
                    }
                }
            }
        }
        let direct = inertia_from_second(second * (10.0 / count));
        let theorem = solid_cylinder(10.0, r, h, dir) + parallel_axis(10.0, c);
        assert!(close(direct, theorem, 2e-3), "{direct} vs {theorem}");
    }

    #[test]
    fn the_test_craft_is_positive_definite_at_every_fill() {
        let c = test_craft();
        let s = union_surface(&c.geometry.primitives, &Resolution::default());
        let m = MassModel::new(&s, &c.geometry.primitives, &c.geometry.skin, 4000.0, &c.geometry.tank, 16000.0);
        let dry_z = m.dry_com.z;
        for k in 0..=10 {
            let p = 16000.0 * f64::from(k) / 10.0;
            let props = m.at(p);
            assert_eq!(props.mass, 4000.0 + p);
            let i = props.inertia;
            let (a, b, cc) = (i.x_axis.x, i.y_axis.y, i.z_axis.z);
            let minor2 = a * b - i.y_axis.x * i.x_axis.y;
            assert!(a > 0.0 && minor2 > 0.0 && i.determinant() > 0.0, "{p} kg: {i}");
            assert!(a + b >= cc && b + cc >= a && cc + a >= b, "triangle inequality at {p} kg: {i}");
            assert!((i - i.transpose()).x_axis.length() < 1e-9 * a, "symmetric");
            // The CoM is the mass-weighted mean of the dry CoM and the
            // propellant's (the tank fills from its base at z = −1).
            if k > 0 {
                let h = 4.7 * p / 16000.0;
                let want = (dry_z * 4000.0 + (-1.0 + 0.5 * h) * p) / (4000.0 + p);
                assert!((props.com.z - want).abs() < 1e-9, "{p} kg: {} vs {want}", props.com.z);
            }
            // Symmetric about Z (four fins, four legs): the centre of mass
            // is on the axis, up to the fin boxes' triangulation (the
            // union trims them against the body along one diagonal of
            // each face: ~20 µm).
            assert!(props.com.truncate().length() < 1e-4, "{p} kg: {}", props.com);
        }
    }
}
