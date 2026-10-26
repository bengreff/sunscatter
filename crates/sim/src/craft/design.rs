//! What is derived once per craft design and shared by every vessel of it
//! (realism-1 §5): the aerodynamic bake (`sim::aero`), the interior volume
//! grid (`super::volume`) and the thermal network over its cells and nodes
//! (`sim::thermal`, D065 revised).
//!
//! Never saved: a vessel's [`super::CraftParams`] holds a [`DesignSlot`]
//! that is filled by [`super::Craft::params`] and, after loading a save,
//! rebuilt from the craft's id on first use ([`design_of`],
//! [`super::registry`]). The bake is deterministic, so a rebuilt design is
//! the same bits.

use super::parts::{CraftDef, Part};
use super::{Cell, Craft, CraftId, MassProps, VolumeGrid};
use crate::aero::{self, AeroBake, BakeOptions, Flow};
use crate::thermal::{Interior, ThermalNetwork};
use std::sync::{Arc, OnceLock};

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
    /// Areas per direction for attitude-dependent drag and radiation
    /// pressure on rails (`docs/features/limbo.md`).
    pub surfaces: crate::forces::SurfaceTable,
    /// The parts ([`super::parts`]) and each one's dry mass properties.
    pub parts: Vec<Part>,
    pub part_mass: Vec<MassProps>,
    /// The part of each cell and of each interior node (the part filling
    /// most of it).
    pub cell_part: Vec<u32>,
    pub node_part: Vec<u32>,
    /// Each node's temperature limit (K): its part's, else the craft's.
    pub node_max: Vec<f64>,
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
        let def = CraftDef { spec: craft.spec.clone(), geometry: craft.geometry.clone(), parts: craft.parts.clone() };
        let owner = def.primitive_part();
        let volume = match craft.parts.len() {
            1 => VolumeGrid::new(&craft.geometry.primitives, bounds, &craft.geometry.tank, t.node_size),
            _ => VolumeGrid::with_parts(&craft.geometry.primitives, &owner, bounds, &def.tanks(), t.node_size),
        };
        let cells = &craft.cells.cells;
        let cell_part: Vec<u32> = cells.iter().map(|c| owner[c.primitive as usize]).collect();
        // Each engine part's mount node; the craft's engine (one part: the
        // file's) for a nozzle without an engine of its own.
        let mount_node = |m: &super::file::Mount| volume.nearest(m.pos + m.dir.normalize() * (0.25 * t.node_size));
        let engine_node = mount_node(&craft.spec.engine.mount);
        let engines = engines(craft);
        let nozzle_of = |c: &super::Cell| {
            let part = owner[c.primitive as usize] as usize;
            let own = craft.parts[part].engine.filter(|_| craft.parts.len() > 1);
            craft.geometry.primitives[c.primitive as usize]
                .nozzle
                .then(|| own.map_or(engine_node, |e| mount_node(&e.mount)))
        };
        // Each cell couples to the node just beneath it (else the nearest);
        // a nozzle, joined only at its throat, to its engine's mount (D077).
        let cell_node = (cells.iter())
            .map(|c| nozzle_of(c).unwrap_or_else(|| volume.nearest(c.centroid - c.normal * (0.25 * t.node_size))))
            .collect();
        let k = t.interior_conductivity;
        let links = volume.links.iter().map(|&(a, b, area)| (a, b, k * area / t.node_size)).collect();
        let interior = Interior { nodes: volume.nodes.len(), cell_node, coupling: t.internal_coupling, links };
        let network = ThermalNetwork::new(cells, &interior);
        let dry_capacity = dry_capacity(craft, &volume, &cell_part, &interior.cell_node);
        // The engine's wall heat into its nozzle's cells by area; several
        // engines' nozzles each take their engine's share (D077).
        let nozzle = |c: &super::Cell| craft.geometry.primitives[c.primitive as usize].nozzle;
        let share = |part: u32| engines.iter().find(|e| e.0 == part).map_or(0.0, |e| e.1);
        let nozzle_area = |part: u32| {
            (cells.iter().zip(&cell_part))
                .filter(|(c, &p)| nozzle(c) && (engines.len() < 2 || p == part))
                .map(|(c, _)| c.area)
                .sum::<f64>()
        };
        let nozzle_cells = (cells.iter().enumerate())
            .filter(|(_, c)| nozzle(c))
            .map(|(i, c)| match engines.len() {
                0 | 1 => (i as u32, c.area / nozzle_area(0)),
                _ => (i as u32, share(cell_part[i]) * c.area / nozzle_area(cell_part[i])),
            })
            .collect();
        let skin_max = cells.iter().map(|c| c.max_k.unwrap_or(t.skin_max_k)).collect();
        let node_part: Vec<u32> = volume.nodes.iter().map(|n| n.part()).collect();
        let node_max =
            node_part.iter().map(|&p| craft.parts[p as usize].node_max_k.unwrap_or(t.internal_max_k)).collect();
        let cd0 = craft.spec.aero.cd0;
        let mean_drag_area = mean_drag_area(&bake, cd0);
        let surfaces = crate::forces::SurfaceTable::new(&bake, cd0);
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
            surfaces,
            parts: craft.parts.clone(),
            part_mass: craft.part_mass.clone(),
            cell_part,
            node_part,
            node_max,
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

/// The engine parts of a craft of several parts and each one's share of
/// the combined engine's wall heat (f·F·Isp, D077); none for one part.
fn engines(craft: &Craft) -> Vec<(u32, f64)> {
    if craft.parts.len() < 2 {
        return Vec::new();
    }
    let heat = |e: &super::file::EngineFile| e.nozzle_heat_fraction * e.thrust_vac * e.isp_vac;
    let all: Vec<(u32, f64)> =
        (craft.parts.iter().enumerate()).filter_map(|(i, p)| p.engine.map(|e| (i as u32, heat(&e)))).collect();
    let total: f64 = all.iter().map(|e| e.1).sum();
    all.into_iter().map(|(i, h)| (i, if total > 0.0 { h / total } else { 0.0 })).collect()
}

/// Each node's heat capacity without propellant (J/K): each part's dry
/// mass less its skin, spread over the volume it fills, at its interior's
/// specific heat (one part: the craft's dry mass over the whole volume). A
/// part too thin for the grid's samples (a fin, a panel) puts its interior
/// in the nodes beneath its cells (`cell_node`), by area.
fn dry_capacity(craft: &Craft, volume: &VolumeGrid, cell_part: &[u32], cell_node: &[u32]) -> Vec<f64> {
    let t = &craft.spec.thermal;
    let cells = &craft.cells.cells;
    if craft.parts.len() == 1 {
        let skin: f64 = cells.iter().map(|c| c.skin_mass()).sum();
        let per_m3 = (craft.spec.dry_mass - skin).max(0.0) / volume.volume();
        let c = craft.parts[0].interior_specific_heat.unwrap_or(t.interior_specific_heat);
        return volume.nodes.iter().map(|n| n.volume * per_m3 * c).collect();
    }
    let n = craft.parts.len();
    let (mut skin, mut space) = (vec![0.0; n], vec![0.0; n]);
    for (c, &p) in cells.iter().zip(cell_part) {
        skin[p as usize] += c.skin_mass();
    }
    for node in &volume.nodes {
        for &(p, count) in &node.parts {
            space[p as usize] += count as f64 * volume.sample_volume;
        }
    }
    let per_k: Vec<f64> = (craft.parts.iter().enumerate())
        .map(|(i, p)| {
            let c = p.interior_specific_heat.unwrap_or(t.interior_specific_heat);
            if space[i] > 0.0 {
                (p.dry_mass - skin[i]).max(0.0) / space[i] * c
            } else {
                0.0
            }
        })
        .collect();
    let mut out: Vec<f64> = (volume.nodes.iter())
        .map(|node| node.parts.iter().map(|&(p, count)| count as f64 * volume.sample_volume * per_k[p as usize]).sum())
        .collect();
    let mut area = vec![0.0; n];
    for (c, &p) in cells.iter().zip(cell_part) {
        area[p as usize] += c.area;
    }
    for ((c, &p), &j) in cells.iter().zip(cell_part).zip(cell_node) {
        let part = &craft.parts[p as usize];
        if space[p as usize] == 0.0 && area[p as usize] > 0.0 {
            let heat = part.interior_specific_heat.unwrap_or(t.interior_specific_heat);
            out[j as usize] += (part.dry_mass - skin[p as usize]).max(0.0) * heat * c.area / area[p as usize];
        }
    }
    out
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

/// The design of craft `id` ([`super::registry`]: a craft file, a stress
/// craft, a piece of one), built once per process (the test craft's is
/// [`super::test_craft`]'s). Panics if `id` cannot be resolved.
pub fn design_of(id: &CraftId) -> Arc<CraftDesign> {
    if id.0 == super::TEST_CRAFT {
        return super::test_craft().design().clone();
    }
    super::registry::craft_of(id).design().clone()
}
