//! Desktop-side helpers for importing/exporting save files and blueprints
//! to/from arbitrary disk locations. Pairs with `paths` (which owns the
//! authoritative user-data dir): this module just bridges between that dir
//! and wherever the user keeps their `.ron` files.
//!
//! Uses `rfd`'s synchronous API. The dialog blocks the calling thread —
//! acceptable since the egui frame is already paused while the dialog is up.
//!
//! Every entry point takes the winit window so the dialog can be parented
//! to it. Without that, macOS opens the panel without ownership and it can
//! end up behind the game window in non-fullscreen mode.

#![cfg(not(target_arch = "wasm32"))]

use std::fs;

use winit::window::Window;

use super::paths;
use super::{sanitize_save_name, SaveGame};
use crate::parts::VesselBlueprint;
use crate::parts::registry::sanitize_filename as blueprint_filename;

/// Build a `rfd::FileDialog` already parented to `window`. Centralizes the
/// set_parent dance so every entry point gets correct dialog ownership.
fn dialog(window: &Window) -> rfd::FileDialog {
    rfd::FileDialog::new().set_parent(window)
}

/// Show a save-file dialog and write the save's RON content to whatever
/// location the user picks. Default filename is `<save_id>.ron`.
pub fn export_save(window: &Window, save_id: &str) {
    let save_path = paths::saves_dir().join(save_id).join("save.ron");
    let content = match fs::read_to_string(&save_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("export_save: read {:?}: {}", save_path, e);
            return;
        }
    };
    let Some(target) = dialog(window)
        .set_file_name(format!("{}.ron", save_id))
        .add_filter("Sunscatter save", &["ron"])
        .save_file()
    else {
        log::info!("Save export cancelled");
        return;
    };
    if let Err(e) = fs::write(&target, content) {
        log::error!("export_save: write {:?}: {}", target, e);
    } else {
        log::info!("Exported save '{}' to {:?}", save_id, target);
    }
}

/// Show an open-file dialog, parse the chosen `.ron` as a `SaveGame`, and
/// install it under the platform saves dir as `<save_id>/save.ron`. The
/// save_id is derived from the save's stored `name` (not the source filename).
pub fn import_save(window: &Window) {
    let Some(source) = dialog(window)
        .add_filter("Sunscatter save", &["ron"])
        .pick_file()
    else {
        log::info!("Save import cancelled");
        return;
    };
    let content = match fs::read_to_string(&source) {
        Ok(c) => c,
        Err(e) => {
            log::error!("import_save: read {:?}: {}", source, e);
            return;
        }
    };
    let parsed: SaveGame = match ron::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            log::error!("import_save: parse {:?}: {}", source, e);
            return;
        }
    };
    let save_id = sanitize_save_name(&parsed.name);
    let dir = paths::saves_dir().join(&save_id);
    if let Err(e) = fs::create_dir_all(&dir) {
        log::error!("import_save: create {:?}: {}", dir, e);
        return;
    }
    let dest = dir.join("save.ron");
    if let Err(e) = fs::write(&dest, content) {
        log::error!("import_save: write {:?}: {}", dest, e);
        return;
    }
    log::info!(
        "Imported save '{}' ({} vessels) as id '{}'",
        parsed.name,
        parsed.vessels.len(),
        save_id,
    );
}

/// Save-as for a single blueprint. Reads from the platform blueprints dir;
/// writes wherever the user picks.
pub fn export_blueprint(window: &Window, name: &str) {
    let stem = blueprint_filename(name);
    let source = paths::blueprints_dir().join(format!("{}.ron", stem));
    let content = match fs::read_to_string(&source) {
        Ok(c) => c,
        Err(e) => {
            log::error!("export_blueprint: read {:?}: {}", source, e);
            return;
        }
    };
    let Some(target) = dialog(window)
        .set_file_name(format!("{}.ron", stem))
        .add_filter("Sunscatter blueprint", &["ron"])
        .save_file()
    else {
        log::info!("Blueprint export cancelled");
        return;
    };
    if let Err(e) = fs::write(&target, content) {
        log::error!("export_blueprint: write {:?}: {}", target, e);
    } else {
        log::info!("Exported blueprint '{}' to {:?}", name, target);
    }
}

/// File-pick → parse → install into the platform blueprints dir. The
/// destination filename comes from the blueprint's stored `name`, not the
/// source filename. The blueprint registry refreshes from disk on the next
/// frame the palette is open, so the new entry appears in the UI.
pub fn import_blueprint(window: &Window) {
    let Some(source) = dialog(window)
        .add_filter("Sunscatter blueprint", &["ron"])
        .pick_file()
    else {
        log::info!("Blueprint import cancelled");
        return;
    };
    let content = match fs::read_to_string(&source) {
        Ok(c) => c,
        Err(e) => {
            log::error!("import_blueprint: read {:?}: {}", source, e);
            return;
        }
    };
    let parsed: VesselBlueprint = match ron::from_str(&content) {
        Ok(b) => b,
        Err(e) => {
            log::error!("import_blueprint: parse {:?}: {}", source, e);
            return;
        }
    };
    let dir = paths::blueprints_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        log::error!("import_blueprint: create {:?}: {}", dir, e);
        return;
    }
    let stem = blueprint_filename(&parsed.name);
    let dest = dir.join(format!("{}.ron", stem));
    if let Err(e) = fs::write(&dest, content) {
        log::error!("import_blueprint: write {:?}: {}", dest, e);
        return;
    }
    log::info!("Imported blueprint '{}' to {:?}", parsed.name, dest);
}
