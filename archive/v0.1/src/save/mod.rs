// On wasm the std::fs paths are unreachable (replaced with IndexedDB via the
// `wasm_storage` submodule); silence the resulting dead-code chorus.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::colony::{ColonyManager, Company, ContractManager, DysonSwarm, FleetManager, Notification, ScienceState};
use crate::game::{Game, VesselId, TrackedVessel};
use crate::parts::{FlightVessel, VesselBlueprint};
use crate::render::ManeuverNode;
use crate::ship::Ship;

#[cfg(target_arch = "wasm32")]
pub mod wasm_storage;

#[cfg(target_arch = "wasm32")]
pub mod wasm_io;

#[cfg(not(target_arch = "wasm32"))]
pub mod paths;

pub mod test_fixture;

#[cfg(not(target_arch = "wasm32"))]
pub mod native_io;

/// Platform-agnostic re-export of the file import/export helpers. Both
/// platforms expose `import_save()`, `export_save(save_id)`,
/// `import_blueprint()`, `export_blueprint(name)`.
#[cfg(target_arch = "wasm32")]
pub use wasm_io as io;
#[cfg(not(target_arch = "wasm32"))]
pub use native_io as io;

/// Deserialize ContractManager with fallback to default for old save formats.
/// Old saves have Contract { contract_type, objective_met } which is incompatible
/// with the new Contract { id, kind, payout, name } format.
fn deserialize_contracts_compat<'de, D>(deserializer: D) -> Result<ContractManager, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Try to deserialize the new format; if it fails, return default
    let value = <ron::Value as Deserialize>::deserialize(deserializer)?;
    match ContractManager::deserialize(value) {
        Ok(cm) => Ok(cm),
        Err(_) => {
            log::warn!("Old contract format detected in save — starting with fresh contracts");
            Ok(ContractManager::default())
        }
    }
}
const SAVE_VERSION: u32 = 1;

/// How many quicksaves to keep per save game. After every quicksave, older
/// ones beyond this cap are deleted (oldest by index first). Mirrored on
/// desktop (filesystem) and wasm (IndexedDB).
pub const MAX_QUICKSAVES_PER_SAVE: usize = 10;

/// Write a file atomically: write to a temp file in the same directory, then rename.
/// Rename is atomic on all platforms, so a crash mid-write can never corrupt the target.
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let tmp_path = path.with_extension("ron.tmp");
    fs::write(&tmp_path, content)
        .map_err(|e| format!("Failed to write temp file {:?}: {}", tmp_path, e))?;
    fs::rename(&tmp_path, path)
        .map_err(|e| format!("Failed to rename {:?} -> {:?}: {}", tmp_path, path, e))?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct SaveGame {
    pub version: u32,
    pub name: String,
    pub simulation_time: f64,
    pub vessels: Vec<SavedVessel>,
    pub next_vessel_id: VesselId,
    pub debris_counter: u32,
    pub blueprints: Vec<VesselBlueprint>,
    pub editor_vessel_name: String,
    // Colony system state (all default for backward compat with old saves)
    #[serde(default)]
    pub colonies: ColonyManager,
    #[serde(default)]
    pub company: Company,
    #[serde(default)]
    pub science: ScienceState,
    #[serde(default)]
    pub tech_unlocked: HashSet<String>,
    #[serde(default)]
    pub tech_line_tiers: HashMap<String, u32>,
    #[serde(default)]
    pub notifications: Vec<Notification>,
    #[serde(default, deserialize_with = "deserialize_contracts_compat")]
    pub contracts: ContractManager,
    #[serde(default)]
    pub fleet: FleetManager,
    #[serde(default)]
    pub dyson_swarm: DysonSwarm,
    #[serde(default)]
    pub dyson_swarms: HashMap<usize, DysonSwarm>,
    /// Editor blueprint at time of save (used by "Revert to Editor")
    #[serde(default)]
    pub editor_blueprint: Option<VesselBlueprint>,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct SavedVessel {
    pub id: VesselId,
    pub name: String,
    pub ship: Ship,
    pub vessel: Option<FlightVessel>,
    pub maneuver_nodes: Vec<ManeuverNode>,
    #[serde(default)]
    pub is_debris: bool,
    #[serde(default)]
    pub interstellar_star_key: Option<(u16, u16, u32)>,
}

/// Metadata about a save file (for the load game UI)
#[derive(Debug, Clone)]
pub struct SaveFileInfo {
    pub name: String,
    pub save_id: String,
    pub vessel_count: usize,
    pub simulation_time: f64,
    pub modified: std::time::SystemTime,
}

/// Metadata about a quicksave file
pub struct QuicksaveInfo {
    pub filename: String,
    pub index: u32,
    pub simulation_time: f64,
    pub modified: std::time::SystemTime,
}

impl Default for SaveGame {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            name: String::new(),
            simulation_time: 0.0,
            vessels: Vec::new(),
            next_vessel_id: 1,
            debris_counter: 0,
            blueprints: Vec::new(),
            editor_vessel_name: String::new(),
            colonies: ColonyManager::default(),
            company: Company::default(),
            science: ScienceState::default(),
            tech_unlocked: HashSet::new(),
            tech_line_tiers: HashMap::new(),
            notifications: Vec::new(),
            contracts: ContractManager::default(),
            fleet: FleetManager::default(),
            dyson_swarm: DysonSwarm::default(),
            dyson_swarms: HashMap::new(),
            editor_blueprint: None,
        }
    }
}

impl Default for SavedVessel {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            ship: Ship::default(),
            vessel: None,
            maneuver_nodes: Vec::new(),
            is_debris: false,
            interstellar_star_key: None,
        }
    }
}

impl SaveGame {
    /// Snapshot the current game state into a SaveGame.
    /// The active vessel is put on-rails before saving.
    pub fn from_game(
        game: &Game,
        save_name: &str,
    ) -> Self {
        let mut vessels = Vec::new();

        // Save the active vessel (put on rails for consistent state)
        let mut active_ship = game.flight.ship.clone();
        active_ship.enter_rails_mode(&game.solar_system);

        vessels.push(SavedVessel {
            id: game.flight.active_vessel_id,
            name: game.flight.active_vessel_name.clone(),
            ship: active_ship.clone(),
            vessel: game.flight.vessel.clone(),
            maneuver_nodes: game.flight.active_maneuver_nodes.clone(),
            is_debris: false,
            interstellar_star_key: game.solar_system.dynamic_star_keys.get(&active_ship.soi_body).copied(),
        });

        // Save all inactive vessels
        for tracked in &game.flight.inactive_vessels {
            vessels.push(SavedVessel {
                id: tracked.id,
                name: tracked.name.clone(),
                ship: tracked.ship.clone(),
                vessel: tracked.vessel.clone(),
                maneuver_nodes: tracked.maneuver_nodes.clone(),
                is_debris: tracked.is_debris,
                interstellar_star_key: game.solar_system.dynamic_star_keys.get(&tracked.ship.soi_body).copied(),
            });
        }

        // Collect blueprints
        let blueprints: Vec<VesselBlueprint> = game.blueprints
            .all_blueprints()
            .into_iter()
            .cloned()
            .collect();

        SaveGame {
            version: SAVE_VERSION,
            name: save_name.to_string(),
            simulation_time: game.time(),
            vessels,
            next_vessel_id: game.flight.next_vessel_id,
            debris_counter: game.flight.debris_counter,
            blueprints,
            editor_vessel_name: game.editor.vessel_name.clone(),
            colonies: game.colony_manager.clone(),
            company: game.company.clone(),
            science: game.science.clone(),
            tech_unlocked: game.tech_tree.unlocked.clone(),
            tech_line_tiers: game.tech_tree.line_tiers.clone(),
            notifications: game.notifications.clone(),
            contracts: game.contracts.clone(),
            fleet: game.fleet.clone(),
            dyson_swarm: DysonSwarm::default(),
            dyson_swarms: game.dyson_swarms.clone(),
            editor_blueprint: game.editor.to_blueprint(&game.part_definitions).ok(),
        }
    }

    /// Serialize this save game as a pretty-printed RON string.
    /// Shared by the production write paths and by tests that need to write
    /// to an explicit directory.
    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("Failed to serialize save game: {}", e))
    }

    /// Test-only escape hatch: write `<dir>/<sanitized_name>/save.ron` to
    /// an explicit directory. Used by fixture-generating tests so they
    /// don't pollute the player's real save dir.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write_to_path(&self, dir: &Path) -> Result<std::path::PathBuf, String> {
        let save_id = sanitize_save_name(&self.name);
        let content = self.to_ron()?;
        let target_dir = dir.join(&save_id);
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("create {:?}: {}", target_dir, e))?;
        let path = target_dir.join("save.ron");
        atomic_write(&path, &content)?;
        Ok(path)
    }

    /// Write this save game. On desktop, writes `<saves_dir>/{name}/save.ron`;
    /// on wasm, writes to the IndexedDB-backed `save:{id}` key.
    pub fn write_to_file(&self) -> Result<(), String> {
        let save_id = sanitize_save_name(&self.name);
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("Failed to serialize save game: {}", e))?;

        #[cfg(target_arch = "wasm32")]
        {
            wasm_storage::write_save(&save_id, &self.name, content, self.simulation_time, self.vessels.len());
            log::info!("Saved game '{}' to IndexedDB", self.name);
            return Ok(());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = paths::saves_dir().join(&save_id);
            if !dir.exists() {
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("Failed to create save directory: {}", e))?;
            }
            let path = dir.join("save.ron");
            atomic_write(&path, &content)?;
            log::info!("Saved game '{}' to {:?}", self.name, path);
            Ok(())
        }
    }

    /// Write a quicksave. On desktop saves to `data/saves/{name}/quicksave_{N}.ron`;
    /// on wasm the quicksave is appended to IndexedDB. Returns the quicksave index.
    pub fn write_quicksave(&self) -> Result<u32, String> {
        let save_id = sanitize_save_name(&self.name);
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("Failed to serialize quicksave: {}", e))?;

        #[cfg(target_arch = "wasm32")]
        {
            let index = wasm_storage::write_quicksave(&save_id, &self.name, content, self.simulation_time);
            log::info!("Quicksaved '{}' as quicksave_{} (IndexedDB)", self.name, index);
            return Ok(index);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = paths::saves_dir().join(&save_id);
            if !dir.exists() {
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("Failed to create save directory: {}", e))?;
            }
            let next_index = next_quicksave_index(&dir);
            let path = dir.join(format!("quicksave_{}.ron", next_index));
            atomic_write(&path, &content)?;
            prune_old_quicksaves(&dir);
            log::info!("Quicksaved '{}' as quicksave_{}", self.name, next_index);
            Ok(next_index)
        }
    }

    /// Persist a launch save (overwrites any existing one).
    pub fn write_launch_save(&self) -> Result<(), String> {
        let save_id = sanitize_save_name(&self.name);
        let content = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("Failed to serialize launch save: {}", e))?;

        #[cfg(target_arch = "wasm32")]
        {
            wasm_storage::write_launch(&save_id, &self.name, content, self.simulation_time, self.vessels.len());
            log::info!("Saved launch state for '{}' to IndexedDB", self.name);
            return Ok(());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = paths::saves_dir().join(&save_id);
            if !dir.exists() {
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("Failed to create save directory: {}", e))?;
            }
            let path = dir.join("launch.ron");
            atomic_write(&path, &content)?;
            log::info!("Saved launch state for '{}'", self.name);
            Ok(())
        }
    }

    /// Load a launch save (filesystem on desktop, IndexedDB on wasm).
    pub fn load_launch_save(save_name: &str) -> Result<Self, String> {
        let save_id = sanitize_save_name(save_name);

        #[cfg(target_arch = "wasm32")]
        let content = wasm_storage::load_launch_content(&save_id)
            .ok_or_else(|| format!("Launch save '{}' not found", save_id))?;

        #[cfg(not(target_arch = "wasm32"))]
        let content = {
            let path = paths::saves_dir().join(&save_id).join("launch.ron");
            fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read launch save: {}", e))?
        };

        let save: SaveGame = ron::from_str(&content)
            .map_err(|e| format!("Failed to parse launch save: {}", e))?;

        if save.version > SAVE_VERSION {
            return Err(format!(
                "Save version too new: max supported {}, got {}",
                SAVE_VERSION, save.version
            ));
        }

        log::info!("Loaded launch save for '{}'", save.name);
        Ok(save)
    }

    /// Load a save game by id. On desktop reads `<saves_dir>/{save_id}/save.ron`
    /// (with legacy flat-file fallback); on wasm reads from IndexedDB.
    ///
    /// The synthetic ID `test_fixture::FIXTURE_ID` short-circuits both paths
    /// and returns a freshly-built fixture save.
    pub fn load_from_file(save_id: &str) -> Result<Self, String> {
        if save_id == test_fixture::FIXTURE_ID {
            return Ok(test_fixture::build());
        }

        #[cfg(target_arch = "wasm32")]
        let content = wasm_storage::load_save_content(save_id)
            .ok_or_else(|| format!("Save '{}' not found", save_id))?;

        #[cfg(not(target_arch = "wasm32"))]
        let content = {
            let folder_path = paths::saves_dir().join(save_id).join("save.ron");
            let path = if folder_path.exists() {
                folder_path
            } else {
                let legacy_path = paths::saves_dir().join(format!("{}.ron", save_id));
                if legacy_path.exists() {
                    legacy_path
                } else {
                    return Err(format!("Save '{}' not found", save_id));
                }
            };
            fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read save file: {}", e))?
        };

        let save: SaveGame = ron::from_str(&content)
            .map_err(|e| format!("Failed to parse save file: {}", e))?;

        if save.version > SAVE_VERSION {
            return Err(format!(
                "Save version too new: max supported {}, got {}",
                SAVE_VERSION, save.version
            ));
        }

        log::info!("Loaded save game '{}' ({} vessels)", save.name, save.vessels.len());
        Ok(save)
    }

    /// Load a quicksave (filesystem on desktop, IndexedDB on wasm).
    pub fn load_quicksave(save_name: &str, qs_filename: &str) -> Result<Self, String> {
        let save_id = sanitize_save_name(save_name);

        #[cfg(target_arch = "wasm32")]
        let content = wasm_storage::load_quicksave_content(&save_id, qs_filename)
            .ok_or_else(|| format!("Quicksave '{}' not found", qs_filename))?;

        #[cfg(not(target_arch = "wasm32"))]
        let content = {
            let path = paths::saves_dir().join(&save_id).join(qs_filename);
            fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read quicksave: {}", e))?
        };

        let save: SaveGame = ron::from_str(&content)
            .map_err(|e| format!("Failed to parse quicksave: {}", e))?;

        if save.version > SAVE_VERSION {
            return Err(format!(
                "Save version too new: max supported {}, got {}",
                SAVE_VERSION, save.version
            ));
        }

        log::info!("Loaded quicksave '{}' from {}", save.name, qs_filename);
        Ok(save)
    }

    /// List all save files. Desktop scans the saves directory; wasm reads
    /// the IndexedDB-backed in-memory cache. The synthetic dev fixture
    /// (`test_fixture`) is always prepended.
    pub fn list_saves() -> Vec<SaveFileInfo> {
        #[cfg(target_arch = "wasm32")]
        {
            let mut out = vec![test_fixture::fixture_info()];
            out.extend(wasm_storage::list_saves());
            return out;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
        let dir = &paths::saves_dir();
        if !dir.exists() {
            return Vec::new();
        }

        let mut saves = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };

        // First pass: folder-based saves (priority)
        let entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        for entry in &entries {
            let path = entry.path();
            if path.is_dir() {
                let save_path = path.join("save.ron");
                if save_path.exists() {
                    let save_id = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    let modified = fs::metadata(&save_path)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);

                    match fs::read_to_string(&save_path) {
                        Ok(content) => {
                            match ron::from_str::<SaveGame>(&content) {
                                Ok(save) => {
                                    seen_ids.insert(save_id.clone());
                                    saves.push(SaveFileInfo {
                                        name: save.name,
                                        save_id,
                                        vessel_count: save.vessels.len(),
                                        simulation_time: save.simulation_time,
                                        modified,
                                    });
                                }
                                Err(e) => {
                                    log::error!("Save file {:?} is corrupted and cannot be loaded: {}", save_path, e);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to read save file {:?}: {}", save_path, e);
                        }
                    }
                }
            }
        }

        // Second pass: legacy flat .ron files (only if not already found as folder)
        for entry in &entries {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "ron") {
                let save_id = path.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if seen_ids.contains(&save_id) {
                    continue;
                }

                let modified = entry.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);

                match fs::read_to_string(&path) {
                    Ok(content) => {
                        match ron::from_str::<SaveGame>(&content) {
                            Ok(save) => {
                                saves.push(SaveFileInfo {
                                    name: save.name,
                                    save_id,
                                    vessel_count: save.vessels.len(),
                                    simulation_time: save.simulation_time,
                                    modified,
                                });
                            }
                            Err(e) => {
                                log::error!("Save file {:?} is corrupted and cannot be loaded: {}", path, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read save file {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort by modification time, most recent first
        saves.sort_by(|a, b| b.modified.cmp(&a.modified));
        // Always prepend the dev fixture so it sits at the top of the list.
        let mut out = vec![test_fixture::fixture_info()];
        out.extend(saves);
        out
        }  // end #[cfg(not(target_arch = "wasm32"))]
    }

    /// Delete a save by id. Desktop removes the folder/legacy file; wasm
    /// removes the matching IDB records and any quicksaves under that id.
    pub fn delete_save(save_id: &str) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        {
            wasm_storage::delete_save(save_id);
            log::info!("Deleted save '{}' from IndexedDB", save_id);
            return Ok(());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let folder_path = paths::saves_dir().join(save_id);
            if folder_path.is_dir() {
                fs::remove_dir_all(&folder_path)
                    .map_err(|e| format!("Failed to delete save folder: {}", e))?;
                log::info!("Deleted save folder: {:?}", folder_path);
                return Ok(());
            }

            let legacy_path = paths::saves_dir().join(format!("{}.ron", save_id));
            if legacy_path.is_file() {
                fs::remove_file(&legacy_path)
                    .map_err(|e| format!("Failed to delete save file: {}", e))?;
                log::info!("Deleted save file: {:?}", legacy_path);
                return Ok(());
            }

            Err(format!("Save '{}' not found", save_id))
        }
    }

    /// List all quicksaves for a given save name.
    /// Returns sorted by index descending (newest first).
    pub fn list_quicksaves(save_name: &str) -> Vec<QuicksaveInfo> {
        let save_id = sanitize_save_name(save_name);

        #[cfg(target_arch = "wasm32")]
        return wasm_storage::list_quicksaves(&save_id);

        #[cfg(not(target_arch = "wasm32"))]
        {
        let dir = paths::saves_dir().join(&save_id);
        if !dir.exists() {
            return Vec::new();
        }

        let mut quicksaves = Vec::new();

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Match quicksave_N.ron pattern
            if let Some(index) = parse_quicksave_index(&filename) {
                let modified = entry.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);

                // Read simulation_time from the file
                let simulation_time = if let Ok(content) = fs::read_to_string(&path) {
                    ron::from_str::<SaveGame>(&content)
                        .map(|s| s.simulation_time)
                        .unwrap_or(0.0)
                } else {
                    0.0
                };

                quicksaves.push(QuicksaveInfo {
                    filename,
                    index,
                    simulation_time,
                    modified,
                });
            }
        }

        // Sort by index descending (newest first)
        quicksaves.sort_by(|a, b| b.index.cmp(&a.index));
        quicksaves
        }  // end #[cfg(not(target_arch = "wasm32"))]
    }

    /// Migrate old-format tech IDs ("1.1", "2.3", etc.) to new descriptive IDs.
    fn migrate_tech_ids(ids: HashSet<String>) -> HashSet<String> {
        let map: &[(&str, &str)] = &[
            ("1.1", "basic_rocketry"),
            ("1.2", "structural_engineering"),
            ("1.3", "kerolox_propulsion"),
            ("1.4", "methalox_propulsion"),
            ("1.5", "hydrolox_propulsion"),
            ("1.6", "crewed_spaceflight"),
            ("2.1", "medium_launch"),
            ("2.2", "medium_hydrolox"),
            ("2.3", "advanced_life_support"),
            ("2.4", "heavy_lift"),
            ("2.5", "large_cryogenic"),
            ("2.6", "colony_engineering"),
            ("3.1", "nuclear_thermal"),
            ("3.2", "advanced_ntr"),
            ("3.3", "ion_propulsion"),
            ("3.4", "advanced_electric"),
            ("3.5", "compact_fission"),
            ("3.6", "heavy_fission"),
            ("3.7", "super_heavy"),
            ("3.8", "extended_missions"),
            ("4.1", "mpd_propulsion"),
            ("4.2", "deep_space_hab"),
            ("4.3", "science_laboratory"),
            ("5.1", "nuclear_pulse"),
            ("5.2", "interstellar_fission"),
            ("5.3", "passive_shielding"),
            ("6.1", "fusion_probe"),
            ("6.2", "fusion_full"),
            ("6.3", "fusion_power"),
            ("6.4", "advanced_fusion"),
            ("6.5", "active_shielding"),
            ("7.1", "am_catalyzed"),
            ("7.2", "am_production"),
            ("7.3", "geodesic_shielding"),
            ("8.1", "am_torch"),
            ("8.2", "am_power"),
            ("8.3", "advanced_am_power"),
            ("9.1", "photon_drive"),
            ("9.2", "ring_accelerator"),
        ];
        ids.into_iter()
            .map(|id| {
                map.iter()
                    .find(|(old, _)| *old == id.as_str())
                    .map(|(_, new)| new.to_string())
                    .unwrap_or(id)
            })
            .collect()
    }

    /// Migrate old-format efficiency line prerequisite node IDs in line_tiers.
    /// The line IDs themselves haven't changed, so no migration needed for keys.
    /// But we do need to check if any stored data references old node IDs.
    fn migrate_line_tiers(tiers: HashMap<String, u32>) -> HashMap<String, u32> {
        // Line tier keys (mining, metallurgy, etc.) haven't changed, so no migration needed
        tiers
    }

    /// Restore this save game's state into the given Game.
    /// Active vessel's maneuver nodes are loaded into `game.flight.active_maneuver_nodes`.
    pub fn restore_to_game(self, game: &mut Game) {
        // Restore simulation time
        game.solar_system.time = self.simulation_time;
        game.solar_system.init_sectors();

        // Restore editor vessel name
        game.editor.vessel_name = self.editor_vessel_name;

        // Merge blueprints
        game.blueprints.merge_blueprints(self.blueprints);

        // Re-inject dynamic stars needed by saved vessels before restoring soi_body indices
        let mut star_remap: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for sv in &self.vessels {
            if let Some(key) = sv.interstellar_star_key {
                let old_soi = sv.ship.soi_body;
                if !star_remap.contains_key(&old_soi) {
                    let coord = crate::bodies::SectorCoord { x: key.0, y: key.1 };
                    let stars = game.galaxy.get_sector(coord).to_vec();
                    if let Some(star_data) = stars.get(key.2 as usize) {
                        let new_idx = game.solar_system.inject_star(star_data, key);
                        star_remap.insert(old_soi, new_idx);
                    }
                }
            }
        }

        // Restore vessels, remapping soi_body for interstellar ships
        let mut vessels_iter = self.vessels.into_iter();
        if let Some(active) = vessels_iter.next() {
            let mut ship = active.ship;
            if let Some(&new_idx) = star_remap.get(&ship.soi_body) {
                ship.soi_body = new_idx;
            }
            game.flight.ship = ship;
            game.flight.vessel = active.vessel;
            game.flight.active_vessel_id = active.id;
            game.flight.active_vessel_name = active.name;
            game.flight.active_maneuver_nodes = active.maneuver_nodes;
        }

        game.flight.inactive_vessels = vessels_iter
            .map(|sv| {
                let mut ship = sv.ship;
                if let Some(&new_idx) = star_remap.get(&ship.soi_body) {
                    ship.soi_body = new_idx;
                }
                TrackedVessel {
                    id: sv.id,
                    name: sv.name,
                    ship,
                    vessel: sv.vessel,
                    maneuver_nodes: sv.maneuver_nodes,
                    is_debris: sv.is_debris,
                }
            })
            .collect();

        game.flight.next_vessel_id = self.next_vessel_id;
        game.flight.debris_counter = self.debris_counter;

        // Restore colony state
        game.colony_manager = self.colonies;
        game.company = self.company;
        game.science = self.science;
        let migrated_unlocked = Self::migrate_tech_ids(self.tech_unlocked);
        let migrated_tiers = Self::migrate_line_tiers(self.tech_line_tiers);
        game.tech_tree.apply_save_state(migrated_unlocked, migrated_tiers);
        game.notifications = self.notifications;
        game.contracts = self.contracts;
        // Sync milestones from discoveries to prevent re-awarding on old saves
        game.contracts.sync_milestones_from_discoveries(
            &game.science.discoveries,
            &game.solar_system,
        );
        // Refill contract pool on load (handles old saves with empty pool)
        game.contracts.refill_pool(
            &game.science.discoveries,
            &game.solar_system,
            game.solar_system.time,
        );
        game.fleet = self.fleet;
        game.fleet.migrate_stationed_ships(
            &mut game.colony_manager,
            game.solar_system.earth_index,
            &game.blueprints,
            &game.part_definitions,
        );
        // Migrate old single dyson_swarm to per-star HashMap
        if self.dyson_swarms.is_empty()
            && (self.dyson_swarm.mirror_count > 0
                || !self.dyson_swarm.deploying.is_empty()
                || self.dyson_swarm.collector_count > 0
                || !self.dyson_swarm.deploying_collectors.is_empty())
        {
            game.dyson_swarms.insert(game.solar_system.sun_index, self.dyson_swarm);
        } else {
            game.dyson_swarms = self.dyson_swarms;
        }
    }
}

/// Parse "quicksave_N.ron" -> Some(N)
fn parse_quicksave_index(filename: &str) -> Option<u32> {
    let stem = filename.strip_suffix(".ron")?;
    let index_str = stem.strip_prefix("quicksave_")?;
    index_str.parse().ok()
}

/// Find the next quicksave index by scanning existing files.
fn next_quicksave_index(dir: &Path) -> u32 {
    let mut max_index = 0u32;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let filename = entry.file_name();
            let filename = filename.to_str().unwrap_or("");
            if let Some(index) = parse_quicksave_index(filename) {
                max_index = max_index.max(index);
            }
        }
    }

    max_index + 1
}

/// Delete oldest quicksaves in `dir` until at most `MAX_QUICKSAVES_PER_SAVE`
/// remain. Best-effort: failures are logged, not surfaced to the caller —
/// the new quicksave has already landed and pruning is housekeeping.
fn prune_old_quicksaves(dir: &Path) {
    let mut indexed: Vec<(u32, std::path::PathBuf)> = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else { continue };
        if let Some(idx) = parse_quicksave_index(name) {
            indexed.push((idx, path));
        }
    }
    if indexed.len() <= MAX_QUICKSAVES_PER_SAVE { return; }
    indexed.sort_by_key(|(i, _)| *i);
    let drop_count = indexed.len() - MAX_QUICKSAVES_PER_SAVE;
    for (_, path) in indexed.into_iter().take(drop_count) {
        if let Err(e) = fs::remove_file(&path) {
            log::warn!("Failed to prune old quicksave {:?}: {}", path, e);
        }
    }
}

pub(crate) fn sanitize_save_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
