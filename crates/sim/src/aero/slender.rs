//! Lift of slender bodies and fins below hypersonic speeds (D070).
//!
//! * **Body sections** (baked, [`BodySections`]): the craft's long axis
//!   (the principal axis of largest spread of its surface area) and the
//!   cross-section area A(x) at stations along it, from the surface
//!   triangles by the divergence theorem: A(x) = −Σ A_t·(n_t·â) over the
//!   triangles upstream of x (a closed surface, cut at x).
//! * **Normal force of a slender body** ([`body_normal`]), per section,
//!   from the nose: the potential (Munk) term q·sin 2α·cos(α/2)·dA/dx where
//!   the area grows (separated where it shrinks: a base keeps it), and the
//!   viscous crossflow term q·η·C_dc·sin²α·2r(x) (Allen & Perkins, NACA TR
//!   1048, 1951; the form of Jorgensen, NASA TR R-474, 1977). Together:
//!   C_N = sin 2α·cos(α/2)·A_b/A + η·C_dc·sin²α·A_p/A. η from Allen &
//!   Perkins' fineness table, C_dc = 1.2 (a cylinder in subcritical
//!   crossflow). It acts along the crossflow (the air's velocity across
//!   the axis) at each section's station on the axis.
//! * **Slenderness weight** ([`slenderness`]): the fineness along the flow
//!   f = L/D (L the craft's extent along the flow, D the diameter of its
//!   projected area); 0 up to f = 2 (blunt: the Newtonian distribution
//!   keeps the lateral force), 1 from f = 4, smooth between.
//! * **Fins** ([`Fin`], [`fin_normal`]): thin parts (a cell whose opposite
//!   face is closer than [`WING_RATIO`] × the craft's size) grouped by
//!   primitive. Normal force q·C_Nα·sin α·cos α·S on the planform S,
//!   C_Nα from the finite-wing lift slope: subsonic Helmbold's
//!   2π·AR/(2 + √(4 + AR²β²)), β = √(1−M²) (Prandtl–Glauert; DATCOM
//!   §1.2.2.1 without sweep); supersonic Ackeret's 4/β with the tip loss of
//!   a rectangular wing, (4/β)·(1 − 1/(2β·AR)), β = √(M²−1); linear
//!   between M 0.8 and 1.2. The aspect ratio from the fin's extent along
//!   the flow (chord) and its area. It stalls: full to 15°, gone at 30°
//!   (the Newtonian pressure carries the flat plate beyond). At the
//!   quarter chord subsonic, the half chord supersonic.

use super::bake::CellGeometry;
use crate::math;
use glam::DVec3;

/// Stations along the body axis.
pub const STATIONS: usize = 64;
/// A cell whose opposite face is closer than this × the craft's size is
/// part of a fin.
pub const WING_RATIO: f64 = 0.03;
/// Crossflow drag coefficient of a circular cylinder, subcritical.
pub const CROSSFLOW_CD: f64 = 1.2;

/// The body's cross-section areas along its long axis.
#[derive(Clone, Debug, PartialEq)]
pub struct BodySections {
    /// Unit long axis (body axes).
    pub axis: DVec3,
    /// The axis passes through this point (the surface's area centroid).
    pub point: DVec3,
    /// Position along the axis (relative to `point`) of the first station.
    pub x0: f64,
    /// Station spacing (m).
    pub dx: f64,
    /// Cross-section area at each station boundary (STATIONS + 1 values).
    pub area: Vec<f64>,
}

impl BodySections {
    /// From triangle centroids, unit normals and areas (a closed surface).
    pub fn new(tris: &[(DVec3, DVec3, f64)], vertices: &[DVec3]) -> Self {
        let total: f64 = tris.iter().map(|t| t.2).sum();
        let point = tris.iter().map(|t| t.0 * t.2).sum::<DVec3>() / total.max(1e-300);
        let axis = long_axis(tris, point);
        let (lo, hi) = vertices
            .iter()
            .map(|v| (*v - point).dot(axis))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(x), b.max(x)));
        let dx = ((hi - lo) / STATIONS as f64).max(1e-9);
        // Each triangle's −A·(n·â) at its centroid's station, then summed
        // from the front: the area enclosed at each boundary.
        let mut bins = vec![0.0; STATIONS];
        for &(c, n, a) in tris {
            let k = (((c - point).dot(axis) - lo) / dx).floor().clamp(0.0, (STATIONS - 1) as f64) as usize;
            bins[k] -= a * n.dot(axis);
        }
        let mut area = Vec::with_capacity(STATIONS + 1);
        let mut sum = 0.0;
        area.push(0.0);
        for b in bins {
            sum += b;
            area.push(sum.max(0.0));
        }
        *area.last_mut().expect("stations") = 0.0;
        BodySections { axis, point, x0: lo, dx, area }
    }

    /// Largest cross-section area (m²).
    pub fn max_area(&self) -> f64 {
        self.area.iter().copied().fold(0.0, f64::max)
    }

    /// Length along the axis (m).
    pub fn length(&self) -> f64 {
        self.dx * STATIONS as f64
    }
}

/// The principal axis of largest spread of the area (power iteration on
/// its second moment, from the coordinate axis of largest spread).
fn long_axis(tris: &[(DVec3, DVec3, f64)], point: DVec3) -> DVec3 {
    let mut m = [[0.0; 3]; 3];
    for &(c, _, a) in tris {
        let r = (c - point).to_array();
        for (i, row) in m.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x += a * r[i] * r[j];
            }
        }
    }
    let start = (0..3).fold(0, |b, i| if m[i][i] > m[b][b] { i } else { b });
    let mut v = DVec3::ZERO;
    v[start] = 1.0;
    for _ in 0..64 {
        let w = DVec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        );
        if w.length_squared() == 0.0 {
            break;
        }
        v = w.normalize();
    }
    v
}

/// Allen & Perkins' crossflow drag proportionality factor η against the
/// fineness ratio L/D (NACA TR 1048, fig. 4; ≈ 1 for an infinite cylinder).
pub fn crossflow_eta(fineness: f64) -> f64 {
    const TABLE: [(f64, f64); 7] =
        [(2.0, 0.56), (4.0, 0.60), (8.0, 0.66), (12.0, 0.69), (16.0, 0.71), (24.0, 0.74), (40.0, 0.78)];
    if fineness <= TABLE[0].0 {
        return TABLE[0].1;
    }
    for w in TABLE.windows(2) {
        if fineness <= w[1].0 {
            let f = (fineness - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + (w[1].1 - w[0].1) * f;
        }
    }
    TABLE[TABLE.len() - 1].1
}

/// Weight of the slender-body model for fineness `f` along the flow: 0 to
/// 2, 1 from 4, smoothstep between.
pub fn slenderness(f: f64) -> f64 {
    let x = ((f - 2.0) / 2.0).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Slender-body normal force and its moment about the craft origin, per
/// unit q (m², m³), for the flow direction `d` (unit, the air's motion).
pub fn body_normal(b: &BodySections, d: DVec3) -> (DVec3, DVec3) {
    // The nose is the end the air comes from.
    let from = -d;
    let (nose, forward) = if from.dot(b.axis) >= 0.0 { (b.axis, true) } else { (-b.axis, false) };
    let cos_a = from.dot(nose).clamp(-1.0, 1.0);
    let cross = d - nose * d.dot(nose);
    let across = cross.length();
    if across < 1e-12 {
        return (DVec3::ZERO, DVec3::ZERO);
    }
    let c = cross / across;
    let alpha = math::acos(cos_a);
    let (sin_a, sin2a) = (across, math::sin(2.0 * alpha));
    let potential = sin2a * math::cos(0.5 * alpha);
    let length = b.length();
    let diameter = 2.0 * (b.max_area() / math::PI).sqrt();
    let viscous = crossflow_eta(length / diameter.max(1e-9)) * CROSSFLOW_CD * sin_a * sin_a;
    let (mut f, mut m) = (DVec3::ZERO, DVec3::ZERO);
    for k in 0..STATIONS {
        // Station k from the nose.
        let (i0, i1) = if forward { (STATIONS - k, STATIONS - k - 1) } else { (k, k + 1) };
        let (a0, a1) = (b.area[i0], b.area[i1]);
        let grow = (a1 - a0).max(0.0);
        let r = (0.5 * (a0 + a1) / math::PI).sqrt();
        let n = potential * grow + viscous * 2.0 * r * b.dx;
        if n == 0.0 {
            continue;
        }
        let x = b.x0 + b.dx * (0.5 * (i0 + i1) as f64);
        let at = b.point + b.axis * x;
        f += c * n;
        m += at.cross(c * n);
    }
    (f, m)
}

/// A fin: the thin cells of one primitive.
#[derive(Clone, Debug, PartialEq)]
pub struct Fin {
    /// Its cells (indices into the bake's geometry).
    pub cells: Vec<u32>,
    /// Plate normal (unit, one side).
    pub normal: DVec3,
    /// Planform area (one side, m²).
    pub area: f64,
    /// Area centroid.
    pub centroid: DVec3,
}

/// Finite-wing normal-force slope (per radian) at Mach `mach` for aspect
/// ratio `ar`.
pub fn fin_slope(mach: f64, ar: f64) -> f64 {
    let sub = |m: f64| {
        let b2 = 1.0 - m * m;
        2.0 * math::PI * ar / (2.0 + (4.0 + ar * ar * b2).sqrt())
    };
    let sup = |m: f64| {
        let b = (m * m - 1.0).sqrt();
        4.0 / b * (1.0 - 1.0 / (2.0 * b * ar)).max(0.5)
    };
    if mach <= 0.8 {
        sub(mach)
    } else if mach >= 1.2 {
        sup(mach)
    } else {
        let w = (mach - 0.8) / 0.4;
        sub(0.8) + (sup(1.2) - sub(0.8)) * w
    }
}

/// Stall: 1 to 15° incidence, 0 from 30°.
fn stall(alpha: f64) -> f64 {
    let deg = alpha.abs() * 180.0 / math::PI;
    ((30.0 - deg) / 15.0).clamp(0.0, 1.0)
}

/// A fin's normal force and moment about the craft origin, per unit q, for
/// the flow direction `d` at Mach `mach`.
pub fn fin_normal(fin: &Fin, cells: &[CellGeometry], d: DVec3, mach: f64) -> (DVec3, DVec3) {
    let s = -fin.normal.dot(d);
    let t = d + fin.normal * s;
    let along = t.length();
    if along < 1e-9 || fin.area <= 0.0 {
        return (DVec3::ZERO, DVec3::ZERO);
    }
    let t = t / along;
    let alpha = math::asin(s.clamp(-1.0, 1.0));
    // Chord: the fin's extent along the flow in its plane.
    let (lo, hi) = fin.cells.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &i| {
        let c = &cells[i as usize];
        let (x, h) = (c.centroid.dot(t), c.half_extent(t));
        (lo.min(x - h), hi.max(x + h))
    });
    let chord = (hi - lo).max(1e-9);
    let ar = fin.area / (chord * chord);
    let cn = fin_slope(mach, ar) * math::sin(alpha) * math::cos(alpha) * stall(alpha);
    let force = -fin.normal * (cn * fin.area);
    // Quarter chord subsonic, half chord supersonic.
    let cp = if mach <= 0.8 {
        0.25
    } else if mach >= 1.2 {
        0.5
    } else {
        0.25 + 0.25 * (mach - 0.8) / 0.4
    };
    let at = fin.centroid + t * (lo + cp * chord - fin.centroid.dot(t));
    (force, at.cross(force))
}
