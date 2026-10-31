//! New vessels: standing on a surface or flying from a state. A craft of
//! catalog parts starts with its ship state at load (tanks filled at their
//! loading temperatures, bottles at their pressure: `crate::craft::state`).

use super::aerothermal::{self, VesselThermal};
use super::attitude::{quat_z_to, Attitude, AttitudeControl};
use super::live::Checked;
use super::{limbo, Phase, ShipClock, Vessel, VesselId};
use crate::craft::Craft;
use crate::frame::NodeId;
use crate::math;
use crate::time::Epoch;
use crate::world::World;
use glam::{DQuat, DVec3};

impl Vessel {
    /// A vessel of `craft` at `t` in `phase` with its attitude, everything
    /// else at its start (systems' lattice from `t`, temperatures uniform).
    fn new(id: VesselId, t: Epoch, craft: &Craft, phase: Phase, attitude: Attitude) -> Self {
        let crew = craft.spec.crew;
        let (cells, nodes) = (craft.design().cells.len(), craft.design().nodes());
        let params = craft.params();
        let mut vessel = Vessel {
            id,
            craft: params,
            phase,
            time: t,
            attitude,
            chute: Default::default(),
            crew,
            debug: false,
            tick_accel: None,
            control: AttitudeControl::default(),
            rest_ticks: 0,
            destroyed: None,
            rest_mode: None,
            checked: Checked(true),
            clock: ShipClock::at(t, 0.0),
            thermal: VesselThermal::uniform(cells, nodes, aerothermal::INITIAL_K, t),
            limbo: Default::default(),
            repoint: false,
            systems: crate::systems::Systems::starting(t),
            breakup: None,
            bubble: Default::default(),
            ship: None,
            clear_of: Vec::new(),
            helm: Default::default(),
            launch: None,
            nodes: Vec::new(),
            program: None,
            software: Default::default(),
        };
        vessel.init_ship();
        vessel
    }

    /// A vessel standing upright on a body's surface at latitude/longitude (deg).
    pub fn landed_at(
        world: &World,
        id: VesselId,
        body: &str,
        lat_deg: f64,
        lon_deg: f64,
        t: Epoch,
        craft: &Craft,
    ) -> Self {
        let src = world.find(body).expect("known body");
        let p = src.physical.as_ref().expect("physical body");
        let deg = math::PI / 180.0;
        let placeholder = Phase::Powered { anchor: src.node, r: DVec3::ZERO, v: DVec3::ZERO };
        let mut v = Vessel::new(id, t, craft, placeholder, Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO });
        let fixed = p.ground_point(lat_deg * deg, lon_deg * deg, v.contact_height());
        let att_fixed = quat_z_to(fixed.raw().normalize());
        v.phase = Phase::Landed { body: src.node, fixed, att_fixed };
        v.sync_landed_attitude(world);
        if !v.holds_where_landed(world) {
            v.wake(world);
        }
        v
    }

    /// A vessel coasting from `(r, v)` relative to `anchor` at `t`.
    pub fn coasting(world: &World, id: VesselId, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3, craft: &Craft) -> Self {
        let phase = Phase::Powered { anchor, r, v };
        let mut vessel = Vessel::new(id, t, craft, phase, Attitude { q: quat_z_to(r.normalize()), omega: DVec3::ZERO });
        // In air that needs it, it is flown live (the live boundary).
        let boundary = vessel.limbo.boundary;
        if limbo::may_coast(&boundary, world, &world.snapshot(t), anchor, (r, v), aerothermal::LEAVE_ATMOSPHERE) {
            vessel.start_coast(world);
        }
        vessel
    }
}
