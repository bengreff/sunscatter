//! Browser-side helpers for moving save files between IndexedDB and the
//! user's local filesystem. Enables cross-build interop with the desktop
//! version: a save written on desktop at `data/saves/<name>/save.ron` can
//! be imported here, and a save in IndexedDB can be exported as a `.ron`
//! file the user drops back into that folder.

#![cfg(target_arch = "wasm32")]

use js_sys::{Array, Function, Promise};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{spawn_local, JsFuture};

use super::{sanitize_save_name, wasm_storage, SaveGame};
use crate::parts::VesselBlueprint;
use crate::parts::registry::sanitize_filename as blueprint_filename;

// The `_window` parameter is unused on wasm — the browser owns dialog
// chrome — but is kept for signature parity with the native backend so
// call sites don't have to cfg-gate around each invocation.

/// Trigger a browser download of the save with id `save_id` as
/// `<save_id>.ron`. Pulls content from the in-memory cache populated by
/// `wasm_storage`.
pub fn export_save(_window: &winit::window::Window, save_id: &str) {
    let Some(content) = wasm_storage::load_save_content(save_id) else {
        log::warn!("export_save: no save with id '{}'", save_id);
        return;
    };
    let filename = format!("{}.ron", save_id);
    if let Err(e) = trigger_download(&filename, &content) {
        log::error!("export_save: {:?}", e);
    } else {
        log::info!("Exported save '{}' to download", save_id);
    }
}

/// Open a file picker, parse the chosen `.ron` as a `SaveGame`, and persist
/// it via `wasm_storage`. Asynchronous; returns immediately. The egui save
/// list is refreshed every frame so the imported save appears in the UI
/// once the IDB write completes.
pub fn import_save(_window: &winit::window::Window) {
    spawn_local(async move {
        match pick_and_read().await {
            Ok(content) => {
                if let Err(e) = ingest(&content) {
                    log::error!("Save import failed: {}", e);
                }
            }
            Err(e) => log::info!("Save import cancelled: {}", e),
        }
    });
}

fn ingest(content: &str) -> Result<(), String> {
    let parsed: SaveGame =
        ron::from_str(content).map_err(|e| format!("parse: {}", e))?;
    let save_id = sanitize_save_name(&parsed.name);
    wasm_storage::write_save(
        &save_id,
        &parsed.name,
        content.to_string(),
        parsed.simulation_time,
        parsed.vessels.len(),
    );
    log::info!(
        "Imported save '{}' ({} vessels) as id '{}'",
        parsed.name,
        parsed.vessels.len(),
        save_id,
    );
    Ok(())
}

/// Trigger a browser download of the blueprint with this stored `name` as
/// `<sanitized_name>.ron`. Pulls content from the in-memory IndexedDB cache.
pub fn export_blueprint(_window: &winit::window::Window, name: &str) {
    let Some(content) = wasm_storage::load_blueprint_content(name) else {
        log::warn!("export_blueprint: no blueprint named '{}'", name);
        return;
    };
    let filename = format!("{}.ron", blueprint_filename(name));
    if let Err(e) = trigger_download(&filename, &content) {
        log::error!("export_blueprint: {:?}", e);
    } else {
        log::info!("Exported blueprint '{}' to download", name);
    }
}

/// File picker → parse → write to IndexedDB. The blueprint registry's
/// `load_all()` reads from the IDB cache, so the next refresh (driven by
/// the editor's blueprint palette) picks up the new entry.
pub fn import_blueprint(_window: &winit::window::Window) {
    spawn_local(async move {
        match pick_and_read().await {
            Ok(content) => {
                if let Err(e) = ingest_blueprint(&content) {
                    log::error!("Blueprint import failed: {}", e);
                }
            }
            Err(e) => log::info!("Blueprint import cancelled: {}", e),
        }
    });
}

fn ingest_blueprint(content: &str) -> Result<(), String> {
    let parsed: VesselBlueprint =
        ron::from_str(content).map_err(|e| format!("parse: {}", e))?;
    wasm_storage::write_blueprint(&parsed.name, content.to_string());
    log::info!("Imported blueprint '{}'", parsed.name);
    Ok(())
}

fn trigger_download(filename: &str, content: &str) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;

    let parts = Array::new();
    parts.push(&JsValue::from_str(content));
    let blob = web_sys::Blob::new_with_str_sequence(&parts)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;

    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")?
        .dyn_into()
        .map_err(|_| JsValue::from_str("anchor cast failed"))?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    let body = document
        .body()
        .ok_or_else(|| JsValue::from_str("no body"))?;
    body.append_child(&anchor)?;
    anchor.click();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

async fn pick_and_read() -> Result<String, String> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;

    let input: web_sys::HtmlInputElement = document
        .create_element("input")
        .map_err(|_| "create input")?
        .dyn_into()
        .map_err(|_| "input cast")?;
    input.set_type("file");
    input.set_accept(".ron,application/x-ron,text/plain");

    // Await the user picking a file via the input's `change` event. The
    // browser exposes no signal for "cancel," so if the user dismisses the
    // picker the spawn_local task leaks a small Closure until reload.
    let input_clone = input.clone();
    let pick = Promise::new(&mut |resolve: Function, _reject: Function| {
        let cb = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        input_clone.set_onchange(Some(cb.unchecked_ref()));
    });
    input.click();
    JsFuture::from(pick)
        .await
        .map_err(|e| format!("picker: {:?}", e))?;

    let file = input
        .files()
        .and_then(|fl| fl.get(0))
        .ok_or("no file selected")?;

    let reader = web_sys::FileReader::new().map_err(|_| "FileReader::new")?;
    let reader_clone = reader.clone();
    let read = Promise::new(&mut |resolve: Function, _reject: Function| {
        let cb = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        reader_clone.set_onloadend(Some(cb.unchecked_ref()));
    });
    reader
        .read_as_text(&file)
        .map_err(|e| format!("read_as_text: {:?}", e))?;
    JsFuture::from(read)
        .await
        .map_err(|e| format!("reader: {:?}", e))?;

    reader
        .result()
        .map_err(|e| format!("result: {:?}", e))?
        .as_string()
        .ok_or_else(|| "non-string result".to_string())
}
