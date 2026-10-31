//! The ship state (D104, D105; `docs/technical/ship-model.md` §2, §5):
//! everything about a craft of catalog parts that changes in flight, saved
//! with the vessel as plain data — each tank's liquid and ullage, bottles,
//! engines, thruster blocks, valves, batteries, mechanisms, joints, each
//! part's integrity, damage and history, the cabin, failure fates, the pad's
//! clamps and the sequence's position.
//!
//! Every list is aligned with its [`ShipDef`] list and every entry names
//! its part (`Part::id`), so a split partitions the state exactly as the
//! definition, and a docking merges it, each field by its own rule
//! (`craft::partition`: [`ShipState::partition`], [`ShipState::merge`]).
//!
//! The state is grouped by subsystem (plumbing, propulsion, attitude
//! control, valves, batteries, mechanisms, structure, power, the cabin and
//! crew, failures, the program, the motion felt): each item of a part
//! holds all of its state, and the vessel-wide fields sit with their
//! subsystem. One state per vessel (D121): what the systems keep between
//! ticks is here too, not in a second state.
//!
//! What the systems do with it is `sim::systems`. Here: the state
//! at load (tanks filled at their loading temperatures, bottles at their
//! pressure) and the plain effect of the commands that have no transient
//! ([`ShipState::apply`]: valves, thruster blocks, power switches,
//! separations, the clamps, the parachutes' arming, the sequence's edits).
//! An engine or a mechanism is commanded only through
//! `systems::command::apply`: its start sequence, tail-off and actuator
//! rate are the engine rule's, in one place (F8 of the fidelity review).

use super::command::{matches, ShipCommand, Stage};
use super::ship::ShipDef;
use crate::catalog::Igniter;
use crate::fluids::{self, fluids};
use crate::systems::command::edit_sequence;
use crate::systems::crew::CrewRun;
use crate::systems::power::BusRun;
use crate::time::Epoch;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Room temperature for what is not loaded cold (K).
const ROOM_K: f64 = 293.15;

/// A tank's contents and how its liquid moves (`systems::plumbing`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TankState {
    pub part: u32,
    /// kg and K.
    pub liquid: f64,
    pub liquid_k: f64,
    /// The ullage's gas (pressurant and vapour), moles and K.
    pub ullage_moles: f64,
    pub ullage_k: f64,
    /// How far the liquid's bulk has moved away from its outlet along the
    /// tank's axis (m; 0: settled on the outlet), and its speed (m/s).
    #[serde(default)]
    pub drift: f64,
    #[serde(default)]
    pub drift_speed: f64,
    /// Pressurant moles let in and gas vented (kg) so far (the ledgers).
    #[serde(default)]
    pub pressurant_in: f64,
    #[serde(default)]
    pub vented: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BottleState {
    pub part: u32,
    pub moles: f64,
    pub k: f64,
}

/// Where an engine is in its life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineStatus {
    #[default]
    Off,
    Starting,
    Running,
    ShuttingDown,
    Failed,
}

/// An engine's state and its output as delivered (the transients').
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineState {
    pub part: u32,
    pub status: EngineStatus,
    /// Seconds in the current status (transients).
    pub timer: f64,
    /// Starts left (`u32::MAX`: hypergolic, unlimited).
    pub ignitions: u32,
    /// Its own throttle setting (the pilot's throttle scales it).
    pub throttle: f64,
    /// Gimbal deflection about the mount's two lateral axes (rad).
    pub gimbal: [f64; 2],
    /// The pilot's throttle and the readouts refer to it.
    pub selected: bool,
    /// Thrust level now, as a fraction of full flow (0 off; ramps up in a
    /// start, decays in the tail-off).
    #[serde(default)]
    pub level: f64,
    /// The level when the shutdown began (the tail-off decays from it).
    #[serde(default)]
    pub cutoff: f64,
    /// Why it stopped or failed last, for the player.
    #[serde(default)]
    pub reason: Option<String>,
    /// Waiting to start while its propellant settles (seconds waited): its
    /// start sequence fires the thrusters that push along its thrust.
    #[serde(default)]
    pub settling: Option<f64>,
    /// The pad's armed release waits for its rated thrust (it was lit when
    /// the release was armed, or at the end of that tick): set only while
    /// [`ShipState::release_armed`] is. It goes with its engine at a split.
    #[serde(default)]
    pub awaited: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThrusterState {
    pub part: u32,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValveState {
    pub part: u32,
    pub open: bool,
    /// Stuck where it is (a failure).
    pub stuck: bool,
    pub cycles: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BatteryState {
    pub part: u32,
    /// J and K.
    pub charge: f64,
    pub k: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MechanismState {
    pub part: u32,
    /// Degrees about its hinge from as built, and where it is going.
    pub angle: f64,
    pub target: f64,
}

/// A joint and its load at the last live tick (`systems::structure`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JointState {
    pub parent: u32,
    pub child: u32,
    pub intact: bool,
    /// Force the parent side exerts on the child side (body axes, N) and
    /// the moment about the joint's centre (N·m).
    #[serde(default)]
    pub force: DVec3,
    #[serde(default)]
    pub moment: DVec3,
    /// The equivalent line load over the allowable at temperature (1 is the
    /// ultimate load).
    #[serde(default)]
    pub ratio: f64,
    /// The factor the load as it stands (every force on the craft, and so
    /// its proper acceleration, scaled together) could grow by before the
    /// joint reaches its ultimate load (`structure::ultimate_scale`; `None`:
    /// no capacity data, no acceleration, or over 100 g).
    #[serde(default)]
    pub to_ultimate: Option<f64>,
}

/// A part's life across flights (D105 §6.6).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    /// Seconds operating, starts, cycles.
    pub run_time: f64,
    pub starts: u32,
    pub cycles: u32,
    pub peak_k: f64,
    pub peak_load: f64,
    pub flights: u32,
    pub landings: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartState {
    pub part: u32,
    /// 1 intact, 0 destroyed.
    pub integrity: f64,
    /// Miner's sums of thermal and vibration damage (fails at 1).
    pub thermal_damage: f64,
    pub vibration_damage: f64,
    pub history: History,
}

/// A crew cabin's air (partial pressures, Pa) and temperature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CabinState {
    pub part: u32,
    pub o2: f64,
    pub n2: f64,
    pub co2: f64,
    pub h2o: f64,
    pub k: f64,
}

/// A statistical failure mode's fate (D105 §6.3): the threshold E drawn
/// once and the hazard accumulated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fate {
    pub part: u32,
    /// The mode's owner, and its index in the owner's modes
    /// (`ShipDef::failures_of`).
    pub owner: super::handle::Owner,
    pub mode: u32,
    pub threshold: f64,
    pub hazard: f64,
    /// Its owner's starts or cycles counted so far (`sim::systems::failure`).
    #[serde(default)]
    pub seen: u32,
}

/// A component's power switch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Switch {
    pub part: u32,
    pub on: bool,
}

/// Everything about a craft of parts that changes in flight.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipState {
    // Plumbing.
    pub tanks: Vec<TankState>,
    pub bottles: Vec<BottleState>,
    /// Whether the plumbing regulates the tanks' pressures (its regulator
    /// sets it when it runs). Until it does, the structure takes each tank's
    /// flight (loading) pressure as its stabilising pressure at the least: a
    /// pressure that falls only because nothing re-pressurises the tank is
    /// not one to break a joint on (D105).
    #[serde(default)]
    pub pressure_regulated: bool,
    // Propulsion.
    pub engines: Vec<EngineState>,
    /// The sequence's clamp release is armed: it lets go at the first tick
    /// every engine it waits for ([`EngineState::awaited`]) delivers its
    /// rated thrust, and aborts the start if one fails (the real pad's
    /// rule, falcon9.md §1.2, §4.2).
    #[serde(default)]
    pub release_armed: bool,
    /// The pilot's throttle at the last step: closing it (open to 0) shuts
    /// the engines down (D114).
    #[serde(default)]
    pub throttle_was: f64,
    /// The engines' realised thrust direction after the gimbal (body axes;
    /// `None` until they first run): the actuators move it at their rate.
    #[serde(default)]
    pub thrust_dir: Option<glam::DVec3>,
    // Attitude control and the aero surfaces.
    pub thrusters: Vec<ThrusterState>,
    /// Each aero surface's deflection (rad; aligned with
    /// `ShipDef::surfaces`, or empty before any deflects).
    #[serde(default)]
    pub fins: Vec<f64>,
    pub valves: Vec<ValveState>,
    pub batteries: Vec<BatteryState>,
    pub mechanisms: Vec<MechanismState>,
    pub joints: Vec<JointState>,
    /// Joints a split cut, kept by their child's piece until a merge joins
    /// their parts again (`craft::partition`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub severed: Vec<JointState>,
    pub parts: Vec<PartState>,
    // Power.
    /// One per component.
    pub power: Vec<Switch>,
    /// The loads without power now (their bus short: shed, or no source)
    /// and the buses as they stood (`systems::power`).
    #[serde(default)]
    pub unpowered: Vec<Lost>,
    #[serde(default)]
    pub buses: Vec<BusRun>,
    // The cabin and its crew.
    #[serde(default)]
    pub cabin: Option<CabinState>,
    /// The crew's exposure to acceleration and their injuries
    /// (`systems::crew`).
    #[serde(default)]
    pub crew: CrewRun,
    // Failures (D105 §6.3, §6.4).
    #[serde(default)]
    pub fates: Vec<Fate>,
    /// The functions lost to failures (by part, as the rest).
    #[serde(default)]
    pub lost: Vec<Lost>,
    /// The failure fates' seed (the save's) and the launch epoch they are
    /// drawn with (D105 §6.3).
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub launch: Option<Epoch>,
    /// Held by the pad's clamps.
    #[serde(default)]
    pub clamped: bool,
    /// The next stage of the sequence.
    #[serde(default)]
    pub sequence: u32,
    /// The stage sequence and the saved functions (D107: the vessel's, not
    /// settings; the craft file's at load, edited in the systems screen).
    #[serde(default)]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub functions: Vec<Stage>,
    /// Orders not yet applied, in the order given (applied by epoch, ties
    /// in that order).
    #[serde(default)]
    pub orders: Vec<Order>,
    // The vessel's motion as the systems felt it, and their clock.
    /// The last tick's proper acceleration at the centre of mass, angular
    /// velocity and angular acceleration (body axes): what the liquids,
    /// joints and accelerometers felt.
    #[serde(default)]
    pub accel: glam::DVec3,
    #[serde(default)]
    pub omega: glam::DVec3,
    #[serde(default)]
    pub alpha: glam::DVec3,
    /// Where the slow systems (the plumbing's heat, relief and boil-off)
    /// last stepped to.
    #[serde(default)]
    pub stepped: Option<Epoch>,
    /// What happened to the ship, for the player (newest last).
    #[serde(default)]
    pub events: Vec<Event>,
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
    /// The part or component whose function it is.
    pub owner: super::handle::Owner,
    pub why: String,
}

/// How many events are kept.
pub const EVENTS_KEPT: usize = 64;

/// How a part failed destroyed (D105 §6.4; `sim::systems::failure`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PartFailure {
    /// At its destruction temperature, or its time at temperature spent.
    Melted { k: f64 },
    /// A tank or bottle over its burst pressure (Pa).
    Burst { pa: f64 },
    /// A blast's overpressure (Pa) over its rating.
    Blast { pa: f64 },
    /// An engine's uncontained failure.
    Uncontained,
}

impl PartFailure {
    /// A few words for the player.
    pub fn describe(&self) -> String {
        match *self {
            PartFailure::Melted { k } => format!("melted at {k:.0} K"),
            PartFailure::Burst { pa } if pa > 0.0 => format!("burst at {:.1} bar", pa / 1e5),
            PartFailure::Burst { .. } => "burst".into(),
            PartFailure::Blast { pa } => format!("blast of {:.1} bar", pa / 1e5),
            PartFailure::Uncontained => "uncontained engine failure".into(),
        }
    }
}

/// What a command asks of the vessel beyond the ship state.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Separate at the joint whose child part is this (`Part::id`).
    Separate(u32),
    /// The joint whose child part is this (`Part::id`) snapped under a load
    /// (N): the child's side tears off with what hangs from it (no
    /// separation system fires; `sim::systems::structure`).
    Snap(u32, f64),
    /// The part (`Part::id`) failed destroyed (`sim::systems::failure`): it
    /// leaves as a wreck of its own.
    Destroy(u32, PartFailure),
    ReleaseClamps,
    ArmChutes,
}

impl ShipState {
    /// The state at load (D104): tanks filled at their loading temperature
    /// with their ullage at its pressure, bottles at their pressure,
    /// engines off with their ignitions, valves and thrusters as built,
    /// batteries full, mechanisms stowed, joints intact, the first engines
    /// the sequence ignites selected.
    pub fn initial(def: &ShipDef) -> ShipState {
        let f = fluids();
        let tanks = (def.tanks.iter())
            .map(|t| {
                let liquid = t.capacity * t.load.fill;
                let density = f.liquid(&t.fluid).map_or(1000.0, |l| l.density(t.load.temperature_k));
                let ullage = (t.volume - liquid / density).max(0.0);
                TankState {
                    part: t.part,
                    liquid,
                    liquid_k: t.load.temperature_k,
                    ullage_moles: fluids::moles(t.load.pressure, ullage, t.load.temperature_k),
                    ullage_k: t.load.temperature_k,
                    ..TankState::default()
                }
            })
            .collect();
        let bottles = (def.bottles.iter())
            .map(|b| BottleState {
                part: b.part,
                moles: fluids::moles(b.pressure, b.volume, b.temperature_k),
                k: b.temperature_k,
            })
            .collect();
        let first = def.sequence.iter().flat_map(|s| &s.actions).find_map(|a| match a {
            ShipCommand::Ignite(s) => Some(def.engines_of(s)),
            _ => None,
        });
        let engines = (def.engines.iter().enumerate())
            .map(|(i, e)| EngineState {
                part: e.part,
                status: EngineStatus::Off,
                timer: 0.0,
                ignitions: if e.data.igniter == Igniter::Hypergolic { u32::MAX } else { e.data.ignitions },
                throttle: 1.0,
                gimbal: [0.0; 2],
                selected: first.as_ref().is_some_and(|f| f.contains(&i)),
                ..EngineState::default()
            })
            .collect();
        let crewed = def.seats.first().map(|s| s.part);
        ShipState {
            tanks,
            bottles,
            engines,
            thrusters: def.thrusters.iter().map(|t| ThrusterState { part: t.part, enabled: t.enabled }).collect(),
            valves: (def.valves.iter())
                .map(|v| ValveState { part: v.part, open: v.item.open, stuck: false, cycles: 0 })
                .collect(),
            batteries: (def.batteries.iter())
                .map(|b| BatteryState { part: b.part, charge: b.item.capacity, k: ROOM_K })
                .collect(),
            mechanisms: (def.mechanisms.iter())
                .map(|m| MechanismState { part: m.part, angle: m.item.stowed_deg, target: m.item.stowed_deg })
                .collect(),
            joints: (def.joints.iter())
                .map(|j| JointState { parent: j.parent, child: j.child, intact: true, ..JointState::default() })
                .collect(),
            severed: Vec::new(),
            parts: (def.parts.iter())
                .map(|p| PartState {
                    part: p.part,
                    integrity: 1.0,
                    thermal_damage: 0.0,
                    vibration_damage: 0.0,
                    history: History::default(),
                })
                .collect(),
            power: def.components.iter().map(|c| Switch { part: c.part, on: true }).collect(),
            // Sea-level air (21 % O2, 78 % N2 by volume, ~1 % H2O), 295 K.
            cabin: crewed.map(|part| CabinState {
                part,
                o2: 21_200.0,
                n2: 79_000.0,
                co2: 50.0,
                h2o: 1_000.0,
                k: 295.0,
            }),
            fates: Vec::new(),
            clamped: def.joints.iter().any(|j| j.kind == super::assembly::JointKind::HoldDown),
            sequence: 0,
            stages: def.sequence.clone(),
            functions: def.functions.clone(),
            ..ShipState::default()
        }
    }

    /// Whether every per-item list matches the definition's (a split or a
    /// merge keeps them so by construction: `craft::partition`).
    pub fn fits(&self, def: &ShipDef) -> bool {
        self.tanks.len() == def.tanks.len()
            && self.bottles.len() == def.bottles.len()
            && self.engines.len() == def.engines.len()
            && self.thrusters.len() == def.thrusters.len()
            && self.valves.len() == def.valves.len()
            && self.batteries.len() == def.batteries.len()
            && self.mechanisms.len() == def.mechanisms.len()
            && self.joints.len() == def.joints.len()
            && self.parts.len() == def.parts.len()
            && self.power.len() == def.components.len()
    }

    /// Whether `owner`'s function is out: lost to a failure, or without
    /// power (`systems::power`).
    pub fn out(&self, owner: super::handle::Owner) -> bool {
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

    /// Liquid in the tanks (kg).
    pub fn liquid(&self) -> f64 {
        self.tanks.iter().map(|t| t.liquid).sum()
    }

    /// Gas in the bottles (kg).
    pub fn gas(&self, def: &ShipDef) -> f64 {
        (self.bottles.iter().zip(&def.bottles)).map(|(s, b)| s.moles * molar_mass(&b.gas)).sum()
    }

    /// The pressurant in the tanks' ullages (kg): the gas the bottles let
    /// in stays aboard until it is vented (`sim::systems::plumbing`).
    pub fn ullage_gas(&self, def: &ShipDef) -> f64 {
        (self.tanks.iter().zip(&def.tanks)).map(|(s, t)| s.ullage_moles * molar_mass(&t.load.pressurant)).sum()
    }

    /// Everything the vessel carries that it can spend (kg): its propellant,
    /// the only total there is (F2: the liquids, the bottles' gas and the
    /// ullages').
    pub fn consumables(&self, def: &ShipDef) -> f64 {
        let tanks = self.tanks.iter().map(|t| (t.liquid, t.ullage_moles));
        consumables_of(def, tanks, self.bottles.iter().map(|b| b.moles))
    }

    /// Applies a command's plain effect (see the module): the state changes
    /// at once; what the vessel must do besides is returned. Naming nothing
    /// the craft has is an error (nothing changes), and so is an engine's or
    /// a mechanism's command: it has a transient, `systems::command::apply`'s.
    pub fn apply(&mut self, def: &ShipDef, command: &ShipCommand) -> Result<Vec<Effect>, String> {
        let none = || Err(format!("{command:?}: names nothing on this craft"));
        let ids = |list: &mut dyn Iterator<Item = &String>, s: &str| -> Vec<usize> {
            list.enumerate().filter(|(_, id)| matches(s, id)).map(|(i, _)| i).collect()
        };
        match command {
            ShipCommand::Ignite(_)
            | ShipCommand::Shutdown(_)
            | ShipCommand::Throttle(..)
            | ShipCommand::Select(_)
            | ShipCommand::Deploy(_)
            | ShipCommand::Retract(_)
            | ShipCommand::Angle(..) => {
                Err(format!("{command:?}: an engine's or a mechanism's command (`systems::command::apply`)"))
            }
            ShipCommand::Open(s) | ShipCommand::Close(s) => {
                let picked = ids(&mut def.valves.iter().map(|v| &v.id), s);
                if picked.is_empty() {
                    return none();
                }
                let open = matches!(command, ShipCommand::Open(_));
                for i in picked {
                    let v = &mut self.valves[i];
                    if !v.stuck && v.open != open {
                        v.open = open;
                        v.cycles += 1;
                    }
                }
                Ok(Vec::new())
            }
            ShipCommand::Separate(name) => {
                let Some(j) = def.joint_named(name) else { return none() };
                if !self.joints[j].intact {
                    return Ok(Vec::new());
                }
                self.joints[j].intact = false;
                Ok(vec![Effect::Separate(def.joints[j].child)])
            }
            ShipCommand::ReleaseClamps => {
                // Every hold-down joint lets go: the vehicle leaves the pad's mount.
                self.clamped = false;
                let mut effects = vec![Effect::ReleaseClamps];
                for (j, d) in def.joints.iter().enumerate() {
                    if d.kind == super::assembly::JointKind::HoldDown && self.joints[j].intact {
                        self.joints[j].intact = false;
                        effects.push(Effect::Separate(d.child));
                    }
                }
                Ok(effects)
            }
            ShipCommand::ArmChutes => Ok(vec![Effect::ArmChutes]),
            ShipCommand::Rcs(s, on) => {
                let picked = ids(&mut def.thrusters.iter().map(|t| &t.id), s);
                if picked.is_empty() {
                    return none();
                }
                for i in picked {
                    self.thrusters[i].enabled = *on;
                }
                Ok(Vec::new())
            }
            ShipCommand::Power(s, on) => {
                let picked = ids(&mut def.components.iter().map(|c| &c.id), s);
                if picked.is_empty() {
                    return none();
                }
                for i in picked {
                    self.power[i].on = *on;
                }
                Ok(Vec::new())
            }
            ShipCommand::Sequence(e) => edit_sequence(def, &mut self.stages, self.sequence, e).map(|()| Vec::new()),
        }
    }
}

/// A craft of parts' consumables (kg): its tanks' liquids and ullage gas,
/// `(kg, mol)` in `def`'s order, and its bottles' gas (mol), each gas at
/// its molar mass ([`consumables_sum`]).
pub fn consumables_of(
    def: &ShipDef,
    tanks: impl IntoIterator<Item = (f64, f64)>,
    bottles: impl IntoIterator<Item = f64>,
) -> f64 {
    let tanks = tanks.into_iter().zip(&def.tanks).map(|((kg, mol), t)| (kg, mol, molar_mass(&t.load.pressurant)));
    consumables_sum(tanks, bottles.into_iter().zip(&def.bottles).map(|(mol, b)| (mol, molar_mass(&b.gas))))
}

/// A gas's molar mass (kg/mol; 0 for a gas the table does not know: it
/// weighs nothing in the consumables).
pub fn molar_mass(gas: &str) -> f64 {
    fluids().gas(gas).map_or(0.0, |g| g.molar_mass)
}

/// The one sum of the consumables (kg): the tanks' liquids, then the
/// bottles' gas, then the ullages', from `(kg, mol, kg/mol)` per tank and
/// `(mol, kg/mol)` per bottle. The ship state ([`ShipState::consumables`])
/// and a coast's law of it (`systems::coast_law`) both take it, so they
/// agree to the bit.
pub fn consumables_sum(
    tanks: impl IntoIterator<Item = (f64, f64, f64)>,
    bottles: impl IntoIterator<Item = (f64, f64)>,
) -> f64 {
    let (mut liquid, mut ullage) = (0.0, 0.0);
    for (kg, mol, molar) in tanks {
        liquid += kg;
        ullage += mol * molar;
    }
    let mut gas = 0.0;
    for (mol, molar) in bottles {
        gas += mol * molar;
    }
    liquid + gas + ullage
}
