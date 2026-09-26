//! Atmospheric halo and accretion disc rendering.

use super::super::state::RenderState;
use super::super::types::Vertex;

impl RenderState {
    /// Draw atmosphere rings around bodies that have atmospheres.
    pub(super) fn add_atmosphere_vertices(
        &self,
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        bodies: &[(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)],
        scale: f64,
    ) {
        let cam_x = self.camera.body_center[0];
        let cam_y = self.camera.body_center[1];
        let off_x = self.camera.ship_offset[0];
        let off_y = self.camera.ship_offset[1];
        let pixels_per_world_unit = self.camera.zoom * self.size.height as f32 / 2.0;

        // Atmosphere uses alpha channel to encode t (0=surface, 1=edge).
        // Fragment shader applies exp(-8*t) for non-linear falloff.
        for &(bx, by, radius, _, atmo_height, atmo_color, _) in bodies {
            if atmo_height <= 0.0 {
                continue;
            }

            // Negative alpha flags atmosphere for the shader's exp(-8*t) falloff.
            // Inner (surface): alpha = -1.0 (t=0), Outer (edge): alpha = -2.0 (t=1)
            let inner_color: [f32; 4] = [atmo_color[0], atmo_color[1], atmo_color[2], -1.0];
            let outer_color: [f32; 4] = [atmo_color[0], atmo_color[1], atmo_color[2], -2.0];

            let cx = bx * scale;
            let cy = by * scale;
            let r_inner = radius * scale;
            let r_outer = (radius + atmo_height) * scale;

            let outer_pixel_radius = r_outer as f32 * pixels_per_world_unit;
            if outer_pixel_radius < 1.0 {
                continue;
            }

            let circumference_pixels = 2.0 * std::f32::consts::PI * outer_pixel_radius;
            let raw_segments = (circumference_pixels / 3.0) as u32;

            if raw_segments <= 4096 {
                let segments = raw_segments.clamp(64, 4096) & !1;
                let base = all_vertices.len() as u32;

                for i in 0..segments {
                    let angle = (i as f64 / segments as f64) * std::f64::consts::TAU;
                    let cos_a = angle.cos();
                    let sin_a = angle.sin();

                    // Subtract large values first to preserve precision at galaxy-scale distances
                    all_vertices.push(Vertex::new([((cx - cam_x - off_x) + r_inner * cos_a) as f32, ((cy - cam_y - off_y) + r_inner * sin_a) as f32], inner_color));
                    all_vertices.push(Vertex::new([((cx - cam_x - off_x) + r_outer * cos_a) as f32, ((cy - cam_y - off_y) + r_outer * sin_a) as f32], outer_color));
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
            } else {
                let arc_segments = 4096u32;

                let dx = (cam_x + off_x) - cx;
                let dy = (cam_y + off_y) - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let cam_angle = dy.atan2(dx);

                let half_h = 1.0f64 / self.camera.zoom as f64;
                let half_w = self.camera.aspect_ratio as f64 * half_h;
                let view_diag = (half_w * half_w + half_h * half_h).sqrt();

                let visible_half = if dist > 1e-10 {
                    (view_diag / dist).min(1.0).asin()
                } else {
                    std::f64::consts::PI
                };
                let arc_half = visible_half.max(0.005 * std::f64::consts::TAU);

                let base = all_vertices.len() as u32;

                for i in 0..=arc_segments {
                    let t = i as f64 / arc_segments as f64;
                    let angle = cam_angle - arc_half + t * 2.0 * arc_half;
                    let cos_a = angle.cos();
                    let sin_a = angle.sin();

                    // Subtract large values first to preserve precision at galaxy-scale distances
                    all_vertices.push(Vertex::new([((cx - cam_x - off_x) + r_inner * cos_a) as f32, ((cy - cam_y - off_y) + r_inner * sin_a) as f32], inner_color));
                    all_vertices.push(Vertex::new([((cx - cam_x - off_x) + r_outer * cos_a) as f32, ((cy - cam_y - off_y) + r_outer * sin_a) as f32], outer_color));
                }

                for i in 0..arc_segments {
                    let i0 = base + i * 2;
                    let i1 = base + i * 2 + 1;
                    let i2 = base + (i + 1) * 2;
                    let i3 = base + (i + 1) * 2 + 1;

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

    /// Draw accretion disc rings around bodies that have them (e.g., Sgr A*)
    pub(super) fn add_accretion_disc_vertices(
        &self,
        all_vertices: &mut Vec<Vertex>,
        all_indices: &mut Vec<u32>,
        bodies: &[(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)],
        accretion_discs: &[Option<crate::bodies::AccretionDisc>],
        scale: f64,
    ) {
        let cam_x = self.camera.body_center[0];
        let cam_y = self.camera.body_center[1];
        let off_x = self.camera.ship_offset[0];
        let off_y = self.camera.ship_offset[1];
        let pixels_per_world_unit = self.camera.zoom * self.size.height as f32 / 2.0;

        for &(bx, by, _radius, _, _, _, body_idx) in bodies {
            let disc = match accretion_discs.get(body_idx) {
                Some(Some(d)) => d,
                _ => continue,
            };

            let cx = bx * scale;
            let cy = by * scale;
            let r_inner = disc.inner_radius * scale;
            let r_outer = disc.outer_radius * scale;

            let outer_pixel_radius = r_outer as f32 * pixels_per_world_unit;
            if outer_pixel_radius < 1.0 {
                continue;
            }

            // Use concentric ring strips with color gradient and atmosphere-style fade
            let num_rings = 16u32;
            let circumference_pixels = 2.0 * std::f32::consts::PI * outer_pixel_radius;
            let segments = ((circumference_pixels / 3.0) as u32).clamp(64, 4096) & !1;

            for ring in 0..num_rings {
                let t0 = ring as f64 / num_rings as f64;
                let t1 = (ring + 1) as f64 / num_rings as f64;
                let ring_r_inner = r_inner + (r_outer - r_inner) * t0;
                let ring_r_outer = r_inner + (r_outer - r_inner) * t1;

                let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;

                // Non-linear color mix: t^1.5 produces white → orange → red transition
                let mix0 = (t0 as f32).powf(1.5);
                let mix1 = (t1 as f32).powf(1.5);

                // Brightness: exp(-6*t³) fades to the same blackness as atmospheres
                // (exp(-6) ≈ 0.0025 at the outer edge), but with a gentler initial
                // falloff so the white→orange→red gradient remains visible
                let bright0 = (-6.0_f32 * (t0 as f32).powi(3)).exp();
                let bright1 = (-6.0_f32 * (t1 as f32).powi(3)).exp();

                let color_inner = [
                    lerp(disc.color_inner[0], disc.color_outer[0], mix0) * bright0,
                    lerp(disc.color_inner[1], disc.color_outer[1], mix0) * bright0,
                    lerp(disc.color_inner[2], disc.color_outer[2], mix0) * bright0,
                    bright0,
                ];
                let color_outer = [
                    lerp(disc.color_inner[0], disc.color_outer[0], mix1) * bright1,
                    lerp(disc.color_inner[1], disc.color_outer[1], mix1) * bright1,
                    lerp(disc.color_inner[2], disc.color_outer[2], mix1) * bright1,
                    bright1,
                ];

                let base = all_vertices.len() as u32;

                for i in 0..segments {
                    let angle = (i as f64 / segments as f64) * std::f64::consts::TAU;
                    let cos_a = angle.cos();
                    let sin_a = angle.sin();

                    // Subtract large values first to preserve precision at galaxy-scale distances
                    all_vertices.push(Vertex::new(
                        [((cx - cam_x - off_x) + ring_r_inner * cos_a) as f32, ((cy - cam_y - off_y) + ring_r_inner * sin_a) as f32],
                        color_inner,
                    ));
                    all_vertices.push(Vertex::new(
                        [((cx - cam_x - off_x) + ring_r_outer * cos_a) as f32, ((cy - cam_y - off_y) + ring_r_outer * sin_a) as f32],
                        color_outer,
                    ));
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
    }
}
