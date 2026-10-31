//! Perturbed coasts (`docs/technical/limbo.md`, D081): what acts on a
//! coasting vessel besides gravity, evaluated at every integrator stage of
//! its stored segment so that any warp only samples the result (rule 4).
//!
//! * **Drag** in every atmosphere up to its model's top (Earth: the US76
//!   table to 1,000 km): `−v_rel·½ρ|v_rel|·C_D·A/m`, the drag area at the
//!   segment's attitude ([`super::pointing`]) from the design's surface
//!   table, bridged from the continuum area to the free-molecular `2·A` in
//!   the Knudsen number (Wilmoth, as the live model), v_rel relative to the
//!   rotating air. Unknown attitude: the orientation averages.
//! * **Radiation pressure** of every star with eclipses
//!   ([`crate::forces::radiation`]) on the exposed cells, with placeholder
//!   specular and diffuse reflectivities ([`Optics`]).
//! * **Boil-off**: a craft of parts' tanks by their law through the coast
//!   ([`crate::systems::coast_law`], D119), the one its ship state reads
//!   at the lattice points; a made-up craft's, a constant propellant mass
//!   flow down to the dry mass.
//!
//! The **live boundary** ([`LiveBoundary`]) says where the coast model's
//! assumptions stop holding and the vessel must fly live ticks.

use super::pointing::Pointing;
use crate::aero;
use crate::body::atmosphere::Atmosphere;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::craft::{CraftId, DesignSlot};
use crate::ephem::Snapshot;
use crate::forces::radiation::starlight;
use crate::forces::{Optics, SurfaceTable};
use crate::frame::NodeId;
use crate::systems::coast_law::CoastLaw;
use crate::systems::plumbing::{self, HeatLaw};
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// Where a coast hands over to live flight (`docs/technical/limbo.md` §2):
/// any of these reached, inside an atmosphere's `top` (live flight sees no
/// air above it). A live vessel coasts again once all are below half
/// (hysteresis) or it is above the top.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LiveBoundary {
    /// Dynamic pressure ½ρv² (Pa). 0.1 Pa: aerodynamic torques on the
    /// test craft (≈ 120 m³·q from 30 m² broadside at C_D 2 with a 2 m
    /// lever) are then ≈ 12 N·m, 0.15 % of its 8 kN·m RCS authority, so
    /// a hold is kept without effort; transitional lift (≲ 15 % continuum
    /// at Kn ≈ 1, L/D 0.3) is ≈ 1e-5 m/s², negligible beside the drag.
    /// Circular low orbit reaches it near 143 km.
    pub dynamic_pressure: f64,
    /// Free-molecular heat flux ½ρv³ (W/m²), the most the flow can
    /// deposit: 1,000 W/m², comparable to sunlight (1,361), which the coast
    /// thermal lattice carries; above it the skin's heating is the live
    /// model's business. Reached near 140 km in low orbit, near 150 km at
    /// 11 km/s.
    pub heating: f64,
    /// Density (kg/m³): coasts never run in dense air, where the bridged
    /// drag area (no lift, hypersonic continuum limit) is wrong whatever the
    /// speed: 1e-7 kg/m³ is about 108 km on Earth. Slow suborbital arcs
    /// hand over here.
    pub density: f64,
}

impl LiveBoundary {
    pub const DEFAULT: LiveBoundary = LiveBoundary { dynamic_pressure: 0.1, heating: 1_000.0, density: 1e-7 };

    /// Whether air of density `rho` (kg/m³) met at `speed` (m/s) needs live
    /// flight.
    pub fn exceeded(&self, rho: f64, speed: f64) -> bool {
        let q = 0.5 * rho * speed * speed;
        q >= self.dynamic_pressure || q * speed >= self.heating || rho >= self.density
    }

    /// Whether a live vessel may start coasting in that air: every value
    /// below half its threshold.
    pub fn clear(&self, rho: f64, speed: f64) -> bool {
        let q = 0.5 * rho * speed * speed;
        q < 0.5 * self.dynamic_pressure && q * speed < 0.5 * self.heating && rho < 0.5 * self.density
    }
}

impl Default for LiveBoundary {
    fn default() -> Self {
        LiveBoundary::DEFAULT
    }
}

/// A vessel's limbo placeholders, saved with it until craft data carries
/// them: skin optics and the live boundary. (Boil-off is a craft of parts'
/// tank law, `crate::systems::coast_law`.)
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LimboSpec {
    pub optics: Optics,
    pub boundary: LiveBoundary,
}

/// A segment's perturbation model.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Limbo {
    /// Whose surface table (rebuilt from the craft's files after a load).
    pub craft: CraftId,
    #[serde(skip)]
    design: DesignSlot,
    pub optics: Optics,
    /// The attitude within the segment.
    pub pointing: Pointing,
    pub dry_mass: f64,
    /// Open parachutes' drag area (m²), along the flow.
    #[serde(default)]
    pub chute_area: f64,
    pub boundary: LiveBoundary,
    /// A craft of parts' tanks and bottles through a coast (D119): its
    /// mass is the dry mass and their law's consumables.
    #[serde(default)]
    pub law: Option<Box<CoastLaw>>,
}

/// The air of one body at a vessel, read from the model up to its top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rarefied {
    pub body: NodeId,
    /// Height above the reference ellipsoid (m).
    pub altitude: f64,
    /// Where live flight stops seeing this air (m, [`Atmosphere::top`]).
    pub top: f64,
    pub rho: f64,
    pub mean_free_path: f64,
    /// The vessel's velocity relative to the rotating air (m/s).
    pub v_rel: DVec3,
}

/// The highest altitude the atmosphere model has air at (m): a table's
/// last row, or `top` for the exponential model.
pub fn ceiling(atm: &Atmosphere) -> f64 {
    atm.table.as_ref().and_then(|t| t.rows().last()).map_or(atm.top, |r| r.0.max(atm.top))
}

/// Calls `f` with the air of every body whose atmosphere model reaches the
/// vessel at `(r, v)` relative to `anchor` (in source order). Altitude by
/// the geocentric-latitude ellipsoid, as `BodyPhysical::altitude`.
pub fn each_air(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3, mut f: impl FnMut(Rarefied)) {
    for s in &world.sources {
        let Some(p) = s.physical.as_ref() else { continue };
        let Some(atm) = &p.atmosphere else { continue };
        let top = ceiling(atm);
        // A moon of a system farther than its extent plus the air's reach is
        // passed over without evaluating its position.
        if let crate::world::Role::Member(c) = s.role {
            let cl = &world.clusters[c];
            if (r - snap.relative_r(cl.node, anchor)).length() > cl.extent + p.radius_eq + top {
                continue;
            }
        }
        // The position first (the same bits as the full kinematics'), so a
        // far body's velocity is never evaluated.
        let at = if s.node == anchor { DVec3::ZERO } else { snap.relative_r(s.node, anchor) };
        let d = r - at;
        let dist = d.length();
        if dist - p.radius_eq >= top {
            continue;
        }
        let pole = p.rotation.pole(snap.t);
        let altitude = dist - p.ellipsoid_radius(d.dot(pole) / dist);
        if altitude >= top {
            continue;
        }
        let air = atm.profile(altitude, s.gm / (p.radius_eq * p.radius_eq));
        if air.rho.is_nan() || air.rho <= 0.0 {
            continue;
        }
        let body_v = if s.node == anchor { DVec3::ZERO } else { snap.relative(s.node, anchor).v };
        let v_air = body_v + (pole * p.rotation.w_rate).cross(d);
        f(Rarefied {
            body: s.node,
            altitude,
            top: atm.top,
            rho: air.rho,
            mean_free_path: air.mean_free_path,
            v_rel: v - v_air,
        });
    }
}

impl Limbo {
    /// A model for craft `craft` (its design in `design`), with the
    /// default optics and boundary.
    pub fn new(craft: CraftId, design: DesignSlot, pointing: Pointing, dry_mass: f64) -> Self {
        Limbo {
            craft,
            design,
            optics: Optics::default(),
            pointing,
            dry_mass,
            chute_area: 0.0,
            boundary: LiveBoundary::DEFAULT,
            law: None,
        }
    }

    /// The craft's ship (a craft of parts), if it has one.
    pub fn ship_def(&self) -> Option<&ShipDef> {
        self.design.get(&self.craft).ship.as_deref()
    }

    /// Each tank's heat for a span of the law starting at `t` at `(r, v)`
    /// relative to `anchor`, held by this pointing
    /// ([`super::coast_heat::law_conditions`]); `None` without a ship.
    pub fn law_heat(&self, world: &World, t: Epoch, state: (NodeId, DVec3, DVec3)) -> Option<Vec<HeatLaw>> {
        let def = self.ship_def()?;
        let (suns, air_k) = super::coast_heat::law_conditions(world, t, state, &self.pointing);
        Some((0..def.tanks.len()).map(|i| plumbing::heat_law(def, i, &suns, air_k)).collect())
    }

    /// The law of a craft of parts' `ship` from `t` on (local time 0 there)
    /// at `state`, held by this pointing; `None` without a ship.
    pub fn law_from(
        &self,
        world: &World,
        t: Epoch,
        state: (NodeId, DVec3, DVec3),
        ship: &ShipState,
    ) -> Option<CoastLaw> {
        let heat = self.law_heat(world, t, state)?;
        Some(CoastLaw::start(self.ship_def()?, ship, &heat))
    }

    /// Starts the law's next span at local time `t` (epoch `epoch`, the
    /// vessel at `state`) if one is due. Returns whether it did.
    pub fn next_span(&mut self, world: &World, (t, epoch): (f64, Epoch), state: (NodeId, DVec3, DVec3)) -> bool {
        if !self.law.as_ref().is_some_and(|l| l.due(t)) {
            return false;
        }
        let Some(heat) = self.law_heat(world, epoch, state) else { return false };
        let design = self.design.get(&self.craft).clone();
        let (Some(def), Some(law)) = (design.ship.as_deref(), self.law.as_mut()) else { return false };
        law.add_span(def, t, &heat);
        true
    }

    /// The design's surface table.
    pub fn surfaces(&self) -> &SurfaceTable {
        &self.design.get(&self.craft).surfaces
    }

    /// The same model with another pointing, its law as it is.
    pub fn with(&self, pointing: Pointing) -> Self {
        Limbo { pointing, ..self.clone() }
    }

    /// Mass at local time `t` of a segment starting at `mass0`: a craft of
    /// parts' law of its tanks, else held.
    pub fn mass(&self, mass0: f64, t: f64) -> f64 {
        if let Some(law) = self.law.as_deref() {
            if let Some(def) = self.ship_def() {
                return self.dry_mass + law.total(def, t);
            }
        }
        mass0
    }

    /// The non-gravitational acceleration (inertial, m/s²) of a vessel of
    /// `mass` at `(r, v)` relative to `anchor` at the snapshot's time.
    pub fn accel(&self, world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3, mass: f64) -> DVec3 {
        let table = self.surfaces();
        let att = self.pointing.attitude(world, snap, anchor, r, v);
        let mut a = DVec3::ZERO;
        each_air(world, snap, anchor, r, v, |air| {
            let speed = air.v_rel.length();
            if speed <= 0.0 {
                return;
            }
            let bridge = aero::bridge(air.mean_free_path / table.length);
            let area = match att {
                // The air travels along −v_rel relative to the craft.
                Some(att) => table.at(att.q.inverse() * (-air.v_rel / speed)).drag_area(bridge),
                None => table.mean.drag_area(bridge),
            } + self.chute_area;
            a -= air.v_rel * (0.5 * air.rho * speed * area / mass);
        });
        starlight(world, snap, anchor, r, |pressure, dir| {
            let force = match att {
                Some(att) => {
                    let s = att.q.inverse() * dir;
                    att.q * table.at(s).radiation(s, self.optics)
                }
                None => dir * table.mean.radiation(self.optics),
            };
            a += force * (pressure / mass);
        });
        a
    }

    /// Whether the air at `(r, v)` needs live flight: the boundary
    /// exceeded inside an atmosphere's top.
    pub fn live_air(&self, world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> bool {
        live_air(&self.boundary, world, snap, anchor, r, v)
    }
}

/// Whether the air at `(r, v)` (relative to `anchor`) needs live flight by
/// `boundary` (inside an atmosphere's top only).
pub fn live_air(boundary: &LiveBoundary, world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> bool {
    let mut live = false;
    each_air(world, snap, anchor, r, v, |air| {
        live |= air.altitude < air.top && boundary.exceeded(air.rho, air.v_rel.length());
    });
    live
}

/// Whether a live vessel at `(r, v)` may start coasting: in every
/// atmosphere it is `margin` above the top, or the air is
/// [`LiveBoundary::clear`].
pub fn may_coast(
    boundary: &LiveBoundary,
    world: &World,
    snap: &Snapshot,
    anchor: NodeId,
    (r, v): (DVec3, DVec3),
    margin: f64,
) -> bool {
    let mut ok = true;
    each_air(world, snap, anchor, r, v, |air| {
        ok &= air.altitude >= air.top + margin || boundary.clear(air.rho, air.v_rel.length());
    });
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ephem::Ephemeris;
    use crate::sol;
    use std::sync::Arc;

    fn world() -> World {
        let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
        World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
    }

    /// Where the boundary falls on Earth: (speed, the altitude band it is
    /// first exceeded in), and the hysteresis band below it.
    #[test]
    fn the_live_boundary_on_earth() {
        let w = world();
        let earth = w.find("Earth").unwrap();
        let p = earth.physical.as_ref().unwrap();
        let atm = p.atmosphere.as_ref().unwrap();
        let b = LiveBoundary::DEFAULT;
        let g0 = earth.gm / (p.radius_eq * p.radius_eq);
        let first = |speed: f64| {
            let mut h = 200_000.0;
            while !b.exceeded(atm.profile(h, g0).rho, speed) {
                h -= 100.0;
            }
            h
        };
        // (speed m/s, lowest and highest altitude km)
        let cases = [(7_800.0, 135.0, 150.0), (11_000.0, 145.0, 165.0), (2_000.0, 104.0, 118.0), (300.0, 100.0, 112.0)];
        for (speed, lo, hi) in cases {
            let h = first(speed) / 1e3;
            println!("{speed} m/s: live below {h:.1} km");
            assert!(h > lo && h < hi, "{speed} m/s: {h} km");
            // Hysteresis: a live vessel there may not coast until higher.
            let rho = atm.profile(first(speed), g0).rho;
            assert!(!b.clear(rho, speed) && b.clear(0.4 * rho, speed));
        }
        assert_eq!(ceiling(atm), 1_000_000.0);
    }
}
