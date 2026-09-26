//! Cross-platform shims for desktop vs wasm. Keeps cfg blocks out of the call
//! sites — `app.rs` calls `platform::*` and lets this module pick the right path.

use std::sync::Arc;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

/// Build a `WindowBuilder`, pre-attaching a canvas on wasm. The caller still
/// calls `.build(&event_loop)` on the result.
pub fn make_window_builder(title: &str, default_size: (u32, u32)) -> WindowBuilder {
    let builder = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(winit::dpi::LogicalSize::new(default_size.0, default_size.1));

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowBuilderExtWebSys;
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("sunscatter"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        return builder.with_canvas(canvas);
    }

    #[allow(unreachable_code)]
    builder
}

/// Run the winit event loop. On native this blocks until exit. On wasm this
/// hands control to the browser and returns immediately.
pub fn spawn_event_loop<F>(event_loop: EventLoop<()>, closure: F)
where
    F: FnMut(winit::event::Event<()>, &winit::event_loop::EventLoopWindowTarget<()>) + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        event_loop.run(closure).unwrap();
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn(closure);
    }
}

/// On wasm, install a window-resize listener that drives the winit window's
/// inner size to fill the viewport at native DPR. No-op on native.
#[allow(unused_variables)]
pub fn install_resize_handler(window: Arc<Window>) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        let resize_window = window.clone();
        let apply = move || {
            if let Some(win) = web_sys::window() {
                let dpr = win.device_pixel_ratio();
                let w = win
                    .inner_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1280.0);
                let h = win
                    .inner_height()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(720.0);
                let physical_w = (w * dpr).max(1.0) as u32;
                let physical_h = (h * dpr).max(1.0) as u32;
                let _ = resize_window
                    .request_inner_size(winit::dpi::PhysicalSize::new(physical_w, physical_h));
            }
        };

        // Apply once on install.
        apply();

        let closure = Closure::<dyn FnMut()>::new(apply);
        if let Some(win) = web_sys::window() {
            let _ = win.add_event_listener_with_callback(
                "resize",
                closure.as_ref().unchecked_ref(),
            );
        }
        closure.forget();
    }
}
