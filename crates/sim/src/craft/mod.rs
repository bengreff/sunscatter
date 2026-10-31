//! Craft: what a vessel is made of (realism-1 §3, D060, D064, D065).
//!
//! A craft is read from `data/craft/<craft>/craft.ron` (numbers) and
//! `geometry.ron` (shape), as one part for the whole ship, or defined as a
//! set of parts ([`parts`], the stress craft and the pieces of a craft
//! that broke up). This module owns everything derived from those at load.

pub mod assembly;
pub mod broad;
pub mod build;
#[cfg(test)]
pub(crate) mod build_tests;
pub mod cells;
pub mod command;
mod configure;
pub mod design;
pub mod engine;
#[cfg(test)]
mod falcon9_tests;
pub mod file;
pub mod handle;
mod layers;
pub mod mass;
pub mod mesh;
pub mod partition;
pub mod parts;
pub mod place;
pub mod program;
pub mod registry;
pub mod ship;
pub mod small;
pub mod state;
#[cfg(test)]
mod state_tests;
pub mod stress;
pub mod volume;

pub use cells::{Cell, CellOptions, Cells, ContactKind, ContactPoint, Neighbour, CELLS_PER_PART};
pub use design::{design_of, CraftDesign, DesignSlot};
pub use engine::{Engine, EngineOutput};
pub use file::{AeroFile, ContactFile, CraftFile, GeometryFile, Primitive, Shape, Skin, Spring, Tank, ThermalLimits};
pub use mass::{MassModel, MassProps};
pub use mesh::{RenderMesh, Resolution, Surface};
pub use parts::{CraftDef, Part, PartPower};
pub use registry::{craft_of, try_craft_of};
pub use volume::{VolumeGrid, VolumeNode};

use crate::body::DataError;
use std::path::{Path, PathBuf};

/// The file inside a craft directory.
pub const CRAFT_FILE: &str = "craft.ron";

/// Which craft a vessel is: a directory name under `data/craft`, a stress
/// craft or a piece of a craft ([`registry`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CraftId(pub String);

/// A loaded craft: its files and everything derived from them at load.
#[derive(Clone, Debug, PartialEq)]
pub struct Craft {
    pub id: CraftId,
    pub spec: CraftFile,
    pub geometry: GeometryFile,
    /// The outer surface (body axes, f64).
    pub surface: Surface,
    /// What the game draws (the same surface in f32).
    pub mesh: RenderMesh,
    /// Surface cells and contact points (D065).
    pub cells: Cells,
    /// The dry craft's mass properties (its tanks' contents are the ship
    /// state's: `systems::plumbing::contents`).
    pub mass: MassModel,
    /// The parts ([`parts`]; a craft file is one) and each one's dry mass
    /// properties.
    pub parts: Vec<Part>,
    pub part_mass: Vec<MassProps>,
    /// A craft of catalog parts: its ship definition ([`ship`]).
    pub ship: Option<std::sync::Arc<ship::ShipDef>>,
    /// The aerodynamic bake and thermal network, built on first use.
    design: DesignSlot,
}

impl Craft {
    /// Derives the surface, render mesh, cells and mass model of a craft of
    /// parts. Each part's dry mass is a shell over its own primitives'
    /// surface, so a part's mass properties do not depend on what it is
    /// attached to (a piece that breaks off keeps them), with its ship's
    /// components on it as solids.
    pub fn from_def(id: CraftId, def: CraftDef, res: &Resolution, opts: &CellOptions) -> Self {
        let CraftDef { spec, geometry, parts, ship } = def;
        let surface = mesh::union_surface(&geometry.primitives, res);
        let cells = cells::build_cells(&surface, &geometry.primitives, &geometry.skin, opts);
        let g = &geometry;
        // Each part: its own shell, and its components as solids.
        let components = ship.as_ref().map_or(&[][..], |s| &s.components[..]);
        let part_mass: Vec<MassProps> = (parts.iter())
            .map(|p| {
                let own: Vec<Primitive> = p.primitives.iter().map(|&q| g.primitives[q as usize].clone()).collect();
                let hosted: Vec<&ship::ComponentDef> = components.iter().filter(|c| c.part == p.id).collect();
                let shell = p.dry_mass - hosted.iter().map(|c| c.mass).sum::<f64>();
                let mut items = vec![MassModel::shell(&mesh::union_surface(&own, res), &own, &g.skin, shell)];
                items.extend(hosted.iter().map(|c| MassProps::solid(&c.envelope, c.mass, c.pose.rot, c.pose.pos)));
                MassProps::sum(&items)
            })
            .collect();
        let mass = MassModel::of_parts(&part_mass);
        let mesh = surface.render_mesh();
        Craft { id, spec, mesh, surface, cells, geometry, mass, parts, part_mass, ship, design: DesignSlot::default() }
    }

    /// The craft's definition (files and parts).
    pub fn def(&self) -> CraftDef {
        CraftDef {
            spec: self.spec.clone(),
            geometry: self.geometry.clone(),
            parts: self.parts.clone(),
            ship: self.ship.clone(),
        }
    }

    /// The design's aerodynamic bake and thermal network (built once).
    pub fn design(&self) -> &std::sync::Arc<CraftDesign> {
        self.design.get_or_init(|| std::sync::Arc::new(CraftDesign::new(self)))
    }

    /// Mass, centre of mass and inertia as loaded: a craft of parts' tanks
    /// and bottles as its loading fills them (`ShipState::initial`, the
    /// plumbing's contents, as a vessel of it starts), else the dry craft.
    pub fn loaded_props(&self) -> MassProps {
        match &self.ship {
            Some(def) => {
                let (slugs, points) = crate::systems::plumbing::contents(def, &state::ShipState::initial(def));
                self.mass.with_slugs(&slugs, &points)
            }
            None => self.mass.dry(),
        }
    }

    /// The parameters a vessel of this craft carries.
    pub fn params(&self) -> CraftParams {
        let s = &self.surface;
        let design = self.design().clone();
        CraftParams {
            id: self.id.clone(),
            mass: self.mass,
            cd_area: design.mean_drag_area,
            chute: self.spec.chute.clone(),
            thermal: self.spec.thermal,
            impact_max_speed: self.spec.impact.max_speed,
            contact: self.spec.contact,
            contacts: self.cells.contacts.clone(),
            bottom_z: match self.cells.contacts.is_empty() {
                true => s.positions.iter().map(|p| p.z).fold(f64::MAX, f64::min),
                false => self.cells.contacts.iter().map(|c| c.pos.z).fold(f64::MAX, f64::min),
            },
            design: DesignSlot::filled(design),
        }
    }
}

/// What a vessel keeps from its craft: [`Craft::params`] of its id. Saved
/// as the id alone and rebuilt from it at load (D063: a save holds no
/// static data; building is deterministic, so the rebuilt parameters are
/// the same bits).
#[derive(Clone, Debug, PartialEq)]
pub struct CraftParams {
    pub id: CraftId,
    pub mass: MassModel,
    /// Attitude-independent hull drag area Cd·A (m²) of coasts and
    /// predictions: [`CraftDesign::mean_drag_area`]. Live ticks use the
    /// full aerodynamics of the design.
    pub cd_area: f64,
    /// The parachutes.
    pub chute: crate::chute::ChuteSpec,
    /// Temperature limits and the interior's heat capacity and coupling.
    pub thermal: ThermalLimits,
    /// Highest touchdown speed that is not a crash (m/s; D066 refines it
    /// per contact point).
    pub impact_max_speed: f64,
    /// Ground-contact springs and friction (D066).
    pub contact: ContactFile,
    /// The points that touch the ground (body axes): feet, then hull points.
    pub contacts: Vec<ContactPoint>,
    /// Lowest point of the craft along body Z (m): its lowest contact point
    /// (the bottom of the feet; a deployed foot's pad is tilted, its rim
    /// lower than its base), else the surface's.
    pub bottom_z: f64,
    /// The design's bake and thermal network, built on first use.
    pub design: DesignSlot,
}

impl serde::Serialize for CraftParams {
    /// The craft's id: the rest is the craft's ([`Craft::params`]).
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.id.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for CraftParams {
    /// The parameters of the craft the id names (an error if it names none).
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = CraftId::deserialize(d)?;
        registry::try_craft_of(&id).map(|c| c.params()).map_err(serde::de::Error::custom)
    }
}

impl CraftParams {
    /// The design's aerodynamic bake and thermal network.
    pub fn design(&self) -> &std::sync::Arc<CraftDesign> {
        self.design.get(&self.id)
    }

    /// Farthest contact point from the centre of mass `com` (m, body
    /// axes): below this height plus [`crate::contact::NEAR`] the vessel
    /// flies live ticks with contact substeps.
    pub fn contact_reach(&self, com: glam::DVec3) -> f64 {
        self.contacts.iter().map(|c| (c.pos - com).length()).fold(0.0, f64::max)
    }
}

/// The definition of the craft of catalog parts in `dir` (`craft.ron`:
/// [`assembly`], built from [`crate::catalog::catalog`]).
pub fn load_definition(dir: &Path) -> Result<CraftDef, DataError> {
    load_assembly(dir).map(|a| a.def)
}

/// Builds the craft of catalog parts in `dir`; every build error is listed.
pub fn load_assembly(dir: &Path) -> Result<build::Assembly, DataError> {
    let path = dir.join(CRAFT_FILE);
    let err = |message: String| DataError { path: path.clone(), message };
    let text = std::fs::read_to_string(&path).map_err(|e| err(e.to_string()))?;
    let file: assembly::AssemblyFile = crate::catalog::ron_options().from_str(&text).map_err(|e| err(e.to_string()))?;
    build::build(&file, crate::catalog::catalog(), &crate::catalog::Ctx::data()).map_err(|e| err(e.join("\n")))
}

/// `data/craft` of this source tree (development builds and tests).
pub fn default_craft_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft")
}
