pub mod app;
pub mod bodies;
pub mod colony;
pub mod editor;
pub mod galaxy;
pub mod game;
pub mod parts;
pub mod platform;
pub mod render;
pub mod save;
pub mod ship;

/// Wasm entry point. Trunk wires this up via `<link data-trunk rel="rust">`.
/// Initializes panic-hook + console logger, then drives `app::run` on the
/// browser's microtask queue.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_start() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = app::run().await {
            log::error!("Sunscatter exited with error: {e}");
        }
    });
}
