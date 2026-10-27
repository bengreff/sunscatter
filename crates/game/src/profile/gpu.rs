//! GPU time per render pass group, from timestamp queries written between
//! Bevy's passes (on Metal only between passes: wgpu offers timestamps on
//! command encoders, `TIMESTAMP_QUERY_INSIDE_ENCODERS`, not inside passes,
//! so Bevy's own pass diagnostics give CPU times only there).
//!
//! Markers, in submission order: the frame's start (before the cameras),
//! then in the 3D camera's schedule before the opaque pass, after it, after
//! the sky (the atmosphere's composite), after the transparent pass, after
//! post-processing, after egui, and the end (after upscaling). Each segment
//! is named by the marker that ends it ([`SEGMENTS`]). Results are read
//! back a frame or two later, three frames in flight at most.

use bevy::core_pipeline::core_3d::{main_opaque_pass_3d, main_transparent_pass_3d};
use bevy::core_pipeline::schedule::camera_driver;
use bevy::core_pipeline::upscaling::upscaling;
use bevy::core_pipeline::{Core3d, Core3dSystems};
use bevy::ecs::schedule::{InternedSystemSet, ScheduleLabel, Schedules};
use bevy::prelude::*;
use bevy::render::renderer::{
    PendingCommandBuffers, RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue,
};
use bevy::render::RenderApp;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The segment each marker ends (index = marker; marker 0 starts the frame).
pub const SEGMENTS: [&str; 8] = [
    "",
    "shadows+prepass+culling+sky tables",
    "opaque (terrain, bodies, ships)",
    "sky (atmosphere)",
    "transparent (rings, decks, lines)",
    "post (resolve, tonemap)",
    "ui (egui)",
    "upscale",
];
const END: usize = SEGMENTS.len() - 1;
const MAX_QUERIES: u32 = 32;
const SLOTS: usize = 3;

/// GPU milliseconds per segment, summed over the frames measured; shared
/// with the main world.
#[derive(Clone, Debug, Default)]
pub struct GpuTotals {
    pub ms: [f64; SEGMENTS.len()],
    pub frames: u64,
}

/// The main world's handle on the render world's totals; `None` inside
/// when the adapter has no timestamp queries.
#[derive(Resource, Clone, Default)]
pub struct GpuSink(pub Arc<Mutex<Option<GpuTotals>>>);

impl GpuSink {
    /// The totals since the last call (`None`: not supported).
    pub fn take(&self) -> Option<GpuTotals> {
        let mut g = self.0.lock().ok()?;
        let out = g.clone();
        if let Some(t) = g.as_mut() {
            *t = GpuTotals::default();
        }
        out
    }

    /// Mean ms per measured frame for each segment that ran.
    pub fn per_frame(t: &GpuTotals) -> Vec<(String, f64)> {
        let n = t.frames.max(1) as f64;
        SEGMENTS
            .iter()
            .zip(t.ms)
            .skip(1)
            .filter(|(_, ms)| *ms > 0.0)
            .map(|(name, ms)| ((*name).to_string(), ms / n))
            .collect()
    }
}

struct Slot {
    readback: wgpu::Buffer,
    /// (marker, query index) in the order written.
    marks: Vec<(usize, u32)>,
    submitted: bool,
    mapped: Arc<AtomicBool>,
}

/// The render world's timestamp state.
#[derive(Resource)]
pub struct GpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    slots: Vec<Slot>,
    current: Option<usize>,
    period_ns: f32,
    sink: GpuSink,
}

impl GpuTimer {
    fn new(device: &RenderDevice, queue: &RenderQueue, sink: GpuSink) -> Self {
        let d = device.wgpu_device();
        let size = u64::from(MAX_QUERIES) * 8;
        let set = d.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("profile timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: MAX_QUERIES,
        });
        let resolve = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("profile resolve"),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..SLOTS)
            .map(|_| Slot {
                readback: d.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("profile readback"),
                    size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                marks: Vec::new(),
                submitted: false,
                mapped: Arc::new(AtomicBool::new(false)),
            })
            .collect();
        GpuTimer { set, resolve, slots, current: None, period_ns: queue.get_timestamp_period(), sink }
    }

    fn write(&mut self, marker: usize, encoder: &mut wgpu::CommandEncoder) {
        let Some(i) = self.current else { return };
        let slot = &mut self.slots[i];
        let q = slot.marks.len() as u32;
        if q < MAX_QUERIES {
            encoder.write_timestamp(&self.set, q);
            slot.marks.push((marker, q));
        }
    }
}

/// Reads finished frames into the totals and picks a free slot for this
/// frame (none free: this frame is not measured).
fn begin(mut timer: ResMut<GpuTimer>) {
    let period = timer.period_ns;
    let sink = timer.sink.clone();
    for slot in &mut timer.slots {
        if !(slot.submitted && slot.mapped.load(Ordering::Acquire)) {
            continue;
        }
        let ticks: Vec<u64> = {
            let view = slot.readback.slice(..).get_mapped_range();
            view.as_chunks::<8>().0.iter().map(|b| u64::from_le_bytes(*b)).collect()
        };
        slot.readback.unmap();
        let marks: Vec<(usize, u64)> = slot.marks.iter().map(|&(m, q)| (m, ticks[q as usize])).collect();
        if let Ok(mut g) = sink.0.lock() {
            let t = g.get_or_insert_with(GpuTotals::default);
            for (m, ms) in super::report::gpu_segments(&marks, period) {
                t.ms[m] += ms;
            }
            t.frames += 1;
        }
        slot.submitted = false;
        slot.mapped.store(false, Ordering::Release);
        slot.marks.clear();
    }
    timer.current = timer.slots.iter().position(|s| !s.submitted);
}

/// The frame's first timestamp, before any camera renders.
fn mark_start(timer: Option<ResMut<GpuTimer>>, device: Res<RenderDevice>, mut pending: ResMut<PendingCommandBuffers>) {
    let Some(mut timer) = timer else { return };
    if timer.current.is_none() {
        return;
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("profile start") });
    timer.write(0, &mut encoder);
    pending.push_encoder(encoder);
}

/// A timestamp between two of the 3D camera's passes.
fn mark<const M: usize>(timer: Option<ResMut<GpuTimer>>, mut ctx: RenderContext) {
    if let Some(mut timer) = timer {
        if timer.current.is_some() {
            timer.write(M, ctx.command_encoder());
        }
    }
}

/// The last timestamp, and the copy of this frame's into its slot.
fn mark_end(timer: Option<ResMut<GpuTimer>>, device: Res<RenderDevice>, mut pending: ResMut<PendingCommandBuffers>) {
    let Some(mut timer) = timer else { return };
    let Some(i) = timer.current else { return };
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("profile end") });
    timer.write(END, &mut encoder);
    let n = timer.slots[i].marks.len() as u32;
    if n > 0 {
        encoder.resolve_query_set(&timer.set, 0..n, &timer.resolve, 0);
        encoder.copy_buffer_to_buffer(&timer.resolve, 0, &timer.slots[i].readback, 0, u64::from(n) * 8);
    }
    pending.push_encoder(encoder);
}

/// Maps this frame's slot once its commands are submitted.
fn finish(mut timer: ResMut<GpuTimer>) {
    let Some(i) = timer.current.take() else { return };
    let slot = &mut timer.slots[i];
    if slot.marks.is_empty() {
        return;
    }
    slot.submitted = true;
    let mapped = slot.mapped.clone();
    slot.readback.slice(..).map_async(wgpu::MapMode::Read, move |r| mapped.store(r.is_ok(), Ordering::Release));
}

/// The system set of a system in `schedule` whose path ends with `suffix`
/// (Bevy's `render_sky` is private, but its set can be ordered against).
fn set_of(world: &World, schedule: impl ScheduleLabel, suffix: &str) -> Option<InternedSystemSet> {
    let s = world.get_resource::<Schedules>()?.get(schedule)?;
    s.graph()
        .systems
        .iter()
        .find(|(_, sys, _)| sys.name().to_string().ends_with(suffix))
        .and_then(|(_, sys, _)| sys.default_system_sets().first().copied())
}

/// Installs the markers in the render app, if the adapter supports
/// timestamps between passes; `sink` stays `None` otherwise.
pub fn install(app: &mut App, sink: GpuSink) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
    let need = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    let device = render_app.world().resource::<RenderDevice>().clone();
    if !device.features().contains(need) {
        warn!("profile: the adapter has no timestamp queries between passes; GPU times are not measured");
        return;
    }
    if let Ok(mut g) = sink.0.lock() {
        *g = Some(GpuTotals::default());
    }
    let queue = render_app.world().resource::<RenderQueue>().clone();
    render_app.insert_resource(GpuTimer::new(&device, &queue, sink));
    let sky = set_of(render_app.world(), Core3d, "render_sky");
    render_app.add_systems(
        RenderGraph,
        (
            begin.in_set(RenderGraphSystems::Begin),
            mark_start.in_set(RenderGraphSystems::Render).before(camera_driver),
            mark_end.after(RenderGraphSystems::Render).before(RenderGraphSystems::Submit),
            finish.in_set(RenderGraphSystems::Finish),
        ),
    );
    render_app.add_systems(
        Core3d,
        (
            mark::<1>.in_set(Core3dSystems::MainPass).before(main_opaque_pass_3d),
            mark::<4>.after(Core3dSystems::MainPass).before(Core3dSystems::EarlyPostProcess),
            mark::<5>.after(Core3dSystems::PostProcess).before(bevy_egui::render::egui_pass),
            mark::<6>.after(bevy_egui::render::egui_pass).before(upscaling),
        ),
    );
    match sky {
        Some(sky) => {
            render_app.add_systems(
                Core3d,
                (
                    mark::<2>.after(main_opaque_pass_3d).before(sky),
                    mark::<3>.after(sky).before(main_transparent_pass_3d),
                ),
            );
        }
        // No atmosphere pass: opaque and sky in one segment.
        None => {
            render_app.add_systems(Core3d, mark::<3>.after(main_opaque_pass_3d).before(main_transparent_pass_3d));
        }
    }
}
