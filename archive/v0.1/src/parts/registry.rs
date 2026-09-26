// On wasm the filesystem-backed save path is unused (Phase 5 swaps to IDB);
// silence the resulting dead-code/unused-import warnings.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use super::VesselBlueprint;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Registry for managing saved blueprints
#[derive(Debug, Default)]
pub struct BlueprintRegistry {
    blueprints: HashMap<String, VesselBlueprint>,
    directory: PathBuf,
}

impl BlueprintRegistry {
    pub fn new<P: AsRef<Path>>(directory: P) -> Self {
        Self {
            blueprints: HashMap::new(),
            directory: directory.as_ref().to_path_buf(),
        }
    }

    /// Load all blueprints from the registry directory.
    /// On wasm this loads user-saved blueprints from the IndexedDB cache
    /// (`init_storage` must have run); on desktop it scans the platform
    /// blueprints dir. A fresh install has no blueprints — the player adds
    /// them via Save Blueprint in the editor or Import in the UI.
    pub fn load_all(&mut self) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        {
            self.blueprints.clear();
            for (name, content) in crate::save::wasm_storage::user_blueprints() {
                match ron::from_str::<VesselBlueprint>(&content) {
                    Ok(bp) => {
                        self.blueprints.insert(bp.name.clone(), bp);
                    }
                    Err(e) => log::warn!("Failed to parse user blueprint {}: {}", name, e),
                }
            }
            log::info!("Loaded {} user blueprints from IndexedDB", self.blueprints.len());
            return Ok(());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.directory.exists() {
                fs::create_dir_all(&self.directory)
                    .map_err(|e| format!("Failed to create blueprints directory: {}", e))?;
                return Ok(());
            }

            self.blueprints.clear();

            for entry in fs::read_dir(&self.directory)
                .map_err(|e| format!("Failed to read blueprints directory: {}", e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
                let path = entry.path();

                if path.extension().map_or(false, |ext| ext == "ron") {
                    match self.load_blueprint_file(&path) {
                        Ok(blueprint) => {
                            log::info!("Loaded blueprint: {}", blueprint.name);
                            self.blueprints.insert(blueprint.name.clone(), blueprint);
                        }
                        Err(e) => {
                            log::warn!("Failed to load blueprint {:?}: {}", path, e);
                        }
                    }
                }
            }

            log::info!("Loaded {} blueprints", self.blueprints.len());
            Ok(())
        }
    }

    /// Load a single blueprint file
    fn load_blueprint_file(&self, path: &Path) -> Result<VesselBlueprint, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        ron::from_str(&content)
            .map_err(|e| format!("Failed to parse RON: {}", e))
    }

    /// Save a blueprint. Desktop persists to disk; wasm persists to IndexedDB.
    pub fn save(&mut self, blueprint: VesselBlueprint) -> Result<(), String> {
        blueprint.validate()?;

        let content = ron::ser::to_string_pretty(&blueprint, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("Failed to serialize blueprint: {}", e))?;

        #[cfg(target_arch = "wasm32")]
        {
            crate::save::wasm_storage::write_blueprint(&blueprint.name, content);
            log::info!("Saved blueprint '{}' to IndexedDB", blueprint.name);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let filename = sanitize_filename(&blueprint.name);
            let path = self.directory.join(format!("{}.ron", filename));
            fs::write(&path, &content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
            log::info!("Saved blueprint '{}' to {:?}", blueprint.name, path);
        }

        self.blueprints.insert(blueprint.name.clone(), blueprint);
        Ok(())
    }

    /// Delete a blueprint. Desktop removes the file; wasm clears the IDB record.
    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        crate::save::wasm_storage::delete_blueprint(name);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let filename = sanitize_filename(name);
            let path = self.directory.join(format!("{}.ron", filename));
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|e| format!("Failed to delete file: {}", e))?;
            }
        }

        self.blueprints.remove(name);
        log::info!("Deleted blueprint: {}", name);
        Ok(())
    }

    /// Get a blueprint by name
    pub fn get(&self, name: &str) -> Option<&VesselBlueprint> {
        self.blueprints.get(name)
    }

    /// Get all blueprint names
    pub fn names(&self) -> Vec<&str> {
        self.blueprints.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a blueprint exists
    pub fn exists(&self, name: &str) -> bool {
        self.blueprints.contains_key(name)
    }

    /// Get number of blueprints
    pub fn len(&self) -> usize {
        self.blueprints.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.blueprints.is_empty()
    }

    /// Get all blueprints (for save game)
    pub fn all_blueprints(&self) -> Vec<&VesselBlueprint> {
        self.blueprints.values().collect()
    }

    /// Merge blueprints into the registry (from save game load), without overwriting existing
    pub fn merge_blueprints(&mut self, blueprints: Vec<VesselBlueprint>) {
        for bp in blueprints {
            if !self.blueprints.contains_key(&bp.name) {
                self.blueprints.insert(bp.name.clone(), bp);
            }
        }
    }
}

/// Sanitize a string to be safe for use as a filename
pub(crate) fn sanitize_filename(name: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My Ship"), "My_Ship");
        assert_eq!(sanitize_filename("test/bad"), "test_bad");
        assert_eq!(sanitize_filename("ship-1_v2"), "ship-1_v2");
    }
}
