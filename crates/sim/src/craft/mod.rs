//! Craft: what a vessel is made of (realism-1 §3, D060, D064, D065).
//!
//! A craft is one part for the whole ship, read from
//! `data/craft/<craft>/craft.ron` (numbers) and `geometry.ron` (shape). This
//! module owns everything derived from those files at load.

pub mod cells;
pub mod design;
pub mod engine;
pub mod file;
pub mod mass;
pub mod mesh;
pub mod volume;

pub use cells::{Cell, CellOptions, Cells, ContactKind, ContactPoint, Neighbour};
pub use design::{design_of, CraftDesign, DesignSlot};
pub use engine::{Engine, EngineOutput};
pub use file::{AeroFile, ContactFile, CraftFile, GeometryFile, Primitive, Shape, Skin, Spring, Tank, ThermalLimits};
pub use mass::{MassModel, MassProps};
pub use mesh::{RenderMesh, Resolution, Surface};
pub use volume::{VolumeGrid, VolumeNode};

use crate::body::DataError;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// File names inside a craft directory.
pub const CRAFT_FILE: &str = "craft.ron";
pub const GEOMETRY_FILE: &str = "geometry.ron";
/// The craft every vessel is for now (D060).
pub const TEST_CRAFT: &str = "test-craft";

/// Which craft a vessel is: its directory name under `data/craft`.
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
    /// Dry shell and tank, for mass properties at any fill.
    pub mass: MassModel,
    /// The aerodynamic bake and thermal network, built on first use.
    design: DesignSlot,
}

impl Craft {
    /// Derives the surface, render mesh and cells from the files.
    pub fn build(id: CraftId, spec: CraftFile, geometry: GeometryFile, res: &Resolution, opts: &CellOptions) -> Self {
        let surface = mesh::union_surface(&geometry.primitives, res);
        let cells = cells::build_cells(&surface, &geometry.primitives, &geometry.skin, opts);
        let g = &geometry;
        let mass = MassModel::new(&surface, &g.primitives, &g.skin, spec.dry_mass, &g.tank, spec.propellant.capacity);
        Craft { id, spec, mesh: surface.render_mesh(), surface, cells, geometry, mass, design: DesignSlot::default() }
    }

    /// The design's aerodynamic bake and thermal network (built once).
    pub fn design(&self) -> &std::sync::Arc<CraftDesign> {
        self.design.get_or_init(|| std::sync::Arc::new(CraftDesign::new(self)))
    }

    /// The parameters a vessel of this craft carries.
    pub fn params(&self) -> CraftParams {
        let s = &self.surface;
        let design = self.design().clone();
        CraftParams {
            id: self.id.clone(),
            engine: Engine::from_file(&self.spec.engine),
            mass: self.mass,
            torque: self.spec.attitude_control.rcs.torque(),
            rcs_flow: self.spec.attitude_control.rcs.flow(),
            cd_area: design.mean_drag_area,
            chute_cd_area: self.spec.chute.cd_area,
            chute_mount: self.spec.chute.mount,
            thermal: self.spec.thermal,
            impact_max_speed: self.spec.impact.max_speed,
            contact: self.spec.contact,
            contacts: self.cells.contacts.clone(),
            bottom_z: s.positions.iter().map(|p| p.z).fold(f64::MAX, f64::min),
            initial_propellant: self.spec.propellant.mass,
            design: DesignSlot::filled(design),
        }
    }
}

/// What a vessel keeps from its craft (saved with it, so a vessel is
/// self-contained and deterministic).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CraftParams {
    pub id: CraftId,
    pub engine: Engine,
    pub mass: MassModel,
    /// Attitude-control torque authority per body axis (N·m): the RCS
    /// couples.
    pub torque: glam::DVec3,
    /// RCS propellant flow per body axis at full command (kg/s).
    pub rcs_flow: glam::DVec3,
    /// Attitude-independent hull drag area Cd·A (m²) of coasts and
    /// predictions: [`CraftDesign::mean_drag_area`]. Live ticks use the
    /// full aerodynamics of the design.
    pub cd_area: f64,
    /// Parachute Cd·A when deployed (m²).
    pub chute_cd_area: f64,
    /// Where the parachute pulls (body axes, m).
    pub chute_mount: glam::DVec3,
    /// Temperature limits and the interior's heat capacity and coupling.
    pub thermal: ThermalLimits,
    /// Highest touchdown speed that is not a crash (m/s; D066 refines it
    /// per contact point).
    pub impact_max_speed: f64,
    /// Ground-contact springs and friction (D066).
    pub contact: ContactFile,
    /// The points that touch the ground (body axes): feet, then hull points.
    pub contacts: Vec<ContactPoint>,
    /// Lowest point of the craft along body Z (m): the bottom of the feet.
    pub bottom_z: f64,
    /// Propellant loaded at the start (kg).
    pub initial_propellant: f64,
    /// The design's bake and thermal network: never saved, rebuilt from
    /// the craft's files after a load.
    #[serde(skip)]
    pub design: DesignSlot,
}

impl CraftParams {
    /// The design's aerodynamic bake and thermal network.
    pub fn design(&self) -> &std::sync::Arc<CraftDesign> {
        self.design.get(&self.id)
    }

    /// Height of the centre of mass above the ground when standing (m).
    pub fn contact_height(&self, propellant: f64) -> f64 {
        self.mass.at(propellant).com.z - self.bottom_z
    }

    /// Farthest contact point from the centre of mass (m): below this
    /// height plus [`crate::contact::NEAR`] the vessel flies live ticks
    /// with contact substeps.
    pub fn contact_reach(&self, propellant: f64) -> f64 {
        let com = self.mass.at(propellant).com;
        self.contacts.iter().map(|c| (c.pos - com).length()).fold(0.0, f64::max)
    }
}

/// Parses and validates a craft from the two files' text. `dir` labels errors.
pub fn parse_craft(id: CraftId, craft_text: &str, geometry_text: &str, dir: &Path) -> Result<Craft, DataError> {
    let err = |file: &str, message: String| DataError { path: dir.join(file), message };
    let spec: CraftFile = ron::from_str(craft_text).map_err(|e| err(CRAFT_FILE, e.to_string()))?;
    file::validate_craft(&spec).map_err(|m| err(CRAFT_FILE, m))?;
    let geometry: GeometryFile = ron::from_str(geometry_text).map_err(|e| err(GEOMETRY_FILE, e.to_string()))?;
    file::validate_geometry(&geometry).map_err(|m| err(GEOMETRY_FILE, m))?;
    Ok(Craft::build(id, spec, geometry, &Resolution::default(), &CellOptions::default()))
}

/// Loads `<dir>/craft.ron` and `<dir>/geometry.ron`; the id is the directory name.
pub fn load_craft(dir: &Path) -> Result<Craft, DataError> {
    let read = |file: &str| {
        let path = dir.join(file);
        std::fs::read_to_string(&path).map_err(|e| DataError { path, message: e.to_string() })
    };
    let id = CraftId(dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
    parse_craft(id, &read(CRAFT_FILE)?, &read(GEOMETRY_FILE)?, dir)
}

/// `data/craft` of this source tree (development builds and tests).
pub fn default_craft_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft")
}

/// The test craft from [`default_craft_dir`], loaded once per process.
/// Panics if its files are missing or invalid.
pub fn test_craft() -> &'static Craft {
    static CRAFT: OnceLock<Craft> = OnceLock::new();
    CRAFT.get_or_init(|| load_craft(&default_craft_dir().join(TEST_CRAFT)).unwrap_or_else(|e| panic!("{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> (String, String) {
        let dir = default_craft_dir().join(TEST_CRAFT);
        (
            std::fs::read_to_string(dir.join(CRAFT_FILE)).unwrap(),
            std::fs::read_to_string(dir.join(GEOMETRY_FILE)).unwrap(),
        )
    }

    fn parse(craft: &str, geometry: &str) -> Result<Craft, DataError> {
        parse_craft(CraftId("t".into()), craft, geometry, Path::new("t"))
    }

    #[test]
    fn the_test_craft_loads_with_the_planned_numbers() {
        let c = test_craft();
        assert_eq!(c.id, CraftId(TEST_CRAFT.into()));
        let s = &c.spec;
        assert_eq!((s.crew, s.dry_mass, s.propellant.mass, s.propellant.capacity), (3, 4000.0, 16000.0, 16000.0));
        assert_eq!((s.engine.thrust_vac, s.engine.isp_vac, s.engine.isp_sl), (300e3, 320.0, 280.0));
        assert_eq!((s.engine.min_throttle, s.engine.gimbal_deg), (0.1, 5.0));
        let rcs = &s.attitude_control.rcs;
        assert_eq!((rcs.thrust, rcs.isp, rcs.fore_z, rcs.aft_z, rcs.radius), (445.0, 290.0, 3.6, -5.4, 1.04));
        assert_eq!((s.chute.cd_area, s.thermal.skin_max_k, s.thermal.internal_max_k), (600.0, 1100.0, 400.0));
        assert_eq!((s.aero.cd0, s.thermal.node_size, s.thermal.internal_coupling), (0.45, 1.0, 2.0));
        assert_eq!((s.impact.max_speed, s.antenna.gain_dbi, s.antenna.power_w), (8.0, 20.0, 20.0));
        assert_eq!(c.geometry.primitives.iter().filter(|p| p.foot).count(), 4);
        let p = c.params();
        assert_eq!(p.bottom_z, -7.7);
        // The mean hypersonic drag area: Cp,max/2 ≈ 0.92 on the mean
        // projected area, which lies between the end-on silhouette (the
        // skirt's disc π·1.5², the fins beyond it and the feet) and the
        // side view (~14.5 m × 2 m plus the fins, skirt and legs, < 45 m²).
        let silhouette = std::f64::consts::PI * (2.25 + 4.0 * 0.09) + 4.0 * 1.3 * 0.08;
        assert!(p.cd_area > 0.92 * silhouette && p.cd_area < 0.92 * 45.0, "{} vs {silhouette}", p.cd_area);
        // The tank is forward (a static margin, D074): full, the CoM is higher.
        assert!(p.contact_height(0.0) > 7.0 && p.contact_height(16000.0) > p.contact_height(0.0));
    }

    #[test]
    fn invalid_files_are_rejected_with_the_file_named() {
        let (craft, geometry) = texts();
        // (edit of craft.ron, edit of geometry.ron, expected file, expected words)
        let cases = [
            (("dry_mass: 4000.0", "dry_mass: -1.0"), ("", ""), CRAFT_FILE, "dry_mass"),
            (("mass: 16000.0, capacity", "mass: 17000.0, capacity"), ("", ""), CRAFT_FILE, "propellant.mass"),
            (("isp_sl: 280.0", "isp_sl: 330.0"), ("", ""), CRAFT_FILE, "isp_sl"),
            (("min_throttle: 0.1", "min_throttle: 0.0"), ("", ""), CRAFT_FILE, "min_throttle"),
            (("dir: (0.0, 0.0, 1.0)", "dir: (0.0, 0.0, 0.0)"), ("", ""), CRAFT_FILE, "mount.dir"),
            (("crew: 3,", "crew: 3, wings: 2,"), ("", ""), CRAFT_FILE, "wings"),
            (("cd0: 0.45", "cd0: 0.0"), ("", ""), CRAFT_FILE, "aero.cd0"),
            (("node_size: 1.0", "node_size: 0.0"), ("", ""), CRAFT_FILE, "node_size"),
            (
                ("nozzle_heat_fraction: 2.2e-3", "nozzle_heat_fraction: 1.5"),
                ("", ""),
                CRAFT_FILE,
                "nozzle_heat_fraction",
            ),
            (("", ""), ("radius: 0.95", "radius: 0.0"), GEOMETRY_FILE, "tank.radius"),
            (("", ""), ("emissivity: 0.8", "emissivity: 1.5"), GEOMETRY_FILE, "emissivity"),
            (("", ""), ("height: 0.604", "height: 2.0"), GEOMETRY_FILE, "height"),
        ];
        for ((c_from, c_to), (g_from, g_to), file, words) in cases {
            let c = if c_from.is_empty() { craft.clone() } else { craft.replacen(c_from, c_to, 1) };
            let g = if g_from.is_empty() { geometry.clone() } else { geometry.replacen(g_from, g_to, 1) };
            assert!(c != craft || g != geometry, "edit {c_from:?}/{g_from:?} did not apply");
            let e = parse(&c, &g).expect_err(words);
            assert!(e.path.ends_with(file) && e.message.contains(words), "{words}: {e}");
        }
        assert!(parse(&craft, &geometry).is_ok());
    }
}
