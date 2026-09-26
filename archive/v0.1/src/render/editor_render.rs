use wgpu::util::DeviceExt;
use egui_wgpu::ScreenDescriptor;

use super::types::Vertex;
use super::state::RenderState;

impl RenderState {
    /// Render the editor scene
    pub fn render_editor(
        &mut self,
        vertices: &[Vertex],
        egui_callback: impl FnOnce(&egui::Context),
    ) -> Result<(), wgpu::SurfaceError> {
        // Update camera buffer before rendering
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build egui UI
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let fps = self.fps;
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui_callback(ctx);
            super::state::fps_overlay(ctx, fps);
        });

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);

        let tris = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        // Create LOCAL buffers for editor - don't modify the shared buffers
        let editor_vertex_buffer;
        let editor_index_buffer;
        let editor_num_indices;

        if !vertices.is_empty() {
            editor_vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Editor Vertex Buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let indices: Vec<u32> = (0..vertices.len() as u32).collect();
            editor_index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Editor Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            editor_num_indices = indices.len() as u32;
        } else {
            // Empty buffers
            editor_vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Editor Vertex Buffer Empty"),
                size: 64,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            });
            editor_index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Editor Index Buffer Empty"),
                size: 64,
                usage: wgpu::BufferUsages::INDEX,
                mapped_at_creation: false,
            });
            editor_num_indices = 0;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Editor Render Encoder"),
            });

        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_descriptor);

        // Render editor geometry
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Editor Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if editor_num_indices > 0 {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_bind_group(1, &self.body_texture_bind_group, &[]);
                render_pass.set_bind_group(2, &self.sprite_atlas.bind_group, &[]);
                render_pass.set_vertex_buffer(0, editor_vertex_buffer.slice(..));
                render_pass.set_index_buffer(editor_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..editor_num_indices, 0, 0..1);
            }
        }

        // Render egui
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Editor Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.egui_renderer.render(&mut render_pass, &tris, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Update the editor camera for rendering
    pub fn set_editor_camera(&mut self, offset: [f64; 2], zoom: f32) {
        self.camera.position = offset;
        self.camera.zoom = zoom;
        self.camera.rotation = 0.0;
        self.update_camera_buffer();
    }

    /// Get the egui context for direct UI access
    pub fn egui_context(&self) -> &egui::Context {
        &self.egui_ctx
    }
}
