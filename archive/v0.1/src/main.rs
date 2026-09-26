// Desktop entry point. On wasm, lib.rs's #[wasm_bindgen(start)] drives startup
// via the cdylib and this main is unused (it must still exist so cargo builds).

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    pollster::block_on(sunscatter_app::app::run()).unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
