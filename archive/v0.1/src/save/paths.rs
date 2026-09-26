//! Platform user-data paths for saves and blueprints. Desktop only — wasm
//! goes through `wasm_storage` (IndexedDB) and doesn't touch the filesystem.
//!
//! Locations:
//!   macOS:   ~/Library/Application Support/sunscatter/{saves,blueprints}/
//!   Linux:   ~/.local/share/sunscatter/{saves,blueprints}/
//!   Windows: %APPDATA%/sunscatter/data/{saves,blueprints}/
//!
//! The repo itself ships no saves or blueprints. A fresh install starts
//! with empty lists; players populate them by clicking New Game / Save
//! Blueprint, or by importing `.ron` files via the UI.

#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

use directories::ProjectDirs;

/// Root data dir for this app (creates nothing). Returns `None` only if the
/// platform has no notion of a user-data dir, which on supported targets
/// (macOS / Linux / Windows) won't happen.
fn project_data_dir() -> PathBuf {
    ProjectDirs::from("", "", "sunscatter")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".sunscatter"))
}

/// Directory holding `<save_id>/save.ron`, `quicksave_*.ron`, `launch.ron`.
pub fn saves_dir() -> PathBuf {
    project_data_dir().join("saves")
}

/// Directory holding `<blueprint_name>.ron` files.
pub fn blueprints_dir() -> PathBuf {
    project_data_dir().join("blueprints")
}

/// Create the saves directory if it doesn't yet exist. Cheap; idempotent.
pub fn ensure_saves_dir() -> Result<PathBuf, String> {
    let dir = saves_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("create saves dir {:?}: {}", dir, e))?;
    Ok(dir)
}

/// Create the blueprints directory if it doesn't yet exist.
pub fn ensure_blueprints_dir() -> Result<PathBuf, String> {
    let dir = blueprints_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("create blueprints dir {:?}: {}", dir, e))?;
    Ok(dir)
}
