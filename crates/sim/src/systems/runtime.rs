//! The ship systems' own state (D104, D105; ship-model.md §5): what the
//! behaviour of a craft of parts needs beyond the plain ship state of
//! `craft::state` — each joint's load, the orders waiting for their epoch, the
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
    /// The factor the load as it stands (every force on the craft, and so
    /// its proper acceleration, scaled together) could grow by before the
    /// joint reaches its ultimate load (`structure::ultimate_scale`; `None`:
    /// no capacity data, no acceleration, or over 100 g).
    #[serde(default)]
    pub to_ultimate: Option<f64>,
}

/// What an order does when its epoch comes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OrderKind {
    /// A ship command (as from the systems screen or a function).
    Command(ShipCommand),
    /// The next stage of the sequence (Space).
    Stage,
    /// Stage `k` of the sequence again, one already run (D114): what can
    /// act does, the rest does nothing; the sequence stays where it is.
    Again(u32),
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
    pub joints: Vec<JointRun>,
    /// The loads of joints a split cut, with their child's piece
    /// (`ShipState::severed`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub severed: Vec<JointRun>,
    /// Orders not yet applied, in the order given (applied by epoch, ties
    /// in that order).
    pub orders: Vec<Order>,
    /// The sequence's clamp release is armed: it lets go at the first tick
    /// every ignited engine delivers its rated thrust, and aborts the start
    /// if one fails (the real pad's rule, falcon9.md §1.2, §4.2).
    pub release_armed: bool,
    /// The engines (indices into `ShipDef::engines`) the armed release
    /// waits for: those lit when it was armed, or at the end of that tick
    /// (with none lit then, nor one settling to start, it is disarmed).
    #[serde(default)]
    pub release_engines: Vec<u32>,
    /// The pilot's throttle at the last step: closing it (open to 0) shuts
    /// the engines down (D114).
    #[serde(default)]
    pub throttle_was: f64,
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
    /// `ShipDef::surfaces`, or empty before any deflects).
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
    /// The loads without power now (their bus short: shed, or no source)
    /// and the buses as they stood (`systems::power`).
    #[serde(default)]
    pub unpowered: Vec<Lost>,
    #[serde(default)]
    pub buses: Vec<super::power::BusRun>,
    /// The crew's exposure to acceleration and their injuries
    /// (`systems::crew`).
    #[serde(default)]
    pub crew: super::crew::CrewRun,
}

impl Runtime {
    /// The systems' state of a ship as loaded: liquids settled, engines
    /// off, joints unloaded, nothing waiting.
    pub fn initial(def: &ShipDef) -> Runtime {
        Runtime {
            joints: (def.joints.iter())
                .map(|j| JointRun { parent: j.parent, child: j.child, ..JointRun::default() })
                .collect(),
            ..Runtime::default()
        }
    }

    /// The state of the piece holding the parts `kept` (`whole` and `piece`
    /// the definitions; `own`: the piece the vessel keeps), each field by
    /// its rule (`craft::partition`):
    /// * by part: the lost functions, unpowered loads, the
    ///   aero surfaces' deflections; the joints within the piece, and those
    ///   the split cuts with their child's piece (`severed`);
    /// * the bus readouts of the buses the piece keeps (a bus the split
    ///   cuts is read in both until each piece's next power step);
    /// * the crew's exposure with the cabin's part (else the vessel's piece);
    /// * the armed release and the engines it waits for with the clamps'
    ///   piece (a split disarms it first: `propulsion::disarm`);
    /// * the orders (sent to the vessel) and the pilot's last throttle with
    ///   the vessel's piece (the others start closed, as their controls do:
    ///   `Controls::for_piece`);
    /// * the engines' thrust direction with a piece that has engines, the
    ///   regulated flag with a piece that has tanks;
    /// * copies in every piece: the motion felt (one rigid body until now),
    ///   the fates' seed and launch, the slow systems' epoch, the event log
    ///   (the history they share).
    pub fn partition(&self, kept: &[u32], whole: &ShipDef, piece: &ShipDef, own: bool) -> Runtime {
        use crate::craft::partition::by;
        let Runtime {
            joints,
            severed,
            orders,
            release_armed,
            release_engines,
            throttle_was,
            accel,
            omega,
            alpha,
            thrust_dir,
            fins,
            seed,
            launch,
            stepped,
            events,
            pressure_regulated,
            lost,
            unpowered,
            buses,
            crew,
        } = self;
        let clamps = piece.joints.iter().any(|j| j.kind == crate::craft::assembly::JointKind::HoldDown);
        let cabin = whole.seats.first().map_or(own, |s| kept.contains(&s.part));
        let within = |j: &JointRun| kept.contains(&j.parent) && kept.contains(&j.child);
        let cut = |j: &JointRun| !kept.contains(&j.parent) && kept.contains(&j.child);
        Runtime {
            joints: joints.iter().filter(|j| within(j)).cloned().collect(),
            severed: (severed.iter().filter(|j| kept.contains(&j.child)))
                .chain(joints.iter().filter(|j| cut(j)))
                .cloned()
                .collect(),
            orders: if own { orders.clone() } else { Vec::new() },
            release_armed: *release_armed && clamps,
            release_engines: if clamps {
                renumber(release_engines, &whole.engines, &piece.engines)
            } else {
                Vec::new()
            },
            throttle_was: if own { *throttle_was } else { 0.0 },
            accel: *accel,
            omega: *omega,
            alpha: *alpha,
            thrust_dir: thrust_dir.filter(|_| !piece.engines.is_empty()),
            fins: (fins.iter().zip(&whole.surfaces)).filter(|(_, s)| kept.contains(&s.part)).map(|(&d, _)| d).collect(),
            seed: *seed,
            launch: *launch,
            stepped: *stepped,
            events: events.clone(),
            pressure_regulated: *pressure_regulated && !piece.tanks.is_empty(),
            lost: by(lost, |l| l.part, kept),
            unpowered: by(unpowered, |l| l.part, kept),
            buses: buses.iter().filter(|r| piece.buses.iter().any(|b| b.name == r.name)).cloned().collect(),
            crew: if cabin { crew.clone() } else { Default::default() },
        }
    }

    /// The inverse of [`Self::partition`] (`ShipState::merge`): `a` the
    /// vessel's piece, whose orders, last throttle, motion, seed, launch and
    /// slow epoch the whole keeps; the per-part lists back in the whole's
    /// order; a joint neither piece has starts unloaded; the events of both
    /// in epoch order (the history they share once); the crew's exposure of
    /// the piece with the cabin (else `a`'s).
    pub fn merge(
        a: &Runtime,
        a_def: &ShipDef,
        b: &Runtime,
        b_def: &ShipDef,
        whole: &ShipDef,
    ) -> Result<Runtime, String> {
        use crate::craft::partition::keyed;
        let a_parts: Vec<u32> = a_def.parts.iter().map(|p| p.part).collect();
        let in_a = |p: u32| a_parts.contains(&p);
        let Runtime {
            joints: _,
            severed: _,
            orders,
            release_armed,
            release_engines: _,
            throttle_was,
            accel,
            omega,
            alpha,
            thrust_dir,
            fins,
            seed,
            launch,
            stepped,
            events,
            pressure_regulated,
            lost,
            unpowered,
            buses,
            crew,
        } = a;
        let mut severed: Vec<JointRun> = a.severed.iter().chain(&b.severed).cloned().collect();
        let (mut ja, mut jb) = (a.joints.iter(), b.joints.iter());
        let mut out_joints = Vec::new();
        for d in &whole.joints {
            let j = match (in_a(d.parent), in_a(d.child)) {
                (true, true) => ja.next().cloned(),
                (false, false) => jb.next().cloned(),
                _ => None,
            };
            let j = j.or_else(|| {
                let i = severed.iter().position(|s| s.parent == d.parent && s.child == d.child)?;
                Some(severed.remove(i))
            });
            out_joints.push(j.unwrap_or(JointRun { parent: d.parent, child: d.child, ..JointRun::default() }));
        }
        // Each piece's surfaces' deflections, by id (none: never deflected).
        let deflection = |s: &crate::craft::ship::Owned<crate::catalog::AeroSurface>| {
            let (run, def) = if in_a(s.part) { (fins, a_def) } else { (&b.fins, b_def) };
            def.surfaces.iter().position(|x| x.id == s.id).and_then(|i| run.get(i).copied()).unwrap_or(0.0)
        };
        let any_fins = !fins.is_empty() || !b.fins.is_empty();
        let mut log: Vec<Event> = events.clone();
        log.extend(b.events.iter().filter(|e| !events.contains(e)).cloned());
        log.sort_by(|x, y| x.at.partial_cmp(&y.at).unwrap_or(std::cmp::Ordering::Equal));
        let drop = log.len().saturating_sub(EVENTS_KEPT);
        log.drain(..drop);
        let waits = |r: &Runtime, d: &ShipDef| renumber(&r.release_engines, &d.engines, &whole.engines);
        let b_has_cabin = !b_def.seats.is_empty() && a_def.seats.is_empty();
        Ok(Runtime {
            joints: out_joints,
            severed,
            orders: orders.clone(),
            release_armed: *release_armed || b.release_armed,
            release_engines: if b.release_armed && !release_armed { waits(b, b_def) } else { waits(a, a_def) },
            throttle_was: *throttle_was,
            accel: *accel,
            omega: *omega,
            alpha: *alpha,
            thrust_dir: thrust_dir.or(b.thrust_dir),
            fins: if any_fins { whole.surfaces.iter().map(deflection).collect() } else { Vec::new() },
            seed: *seed,
            launch: *launch,
            stepped: *stepped,
            events: log,
            pressure_regulated: *pressure_regulated || b.pressure_regulated,
            lost: keyed(lost, &b.lost, |l| l.part),
            unpowered: keyed(unpowered, &b.unpowered, |l| l.part),
            buses: (whole.buses.iter())
                .filter_map(|w| buses.iter().chain(&b.buses).find(|r| r.name == w.name).cloned())
                .collect(),
            crew: if b_has_cabin { b.crew.clone() } else { crew.clone() },
        })
    }

    /// Whether the per-item lists match the definition.
    pub fn fits(&self, def: &ShipDef) -> bool {
        self.joints.len() == def.joints.len()
    }

    /// Builds the per-item lists as loaded for a state saved before the
    /// systems ran (none of them yet). Lists that disagree with the
    /// definition otherwise are a bug (a split or merge without its rule):
    /// a debug assertion, and in a release build the lists rebuilt with an
    /// alarm in the log rather than state stepped against the wrong items.
    pub fn refit(&mut self, def: &ShipDef) {
        if self.fits(def) {
            return;
        }
        let never_ran = self.joints.is_empty();
        debug_assert!(never_ran, "the systems' lists disagree with the definition {}", def.name);
        let fresh = Runtime::initial(def);
        self.joints = fresh.joints;
        if !never_ran {
            let at = self.stepped.or(self.launch).unwrap_or(Epoch::new(0, 0.0));
            self.event(at, "Systems: their state disagreed with the craft and was reset (a bug)", true);
        }
    }

    /// Whether `owner`'s function is out: lost to a failure, or without
    /// power (`systems::power`).
    pub fn out(&self, owner: &str) -> bool {
        self.lost.iter().chain(&self.unpowered).any(|l| l.owner == owner)
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

/// Indices into the engines `from` of the engines `list`, as indices into
/// `to` (those `to` lacks dropped), by engine id.
fn renumber(list: &[u32], from: &[crate::craft::ship::EngineDef], to: &[crate::craft::ship::EngineDef]) -> Vec<u32> {
    (list.iter())
        .filter_map(|&i| from.get(i as usize))
        .filter_map(|e| to.iter().position(|t| t.id == e.id).map(|k| k as u32))
        .collect()
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

    /// Each piece of a split keeps its own aero surfaces' deflections,
    /// aligned with its own definition's surfaces (review, 2026-10-27: they
    /// were copied whole, so a piece's fin read another's); never deflected,
    /// none.
    #[test]
    fn a_split_partitions_the_fin_deflections() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mut whole = Runtime::initial(&def);
        assert!(whole.partition(&[], &def, &def.subset(&[]), true).fins.is_empty(), "never deflected");
        whole.fins = (0..def.surfaces.len()).map(|i| 0.01 * (i + 1) as f64).collect();
        let booster: Vec<u32> = (def.surfaces.iter()).filter(|s| s.id.starts_with("s1.")).map(|s| s.part).collect();
        let rest: Vec<u32> = (def.surfaces.iter()).filter(|s| !s.id.starts_with("s1.")).map(|s| s.part).collect();
        assert!(!booster.is_empty() && !rest.is_empty(), "the grid fins and Dragon's");
        for kept in [booster, rest] {
            let piece = def.subset(&kept);
            let fins = whole.partition(&kept, &def, &piece, true).fins;
            assert_eq!(fins.len(), piece.surfaces.len());
            for (s, d) in piece.surfaces.iter().zip(&fins) {
                let i = def.surfaces.iter().position(|w| w.id == s.id).unwrap();
                assert_eq!(*d, whole.fins[i], "{}", s.id);
            }
        }
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
