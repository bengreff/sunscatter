//! Celestial body disc rendering, ring indicators, and launchpad.

use super::super::state::RenderState;
use super::super::types::{BodyData, ShipRenderData, Vertex};

impl RenderState {
    /// Helper to add body vertices (extracted for reuse)
    pub(super) fn add_body_vertices(
        &mut self,
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        bodies: &[(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)],
        scale: f64,
    ) {
        // Store body data for hit testing
        self.bodies.clear();

        // Calculate world units per pixel for indicator sizing
        let pixels_per_world_unit = self.camera.zoom * self.size.height as f32 / 2.0;
        let indicator_screen_radius = 16.0f32;
        let indicator_world_radius = (indicator_screen_radius / pixels_per_world_unit) as f64;
        let min_body_pixels = 5.0f32;

        let cam_x = self.camera.body_center[0];
        let cam_y = self.camera.body_center[1];
        let off_x = self.camera.ship_offset[0];
        let off_y = self.camera.ship_offset[1];

        // Two-pass indicator rendering: defer the root body's (first body) indicator
        // to render AFTER all other bodies, so it appears on top (e.g. Sun over planets).
        let mut deferred_root_indicator: Option<(f32, f32, [f32; 4])> = None;

        for (body_array_idx, (x, y, radius, color, _atmo_height, _atmo_color, body_idx)) in bodies.iter().enumerate() {
            let rel_x = ((*x * scale) - cam_x - off_x) as f32;
            let rel_y = ((*y * scale) - cam_y - off_y) as f32;
            let r = (*radius * scale) as f32;

            let cx = *x * scale;
            let cy = *y * scale;
            let r_f64 = *radius * scale;

            // Bodies with radius=0 are hidden (e.g., planets/moons in galaxy view)
            // Push empty BodyData to keep indices aligned but skip rendering/hit testing
            if *radius <= 0.0 {
                self.bodies.push(BodyData {
                    x: cx,
                    y: cy,
                    radius: 0.0,
                    indicator_radius: 0.0,
                });
                continue;
            }

            let body_pixel_radius = r * pixels_per_world_unit;
            let body_pixels = body_pixel_radius * 2.0;
            let needs_indicator = body_pixels < min_body_pixels;

            self.bodies.push(BodyData {
                x: cx,
                y: cy,
                radius: r_f64,
                indicator_radius: if needs_indicator { indicator_world_radius } else { 0.0 },
            });

            let min_draw_pixels = 1.0;
            let body_is_visible = body_pixels >= min_draw_pixels;

            if body_is_visible {
                let base_index = all_vertices.len() as u32;
                let draw_r = r;
                let texture_layer = self.body_texture_map.layer_for_body(*body_idx);

                let draw_pixel_radius = draw_r * pixels_per_world_unit;
                let circumference_pixels = 2.0 * std::f32::consts::PI * draw_pixel_radius;
                let raw_segments = (circumference_pixels / 3.0) as u32;

                if raw_segments <= 4096 {
                    // Full circle: polygon triangle fan
                    let segments = raw_segments.clamp(64, 4096) & !1;

                    if let Some(layer) = texture_layer {
                        all_vertices.push(Vertex::textured([rel_x, rel_y], [0.5, 0.5], layer));
                        for i in 0..segments {
                            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                            let u = 0.5 + 0.5 * angle.cos();
                            let v = 0.5 - 0.5 * angle.sin();
                            all_vertices.push(Vertex::textured(
                                [rel_x + draw_r * angle.cos(), rel_y + draw_r * angle.sin()],
                                [u, v],
                                layer,
                            ));
                        }
                    } else {
                        all_vertices.push(Vertex::new([rel_x, rel_y], *color));
                        for i in 0..segments {
                            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
                            all_vertices.push(Vertex::new([rel_x + draw_r * angle.cos(), rel_y + draw_r * angle.sin()], *color));
                        }
                    }

                    for i in 0..segments {
                        all_indices.push(base_index);
                        all_indices.push(base_index + i + 1);
                        all_indices.push(base_index + ((i + 1) % segments) + 1);
                    }
                } else {
                    // Arc mode: 4096 segments on the visible ~1% of circumference
                    let arc_segments = 4096u32;

                    // Direction from body center to camera (f64 precision)
                    let dx = (cam_x + off_x) - cx;
                    let dy = (cam_y + off_y) - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let cam_angle = dy.atan2(dx);

                    // Viewport diagonal in world units
                    let half_h = 1.0f64 / self.camera.zoom as f64;
                    let half_w = self.camera.aspect_ratio as f64 * half_h;
                    let view_diag = (half_w * half_w + half_h * half_h).sqrt();

                    // Half-angle of visible arc from body center
                    let visible_half = if dist > 1e-10 {
                        (view_diag / dist).min(1.0).asin()
                    } else {
                        std::f64::consts::PI
                    };

                    // At least 1% of circumference (0.5% each side)
                    let arc_half = visible_half.max(0.005 * std::f64::consts::TAU);

                    if let Some(layer) = texture_layer {
                        // Center vertex with UV center
                        all_vertices.push(Vertex::textured([rel_x, rel_y], [0.5, 0.5], layer));

                        for i in 0..=arc_segments {
                            let t = i as f64 / arc_segments as f64;
                            let angle = cam_angle - arc_half + t * 2.0 * arc_half;
                            // Subtract large values first to preserve precision at galaxy-scale distances
                            let vx = (cx - cam_x - off_x) + r_f64 * angle.cos();
                            let vy = (cy - cam_y - off_y) + r_f64 * angle.sin();
                            let u = 0.5 + 0.5 * (angle.cos() as f32);
                            let v = 0.5 - 0.5 * (angle.sin() as f32);
                            all_vertices.push(Vertex::textured(
                                [vx as f32, vy as f32],
                                [u, v],
                                layer,
                            ));
                        }
                    } else {
                        all_vertices.push(Vertex::new([rel_x, rel_y], *color));

                        for i in 0..=arc_segments {
                            let t = i as f64 / arc_segments as f64;
                            let angle = cam_angle - arc_half + t * 2.0 * arc_half;
                            // Subtract large values first to preserve precision at galaxy-scale distances
                            let vx = (cx - cam_x - off_x) + r_f64 * angle.cos();
                            let vy = (cy - cam_y - off_y) + r_f64 * angle.sin();
                            all_vertices.push(Vertex::new([vx as f32, vy as f32], *color));
                        }
                    }

                    // Triangle fan
                    for i in 0..arc_segments {
                        all_indices.push(base_index);
                        all_indices.push(base_index + 1 + i);
                        all_indices.push(base_index + 2 + i);
                    }
                }
            }

            if needs_indicator {
                let ring_color = if color[0] + color[1] + color[2] < 0.1 {
                    [0.7, 0.5, 0.3, 1.0] // Warm amber fallback for black holes
                } else {
                    *color
                };

                // Defer the first body's indicator to render last (on top of all others)
                if body_array_idx == 0 {
                    deferred_root_indicator = Some((rel_x, rel_y, ring_color));
                } else {
                    Self::draw_ring_indicator(all_vertices, all_indices, rel_x, rel_y, indicator_world_radius, ring_color);
                }
            }
        }

        // Second pass: draw deferred root body indicator on top of everything
        if let Some((rel_x, rel_y, ring_color)) = deferred_root_indicator {
            Self::draw_ring_indicator(all_vertices, all_indices, rel_x, rel_y, indicator_world_radius, ring_color);
        }
    }

    /// Draw a ring indicator at the given position (shared by body indicator rendering).
    pub(super) fn draw_ring_indicator(
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        rel_x: f32,
        rel_y: f32,
        indicator_world_radius: f64,
        ring_color: [f32; 4],
    ) {
        let base_index = all_vertices.len() as u32;
        let ring_outer = indicator_world_radius as f32;
        let ring_inner = (indicator_world_radius * 0.7) as f32;
        let ring_segments = 64u32;

        for i in 0..ring_segments {
            let angle = (i as f32 / ring_segments as f32) * std::f32::consts::TAU;
            let cos_a = angle.cos();
            let sin_a = angle.sin();

            all_vertices.push(Vertex::new([rel_x + ring_outer * cos_a, rel_y + ring_outer * sin_a], ring_color));
            all_vertices.push(Vertex::new([rel_x + ring_inner * cos_a, rel_y + ring_inner * sin_a], [ring_color[0] * 0.3, ring_color[1] * 0.3, ring_color[2] * 0.3, ring_color[3] * 0.5]));
        }

        for i in 0..ring_segments {
            let i0 = base_index + i * 2;
            let i1 = base_index + i * 2 + 1;
            let i2 = base_index + ((i + 1) % ring_segments) * 2;
            let i3 = base_index + ((i + 1) % ring_segments) * 2 + 1;

            all_indices.push(i0);
            all_indices.push(i2);
            all_indices.push(i1);

            all_indices.push(i1);
            all_indices.push(i2);
            all_indices.push(i3);
        }
    }

    /// Draw the launchpad on Earth's surface when in ship view.
    pub(super) fn add_launchpad_vertices(
        &self,
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        bodies: &[(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)],
        scale: f64,
        ship: &ShipRenderData,
        earth_index: usize,
    ) {
        use crate::game::{LAUNCHPAD_SURFACE_ANGLE,
                          LAUNCHPAD_HEIGHT, LAUNCHPAD_TOP_WIDTH, LAUNCHPAD_BOTTOM_WIDTH};
        let pixels_per_world_unit = self.camera.zoom * self.size.height as f32 / 2.0;
        let ship_pixels = ship.size as f32 * pixels_per_world_unit * 2.0;

        // Only draw launchpad in ship view
        if ship_pixels < 5.0 {
            return;
        }

        // Find the launchpad body
        let Some((bx, by, radius, _, _, _, _)) = bodies.get(earth_index) else { return };

        // Compute body center relative to camera in f64 first, preserving precision.
        // At galaxy-scale distances (body position ~2.46e11 world units), the launchpad
        // dimensions (~6e-8 world units) are below f64 ULP if computed in absolute coords.
        // By subtracting body_center and ship_offset first, we keep all values near zero.
        let cam_x = self.camera.body_center[0];
        let cam_y = self.camera.body_center[1];
        let off_x = self.camera.ship_offset[0];
        let off_y = self.camera.ship_offset[1];
        let rel_cx = bx * scale - cam_x - off_x;
        let rel_cy = by * scale - cam_y - off_y;
        let r = radius * scale;

        let lp_angle = LAUNCHPAD_SURFACE_ANGLE;
        let lp_height = LAUNCHPAD_HEIGHT * scale;
        let lp_top_half = (LAUNCHPAD_TOP_WIDTH * 0.5) * scale;
        let lp_bot_half = (LAUNCHPAD_BOTTOM_WIDTH * 0.5) * scale;

        // Surface point at launchpad center (relative to camera)
        let sx = rel_cx + r * lp_angle.cos();
        let sy = rel_cy + r * lp_angle.sin();

        // Radial outward and tangent directions
        let rad_x = lp_angle.cos();
        let rad_y = lp_angle.sin();
        let tan_x = -rad_y;
        let tan_y = rad_x;

        // 4 corners relative to camera
        let bl_x = sx - lp_bot_half * tan_x;
        let bl_y = sy - lp_bot_half * tan_y;
        let br_x = sx + lp_bot_half * tan_x;
        let br_y = sy + lp_bot_half * tan_y;
        let tl_x = sx - lp_top_half * tan_x + lp_height * rad_x;
        let tl_y = sy - lp_top_half * tan_y + lp_height * rad_y;
        let tr_x = sx + lp_top_half * tan_x + lp_height * rad_x;
        let tr_y = sy + lp_top_half * tan_y + lp_height * rad_y;

        let lp_color: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
        let base = all_vertices.len() as u32;

        all_vertices.push(Vertex::new([bl_x as f32, bl_y as f32], lp_color));
        all_vertices.push(Vertex::new([br_x as f32, br_y as f32], lp_color));
        all_vertices.push(Vertex::new([tl_x as f32, tl_y as f32], lp_color));
        all_vertices.push(Vertex::new([tr_x as f32, tr_y as f32], lp_color));

        // Two triangles for the trapezoid
        all_indices.push(base);
        all_indices.push(base + 1);
        all_indices.push(base + 2);
        all_indices.push(base + 1);
        all_indices.push(base + 3);
        all_indices.push(base + 2);
    }
}
