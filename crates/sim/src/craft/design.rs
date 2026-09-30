//! What is derived once per craft design and shared by every vessel of it
//! (realism-1 §5): the aerodynamic bake (`sim::aero`), the interior volume
//! grid (`super::volume`) and the thermal network over its cells and nodes
//! (`sim::thermal`, D065 revised).
//!
//! Never saved: a vessel's [`super::CraftParams`] holds a [`DesignSlot`]
//! that is filled by [`super::Craft::params`] and, after loading a save,
//! rebuilt from the craft's files on first use ([`design_of`]). The bake is
//! deterministic, so a rebuilt design is the same bits.

use super::{Cell, Craft, CraftId, VolumeGrid};
use crate::aero::{self, AeroBake, BakeOptions, Flow};
use crate::thermal::{Interior, ThermalNetwork};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Mach number of the orientation-averaged drag area (hypersonic).
const MEAN_DRAG_MACH: f64 = 10.0;

/// A craft design's aerodynamic and thermal models.
#[derive(Debug, PartialEq)]
pub struct CraftDesign {
    pub bake: AeroBake,
    pub network: ThermalNetwork,
    /// The interior nodes.
    pub volume: VolumeGrid,
    /// Heat capacity of each node without propellant (J/K).
    pub dry_capacity: Vec<f64>,
    /// Specific heat of the propellant (J/(kg·K)).
    pub propellant_specific_heat: f64,
    /// Tank capacity (kg).
    pub tank_capacity: f64,
    /// The node at the engine's mount.
    pub engine_node: u32,
    /// The nozzle's cells and each one's share of the engine's wall heat
    /// (by area; D077).
    pub nozzle_cells: Vec<(u32, f64)>,
    /// Each cell's temperature limit (K): its material's, else the craft's.
    pub skin_max: Vec<f64>,
    /// The surface cells (normals, areas, emissivities for the heat inputs).
    pub cells: Vec<Cell>,
    /// Subsonic drag coefficient (craft data).
    pub cd0: f64,
    /// The hypersonic continuum drag area Cd·A (m²) averaged over the bake's
    /// flow directions: the attitude-independent drag of coasts and
    /// predictions (a vessel flown live uses the full aerodynamics).
    pub mean_drag_area: f64,
}

impl CraftDesign {
    /// Bakes the design of `craft` (level-3 grid, 64×64 raster).
    pub fn new(craft: &Craft) -> Self {
        let bake = aero::bake(&craft.surface, &craft.cells, &BakeOptions::default());
        let t = &craft.spec.thermal;
        let s = &craft.surface;
        let bounds = s
            .positions
            .iter()
            .fold((glam::DVec3::splat(f64::MAX), glam::DVec3::splat(f64::MIN)), |b, p| (b.0.min(*p), b.1.max(*p)));
        let volume = VolumeGrid::new(&craft.geometry.primitives, bounds, &craft.geometry.tank, t.node_size);
        let cells = &craft.cells.cells;
        let m = &craft.spec.engine.mount;
        let engine_node = volume.nearest(m.pos + m.dir.normalize() * (0.25 * t.node_size));
        let nozzle = |c: &super::Cell| craft.geometry.primitives[c.primitive as usize].nozzle;
        // Each cell couples to the node just beneath it (else the nearest);
        // the nozzle, joined only at its throat, to the mount's (D077).
        let cell_node = (cells.iter())
            .map(|c| if nozzle(c) { engine_node } else { volume.nearest(c.centroid - c.normal * (0.25 * t.node_size)) })
            .collect();
        let k = t.interior_conductivity;
        let links = volume.links.iter().map(|&(a, b, area)| (a, b, k * area / t.node_size)).collect();
        let interior = Interior { nodes: volume.nodes.len(), cell_node, coupling: t.internal_coupling, links };
        let network = ThermalNetwork::new(cells, &interior);
        // The dry mass less the skin, spread by volume.
        let skin: f64 = cells.iter().map(|c| c.skin_mass()).sum();
        let per_m3 = (craft.spec.dry_mass - skin).max(0.0) / volume.volume();
        let dry_capacity = volume.nodes.iter().map(|n| n.volume * per_m3 * t.interior_specific_heat).collect();
        let nozzle_area: f64 = cells.iter().filter(|c| nozzle(c)).map(|c| c.area).sum();
        let nozzle_cells = (cells.iter().enumerate())
            .filter(|(_, c)| nozzle(c))
            .map(|(i, c)| (i as u32, c.area / nozzle_area))
            .collect();
        let skin_max = cells.iter().map(|c| c.max_k.unwrap_or(t.skin_max_k)).collect();
        let cd0 = craft.spec.aero.cd0;
        let mean_drag_area = mean_drag_area(&bake, cd0);
        CraftDesign {
            bake,
            network,
            volume,
            dry_capacity,
            propellant_specific_heat: t.propellant_specific_heat,
            tank_capacity: craft.spec.propellant.capacity,
            engine_node,
            nozzle_cells,
            skin_max,
            cells: cells.clone(),
            cd0,
            mean_drag_area,
        }
    }

    /// Each node's heat capacity (J/K) with `propellant` kg in the tank.
    pub fn node_capacity(&self, propellant: f64, out: &mut [f64]) {
        let fill = if self.tank_capacity > 0.0 { propellant / self.tank_capacity } else { 0.0 };
        self.volume.propellant(propellant, fill, out);
        for (o, dry) in out.iter_mut().zip(&self.dry_capacity) {
            *o = dry + *o * self.propellant_specific_heat;
        }
    }

    pub fn nodes(&self) -> usize {
        self.dry_capacity.len()
    }
}

/// The drag area (force along the flow per unit q) averaged over the
/// grid's directions, hypersonic continuum, Earth air's γ.
pub fn mean_drag_area(bake: &AeroBake, cd0: f64) -> f64 {
    let sum: f64 = (bake.grid.dirs.iter())
        .map(|&d| {
            let flow =
                Flow { dir: d, q: 1.0, mach: MEAN_DRAG_MACH, gamma: aero::air::EARTH_AIR_GAMMA, ..Flow::default() };
            aero::aero_forces(bake, &flow, cd0).0.dot(d)
        })
        .sum();
    sum / bake.grid.dirs.len() as f64
}

/// A vessel's handle on its design: filled at creation, rebuilt after a
/// load. Not serialised; compares equal to any other slot (it is derived
/// from the craft id, which is compared).
#[derive(Clone, Default)]
pub struct DesignSlot(OnceLock<Arc<CraftDesign>>);

impl DesignSlot {
    pub fn filled(design: Arc<CraftDesign>) -> Self {
        let slot = OnceLock::new();
        let _ = slot.set(design);
        DesignSlot(slot)
    }

    /// The design, built by `build` if the slot is empty.
    pub fn get_or_init(&self, build: impl FnOnce() -> Arc<CraftDesign>) -> &Arc<CraftDesign> {
        self.0.get_or_init(build)
    }

    /// The design, built from craft `id`'s files if the slot is empty.
    pub fn get(&self, id: &CraftId) -> &Arc<CraftDesign> {
        self.0.get_or_init(|| design_of(id))
    }
}

impl PartialEq for DesignSlot {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for DesignSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.get().is_some() { "DesignSlot(built)" } else { "DesignSlot(empty)" })
    }
}

/// The design of craft `id` from [`super::default_craft_dir`], built once
/// per process (the test craft's is [`super::test_craft`]'s). Panics if the
/// craft's files are missing or invalid.
pub fn design_of(id: &CraftId) -> Arc<CraftDesign> {
    if id.0 == super::TEST_CRAFT {
        return super::test_craft().design().clone();
    }
    static DESIGNS: OnceLock<Mutex<BTreeMap<String, Arc<CraftDesign>>>> = OnceLock::new();
    let mut designs = DESIGNS.get_or_init(Default::default).lock().expect("design cache");
    designs
        .entry(id.0.clone())
        .or_insert_with(|| {
            let craft = super::load_craft(&super::default_craft_dir().join(&id.0)).unwrap_or_else(|e| panic!("{e}"));
            craft.design().clone()
        })
        .clone()
}
