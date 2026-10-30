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
//! What the systems do with it is `sim::systems` (RUNTIME). Here: the state
//! at load (tanks filled at their loading temperatures, bottles at their
//! pressure) and the plain effect of each [`ShipCommand`] ([`ShipState::apply`]:
//! engines start and stop at once, valves and mechanisms move at once; the
//! transients are the systems').

use super::command::{matches, ShipCommand, Stage};
use super::ship::ShipDef;
use crate::catalog::Igniter;
use crate::fluids::{self, fluids};
use crate::systems::command::edit_sequence;
use serde::{Deserialize, Serialize};

/// Room temperature for what is not loaded cold (K).
const ROOM_K: f64 = 293.15;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankState {
    pub part: u32,
    /// kg and K.
    pub liquid: f64,
    pub liquid_k: f64,
    /// The ullage's gas (pressurant and vapour), moles and K.
    pub ullage_moles: f64,
    pub ullage_k: f64,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JointState {
    pub parent: u32,
    pub child: u32,
    pub intact: bool,
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
    /// The owner's id and the mode's name.
    pub owner: String,
    pub mode: String,
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
    pub tanks: Vec<TankState>,
    pub bottles: Vec<BottleState>,
    pub engines: Vec<EngineState>,
    pub thrusters: Vec<ThrusterState>,
    pub valves: Vec<ValveState>,
    pub batteries: Vec<BatteryState>,
    pub mechanisms: Vec<MechanismState>,
    pub joints: Vec<JointState>,
    /// Joints a split cut, kept by their child's piece until a merge joins
    /// their parts again (`craft::partition`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub severed: Vec<JointState>,
    pub parts: Vec<PartState>,
    /// One per component.
    pub power: Vec<Switch>,
    #[serde(default)]
    pub cabin: Option<CabinState>,
    #[serde(default)]
    pub fates: Vec<Fate>,
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
    /// The systems' own state (`sim::systems::runtime`, RUNTIME).
    #[serde(default)]
    pub runtime: crate::systems::Runtime,
}

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
            joints: def.joints.iter().map(|j| JointState { parent: j.parent, child: j.child, intact: true }).collect(),
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
            runtime: crate::systems::Runtime::initial(def),
        }
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
    /// the craft has is an error (nothing changes).
    pub fn apply(&mut self, def: &ShipDef, command: &ShipCommand) -> Result<Vec<Effect>, String> {
        let none = || Err(format!("{command:?}: names nothing on this craft"));
        let ids = |list: &mut dyn Iterator<Item = &String>, s: &str| -> Vec<usize> {
            list.enumerate().filter(|(_, id)| matches(s, id)).map(|(i, _)| i).collect()
        };
        // On the pad the clamps' ground supply lights the engines (Falcon 9's
        // TEA-TEB on the pad: falcon9.md §2.1); in flight each start uses one
        // of the engine's own ignitions.
        let ground = self.clamped;
        match command {
            ShipCommand::Ignite(s)
            | ShipCommand::Shutdown(s)
            | ShipCommand::Throttle(s, _)
            | ShipCommand::Select(s) => {
                let picked = def.engines_of(s);
                if picked.is_empty() {
                    return none();
                }
                if matches!(command, ShipCommand::Select(_)) {
                    for (i, e) in self.engines.iter_mut().enumerate() {
                        e.selected = picked.contains(&i);
                    }
                }
                for &i in &picked {
                    let e = &mut self.engines[i];
                    match command {
                        ShipCommand::Ignite(_)
                            if matches!(e.status, EngineStatus::Off) && (ground || e.ignitions > 0) =>
                        {
                            e.status = EngineStatus::Running;
                            e.timer = 0.0;
                            e.selected = true;
                            if !ground && e.ignitions != u32::MAX {
                                e.ignitions -= 1;
                            }
                        }
                        ShipCommand::Shutdown(_) if !matches!(e.status, EngineStatus::Off | EngineStatus::Failed) => {
                            e.status = EngineStatus::Off;
                            e.timer = 0.0;
                        }
                        ShipCommand::Throttle(_, x) => e.throttle = x.clamp(0.0, 1.0),
                        _ => {}
                    }
                }
                Ok(Vec::new())
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
            ShipCommand::Deploy(s) | ShipCommand::Retract(s) | ShipCommand::Angle(s, _) => {
                let picked = ids(&mut def.mechanisms.iter().map(|m| &m.id), s);
                if picked.is_empty() {
                    return none();
                }
                for i in picked {
                    let m = &def.mechanisms[i].item;
                    let target = match command {
                        ShipCommand::Deploy(_) => m.deployed_deg,
                        ShipCommand::Retract(_) if m.retractable => m.stowed_deg,
                        ShipCommand::Retract(_) => self.mechanisms[i].angle,
                        ShipCommand::Angle(_, a) => {
                            a.clamp(m.stowed_deg.min(m.deployed_deg), m.stowed_deg.max(m.deployed_deg))
                        }
                        _ => unreachable!(),
                    };
                    self.mechanisms[i].target = target;
                    self.mechanisms[i].angle = target;
                }
                Ok(Vec::new())
            }
            ShipCommand::Separate(name) => {
                let Some(j) = def.joints.iter().position(|j| j.name == *name) else { return none() };
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

    /// Runs the next stage of the sequence in this craft's view
    /// (`craft::program`): each of its commands' plain effect. `None` when
    /// it has none left.
    pub fn next_stage(&mut self, def: &ShipDef) -> Option<(String, Vec<Effect>)> {
        let k = super::program::next(def, &self.stages, self.sequence)?;
        let stage = super::program::mine(def, &self.stages[k]);
        self.sequence = k as u32 + 1;
        let mut effects = Vec::new();
        for a in &stage.actions {
            effects.extend(self.apply(def, a).unwrap_or_default());
        }
        Some((stage.name.clone(), effects))
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
