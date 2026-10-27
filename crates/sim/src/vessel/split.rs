//! Splitting a vessel (D032, D083; `docs/features/parts.md`): a set of its
//! parts leaves as a new vessel, a full vessel with its own trajectory,
//! aerodynamics and heating.
//!
//! A split is a rigid-body event at the vessel's time, so it is as
//! deterministic as that time (a live tick's end, or wherever a caller
//! stopped the vessel): the stored trajectory stays the truth, a coasting
//! vessel's pieces start new coast segments at the split epoch, and chunked
//! equals single pass.
//!
//! * **Pieces** ([`SplitPlan`]): the connected groups of parts once the
//!   attachments crossing the split are cut ([`crate::craft::parts::components`]);
//!   a destroyed part ([`Breakup`]) is a piece of its own, a wreck. The
//!   vessel keeps the group holding its lowest control part (else the
//!   heaviest); the others become new vessels. Each piece's craft is a
//!   piece of the vessel's base craft (`base#parts`, [`crate::craft::registry`]),
//!   built once per process — the expensive step (its design), which a
//!   caller may run off its frame ([`SplitPlan::crafts`]).
//! * **Motion**: every piece keeps the body axes, attitude and angular
//!   velocity ω; its centre of mass sits where it was in the craft, moving
//!   at v + ω × r (r from the craft's centre of mass). Linear and angular
//!   momentum are conserved, since the pieces' mass properties sum to the
//!   craft's (each part's dry mass is its own shell; every tank is at the
//!   same fill).
//! * **State carried**: propellant by tank (each piece's tanks at the
//!   craft's fill), skin and interior temperatures (each new cell or node
//!   takes the temperature of the nearest one of its part), the ship clock,
//!   systems (power and boil-off shared by the parts' placeholders), debug
//!   mode, the limbo placeholders; the parachutes go with the part that
//!   carries them, the crew with the seats (the vessel's piece first). The
//!   vessel keeps its flight plan and its attitude control; a new piece
//!   starts with neither.
//! * **Phase**: live stays live, a coast restarts as coasts from the split
//!   epoch, a landed or crashed vessel is woken into live ticks.

use super::{AttitudeControl, Destruction, Phase, Vessel, VesselIds, VesselThermal};
use crate::chute::{CanopyState, ChuteState};
use crate::craft::parts::components;
use crate::craft::{registry, Craft, CraftDesign, CraftId};
use crate::thermal::ThermalState;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;
use std::sync::Arc;

/// What breaks a vessel up, found at a live tick's end
/// ([`super::breakage`]): parts destroyed (each leaves as a wreck of its
/// own) and parts torn off their parent by a load (with what hangs from
/// them, intact). Indices into the vessel's parts.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Breakup {
    pub epoch: Epoch,
    pub destroyed: Vec<(u32, Destruction)>,
    /// Torn parts and the load (N) that tore them.
    pub torn: Vec<(u32, f64)>,
    /// Parts separated from their parents by command (a separation
    /// system fired: `crate::craft::command::ShipCommand::Separate`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub separated: Vec<u32>,
}

/// The pieces a split makes, the vessel's own first.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitPlan {
    /// Each piece's parts (indices into the vessel's parts, ascending).
    pub pieces: Vec<Vec<u32>>,
    /// Each piece's craft (a piece of the vessel's base craft).
    pub ids: Vec<CraftId>,
    /// Why a piece is a wreck, if it is.
    pub destroyed: Vec<Option<Destruction>>,
}

impl SplitPlan {
    /// The pieces' craft with their designs, built through
    /// [`registry::craft_of`] (kept per id for the process), one thread per
    /// piece (each build is independent and deterministic): where a
    /// split's cost is.
    pub fn crafts(&self) -> Vec<Arc<Craft>> {
        std::thread::scope(|s| {
            let built: Vec<_> = (self.ids.iter())
                .map(|id| {
                    s.spawn(move || {
                        let craft = registry::craft_of(id);
                        craft.design();
                        craft
                    })
                })
                .collect();
            built.into_iter().map(|b| b.join().expect("a piece's craft")).collect()
        })
    }
}

/// The chute state of a piece without the parachutes: none left to open.
pub(super) const LOST_CHUTE: ChuteState =
    ChuteState { armed: false, drogue: CanopyState::Released, main: CanopyState::Released };

impl Vessel {
    /// The breakup found at the last live tick, waiting for [`Self::split`]
    /// (the vessel does not advance until then).
    pub fn breakup(&self) -> Option<&Breakup> {
        self.breakup.as_ref()
    }

    /// The pieces of separating the parts `leaving` (indices into the
    /// craft's parts) from the rest: every attachment across that boundary
    /// cut, each connected group a piece.
    pub fn plan_split(&self, leaving: &[u32]) -> SplitPlan {
        let parts = &self.craft.design().parts;
        let out = |i: usize| leaving.contains(&(i as u32));
        let cut: Vec<u32> = (0..parts.len())
            .filter(|&i| parts[i].parent.is_some_and(|up| out(i) != out(up as usize)))
            .map(|i| i as u32)
            .collect();
        self.pieces_of(&cut, &[])
    }

    /// The pieces of detaching the parts `parts` (indices into the craft's
    /// parts) from their parents: each leaves with what hangs from it (a
    /// decoupler's separation).
    pub fn plan_detach(&self, parts: &[u32]) -> SplitPlan {
        self.pieces_of(parts, &[])
    }

    /// The pieces of the pending breakup, if any.
    pub fn plan_breakup(&self) -> Option<SplitPlan> {
        let b = self.breakup.as_ref()?;
        let mut torn: Vec<u32> = b.torn.iter().map(|t| t.0).collect();
        torn.extend(&b.separated);
        Some(self.pieces_of(&torn, &b.destroyed))
    }

    /// The pieces after cutting the parts in `cut` from their parents and
    /// destroying those in `destroyed`: the intact groups (the vessel's
    /// first: its lowest control part's, else the heaviest), then each
    /// destroyed part alone. With no intact group left, the vessel keeps
    /// the first wreck.
    fn pieces_of(&self, cut: &[u32], destroyed: &[(u32, Destruction)]) -> SplitPlan {
        let design = self.craft.design().clone();
        let parts = &design.parts;
        let removed: Vec<u32> = destroyed.iter().map(|d| d.0).collect();
        let mut intact = components(parts, cut, &removed);
        let mass = |g: &[u32]| g.iter().map(|&p| parts[p as usize].dry_mass).sum::<f64>();
        let control = |g: &[u32]| g.iter().copied().find(|&p| parts[p as usize].control);
        let keep = (0..intact.len()).min_by(|&a, &b| {
            let (ca, cb) = (control(&intact[a]), control(&intact[b]));
            match (ca, cb) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => mass(&intact[b]).total_cmp(&mass(&intact[a])).then(intact[a][0].cmp(&intact[b][0])),
            }
        });
        if let Some(k) = keep {
            intact.swap(0, k);
            intact[1..].sort_by_key(|g| g[0]);
        }
        let mut causes = vec![None; intact.len()];
        let mut wrecks: Vec<(u32, Destruction)> = destroyed.to_vec();
        wrecks.sort_by_key(|w| w.0);
        wrecks.dedup_by_key(|w| w.0);
        for (p, cause) in wrecks {
            intact.push(vec![p]);
            causes.push(Some(cause));
        }
        let ids = intact.iter().map(|g| self.piece_id(&design, g)).collect();
        SplitPlan { pieces: intact, ids, destroyed: causes }
    }

    /// The craft id of the piece made of `parts` (indices into the parts),
    /// with those of its parts whose mechanisms are deployed.
    fn piece_id(&self, design: &CraftDesign, parts: &[u32]) -> CraftId {
        let ids: Vec<u32> = parts.iter().map(|&p| design.parts[p as usize].id).collect();
        let deployed: Vec<u32> = self.deployed_parts().into_iter().filter(|p| ids.contains(p)).collect();
        CraftId::piece(self.craft.id.base(), &ids).with_deployed(&deployed)
    }

    /// Splits the vessel by `plan` with the pieces' `crafts` (in the plan's
    /// order, [`SplitPlan::crafts`]): the vessel becomes the first piece and
    /// the others are returned as new vessels, with ids from `ids`. Clears a
    /// pending breakup.
    pub fn split(
        &mut self,
        world: &World,
        plan: &SplitPlan,
        crafts: &[Arc<Craft>],
        ids: &mut VesselIds,
    ) -> Vec<Vessel> {
        assert_eq!(plan.pieces.len(), crafts.len(), "one craft per piece");
        self.breakup = None;
        // One piece of another craft: the same parts, mechanisms moved.
        let reconfigure = plan.pieces.len() == 1 && crafts[0].id != self.craft.id;
        if plan.pieces.len() < 2 && !reconfigure {
            return Vec::new();
        }
        if matches!(self.phase, Phase::Landed { .. } | Phase::Crashed { .. }) {
            self.wake(world);
        }
        let coasting = matches!(self.phase, Phase::Coasting { .. });
        if coasting {
            self.thermal_to_now(world);
        }
        let (anchor, r, v) = self.state(world);
        // Every piece gets its own phase below: drop the stored trajectory
        // first, so the pieces' copies are light.
        self.phase = Phase::Powered { anchor, r, v };
        let old = self.craft.design().clone();
        let props = self.mass_props();
        let capacity = self.craft.mass.capacity;
        let fill = if capacity > 0.0 { self.propellant / capacity } else { 0.0 };
        let (q, omega) = (self.attitude.q, self.attitude.omega);
        let total = Shares::of(&old.parts);
        let mut crew = self.crew;
        let mut pieces: Vec<Vessel> = Vec::new();
        for (k, craft) in crafts.iter().enumerate() {
            // Ground (the pad's mount left behind when the clamps release)
            // stays with the ground: it is not a vessel.
            let ground = craft.ship.as_ref().is_some_and(|s| !s.parts.is_empty() && s.parts.iter().all(|p| p.ground));
            if k > 0 && ground {
                continue;
            }
            let design = craft.design().clone();
            let mut piece = self.clone();
            piece.craft = craft.params();
            let capacity = piece.craft.mass.capacity;
            match (&self.ship, &craft.ship) {
                // A craft of parts: each tank and bottle goes with its part.
                (Some(state), Some(def)) => {
                    let ids: Vec<u32> = design.parts.iter().map(|p| p.id).collect();
                    let mine = state.partition(&ids, def);
                    piece.propellant = mine.consumables(def);
                    piece.ship = Some(mine);
                    piece.sync_engine();
                }
                _ => piece.propellant = (fill * capacity).min(capacity),
            }
            // A reconfiguration moves parts about the centre of mass, which
            // stays; the angular momentum stays with the new inertia.
            let offset = if reconfigure { DVec3::ZERO } else { q * (piece.mass_props().com - props.com) };
            let state = (anchor, r + offset, v + omega.cross(offset));
            if reconfigure {
                let rot = glam::DMat3::from_quat(q);
                let world = |i: glam::DMat3| rot * i * rot.transpose();
                let spin = world(props.inertia) * omega;
                piece.attitude.omega = world(piece.mass_props().inertia).inverse() * spin;
            }
            piece.phase = Phase::Powered { anchor: state.0, r: state.1, v: state.2 };
            piece.tick_accel = None;
            piece.rest_ticks = 0;
            piece.rest_mode = None;
            piece.repoint = false;
            let mut th = VesselThermal::uniform(design.cells.len(), design.nodes(), 0.0, self.thermal.epoch);
            th.state = carry_temperatures(&old, &self.thermal.state, &design);
            piece.thermal = th;
            let mine = Shares::of(&design.parts);
            piece.systems = total.share(&mine, &self.systems, k == 0);
            if !design.parts.iter().any(|p| p.chute) {
                piece.chute = LOST_CHUTE;
            }
            let seats: u32 = design.parts.iter().map(|p| p.crew).sum();
            piece.crew = crew.min(seats);
            crew -= piece.crew;
            if k > 0 {
                piece.id = ids.allocate();
                piece.plan = Default::default();
                piece.control = AttitudeControl::default();
            } else {
                piece.control.base = None;
            }
            if let Some(cause) = plan.destroyed[k] {
                piece.destroy(cause);
            }
            if coasting {
                piece.start_coast_from(world, state);
            }
            pieces.push(piece);
        }
        let rest = pieces.split_off(1);
        *self = pieces.pop().expect("the vessel's piece");
        rest
    }
}

/// The parts' power and tank placeholders, to share a vessel's systems.
struct Shares {
    produced: f64,
    battery: f64,
    consumed: f64,
    capacity: f64,
}

impl Shares {
    fn of(parts: &[crate::craft::Part]) -> Self {
        let sum = |f: &dyn Fn(&crate::craft::Part) -> f64| parts.iter().map(f).sum::<f64>();
        Shares {
            produced: sum(&|p| p.power.produced),
            battery: sum(&|p| p.power.battery),
            consumed: sum(&|p| p.power.consumed),
            capacity: sum(&|p| p.tank.map_or(0.0, |t| t.capacity)),
        }
    }

    /// The systems of a piece with the placeholders `mine` out of these:
    /// panel, battery (and its charge) and loads in proportion, boil-off by
    /// tank capacity; the vessel's piece (`own`) keeps the ledgers.
    fn share(&self, mine: &Shares, systems: &crate::systems::Systems, own: bool) -> crate::systems::Systems {
        let ratio = |a: f64, b: f64| if b > 0.0 { a / b } else { 0.0 };
        let mut s = systems.clone();
        let p = &mut s.power;
        let (sun, battery) = (ratio(mine.produced, self.produced), ratio(mine.battery, self.battery));
        p.spec.panel_area *= sun;
        p.produced *= sun;
        p.spec.capacity *= battery;
        p.charge *= battery;
        p.spec.load *= ratio(mine.consumed, self.consumed);
        s.consumables.boil_off *= ratio(mine.capacity, self.capacity);
        if !own {
            p.shortfall = 0.0;
            s.consumables.boiled = 0.0;
        }
        s
    }
}

/// The temperatures of design `to` (a piece) from `state` of design
/// `from` (the craft it came from): each cell and node takes the nearest
/// one of its own part (by part id), else the nearest of all.
pub fn carry_temperatures(from: &CraftDesign, state: &ThermalState, to: &CraftDesign) -> ThermalState {
    // The old cells and nodes of each part id.
    let by_id = |tags: &[u32]| {
        let mut map: std::collections::BTreeMap<u32, Vec<usize>> = Default::default();
        for (j, &p) in tags.iter().enumerate() {
            map.entry(from.parts[p as usize].id).or_default().push(j);
        }
        map
    };
    let (cells, nodes) = (by_id(&from.cell_part), by_id(&from.node_part));
    let nearest = |candidates: &mut dyn Iterator<Item = usize>, at: DVec3, pos: &dyn Fn(usize) -> DVec3| {
        candidates.fold(None, |best: Option<(f64, usize)>, j| {
            let d = (pos(j) - at).length_squared();
            if best.is_none_or(|(b, _)| d < b) {
                Some((d, j))
            } else {
                best
            }
        })
    };
    let pick = |own: Option<&Vec<usize>>, all: usize, at: DVec3, pos: &dyn Fn(usize) -> DVec3| {
        own.and_then(|c| nearest(&mut c.iter().copied(), at, pos))
            .or_else(|| nearest(&mut (0..all), at, pos))
            .expect("a craft has cells and a node")
            .1
    };
    let cell_at = |j: usize| from.cells[j].centroid;
    let skin = (0..to.cells.len())
        .map(|i| {
            let own = cells.get(&to.parts[to.cell_part[i] as usize].id);
            state.skin[pick(own, from.cells.len(), to.cells[i].centroid, &cell_at)]
        })
        .collect();
    let node_at = |j: usize| from.volume.nodes[j].centre;
    let nodes = (0..to.volume.nodes.len())
        .map(|i| {
            let own = nodes.get(&to.parts[to.node_part[i] as usize].id);
            state.nodes[pick(own, from.volume.nodes.len(), to.volume.nodes[i].centre, &node_at)]
        })
        .collect();
    ThermalState { skin, nodes }
}
