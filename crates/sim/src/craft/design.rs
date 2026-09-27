//! What is derived once per craft design and shared by every vessel of it
//! (realism-1 §5): the aerodynamic bake (`sim::aero`) and the thermal
//! network (`sim::thermal`).
//!
//! Never saved: a vessel's [`super::CraftParams`] holds a [`DesignSlot`]
//! that is filled by [`super::Craft::params`] and, after loading a save,
//! rebuilt from the craft's files on first use ([`design_of`]). The bake is
//! deterministic, so a rebuilt design is the same bits.

use super::{Cell, Craft, CraftId};
use crate::aero::{self, AeroBake, BakeOptions, Flow};
use crate::thermal::{InternalNode, ThermalNetwork};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Mach number of the orientation-averaged drag area (hypersonic).
const MEAN_DRAG_MACH: f64 = 10.0;

/// A craft design's aerodynamic and thermal models.
#[derive(Debug, PartialEq)]
pub struct CraftDesign {
    pub bake: AeroBake,
    pub network: ThermalNetwork,
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
        let internal = InternalNode { capacity: t.internal_capacity, coefficient: t.internal_coupling };
        let network = ThermalNetwork::new(&craft.cells.cells, internal);
        let cd0 = craft.spec.aero.cd0;
        let mean_drag_area = mean_drag_area(&bake, cd0);
        CraftDesign { bake, network, cells: craft.cells.cells.clone(), cd0, mean_drag_area }
    }
}

/// The drag area (force along the flow per unit q) averaged over the
/// grid's directions, hypersonic continuum, Earth air's γ.
pub fn mean_drag_area(bake: &AeroBake, cd0: f64) -> f64 {
    let sum: f64 = (bake.grid.dirs.iter())
        .map(|&d| {
            let flow = Flow { dir: d, q: 1.0, mach: MEAN_DRAG_MACH, knudsen: 0.0, gamma: aero::air::EARTH_AIR_GAMMA };
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
