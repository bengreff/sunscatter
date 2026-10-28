//! What a craft of parts' joints see over a tick (ship-model.md §5.6),
//! gathered by the vessel for `systems::structure`: its parts (ids,
//! parents, dry mass properties, temperatures) and the forces other than
//! gravity on them — each engine's thrust at its mount (from the state at
//! the tick's start, as the tick flew them), the air on each part
//! (`structure::aero_pushes`) and the parachute on its part, the ground at
//! each contact point at the tick's end, the pad's clamps while held.

use super::aerothermal::AeroLoads;
use super::Vessel;
use crate::contact::{self, Ground, Pose};
use crate::craft::engine::ShipThrust;
use crate::craft::MassProps;
use crate::frame::NodeId;
use crate::systems::structure::{self, Bodies, Push};
use crate::world::World;
use glam::DVec3;

/// The parts and forces of a tick (owned; [`Self::bodies`] borrows them).
pub(super) struct Gathered {
    ids: Vec<u32>,
    parents: Vec<Option<u32>>,
    dry: Vec<MassProps>,
    kelvin: Vec<f64>,
    pushes: Vec<Push>,
    ambient: f64,
    hottest: Vec<f64>,
    q: f64,
    mach: f64,
}

impl Gathered {
    /// More forces on the parts (the grid fins').
    pub fn push(&mut self, more: Vec<Push>) {
        self.pushes.extend(more);
    }

    pub fn bodies(&self) -> Bodies<'_> {
        Bodies {
            ids: &self.ids,
            parents: &self.parents,
            dry: &self.dry,
            kelvin: &self.kelvin,
            pushes: &self.pushes,
            ambient: self.ambient,
            hottest: &self.hottest,
            q: self.q,
            mach: self.mach,
        }
    }
}

/// The ground under a live tick's end, for its contact forces.
pub(super) struct GroundAt {
    pub anchor: NodeId,
    pub body: NodeId,
    pub r: DVec3,
    pub v: DVec3,
}

impl Vessel {
    /// Gives a craft of parts the save's seed for its failure fates (D105
    /// §6.3; a new game's, set on each vessel it creates): its fates are
    /// drawn again from it. Nothing for a craft of one part.
    pub fn seed_fates(&mut self, seed: u64) {
        if let Some(state) = self.ship.as_mut() {
            crate::systems::failure::reseed(state, seed);
        }
    }

    /// Each part's temperature now (K): the mean of its skin cells
    /// (`structure::part_kelvin`), by part id (`Part::id`).
    pub fn part_kelvin(&self) -> Vec<(u32, f64)> {
        let design = self.craft.design();
        let kelvin = structure::part_kelvin(&design.cell_part, &self.thermal.state.skin, design.parts.len(), 295.0);
        design.parts.iter().map(|p| p.id).zip(kelvin).collect()
    }

    /// The parts with the engines' thrust `out` along `dir` (body axes; the
    /// runtime's realised direction if `None`) and the ambient pressure.
    fn gather(&self, out: &ShipThrust, dir: Option<DVec3>, ambient: f64) -> Gathered {
        let design = self.craft.design();
        let ids: Vec<u32> = design.parts.iter().map(|p| p.id).collect();
        let st = &self.thermal.state;
        let kelvin = structure::part_kelvin(&design.cell_part, &st.skin, ids.len(), 295.0);
        let hottest = structure::part_hottest(&design.node_part, &st.nodes, ids.len(), 295.0);
        let mut pushes = Vec::new();
        if let Some((def, state)) = self.ship_parts() {
            let index = |id: u32| ids.iter().position(|&x| x == id);
            pushes = structure::engine_pushes(def, &out.per_engine, dir.or(state.runtime.thrust_dir), index);
        }
        Gathered {
            parents: design.parts.iter().map(|p| p.parent).collect(),
            dry: design.part_mass.clone(),
            ids,
            kelvin,
            pushes,
            ambient,
            hottest,
            q: 0.0,
            mach: 0.0,
        }
    }

    /// A live tick's (see the module): `out` the engines' output it flew
    /// along `dir`, `air` its loads, `ground` the contact at its end.
    pub(super) fn gather_live(
        &self,
        world: &World,
        (out, dir): (&ShipThrust, DVec3),
        air: Option<&AeroLoads>,
        ground: Option<&GroundAt>,
        ambient: f64,
    ) -> Gathered {
        let mut g = self.gather(out, Some(dir), ambient);
        let design = self.craft.design();
        let n = g.ids.len();
        if let Some(a) = air {
            (g.q, g.mach) = (a.flow.q, a.flow.mach);
            let exposure = design.bake.exposure_weights(a.flow.dir);
            let cells = &design.bake.geometry;
            let cp_max = crate::aero::air::cp_max(a.flow.gamma, a.flow.mach);
            let axis = design.bake.sections.axis;
            let air = (a.flow.dir, a.flow.q, cp_max);
            let shares =
                structure::aero_pushes(&design.cell_part, cells, |i| exposure.at(i), air, axis, (a.force, a.moment), n);
            g.pushes.extend(shares);
            let chute = design.parts.iter().position(|p| p.chute);
            if let (Some((force, mount)), Some(part)) = (a.chute, chute) {
                g.pushes.push(Push { part, force, at: mount, torque: DVec3::ZERO });
            }
        }
        if let Some(at) = ground {
            let snap = world.snapshot(self.time);
            let ground = Ground::new(world, &snap, at.anchor, at.body);
            let props = self.mass_props();
            let (q, omega) = (self.attitude.q, self.attitude.omega);
            let pose = Pose { r: at.r, v: at.v, q, omega, com: props.com, mass: props.mass, inertia: props.inertia };
            let dt = super::TICK / contact::SUBSTEPS as f64;
            let points = &self.craft.contacts;
            let forces = contact::normal_forces(&ground, &pose, points, &self.craft.contact, dt);
            for (cp, f) in points.iter().zip(forces).filter(|(_, f)| *f != DVec3::ZERO) {
                let part = design.cell_part[cp.cell as usize] as usize;
                g.pushes.push(Push { part, force: q.inverse() * f, at: cp.pos, torque: DVec3::ZERO });
            }
        }
        g
    }

    /// The deployed grid fins' forces over a tick (`fins` with the `flow` at
    /// them, at the deflections they held: `systems::fins`, the one model),
    /// each at its fin on its part.
    pub(super) fn fin_pushes(
        &self,
        fins: &[crate::systems::fins::Fin],
        flow: Option<crate::systems::fins::FinFlow>,
    ) -> Vec<Push> {
        use crate::systems::fins;
        let (Some(flow), Some((def, state))) = (flow, self.ship_parts()) else { return Vec::new() };
        let design = self.craft.design();
        let held = fins::held(state, fins);
        (fins.iter().zip(held))
            .filter_map(|(f, d)| {
                let id = def.surfaces.get(f.surface)?.part;
                let part = design.parts.iter().position(|p| p.id == id)?;
                let (force, _) = fins::forces(std::slice::from_ref(f), &flow, DVec3::ZERO, &[d]);
                Some(Push { part, force, at: f.at, torque: DVec3::ZERO })
            })
            .collect()
    }

    /// A tick held on the pad's clamps: the engines' `out` and the clamps'
    /// force `on_vehicle` (body axes) at each intact hold-down joint's
    /// parent, shared between them.
    pub(super) fn gather_held(&self, out: &ShipThrust, on_vehicle: DVec3, ambient: f64) -> Gathered {
        let mut g = self.gather(out, None, ambient);
        if let Some((def, state)) = self.ship_parts() {
            let held: Vec<&crate::craft::ship::JointDef> = (def.joints.iter().zip(&state.joints))
                .filter(|(d, s)| d.kind == crate::craft::assembly::JointKind::HoldDown && s.intact)
                .map(|(d, _)| d)
                .collect();
            for d in &held {
                if let Some(part) = g.ids.iter().position(|&x| x == d.parent) {
                    let force = on_vehicle / held.len() as f64;
                    g.pushes.push(Push { part, force, at: d.pos, torque: DVec3::ZERO });
                }
            }
        }
        g
    }
}
