//! The ship systems' own state (D104, D105; ship-model.md §5): what the
//! behaviour of a craft of parts needs beyond the plain ship state of
//! `craft::state` — each tank's settling, each engine's delivered thrust
//! level, each joint's load, the orders waiting for their epoch, the
//! clamps' armed release, the last tick's motion as the systems felt it,
//! the failure seed and the event log. Saved inside the ship state
//! (`ShipState::runtime`); every per-item list is aligned with its
//! `ShipDef` list and names its part, so a split partitions it like the
//! rest ([`Runtime::partition`]).

use crate::craft::command::ShipCommand;
use crate::craft::ship::ShipDef;
use crate::time::Epoch;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// A tank's liquid as the systems track it beyond its mass and temperature.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TankRun {
    pub part: u32,
    /// How far the liquid's bulk has moved away from its outlet along the
    /// tank's axis (m; 0: settled on the outlet), and its speed (m/s).
    pub drift: f64,
    pub drift_speed: f64,
    /// Pressurant moles let in and gas vented (kg) so far (the ledgers).
    #[serde(default)]
    pub pressurant_in: f64,
    #[serde(default)]
    pub vented: f64,
}

/// An engine's output as delivered (the transients' state).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineRun {
    pub part: u32,
    /// Thrust level now, as a fraction of full flow (0 off; ramps up in a
    /// start, decays in the tail-off).
    pub level: f64,
    /// The level when the shutdown began (the tail-off decays from it).
    pub cutoff: f64,
    /// Why it stopped or failed last, for the player.
    #[serde(default)]
    pub reason: Option<String>,
    /// Waiting to start while its propellant settles (seconds waited): its
    /// start sequence fires the thrusters that push along its thrust.
    #[serde(default)]
    pub settling: Option<f64>,
}

/// A joint's load at the last live tick.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JointRun {
    pub parent: u32,
    pub child: u32,
    /// Force the parent side exerts on the child side (body axes, N) and
    /// the moment about the joint's centre (N·m).
    pub force: DVec3,
    pub moment: DVec3,
    /// The equivalent line load over the allowable at temperature (1 is the
    /// ultimate load).
    pub ratio: f64,
}

/// What an order does when its epoch comes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OrderKind {
    /// A ship command (as from the systems screen or a function).
    Command(ShipCommand),
    /// The next stage of the sequence (Space).
    Stage,
    /// A saved function by name.
    Function(String),
}

/// An order waiting for its epoch: commands from aboard arrive at once,
/// from mission control after the light time (D067).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub at: Epoch,
    pub kind: OrderKind,
}

/// Something that happened to the ship, for the player (newest last).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub at: Epoch,
    pub text: String,
    /// Bad news (a failure, an abort).
    pub alarm: bool,
}

/// A function lost to a failure (D105 §6.4): a computer, a sensor, a pump,
/// a mechanism's actuator (`sim::systems::failure`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lost {
    pub part: u32,
    /// The part's or component's id.
    pub owner: String,
    pub why: String,
}

/// How many events are kept.
pub const EVENTS_KEPT: usize = 64;

/// The ship systems' state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Runtime {
    pub tanks: Vec<TankRun>,
    pub engines: Vec<EngineRun>,
    pub joints: Vec<JointRun>,
    /// Orders not yet applied, in the order given (applied by epoch, ties
    /// in that order).
    pub orders: Vec<Order>,
    /// The sequence's clamp release is armed: it lets go at the first tick
    /// every ignited engine delivers its rated thrust, and aborts the start
    /// if one fails (the real pad's rule, falcon9.md §1.2, §4.2).
    pub release_armed: bool,
    /// The engines (indices into `ShipDef::engines`) the armed release
    /// waits for: those lit when it was armed (or first lit after).
    #[serde(default)]
    pub release_engines: Vec<u32>,
    /// The last tick's proper acceleration at the centre of mass, angular
    /// velocity and angular acceleration (body axes): what the liquids,
    /// joints and accelerometers felt.
    pub accel: DVec3,
    pub omega: DVec3,
    pub alpha: DVec3,
    /// The engines' realised thrust direction after the gimbal (body axes;
    /// `None` until they first run): the actuators move it at their rate.
    pub thrust_dir: Option<DVec3>,
    /// Each aero surface's deflection (rad; aligned with
    /// `ShipDef::surfaces`, kept by every piece: a fin is on one).
    #[serde(default)]
    pub fins: Vec<f64>,
    /// The failure fates' seed (the save's) and the launch epoch they are
    /// drawn with (D105 §6.3).
    pub seed: u64,
    pub launch: Option<Epoch>,
    /// Where the slow systems (the plumbing's heat, relief and boil-off)
    /// last stepped to.
    #[serde(default)]
    pub stepped: Option<Epoch>,
    pub events: Vec<Event>,
    /// Whether the plumbing regulates the tanks' pressures (its regulator
    /// sets it when it runs). Until it does, the structure takes each tank's
    /// flight (loading) pressure as its stabilising pressure at the least: a
    /// pressure that falls only because nothing re-pressurises the tank is
    /// not one to break a joint on (D105).
    #[serde(default)]
    pub pressure_regulated: bool,
    /// The functions lost to failures (by part, as the rest).
    #[serde(default)]
    pub lost: Vec<Lost>,
}

impl Runtime {
    /// The systems' state of a ship as loaded: liquids settled, engines
    /// off, joints unloaded, nothing waiting.
    pub fn initial(def: &ShipDef) -> Runtime {
        Runtime {
            tanks: def.tanks.iter().map(|t| TankRun { part: t.part, ..TankRun::default() }).collect(),
            engines: def.engines.iter().map(|e| EngineRun { part: e.part, ..EngineRun::default() }).collect(),
            joints: (def.joints.iter())
                .map(|j| JointRun { parent: j.parent, child: j.child, ..JointRun::default() })
                .collect(),
            ..Runtime::default()
        }
    }

    /// The state of the piece holding the parts `kept`: each item of those
    /// parts, joints within the piece; the orders, the motion, the seed and
    /// the events go with every piece (an order naming what a piece lacks
    /// does nothing there).
    pub fn partition(&self, kept: &[u32]) -> Runtime {
        fn by<T: Clone>(items: &[T], part: impl Fn(&T) -> u32, kept: &[u32]) -> Vec<T> {
            items.iter().filter(|x| kept.contains(&part(x))).cloned().collect()
        }
        Runtime {
            tanks: by(&self.tanks, |t| t.part, kept),
            engines: by(&self.engines, |e| e.part, kept),
            joints: by(&self.joints, |j| if kept.contains(&j.parent) { j.child } else { u32::MAX }, kept),
            lost: by(&self.lost, |l| l.part, kept),
            release_armed: false,
            release_engines: Vec::new(),
            ..self.clone()
        }
    }

    /// Whether the lists match the definition (a state saved before the
    /// systems ran has none): if not, they are rebuilt as loaded.
    pub fn fits(&self, def: &ShipDef) -> bool {
        self.tanks.len() == def.tanks.len()
            && self.engines.len() == def.engines.len()
            && self.joints.len() == def.joints.len()
    }

    /// Rebuilds the per-item lists as loaded if they do not match the
    /// definition (a ship state saved before the systems ran), keeping the
    /// rest.
    pub fn refit(&mut self, def: &ShipDef) {
        if !self.fits(def) {
            let fresh = Runtime::initial(def);
            (self.tanks, self.engines, self.joints) = (fresh.tanks, fresh.engines, fresh.joints);
        }
    }

    /// Records an event (the oldest dropped beyond [`EVENTS_KEPT`]).
    pub fn event(&mut self, at: Epoch, text: impl Into<String>, alarm: bool) {
        self.events.push(Event { at, text: text.into(), alarm });
        if self.events.len() > EVENTS_KEPT {
            self.events.remove(0);
        }
    }

    /// Queues an order (kept in epoch order; equal epochs in arrival order).
    pub fn order(&mut self, at: Epoch, kind: OrderKind) {
        let i = self.orders.iter().position(|o| o.at > at).unwrap_or(self.orders.len());
        self.orders.insert(i, Order { at, kind });
    }

    /// The epoch of the next order, if any.
    pub fn next_order(&self) -> Option<Epoch> {
        self.orders.first().map(|o| o.at)
    }

    /// Removes and returns the orders due at `now` (epoch at or before it),
    /// in order.
    pub fn due(&mut self, now: Epoch) -> Vec<Order> {
        let n = self.orders.iter().take_while(|o| o.at <= now).count();
        self.orders.drain(..n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_come_due_in_epoch_order_ties_as_given() {
        let t0 = Epoch::new(1_000_000_000, 0.0);
        let mut r = Runtime::default();
        let stage = |n: &str| OrderKind::Function(n.into());
        r.order(t0.add_seconds(2.0), stage("c"));
        r.order(t0.add_seconds(1.0), stage("a"));
        r.order(t0.add_seconds(2.0), stage("d"));
        r.order(t0.add_seconds(1.0), stage("b"));
        assert_eq!(r.next_order(), Some(t0.add_seconds(1.0)));
        assert!(r.due(t0.add_seconds(0.5)).is_empty());
        let names = |o: Vec<Order>| {
            o.into_iter()
                .map(|o| match o.kind {
                    OrderKind::Function(n) => n,
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(names(r.due(t0.add_seconds(1.0))), ["a", "b"]);
        assert_eq!(names(r.due(t0.add_seconds(9.0))), ["c", "d"]);
        assert_eq!(r.next_order(), None);
    }

    #[test]
    fn events_keep_the_newest() {
        let t0 = Epoch::new(1_000_000_000, 0.0);
        let mut r = Runtime::default();
        for k in 0..100 {
            r.event(t0.add_seconds(k as f64), format!("{k}"), false);
        }
        assert_eq!(r.events.len(), EVENTS_KEPT);
        assert_eq!(r.events.last().unwrap().text, "99");
        assert_eq!(r.events[0].text, format!("{}", 100 - EVENTS_KEPT));
    }
}
