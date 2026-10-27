//! The 3D view's resolution policy (D072): the window's full resolution
//! whenever the frame fits the display's refresh; when the GPU cannot keep
//! up, the main pass (terrain, ships, sky) renders at a lower internal
//! resolution, never below half of it (on a Retina display, its logical
//! resolution), and is upscaled before tonemapping with a sharp
//! Catmull-Rom filter (`upscale.wgsl`). The interface (egui) is always
//! drawn at full resolution. No setting: [`Controller`] picks the scale
//! from the frame times, and gives it back as soon as there is room.
//!
//! The main pass is rendered with Bevy's `MainPassResolutionOverride` (set
//! on the camera's render entity each frame); the upscale runs in the 3D
//! camera's schedule between the main passes and post-processing.

use crate::camera::MainCamera;
use bevy::asset::{embedded_asset, load_embedded_asset};
use bevy::camera::MainPassResolutionOverride;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::sync_world::RenderEntity;
use bevy::render::view::ViewTarget;
use bevy::render::{Extract, ExtractSchedule, RenderApp, RenderStartup};
use std::collections::{HashMap, VecDeque};

/// The internal scales tried, from full resolution down to half.
pub const STEPS: [f32; 6] = [1.0, 0.9, 0.8, 0.7, 0.6, 0.5];
/// A frame "misses" when it takes this many display periods.
const MISS: f64 = 1.4;
/// The share of missed frames over [`WINDOW`] seconds that steps down.
const MISS_SHARE: f64 = 0.25;
const WINDOW: f64 = 0.5;
/// Seconds without a miss before trying the next finer step, at first and
/// at most (the wait doubles after each failed try).
const WAIT: f64 = 3.0;
const MAX_WAIT: f64 = 60.0;

/// The resolution policy's state: a pure rule over frame times.
#[derive(Clone, Debug)]
pub struct Controller {
    pub step: usize,
    /// Recent frames: (time, missed).
    recent: VecDeque<(f64, bool)>,
    now: f64,
    last_change: f64,
    last_miss: f64,
    wait: f64,
    /// A finer step being tried since this time.
    trying: Option<f64>,
    /// After stepping down: the step and mean frame time before, to check
    /// that the frame got faster (if not, the frame is not limited by
    /// pixels).
    before: Option<(usize, f64)>,
    /// No stepping down before this time (the frame was found limited by
    /// something else than pixels).
    no_down_until: f64,
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            step: 0,
            recent: VecDeque::new(),
            now: 0.0,
            last_change: 0.0,
            last_miss: 0.0,
            wait: WAIT,
            trying: None,
            before: None,
            no_down_until: 0.0,
        }
    }
}

impl Controller {
    pub fn scale(&self) -> f32 {
        STEPS[self.step]
    }

    fn mean_dt(&self) -> f64 {
        // The window's mean frame time, from the frame count.
        let n = self.recent.len().max(1) as f64;
        WINDOW / n
    }

    fn change(&mut self, step: usize) {
        self.step = step;
        self.last_change = self.now;
        self.recent.clear();
    }

    /// One frame of `dt` seconds on a display refreshing every `period`
    /// seconds; returns the new scale when it changes.
    pub fn update(&mut self, dt: f64, period: f64) -> Option<f32> {
        if !(dt.is_finite() && dt > 0.0 && period > 0.0) {
            return None;
        }
        self.now += dt;
        let missed = dt > MISS * period;
        if missed {
            self.last_miss = self.now;
        }
        self.recent.push_back((self.now, missed));
        while self.recent.front().is_some_and(|(t, _)| self.now - t > WINDOW) {
            self.recent.pop_front();
        }
        let since = self.now - self.last_change;
        if since < WINDOW {
            return None;
        }
        let share = self.recent.iter().filter(|(_, m)| *m).count() as f64 / self.recent.len().max(1) as f64;
        // A step down that did not make frames faster: limited elsewhere
        // (the CPU); go back and stay a long while.
        if let Some((step_before, dt_before)) = self.before.take() {
            if self.mean_dt() > 0.95 * dt_before {
                self.change(step_before);
                self.no_down_until = self.now + MAX_WAIT;
                return Some(self.scale());
            }
        }
        if share >= MISS_SHARE && self.now >= self.no_down_until {
            if self.trying.take().is_some() {
                self.wait = (self.wait * 2.0).min(MAX_WAIT);
            }
            if self.step + 1 < STEPS.len() {
                self.before = Some((self.step, self.mean_dt()));
                self.change(self.step + 1);
                return Some(self.scale());
            }
            return None;
        }
        if let Some(t) = self.trying {
            if self.now - t > 2.0 * WINDOW {
                // The finer step held.
                self.trying = None;
                self.wait = WAIT;
            }
            return None;
        }
        if self.step > 0 && self.now - self.last_miss >= self.wait && since >= self.wait {
            self.change(self.step - 1);
            self.trying = Some(self.now);
            return Some(self.scale());
        }
        None
    }
}

/// The policy's state in the main world.
#[derive(Resource, Default, Clone)]
pub struct RenderScale {
    pub controller: Controller,
    /// A scale held from outside the policy (the benchmark, a pin).
    fixed: Option<f32>,
}

impl RenderScale {
    pub fn scale(&self) -> f32 {
        self.fixed.unwrap_or_else(|| self.controller.scale())
    }
}

/// The internal size for a viewport at `scale` (at least 1 pixel; even
/// sizes are not needed).
pub fn scaled_size(full: UVec2, scale: f32) -> UVec2 {
    (full.as_vec2() * scale).round().as_uvec2().max(UVec2::ONE).min(full)
}

/// The display's refresh period (s): the fastest monitor's, else 120 Hz.
fn refresh_period(monitors: &Query<&bevy::window::Monitor>) -> f64 {
    let mhz = monitors.iter().filter_map(|m| m.refresh_rate_millihertz).max().unwrap_or(120_000);
    1000.0 / f64::from(mhz.max(1))
}

/// `SUNSCATTER_RENDER_SCALE=<scale>` pins the scale (a developer's and
/// the demo's override, for measuring); otherwise it adapts.
fn pinned() -> Option<f32> {
    std::env::var("SUNSCATTER_RENDER_SCALE").ok()?.parse::<f32>().ok().map(|s| s.clamp(0.25, 1.0))
}

/// Updates the scale from this frame's time. While the benchmark runs the
/// scale is held: full resolution, or half for its "−full resolution" row.
fn adapt(
    time: Res<Time<Real>>,
    monitors: Query<&bevy::window::Monitor>,
    bench: Res<crate::bench::Bench>,
    parts: Res<crate::look::Parts>,
    mut scale: ResMut<RenderScale>,
) {
    if let Some(s) = pinned() {
        scale.fixed = Some(s);
        return;
    }
    if bench.running() {
        scale.fixed = Some(if parts.native { 1.0 } else { 0.5 });
        return;
    }
    scale.fixed = None;
    let period = refresh_period(&monitors);
    if let Some(s) = scale.controller.update(time.delta_secs_f64(), period) {
        info!("render scale {s:.2} (the 3D view at {:.0} % of the window's pixels)", f64::from(s * s) * 100.0);
    }
}

/// Puts the override on the camera's render entity (none at full scale).
fn extract_override(
    mut commands: Commands,
    scale: Extract<Res<RenderScale>>,
    cams: Extract<Query<(RenderEntity, &Camera), With<MainCamera>>>,
) {
    for (entity, camera) in &cams {
        let Some(full) = camera.physical_viewport_size() else { continue };
        let size = scaled_size(full, scale.scale());
        let mut e = commands.entity(entity);
        if size == full {
            e.remove::<MainPassResolutionOverride>();
        } else {
            e.insert(MainPassResolutionOverride(size));
        }
    }
}

/// The upscale's pipeline.
#[derive(Resource)]
struct UpscalePipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    shader: Handle<Shader>,
    fullscreen: FullscreenShader,
    pipelines: HashMap<TextureFormat, CachedRenderPipelineId>,
}

/// What `upscale.wgsl` reads: the rendered part of the texture (its size
/// over the texture's) and the texture's texel size.
#[derive(Clone, Copy, ShaderType)]
struct UpscaleParams {
    ratio: Vec2,
    texel: Vec2,
}

fn init_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    assets: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "render_scale_upscale",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<UpscaleParams>(false),
            ),
        ),
    );
    let sampler = device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    commands.insert_resource(UpscalePipeline {
        layout,
        sampler,
        shader: load_embedded_asset!(assets.as_ref(), "upscale.wgsl"),
        fullscreen: fullscreen.clone(),
        pipelines: HashMap::new(),
    });
}

/// Upscales the main pass's part of the main texture to all of it.
fn upscale(
    view: ViewQuery<(&ViewTarget, &MainPassResolutionOverride)>,
    pipeline_cache: Res<PipelineCache>,
    mut pipeline: ResMut<UpscalePipeline>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (target, size) = view.into_inner();
    let format = target.main_texture_format();
    let layout = pipeline.layout.clone();
    let shader = pipeline.shader.clone();
    let vertex = pipeline.fullscreen.to_vertex_state();
    let id = *pipeline.pipelines.entry(format).or_insert_with(|| {
        pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("render_scale_upscale".into()),
            layout: vec![layout],
            vertex,
            fragment: Some(FragmentState {
                shader,
                targets: vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })],
                ..default()
            }),
            ..default()
        })
    });
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(id) else { return };
    let post = target.post_process_write();
    let full = post.source_texture.size();
    let params = UpscaleParams {
        ratio: size.0.as_vec2() / Vec2::new(full.width as f32, full.height as f32),
        texel: Vec2::new(1.0 / full.width as f32, 1.0 / full.height as f32),
    };
    let mut bytes = encase::UniformBuffer::new(Vec::<u8>::new());
    if bytes.write(&params).is_err() {
        return;
    }
    let buffer = device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("render_scale_upscale"),
        contents: bytes.as_ref(),
        usage: BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(
        "render_scale_upscale",
        &pipeline_cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((post.source, &pipeline.sampler, buffer.as_entire_binding())),
    );
    let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
        label: Some("render_scale_upscale"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations { load: LoadOp::Clear(Default::default()), store: StoreOp::Store },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

pub struct RenderScalePlugin;

impl Plugin for RenderScalePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "upscale.wgsl");
        app.init_resource::<RenderScale>().add_systems(Last, adapt);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(ExtractSchedule, extract_override)
            .add_systems(Core3d, upscale.after(Core3dSystems::MainPass).before(Core3dSystems::EarlyPostProcess));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: f64 = 1.0 / 120.0;

    /// Runs `secs` seconds of frames of `dt` each; the scale at the end.
    fn run(c: &mut Controller, dt: f64, secs: f64) -> f32 {
        let mut t = 0.0;
        while t < secs {
            c.update(dt, P);
            t += dt;
        }
        c.scale()
    }

    #[test]
    fn full_resolution_while_the_frame_fits() {
        let mut c = Controller::default();
        assert_eq!(run(&mut c, P, 30.0), 1.0);
        assert_eq!(run(&mut c, 1.2 * P, 30.0), 1.0, "a little slow is not a miss");
    }

    #[test]
    fn steps_down_while_frames_are_missed_and_no_lower_than_half() {
        let mut c = Controller::default();
        // A GPU-bound frame taking 3 periods at full scale, its time
        // falling with the pixels: the controller descends until it fits.
        let mut t = 0.0;
        while t < 20.0 {
            let dt = (0.2 + 2.8 * f64::from(c.scale()).powi(2)) * P;
            c.update(dt, P);
            t += dt;
        }
        let s = c.scale();
        assert!((0.5..=0.7).contains(&s), "{s}");
        // Hopelessly slow: half resolution, no lower.
        let mut slow = Controller::default();
        let mut t = 0.0;
        while t < 20.0 {
            let dt = (0.5 + 9.5 * f64::from(slow.scale()).powi(2)) * P;
            slow.update(dt, P);
            t += dt;
        }
        assert_eq!(slow.scale(), 0.5);
    }

    #[test]
    fn gives_resolution_back_when_there_is_room() {
        let mut c = Controller::default();
        let mut t = 0.0;
        while t < 3.0 {
            let dt = 3.0 * f64::from(c.scale()).powi(2) * P;
            c.update(dt, P);
            t += dt;
        }
        assert!(c.scale() < 1.0);
        // The scene becomes light: back to full resolution step by step.
        assert_eq!(run(&mut c, P, 60.0), 1.0);
    }

    #[test]
    fn a_cpu_bound_frame_keeps_full_resolution() {
        let mut c = Controller::default();
        // 3 periods whatever the scale: stepping down did not help.
        let s = run(&mut c, 3.0 * P, 10.0);
        assert!(s >= 0.9, "{s}");
    }

    #[test]
    fn scaled_sizes() {
        // (full, scale, internal)
        let cases = [
            (UVec2::new(3024, 1964), 1.0, UVec2::new(3024, 1964)),
            (UVec2::new(3024, 1964), 0.5, UVec2::new(1512, 982)),
            (UVec2::new(3024, 1964), 0.7, UVec2::new(2117, 1375)),
            (UVec2::new(1, 1), 0.5, UVec2::new(1, 1)),
        ];
        for (full, s, want) in cases {
            assert_eq!(scaled_size(full, s), want, "{full} {s}");
        }
    }
}
