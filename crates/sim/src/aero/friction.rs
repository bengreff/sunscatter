//! Skin friction on the wetted cells (D070, D085): the boundary layer of
//! [`super::boundary`] per cell, run length from the most upstream point
//! of the craft along the flow.
//!
//! * **Where:** every cell with flow along it, the shear along the flow's
//!   tangential direction t = d − n(n·d); cells facing downstream more
//!   steeply than 45° (bases, the lee of blunt steps) are in separated flow
//!   and get none. Run length x = distance along the flow from the most
//!   upstream point of the cells; each cell takes the layer over its extent
//!   x ± h along the flow (h from its second moment of area).
//! * **The layer** ([`super::boundary::Edge::layer`]): laminar (Blasius,
//!   Eckert's reference temperature) to transition by Re_θ, turbulent
//!   (White's law, Van Driest II) after, each with the wall's temperature
//!   (adiabatic without one); the edge state from the cell's incidence.

use super::bake::CellGeometry;
use super::boundary::{Layer, Stream};
use glam::DVec3;

/// Cells facing downstream more steeply than this (n·d above) are separated.
pub const SEPARATED: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Each cell's run lengths along `d`: (position along d, half extent), and
/// the most upstream point.
pub fn spans(cells: &[CellGeometry], d: DVec3) -> (Vec<(f64, f64)>, f64) {
    let span: Vec<(f64, f64)> = cells.iter().map(|c| (c.centroid.dot(d), c.half_extent(d))).collect();
    let front = span.iter().map(|&(x, h)| x - h).fold(f64::INFINITY, f64::min);
    (span, front)
}

/// The boundary layer of every attached cell for flow direction `d`
/// (unit): `each(i, s, layer)` with s = sin θ = −n·d, for exposures
/// `exposure(i)` and wall temperatures `wall` (K per cell; `None`:
/// adiabatic); `heat` asks for the heat fluxes too.
pub fn layers(
    cells: &[CellGeometry],
    d: DVec3,
    stream: &Stream,
    exposure: impl Fn(usize) -> f64,
    wall: Option<&[f64]>,
    heat: bool,
    mut each: impl FnMut(usize, f64, &Layer),
) {
    if stream.re_per_m <= 0.0 || cells.is_empty() {
        return;
    }
    let (span, front) = spans(cells, d);
    for (i, (c, &(x, h))) in cells.iter().zip(&span).enumerate() {
        let nd = c.normal.dot(d);
        if nd > SEPARATED {
            continue;
        }
        let s = -nd;
        let f = if s > 0.0 { exposure(i) } else { 0.0 };
        let x = x - front;
        let layer = stream.edge(s, f).layer((x - h).max(0.0), x + h, wall.map(|w| w[i]), stream.temperature, heat);
        each(i, s, &layer);
    }
}

/// Friction force and its moment about the craft origin, per unit q (m²,
/// m³), for the flow direction `d` (unit).
pub fn friction(
    cells: &[CellGeometry],
    d: DVec3,
    stream: &Stream,
    exposure: impl Fn(usize) -> f64,
    wall: Option<&[f64]>,
) -> (DVec3, DVec3) {
    let (mut f, mut m) = (DVec3::ZERO, DVec3::ZERO);
    layers(cells, d, stream, exposure, wall, false, |i, s, layer| {
        let c = &cells[i];
        let along = (1.0 - s * s).max(0.0).sqrt();
        if along < 1e-12 || layer.shear == 0.0 {
            return;
        }
        let shear = (d + c.normal * s) * (layer.shear / along * c.area);
        f += shear;
        m += c.centroid.cross(shear);
    });
    (f, m)
}
