//! Mass properties (realism-1 §3a): centre of mass and inertia tensor of the
//! dry craft plus its propellant, recomputed as the propellant drains.
//!
//! The dry mass is spread over the surface as a thin shell, in proportion
//! to each surface's skin areal mass (scaled so the total is `dry_mass`:
//! the shell stands in for structure and equipment too). A made-up craft's
//! propellant is a solid cylinder filling the tank from its base (it
//! settles against the thrust), so its centre of mass and inertia change
//! with the fill level. A craft of parts' liquids are each a cylinder where
//! the plumbing has its slug ([`Slug`], [`MassModel::with_slugs`]).
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
    /// Several tanks (a craft of parts): all filled to the same fraction
    /// (they feed together), their propellant's moments as polynomials in
    /// that fraction. `None`: the one tank above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tanks: Option<TankMoments>,
}

/// The propellant of several tanks filled to the same fraction f, each a
/// solid cylinder filling from its base: its first moment `f·A + f²·B` and
/// second moment `Σ m x xᵀ = f·S₁ + f²·S₂ + f³·S₃` about the body origin
/// (per tank of capacity c, height H, radius r, base b and axis d: mass
/// f·c, centroid b + d·fH/2, spread c·f·(r²/4)(𝟙 − d dᵀ) + c·f³·H²/12·d dᵀ).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TankMoments {
    pub first: [DVec3; 2],
    pub second: [DMat3; 3],
}

/// A tank's liquid as one solid cylinder: `mass` (kg) from `base` along the
/// unit `axis` for `height`, of `radius` (m, body axes). Where it sits is
/// the plumbing's (`systems::plumbing::slug`: settled on the outlet, or
/// moved along the tank by the settling acceleration); a slosh pendulum
/// (Abramson, NASA SP-106) would move a share of it across the axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slug {
    pub mass: f64,
    pub base: DVec3,
    pub axis: DVec3,
    pub height: f64,
    pub radius: f64,
}

impl TankMoments {
    /// The moments of `tanks` (region, capacity).
    pub fn of(tanks: &[(Tank, f64)]) -> Self {
        let mut m = TankMoments { first: [DVec3::ZERO; 2], second: [DMat3::ZERO; 3] };
        for &(t, c) in tanks {
            let (b, d, h, r) = (t.base, t.axis.normalize(), t.axis.length(), t.radius);
            m.first[0] += b * c;
            m.first[1] += d * (c * h / 2.0);
            let across = DMat3::IDENTITY - outer(d, d);
            m.second[0] += (outer(b, b) + across * (r * r / 4.0)) * c;
            m.second[1] += (outer(b, d) + outer(d, b)) * (c * h / 2.0);
            m.second[2] += outer(d, d) * (c * h * h / 3.0);
        }
        m
    }
}

impl MassProps {
    /// Bodies together: masses added, the centre of mass weighted, the
    /// inertias about it (parallel-axis theorem). Nothing: zero mass at the
    /// origin.
    pub fn sum(items: &[MassProps]) -> MassProps {
        let mass: f64 = items.iter().map(|p| p.mass).sum();
        if mass <= 0.0 {
            return MassProps { mass: 0.0, com: DVec3::ZERO, inertia: DMat3::ZERO };
        }
        let com = items.iter().map(|p| p.com * p.mass).sum::<DVec3>() / mass;
        let inertia = items.iter().fold(DMat3::ZERO, |i, p| i + p.inertia + parallel_axis(p.mass, p.com - com));
        MassProps { mass, com, inertia }
    }

    /// A uniform solid `shape` of `mass` (a component's envelope: a box, a
    /// cylinder or a sphere; anything else as its bounding cylinder),
    /// turned by `rot` and moved to `at`.
    pub fn solid(shape: &super::Shape, mass: f64, rot: DMat3, at: DVec3) -> MassProps {
        use super::Shape;
        let (centre, local) = match *shape {
            Shape::Box { center, half } => {
                let s = half * half * (4.0 / 12.0) * mass;
                (center, DMat3::from_diagonal(DVec3::new(s.y + s.z, s.x + s.z, s.x + s.y)))
            }
            Shape::SphereCap { center, radius, height, .. } if height >= 2.0 * radius => {
                (center, DMat3::from_diagonal(DVec3::splat(0.4 * mass * radius * radius)))
            }
            Shape::Cylinder { base, axis, radius } => {
                (base + axis * 0.5, solid_cylinder(mass, radius, axis.length(), axis.normalize()))
            }
            Shape::Frustum { base, axis, r_base, r_top } => {
                (base + axis * 0.5, solid_cylinder(mass, r_base.max(r_top), axis.length(), axis.normalize()))
            }
            Shape::SphereCap { center, radius, .. } => {
                (center, DMat3::from_diagonal(DVec3::splat(0.4 * mass * radius * radius)))
            }
            Shape::Strut { from, to, radius } => {
                ((from + to) * 0.5, solid_cylinder(mass, radius, (to - from).length(), (to - from).normalize()))
            }
        };
        MassProps { mass, com: rot * centre + at, inertia: rot * local * rot.transpose() }
    }
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
            tanks: None,
        }
    }

    /// A part's dry mass properties: `dry_mass` spread over its own
    /// `surface` by skin areal mass (as [`Self::new`]).
    pub fn shell(surface: &Surface, primitives: &[Primitive], default_skin: &Skin, dry_mass: f64) -> MassProps {
        let (m, first, second) =
            shell_moments(surface, |p| primitives[p as usize].skin.unwrap_or(*default_skin).areal_mass);
        let com = first / m;
        let about_origin = inertia_from_second(second * (dry_mass / m));
        MassProps { mass: dry_mass, com, inertia: about_origin - parallel_axis(dry_mass, com) }
    }

    /// The model of a craft of parts: their dry mass properties summed
    /// (parallel-axis theorem) and their tanks (region, capacity), filled
    /// to the same fraction.
    pub fn of_parts(dry: &[MassProps], tanks: &[(Tank, f64)]) -> Self {
        let mass: f64 = dry.iter().map(|p| p.mass).sum();
        let com = dry.iter().map(|p| p.com * p.mass).sum::<DVec3>() / mass;
        let inertia = dry.iter().fold(DMat3::ZERO, |i, p| i + p.inertia + parallel_axis(p.mass, p.com - com));
        let none = (Tank { base: DVec3::ZERO, axis: DVec3::Z, radius: 0.0 }, 0.0);
        let (tank, _) = tanks.first().copied().unwrap_or(none);
        MassModel {
            dry_mass: mass,
            dry_com: com,
            dry_inertia: inertia,
            tank_base: tank.base,
            tank_dir: tank.axis.normalize(),
            tank_height: tank.axis.length(),
            tank_radius: tank.radius,
            capacity: tanks.iter().map(|t| t.1).sum(),
            tanks: (tanks.len() > 1).then(|| TankMoments::of(tanks)),
        }
    }

    /// Mass properties with each tank's own contents (a craft of catalog
    /// parts): the dry craft, each tank's liquid where it sits (a solid
    /// cylinder, [`Slug`]: `systems::plumbing::contents`) and the point
    /// masses (the gas).
    pub fn with_slugs(&self, slugs: &[Slug], points: &[(DVec3, f64)]) -> MassProps {
        let mut items = vec![MassProps { mass: self.dry_mass, com: self.dry_com, inertia: self.dry_inertia }];
        for s in slugs.iter().filter(|s| s.mass > 0.0) {
            let com = s.base + s.axis * (0.5 * s.height);
            items.push(MassProps { mass: s.mass, com, inertia: solid_cylinder(s.mass, s.radius, s.height, s.axis) });
        }
        for &(at, mass) in points {
            if mass > 0.0 {
                items.push(MassProps { mass, com: at, inertia: DMat3::ZERO });
            }
        }
        MassProps::sum(&items)
    }

    /// Mass properties with `propellant` kg in the tank (clamped to
    /// [0, capacity]).
    pub fn at(&self, propellant: f64) -> MassProps {
        let p = propellant.clamp(0.0, self.capacity);
        let mass = self.dry_mass + p;
        if p == 0.0 {
            return MassProps { mass, com: self.dry_com, inertia: self.dry_inertia };
        }
        if let Some(t) = &self.tanks {
            let f = p / self.capacity;
            let first = t.first[0] * f + t.first[1] * (f * f);
            let second = t.second[0] * f + t.second[1] * (f * f) + t.second[2] * (f * f * f);
            let com = (self.dry_com * self.dry_mass + first) / mass;
            // The propellant's second moment about the centre of mass.
            let about = second - outer(first, com) - outer(com, first) + outer(com, com) * p;
            let inertia =
                self.dry_inertia + parallel_axis(self.dry_mass, self.dry_com - com) + inertia_from_second(about);
            return MassProps { mass, com, inertia };
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
    use super::super::Shape;
    use super::*;

    fn close(a: DMat3, b: DMat3, rel: f64) -> bool {
        let scale = b.x_axis.length().max(b.y_axis.length()).max(b.z_axis.length());
        (a - b).x_axis.length().max((a - b).y_axis.length()).max((a - b).z_axis.length()) <= rel * scale
    }

    /// A closed cylinder's dry shell of 1,000 kg (a part's, [`MassModel::shell`]).
    fn cylinder_shell(radius: f64, h: f64, base: DVec3) -> (MassProps, f64) {
        let skin = crate::craft::small::rocket_ref().geometry.skin;
        let prim = Primitive {
            name: String::new(),
            shape: Shape::Cylinder { base, axis: DVec3::Z * h, radius },
            foot: false,
            skin: None,
            nozzle: false,
        };
        let s = union_surface(std::slice::from_ref(&prim), &Resolution::default());
        let area = 2.0 * std::f64::consts::PI * radius * (radius + h);
        (MassModel::shell(&s, &[prim], &skin, 1000.0), area)
    }

    #[test]
    fn closed_cylindrical_shell_matches_the_analytic_inertia() {
        let (r, h) = (1.0, 2.0);
        let (m, area) = cylinder_shell(r, h, DVec3::new(0.5, -0.3, 1.0));
        let sigma = 1000.0 / area;
        let (side, cap) = (sigma * 2.0 * std::f64::consts::PI * r * h, sigma * std::f64::consts::PI * r * r);
        let izz = side * r * r + 2.0 * cap * r * r / 2.0;
        let ixx = side * (r * r / 2.0 + h * h / 12.0) + 2.0 * cap * (r * r / 4.0 + h * h / 4.0);
        let exact = DMat3::from_diagonal(DVec3::new(ixx, ixx, izz));
        assert!(close(m.inertia, exact, 5e-3), "{} vs {exact}", m.inertia);
        assert!((m.com - DVec3::new(0.5, -0.3, 2.0)).length() < 1e-9);
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

    /// The small rocket's tanks at every fill (its plumbing's contents, as
    /// a vessel's mass properties take them): positive definite, the
    /// triangle inequality, symmetric, its mass the dry parts and what the
    /// tanks and bottle hold, its centre of mass near its axis (the
    /// components off it: the battery, cells, IMU and GPS).
    #[test]
    fn the_small_rocket_is_positive_definite_at_every_fill() {
        let c = crate::craft::small::rocket_ref();
        let def = c.ship.as_ref().unwrap();
        let mut state = crate::craft::state::ShipState::initial(def);
        for k in 0..=10 {
            let f = f64::from(k) / 10.0;
            for (t, s) in def.tanks.iter().zip(state.tanks.iter_mut()) {
                s.liquid = f * t.capacity;
            }
            let (slugs, points) = crate::systems::plumbing::contents(def, &state);
            let props = c.mass.with_slugs(&slugs, &points);
            let want = c.mass.dry_mass + state.consumables(def);
            assert!((props.mass - want).abs() < 1e-9 * want, "fill {f}: {} vs {want}", props.mass);
            let i = props.inertia;
            let (a, b, cc) = (i.x_axis.x, i.y_axis.y, i.z_axis.z);
            let minor2 = a * b - i.y_axis.x * i.x_axis.y;
            assert!(a > 0.0 && minor2 > 0.0 && i.determinant() > 0.0, "fill {f}: {i}");
            assert!(a + b >= cc && b + cc >= a && cc + a >= b, "triangle inequality at fill {f}: {i}");
            assert!((i - i.transpose()).x_axis.length() < 1e-9 * a, "symmetric");
            assert!(props.com.truncate().length() < 0.01, "fill {f}: {}", props.com);
        }
    }

    /// Several tanks at the same fill: the polynomial moments against each
    /// tank's slug (a solid cylinder from its base) summed by the
    /// parallel-axis theorem, at fills from empty to full; and the dry parts
    /// summed the same way.
    #[test]
    fn several_tanks_fill_together_as_their_slugs_summed() {
        let tanks = [
            (Tank { base: DVec3::new(0.0, 0.0, -1.0), axis: DVec3::Z * 4.0, radius: 1.5 }, 20_000.0),
            (Tank { base: DVec3::new(3.0, 0.5, 0.0), axis: DVec3::new(0.0, 0.6, 2.5), radius: 0.7 }, 4_000.0),
            (Tank { base: DVec3::new(-2.0, -1.0, 2.0), axis: DVec3::new(1.0, 0.0, 0.0), radius: 0.4 }, 900.0),
        ];
        let dry = [
            MassProps {
                mass: 3_000.0,
                com: DVec3::new(0.1, 0.0, 1.0),
                inertia: solid_cylinder(3_000.0, 1.5, 4.0, DVec3::Z),
            },
            MassProps {
                mass: 500.0,
                com: DVec3::new(3.0, 0.8, 1.2),
                inertia: DMat3::from_diagonal(DVec3::splat(80.0)),
            },
        ];
        let m = MassModel::of_parts(&dry, &tanks);
        let dry_mass = 3_500.0;
        let dry_com = (dry[0].com * 3_000.0 + dry[1].com * 500.0) / dry_mass;
        assert!((m.dry_com - dry_com).length() < 1e-12);
        let capacity = 24_900.0;
        assert_eq!((m.dry_mass, m.capacity), (dry_mass, capacity));
        for f in [0.0, 1e-6, 0.13, 0.5, 0.97, 1.0] {
            // Each body: (mass, centre, inertia about its centre).
            let mut bodies: Vec<(f64, DVec3, DMat3)> = dry.iter().map(|p| (p.mass, p.com, p.inertia)).collect();
            for (t, c) in tanks {
                let (d, h) = (t.axis.normalize(), t.axis.length() * f);
                if f > 0.0 {
                    bodies.push((c * f, t.base + d * (0.5 * h), solid_cylinder(c * f, t.radius, h, d)));
                }
            }
            let mass: f64 = bodies.iter().map(|b| b.0).sum();
            let com = bodies.iter().map(|b| b.1 * b.0).sum::<DVec3>() / mass;
            let inertia = bodies.iter().fold(DMat3::ZERO, |i, b| i + b.2 + parallel_axis(b.0, b.1 - com));
            let props = m.at(capacity * f);
            assert!((props.mass - mass).abs() < 1e-9 * mass, "{f}");
            assert!((props.com - com).length() < 1e-10, "{f}: {} vs {com}", props.com);
            assert!(close(props.inertia, inertia, 1e-12), "{f}: {} vs {inertia}", props.inertia);
        }
    }
}
