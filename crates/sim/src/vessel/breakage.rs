//! Breakage triggers (D065, D066, D083; `docs/features/parts.md`):
//! placeholder limits judged at the end of every live tick.
//!
//! * **Temperature**: a part with a skin cell above its material's limit or
//!   an interior node above the part's limit is **destroyed** (D065: "the
//!   part is destroyed cleanly"): it leaves as a wreck of its own, and what
//!   was attached through it comes apart into its connected groups.
//! * **Impact** (D066, judged per contact point): the part of the point
//!   that hit too fast is destroyed.
//! * **Load**: a part whose cells carry more than its `max_load` is **torn
//!   off** its parent, intact, with what hangs from it (a root part has
//!   none, and stays). Its load is the
//!   Newtonian pressure of the tick's flow on its exposed cells
//!   (q·Cp,max·e·A·cos²θ, the most the flow pushes on them) plus the ground's
//!   normal force at its contact points (the same spring-dampers as the
//!   contact).
//!
//! A craft of one part (the test craft) keeps the whole-craft rule: over a
//! temperature limit or hit too fast, the vessel is destroyed and flies on as
//! a wreck. Debug mode (D064) disables every trigger. A breakup is applied by
//! [`Vessel::split`] (the vessel stops at the tick where it happened until
//! then; `sim::vessel::split`).

use super::split::Breakup;
use super::{Destruction, Phase, Vessel};
use crate::aero::{air, Flow};
use crate::contact::{self, Ground, Pose};
use crate::frame::NodeId;
use crate::thermal::Overheat;
use crate::world::World;
use glam::DVec3;

impl Vessel {
    /// After a live tick with the tick's `flow` (body axes, if in air) and
    /// the ground `contact` (anchor, body) of its substeps: the parts over a
    /// limit break (a one-part craft overheats whole, D065).
    pub(super) fn check_limits(&mut self, world: &World, flow: Option<&Flow>, contact: Option<(NodeId, NodeId)>) {
        if self.debug || self.destroyed.is_some() {
            return;
        }
        let design = self.craft.design().clone();
        if design.parts.len() <= 1 {
            self.check_overheat();
            return;
        }
        let mut b = self.breakup.take().unwrap_or(Breakup { epoch: self.time, destroyed: vec![], torn: vec![] });
        b.epoch = self.time;
        let st = &self.thermal.state;
        let hot = |part: u32, at: Overheat, temperature: f64, b: &mut Breakup| {
            if !b.destroyed.iter().any(|d| d.0 == part) {
                b.destroyed.push((part, Destruction::Overheat { at, temperature }));
            }
        };
        for (i, (&t, &max)) in st.skin.iter().zip(&design.skin_max).enumerate() {
            if t > max {
                hot(design.cell_part[i], Overheat::Cell(i as u32), t, &mut b);
            }
        }
        for (j, (&t, &max)) in st.nodes.iter().zip(&design.node_max).enumerate() {
            if t > max {
                hot(design.node_part[j], Overheat::Node(j as u32), t, &mut b);
            }
        }
        // A root part (no parent: the craft's, or a piece's) has nothing to
        // be torn from: it stays.
        let loads = self.part_loads(world, flow, contact);
        for (p, &load) in loads.iter().enumerate() {
            let p = p as u32;
            let part = &design.parts[p as usize];
            if part.parent.is_some() && load > part.max_load && !b.destroyed.iter().any(|d| d.0 == p) {
                b.torn.push((p, load));
            }
        }
        b.destroyed.sort_by_key(|d| d.0);
        if !(b.destroyed.is_empty() && b.torn.is_empty()) {
            self.breakup = Some(b);
        }
    }

    /// A contact point closed too fast during a tick (D066): a one-part
    /// craft is destroyed; otherwise the point's part is, at the tick's end.
    pub(super) fn impacted(&mut self, im: contact::Impact) {
        let cause = Destruction::Impact { speed: im.speed, point: im.point };
        let design = self.craft.design().clone();
        if design.parts.len() <= 1 {
            self.destroy(cause);
            return;
        }
        let part = design.cell_part[self.craft.contacts[im.point as usize].cell as usize];
        let b = self.breakup.get_or_insert_with(|| Breakup { epoch: self.time, destroyed: vec![], torn: vec![] });
        if !b.destroyed.iter().any(|d| d.0 == part) {
            b.destroyed.push((part, cause));
        }
    }

    /// Each part's load (N): the flow's pressure on its exposed cells and
    /// the ground's normal force at its contact points, at the vessel's state.
    pub fn part_loads(&self, world: &World, flow: Option<&Flow>, contact: Option<(NodeId, NodeId)>) -> Vec<f64> {
        let design = self.craft.design();
        let mut force = vec![DVec3::ZERO; design.parts.len()];
        if let Some(f) = flow.filter(|f| f.q > 0.0 && f.dir.length_squared() > 0.0) {
            let d = f.dir.normalize();
            let cp = air::cp_max(f.gamma, f.mach);
            let exposure = design.bake.exposure_weights(d);
            for (i, g) in design.bake.geometry.iter().enumerate() {
                let cos = -g.normal.dot(d);
                if cos > 0.0 {
                    let p = design.cell_part[i] as usize;
                    force[p] -= g.normal * (f.q * cp * exposure.at(i) * g.area * cos * cos);
                }
            }
        }
        let mut load: Vec<f64> = force.iter().map(|f| f.length()).collect();
        if let (Some((anchor, body)), Phase::Powered { r, v, .. }) = (contact, &self.phase) {
            let ground = Ground::new(world, &world.snapshot(self.time), anchor, body);
            let props = self.mass_props();
            let (q, omega) = (self.attitude.q, self.attitude.omega);
            let pose = Pose { r: *r, v: *v, q, omega, com: props.com, mass: props.mass, inertia: props.inertia };
            let dt = super::TICK / contact::SUBSTEPS as f64;
            let normal = contact::normal_loads(&ground, &pose, &self.craft.contacts, &self.craft.contact, dt);
            for (cp, f) in self.craft.contacts.iter().zip(normal) {
                load[design.cell_part[cp.cell as usize] as usize] += f;
            }
        }
        load
    }
}
