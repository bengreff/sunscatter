//! Galaxy-view rendering: starfield texture, procedural star dots, galactic orbits.

use super::StarRenderData;
use super::super::state::RenderState;
use super::super::types::{Vertex, orbit_segments};

impl RenderState {
    /// Render galaxy star field with post-process blur.
    /// 1. Rasterize solid squares (raw RON colors) into a 200×200 CPU pixel buffer
    /// 2. Apply separable gaussian blur (5-tap kernel, 2 passes)
    /// 3. Emit blurred pixels as vertex-colored quads
    pub(super) fn add_galaxy_texture_quad(
        &self,
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        in_galaxy_view: bool,
        scale: f64,
    ) {
        if !in_galaxy_view {
            return;
        }
        let layer = match self.body_texture_map.galaxy_layer {
            Some(l) => l,
            None => return,
        };

        // Galaxy image spans 100,000 ly centered on Sgr A* (50,000 ly per side)
        let half = 50_000.0 * crate::bodies::LIGHT_YEAR * scale;
        let cam_x = self.camera.body_center[0];
        let cam_y = self.camera.body_center[1];
        let off_x = self.camera.ship_offset[0];
        let off_y = self.camera.ship_offset[1];

        let x0 = (-half - cam_x - off_x) as f32;
        let x1 = (half - cam_x - off_x) as f32;
        let y0 = (-half - cam_y - off_y) as f32;
        let y1 = (half - cam_y - off_y) as f32;

        let base = all_vertices.len() as u32;

        // UV: y-flip so image top maps to +y world. Use epsilon to avoid (0,0) which
        // the shader treats as "no texture" (solid color fallback).
        let e = 0.001;
        all_vertices.push(Vertex::textured([x0, y0], [e, 1.0], layer));
        all_vertices.push(Vertex::textured([x1, y0], [1.0, 1.0], layer));
        all_vertices.push(Vertex::textured([x1, y1], [1.0, e], layer));
        all_vertices.push(Vertex::textured([x0, y1], [e, e], layer));

        all_indices.push(base);
        all_indices.push(base + 1);
        all_indices.push(base + 2);

        all_indices.push(base);
        all_indices.push(base + 2);
        all_indices.push(base + 3);
    }

    /// Render procedural stars as small colored hexagons (or real circles when zoomed in).
    /// Also stores screen positions for hover/click hit testing.
    /// Static method to avoid borrow conflicts with self.current_procedural_stars.
    pub(super) fn add_procedural_stars_impl(
        camera: &super::super::camera::Camera,
        size: winit::dpi::PhysicalSize<u32>,
        screen_positions: &mut Vec<(usize, [f32; 2])>,
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        stars: &[StarRenderData],
        scale: f64,
        focused_star: Option<usize>,
    ) {
        screen_positions.clear();

        if stars.is_empty() {
            return;
        }

        // Precomputed unit circle offsets for 6-segment hexagon (indistinguishable
        // from circle at 2-5px). Avoids 12 trig calls per star.
        const OFFSETS: [(f32, f32); 6] = [
            (1.0, 0.0),        // 0°
            (0.5, 0.866025),   // 60°
            (-0.5, 0.866025),  // 120°
            (-1.0, 0.0),       // 180°
            (-0.5, -0.866025), // 240°
            (0.5, -0.866025),  // 300°
        ];

        // Reserve for on-screen stars only (off-screen culling skips most).
        // Estimate ~20% visible at typical zoom; cap to avoid over-allocation.
        let visible_estimate = (stars.len() / 5).min(500_000);
        all_vertices.reserve(visible_estimate * 7);
        all_indices.reserve(visible_estimate * 18);

        // Two-step camera subtraction for precision at galaxy scale.
        // body_center and ship_offset are kept separate in f64, subtracted
        // sequentially before casting to f32 — same pattern as body rendering.
        let cam_x = camera.body_center[0] as f64;
        let cam_y = camera.body_center[1] as f64;
        let off_x = camera.ship_offset[0] as f64;
        let off_y = camera.ship_offset[1] as f64;

        let pixel_ndc = 1.0 / (camera.zoom as f64 * size.height as f64 * 0.5);
        let screen_half_w = size.width as f32 * 0.5;
        let screen_half_h = size.height as f32 * 0.5;
        let aspect = camera.aspect_ratio;
        let zoom = camera.zoom;
        let cos_r = camera.rotation.cos();
        let sin_r = camera.rotation.sin();

        // Culling margins: generous enough to include stars whose hexagons
        // or circles extend past the viewport edge
        let cull_margin = 100.0f32;
        let screen_w = size.width as f32;
        let screen_h = size.height as f32;

        for (i, star) in stars.iter().enumerate() {
            let sx = (star.x * scale - cam_x - off_x) as f32;
            let sy = (star.y * scale - cam_y - off_y) as f32;

            // Convert to screen pixel coords (must include camera rotation)
            let rot_x = sx * cos_r - sy * sin_r;
            let rot_y = sx * sin_r + sy * cos_r;
            let ndc_x = rot_x * zoom / aspect;
            let ndc_y = rot_y * zoom;
            let screen_px = (ndc_x + 1.0) * screen_half_w;
            let screen_py = (1.0 - ndc_y) * screen_half_h;

            // Skip vertex generation entirely for off-screen stars
            if screen_px < -cull_margin || screen_px > screen_w + cull_margin
                || screen_py < -cull_margin || screen_py > screen_h + cull_margin
            {
                continue;
            }

            // Skip rendering AND hit-testing for focused multi-star catalog systems.
            // The companion star bodies (injected by inject_catalog_planets) replace this
            // dot visually — rendering both creates a duplicate at the barycenter.
            // Hit-testing must also be skipped: the invisible barycenter dot would otherwise
            // capture clicks, clearing tracked_body and re-centering the camera on star A.
            if focused_star == Some(i) && star.num_catalog_stars > 1 {
                continue;
            }

            // Store screen position for hit testing (hover/click)
            screen_positions.push((i, [screen_px, screen_py]));

            // Check if physical radius is large enough on screen for a real circle
            // radius_world is in camera-relative world units (shader applies zoom)
            let radius_world = star.radius_m * scale as f64;
            let radius_px = radius_world * zoom as f64 * size.height as f64 * 0.5;

            // Use pre-computed alpha from build_procedural_star_data
            let color = [star.color[0], star.color[1], star.color[2], star.alpha];

            if radius_px > 1.0 {
                // Draw a real circle with adaptive segment count
                // Use full opacity for real circles — dimming is only for distant dots
                let circle_color = [star.color[0], star.color[1], star.color[2], 1.0f32];
                let r_ndc = radius_world as f32;
                let circumference = 2.0 * std::f64::consts::PI * radius_px;
                let segments = (circumference / 3.0).clamp(16.0, 256.0) as usize;

                let base = all_vertices.len() as u32;
                // Center vertex
                all_vertices.push(Vertex::new([sx, sy], circle_color));
                for seg in 0..segments {
                    let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
                    all_vertices.push(Vertex::new(
                        [sx + r_ndc * angle.cos(), sy + r_ndc * angle.sin()],
                        circle_color,
                    ));
                }
                for seg in 0..segments {
                    let next = if seg + 1 < segments { seg + 1 } else { 0 };
                    all_indices.push(base);
                    all_indices.push(base + 1 + seg as u32);
                    all_indices.push(base + 1 + next as u32);
                }
            } else {
                // Small hexagon dot — use pre-computed lum_factor
                let half = (pixel_ndc * star.lum_factor as f64 * 2.0) as f32;

                let base = all_vertices.len() as u32;
                all_vertices.push(Vertex::new([sx, sy], color));
                for &(ox, oy) in &OFFSETS {
                    all_vertices.push(Vertex::new([sx + half * ox, sy + half * oy], color));
                }
                all_indices.extend_from_slice(&[
                    base, base + 1, base + 2,
                    base, base + 2, base + 3,
                    base, base + 3, base + 4,
                    base, base + 4, base + 5,
                    base, base + 5, base + 6,
                    base, base + 6, base + 1,
                ]);
            }

            // Ring indicator for catalog stars — distinguishes them from procedural stars.
            // Same style as body indicators: 64-segment ring, 16px outer, 70% inner.
            if star.catalog_index > 0 {
                let ring_outer = (16.0 * pixel_ndc) as f32;
                let ring_inner = (16.0 * 0.7 * pixel_ndc) as f32;
                let ring_segments = 64u32;
                let ring_color = [star.color[0], star.color[1], star.color[2], 1.0];
                let ring_inner_color = [star.color[0] * 0.3, star.color[1] * 0.3, star.color[2] * 0.3, 0.5];

                let base = all_vertices.len() as u32;
                for seg in 0..ring_segments {
                    let angle = (seg as f32 / ring_segments as f32) * std::f32::consts::TAU;
                    let cos_a = angle.cos();
                    let sin_a = angle.sin();
                    all_vertices.push(Vertex::new([sx + ring_outer * cos_a, sy + ring_outer * sin_a], ring_color));
                    all_vertices.push(Vertex::new([sx + ring_inner * cos_a, sy + ring_inner * sin_a], ring_inner_color));
                }

                for seg in 0..ring_segments {
                    let i0 = base + seg * 2;
                    let i1 = base + seg * 2 + 1;
                    let i2 = base + ((seg + 1) % ring_segments) * 2;
                    let i3 = base + ((seg + 1) % ring_segments) * 2 + 1;
                    all_indices.push(i0);
                    all_indices.push(i2);
                    all_indices.push(i1);
                    all_indices.push(i1);
                    all_indices.push(i2);
                    all_indices.push(i3);
                }
            }
        }
    }

    /// Draw the galactic orbit ellipse for the focused star.
    /// The orbit is around the galactic center (0,0) using the star's orbital elements.
    pub(super) fn add_galactic_orbit_line(
        camera: &super::super::camera::Camera,
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        star: &StarRenderData,
        scale: f64,
        screen_height: f32,
    ) {
        let a = star.semi_major_axis_m * scale;
        let e = star.eccentricity as f64;
        let arg_peri = star.arg_periapsis as f64;

        if a <= 0.0 || e >= 1.0 {
            return;
        }

        let b = a * (1.0 - e * e).sqrt();
        let c = a * e;
        // Orbit center (focus is at galactic center = 0,0)
        let center_x = -c * arg_peri.cos();
        let center_y = -c * arg_peri.sin();
        let cos_omega = arg_peri.cos();
        let sin_omega = arg_peri.sin();

        let cam_x = camera.body_center[0] as f64;
        let cam_y = camera.body_center[1] as f64;
        let off_x = camera.ship_offset[0] as f64;
        let off_y = camera.ship_offset[1] as f64;

        let line_width = 0.002 / camera.zoom as f64;
        let orbit_color = [star.color[0] * 0.4, star.color[1] * 0.4, star.color[2] * 0.4, 0.5];

        let segments = orbit_segments(a, camera.zoom, screen_height);
        let base = all_vertices.len() as u32;

        // For e < 0.1, galaxy::kepler_position() uses a first-order expansion
        // (ν ≈ M + 2e·sin M, r ≈ a·(1 − e·cos M)) rather than a true Keplerian
        // ellipse. We must draw the matching curve here — parameterized by mean
        // anomaly M — so the orbit line passes through the rendered star position.
        // For e ≥ 0.1, the point at mean anomaly M lies on a true ellipse, so we
        // use the standard ellipse parameterization (by eccentric anomaly).
        let use_first_order = e < 0.1;
        let point = |t: f64| -> (f64, f64) {
            if use_first_order {
                // t is mean anomaly M
                let sin_m = t.sin();
                let cos_m = t.cos();
                let nu = t + 2.0 * e * sin_m;
                let r = a * (1.0 - e * cos_m);
                let angle = nu + arg_peri;
                (r * angle.cos(), r * angle.sin())
            } else {
                // t is eccentric anomaly E; ellipse with focus at origin
                let ex = a * t.cos();
                let ey = b * t.sin();
                (
                    center_x + ex * cos_omega - ey * sin_omega,
                    center_y + ex * sin_omega + ey * cos_omega,
                )
            }
        };

        for i in 0..segments {
            let t = (i as f64 / segments as f64) * std::f64::consts::TAU;
            let (wx, wy) = point(t);

            let rel_x = (wx - cam_x - off_x) as f32;
            let rel_y = (wy - cam_y - off_y) as f32;

            // Perpendicular direction for line thickness
            let next_t = ((i + 1) as f64 / segments as f64) * std::f64::consts::TAU;
            let (nwx, nwy) = point(next_t);

            let dx = (nwx - wx) as f32;
            let dy = (nwy - wy) as f32;
            let len = (dx * dx + dy * dy).sqrt().max(1e-20);
            let nx = -dy / len * line_width as f32;
            let ny = dx / len * line_width as f32;

            all_vertices.push(Vertex::new([rel_x + nx, rel_y + ny], orbit_color));
            all_vertices.push(Vertex::new([rel_x - nx, rel_y - ny], orbit_color));
        }

        for i in 0..segments {
            let i0 = base + i * 2;
            let i1 = base + i * 2 + 1;
            let i2 = base + ((i + 1) % segments) * 2;
            let i3 = base + ((i + 1) % segments) * 2 + 1;
            all_indices.push(i0);
            all_indices.push(i2);
            all_indices.push(i1);
            all_indices.push(i1);
            all_indices.push(i2);
            all_indices.push(i3);
        }
    }
}
