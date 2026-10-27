//! Forces on a vessel (translational), expressed in an anchor's frame.
//!
//! In an anchor frame the equation of motion is
//! `a = Σ_s g_s(r − r_s) + non-gravitational − a_anchor`, where `a_anchor` is the
//! anchor's *kinematic* acceleration from its ephemeris. That subtraction is
//! exact for any anchor, so the choice of anchor cannot change the physics
//! (checked by the anchor-invariance tests).
//!
//! Sources whose tidal acceleration is below the world cutoff are *cut*
//! (D023): the vessel feels their pull at the anchor instead of at itself, so
//! their share of `a_anchor` cancels and only the tidal term, bounded by the
//! cutoff, is neglected. (Dropping a cut source's pull altogether would leave
//! its full pull on the anchor as a fictitious force on the vessel.)

pub mod radiation;
pub mod surface;

pub use surface::{MeanSurface, Optics, Surface, SurfaceTable};

use crate::ephem::Snapshot;
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::gen::schwarzschild_accel;
use crate::relativity::proper_time_rate;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// Aerodynamic properties used for drag (isotropic in the prototype).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DragModel {
    /// Drag coefficient times reference area (m²).
    pub cd_area: f64,
    pub mass: f64,
}

/// A gravity source simulated in full: its index in `World::sources` and
/// the degree of its gravity field the vessel includes (0: none; D090,
/// [`field_degree`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Active {
    pub source: usize,
    pub degree: u16,
}

/// The gravity sources simulated in full (sorted by source index), each with
/// the degree of its field the vessel includes. Every other source is
/// *cut*: it pulls the vessel exactly as it pulls the anchor, so only its
/// tidal term (the difference) is neglected. A segment only ever adds
/// sources and raises degrees ([`ActiveSources::grow`]), at step boundaries,
/// so the set is part of the stored integrator state.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActiveSources(pub Vec<Active>);

impl ActiveSources {
    /// The anchor (if it is a source) and every source whose tidal
    /// acceleration at the vessel relative to the anchor,
    /// `|g_s(vessel) − g_s(anchor)|`, is at least the world cutoff (sources
    /// a bound proves below it are not evaluated: `World::may_be_active`),
    /// each with the degree of its field that can matter ([`field_degree`]).
    pub fn select(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> Self {
        let view = world.gravity_view(snap, anchor, r);
        let keep = world
            .may_be_active_all(snap, anchor, r, &view)
            .filter(|&i| {
                let s = &world.sources[i];
                s.node == anchor || tidal(s.gm, snap.relative_r(s.node, anchor), r) >= world.cutoff
            })
            .map(|source| Active { source, degree: field_degree(world, snap, anchor, r, source) })
            .collect();
        ActiveSources(keep)
    }

    /// Every source, each field at its full degree: the full model (tests).
    pub fn all(world: &World) -> Self {
        let degree = |s: &crate::world::Source| {
            s.physical.as_ref().and_then(|p| p.gravity.as_ref()).map_or(0, |f| f.degree() as u16)
        };
        ActiveSources(world.sources.iter().enumerate().map(|(i, s)| Active { source: i, degree: degree(s) }).collect())
    }

    /// Adds the sources [`Self::select`] would pick now (the same set as
    /// `add(&select(..))`, without allocating; sources already in the set
    /// are not re-tested) and raises each member's field degree to what it
    /// needs now. Returns whether anything changed.
    pub fn grow(&mut self, world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> bool {
        let mut grew = false;
        for k in 0..self.0.len() {
            let a = self.0[k];
            let full = world.sources[a.source].physical.as_ref().and_then(|p| p.gravity.as_ref());
            if full.is_some_and(|f| usize::from(a.degree) < f.degree()) {
                let degree = field_degree(world, snap, anchor, r, a.source);
                if degree > a.degree {
                    self.0[k].degree = degree;
                    grew = true;
                }
            }
        }
        let view = world.gravity_view(snap, anchor, r);
        for i in world.may_be_active_all(snap, anchor, r, &view) {
            let s = &world.sources[i];
            if let Err(pos) = self.0.binary_search_by_key(&i, |a| a.source) {
                if s.node == anchor || tidal(s.gm, snap.relative_r(s.node, anchor), r) >= world.cutoff {
                    self.0.insert(pos, Active { source: i, degree: field_degree(world, snap, anchor, r, i) });
                    grew = true;
                }
            }
        }
        grew
    }

    /// Adds the sources of `other` and raises degrees to `other`'s. Returns
    /// whether anything changed.
    pub fn add(&mut self, other: &ActiveSources) -> bool {
        let mut changed = false;
        for a in &other.0 {
            match self.0.binary_search_by_key(&a.source, |b| b.source) {
                Ok(k) if self.0[k].degree < a.degree => {
                    self.0[k].degree = a.degree;
                    changed = true;
                }
                Ok(_) => {}
                Err(pos) => {
                    self.0.insert(pos, *a);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Whether source `i` is simulated in full.
    pub fn contains(&self, i: usize) -> bool {
        self.0.binary_search_by_key(&i, |a| a.source).is_ok()
    }

    /// The degree of source `i`'s field included (`None` if it is cut).
    pub fn degree(&self, i: usize) -> Option<u16> {
        self.0.binary_search_by_key(&i, |a| a.source).ok().map(|k| self.0[k].degree)
    }
}

/// The degree of source `i`'s gravity field a vessel at `r` (relative to
/// `anchor`) includes (D090): where the bounds of every degree above it,
/// summed, could reach a hundredth of the cutoff at the vessel
/// ([`field_threshold`], [`crate::gravity::Field::included_degree`]). The
/// field acts on the vessel only (the anchor's acceleration is its
/// ephemeris'), so leaving a degree out errs by that degree at the vessel,
/// whatever the anchor: the rule depends on the vessel's distance from the
/// body alone (rule 1). With a zero cutoff, every degree of the data.
pub fn field_degree(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, i: usize) -> u16 {
    let s = &world.sources[i];
    let Some(f) = s.physical.as_ref().and_then(|p| p.gravity.as_deref()) else { return 0 };
    let r_s = if s.node == anchor { DVec3::ZERO } else { snap.relative_r(s.node, anchor) };
    let degree = f.included_degree(f.gm.unwrap_or(s.gm), (r - r_s).length(), field_threshold(world));
    degree as u16
}

/// Half-wavelengths of a field's highest included degree an integration
/// step may span ([`ForceContext::max_step`]). Measured (`examples/
/// gravity_convergence`, `GRAVITY_COAST=1 GRAVITY_HCAP`): at 3 a coast at
/// the coasts' tolerance reproduces the tightly integrated truncated field
/// to within 10 % of its truncation error or 1.3 m a day (LEO degrees
/// 30–160, low lunar orbits degrees 30–300); at 6 the high degrees alias
/// (1.3 km a day at degree 40 in LEO, where degree 10 errs by 350 m);
/// uncapped, degree 60 in LEO errs by 700 m a day against its 8 m.
pub const FIELD_STEP: f64 = 3.0;

/// A field's degrees whose summed bounds stay under this (m/s²) move a
/// step's position by less than the coasts' tolerance (0.1 m) even over the
/// longest step (3 h): they do not cap the step ([`ForceContext::max_step`]).
pub const FIELD_STEP_ACCEL: f64 = 1e-9;

/// Budget units (about a microsecond each on the reference Mac,
/// `vessel::STEP_UNITS`) of one force evaluation's gravity fields at the
/// degrees included: ~1.6 ns a term (`examples/gravity_convergence`), so a
/// low lunar orbit's degree 100 is ~8 units an evaluation, LEO's 30 ~1.
pub fn field_units(world: &World, active: &ActiveSources) -> usize {
    let terms: usize = active
        .0
        .iter()
        .filter(|a| a.degree >= 2)
        .filter_map(|a| {
            let f = world.sources[a.source].physical.as_ref()?.gravity.as_deref()?;
            Some(f.evaluation_terms(usize::from(a.degree)))
        })
        .sum();
    terms * 16 / 10_000
}

/// The acceleration a body's neglected field degrees may reach at a vessel
/// (m/s²): a hundredth of the cutoff, as for the other model
/// simplifications (clusters, faint bodies; D023).
pub fn field_threshold(world: &World) -> f64 {
    crate::world::FAINT_ERROR_FRACTION * world.cutoff
}

/// Point-mass gravity at `d` from a source.
fn point_gravity(gm: f64, d: DVec3) -> DVec3 {
    let d2 = d.length_squared();
    -d * (gm / (d2 * d2.sqrt()))
}

/// Magnitude of a source's tidal acceleration at `r` relative to the origin
/// (the anchor), for a source at `r_s`.
fn tidal(gm: f64, r_s: DVec3, r: DVec3) -> f64 {
    (point_gravity(gm, r - r_s) - point_gravity(gm, -r_s)).length()
}

/// Everything the dynamics need for one evaluation.
pub struct ForceContext<'a> {
    pub world: &'a World,
    pub anchor: NodeId,
    pub active: &'a ActiveSources,
    pub drag: Option<DragModel>,
    /// Thrust acceleration (inertial axes, m/s²), constant over the step.
    pub thrust: DVec3,
}

impl ForceContext<'_> {
    /// Acceleration of a vessel at `(r, v)` relative to the anchor at time `t`.
    pub fn accel(&self, t: Epoch, r: DVec3, v: DVec3) -> DVec3 {
        let snap = self.world.snapshot(t);
        self.accel_with(&snap, r, v)
    }

    pub fn accel_with(&self, snap: &Snapshot, r: DVec3, v: DVec3) -> DVec3 {
        self.eval(snap, r, v, false).0
    }

    /// The acceleration and the proper-time rate `dδ/dt`
    /// ([`crate::relativity::proper_time_rate`]) of a vessel at `(r, v)`:
    /// the potential of every source (cut or not) at the vessel, with each
    /// active source's field to its included degree, and its velocity
    /// relative to the barycentre (the anchor's through the frame
    /// tree, plus `v`). The acceleration is the same bits as
    /// [`Self::accel_with`].
    pub fn accel_rate(&self, snap: &Snapshot, r: DVec3, v: DVec3) -> (DVec3, f64) {
        self.eval(snap, r, v, true)
    }

    /// The longest integration step that resolves the gravity fields
    /// included here (D090): [`FIELD_STEP`] half-wavelengths of each field's
    /// highest degree that can move a step's position by the coasts'
    /// tolerance ([`FIELD_STEP_ACCEL`]), at the vessel's speed over that
    /// body's rotating surface, `FIELD_STEP · π d / (N v)`. Longer steps
    /// alias the high degrees, which the error control does not see.
    pub fn max_step(&self, snap: &Snapshot, r: DVec3, v: DVec3) -> f64 {
        let mut cap = f64::INFINITY;
        for a in self.active.0.iter().filter(|a| a.degree >= 2) {
            let s = &self.world.sources[a.source];
            let Some(p) = &s.physical else { continue };
            let Some(f) = p.gravity.as_deref() else { continue };
            let dist = (r - snap.relative_r(s.node, self.anchor)).length();
            let n = f.included_degree(f.gm.unwrap_or(s.gm), dist, FIELD_STEP_ACCEL).min(usize::from(a.degree));
            if n < 2 {
                continue;
            }
            let k = snap.relative(s.node, self.anchor);
            let d = r - k.r;
            let speed = (v - k.v - p.rotation.omega(snap.t).raw().cross(d)).length();
            if speed > 0.0 {
                cap = cap.min(FIELD_STEP * crate::math::PI * dist / (n as f64 * speed));
            }
        }
        cap
    }

    fn eval(&self, snap: &Snapshot, r: DVec3, v: DVec3, rate: bool) -> (DVec3, f64) {
        let root = self.world.eph.root();
        let anchor_kin = snap.relative(self.anchor, root);
        let mut a = -anchor_kin.a + self.thrust;
        let mut u = 0.0;
        let mut active = self.active.0.iter().peekable();
        // A far moon system pulls as one mass at its barycenter (D023
        // clusters, `World::open_clusters`): its members do not count, or
        // near it its stand-in does not; a faint body far away does not
        // count (`World::counts_in`).
        let view = self.world.gravity_view(snap, self.anchor, r);
        let path = snap.path_to(self.anchor);
        for i in self.world.counted(&view) {
            let s = &self.world.sources[i];
            while active.next_if(|a| a.source < i).is_some() {}
            let entry = active.next_if(|a| a.source == i);
            // Positions only, except where a velocity is needed (drag, 1PN).
            let r_s = path.relative_r(s.node);
            let Some(entry) = entry else {
                // Cut: it pulls the vessel as it pulls the anchor, which
                // cancels its share of the anchor's acceleration (rule 1).
                if s.node != self.anchor {
                    a += point_gravity(s.gm, -r_s);
                }
                if rate {
                    u += s.gm / (r - r_s).length();
                }
                continue;
            };
            let d = r - r_s;
            let d2 = d.length_squared();
            let dist = d2.sqrt();
            a -= d * (s.gm / (d2 * dist));
            if rate {
                u += s.gm / dist;
            }
            if let Some(p) = &s.physical {
                // The body's figure to the degree that can matter here (D090,
                // `field_degree`); its potential joins the clock's (J2 is
                // ~3e-13 of the rate in LEO, the rest under 1e-15).
                if let Some(field) = p.gravity.as_deref().filter(|_| entry.degree >= 2) {
                    let gm = field.gm.unwrap_or(s.gm);
                    let (g, pot) = field.accel(gm, d, &p.rotation, snap.t, usize::from(entry.degree), rate);
                    a += g;
                    u += pot;
                }
                if let (Some(atm), Some(drag)) = (&p.atmosphere, self.drag) {
                    // Cheap spherical bound before the exact ellipsoid altitude.
                    if d2.sqrt() - p.radius_eq < atm.top {
                        let fixed = p.rotation.to_fixed(Vec3::from_raw(d), snap.t);
                        let rho = atm.density(p.altitude(fixed));
                        if rho > 0.0 {
                            let body_v = snap.relative(s.node, self.anchor).v;
                            let v_air = body_v + p.rotation.omega(snap.t).raw().cross(d);
                            let v_rel = v - v_air;
                            a -= v_rel * (0.5 * rho * v_rel.length() * drag.cd_area / drag.mass);
                        }
                    }
                }
            }
            if s.relativistic {
                a += schwarzschild_accel(d, v - snap.relative(s.node, self.anchor).v, s.gm);
            }
        }
        let tau_rate = if rate { proper_time_rate(u, (anchor_kin.v + v).length_squared()) } else { 0.0 };
        (a, tau_rate)
    }
}

/// Altitude of a vessel above a body's solid surface (terrain, or sea level
/// where the body has an ocean), and its body-fixed
/// position, from an anchor-relative position.
pub fn altitude_above(
    world: &World,
    snap: &Snapshot,
    anchor: NodeId,
    body: NodeId,
    r: DVec3,
) -> (f64, Vec3<BodyFixed>) {
    let src = world.source(body).expect("body is a source");
    let p = src.physical.as_ref().expect("body has physical data");
    let d = r - snap.relative_r(body, anchor);
    let fixed = p.rotation.to_fixed(Vec3::from_raw(d), snap.t);
    (p.altitude_above_surface(fixed), fixed)
}

/// Ambient pressure (Pa) at an anchor-relative position: the atmosphere of
/// whichever body the vessel is in (zero in vacuum). Uses the same altitude
/// above the ellipsoid as drag.
pub fn ambient_pressure(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> f64 {
    let mut p_total = 0.0;
    let d_star = world.star_distance(snap, anchor, r);
    for src in &world.sources {
        let Some(p) = src.physical.as_ref() else { continue };
        let Some(atm) = &p.atmosphere else { continue };
        if !world.may_reach(snap, anchor, r, d_star, src, p.radius_eq + atm.top) {
            continue;
        }
        let d = r - snap.relative_r(src.node, anchor);
        if d.length() - p.radius_eq >= atm.top {
            continue;
        }
        let fixed = p.rotation.to_fixed(Vec3::from_raw(d), snap.t);
        p_total += atm.pressure(p.altitude(fixed));
    }
    p_total
}
