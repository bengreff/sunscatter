//! The profiler (`SUNSCATTER_PROFILE=1`): every few seconds, a ranked table
//! of where the frame goes, printed to the terminal, appended to
//! `profile.log` (next to `settings.ron`) and
//! summarised in the performance overlay (Esc menu).
//!
//! - CPU time per system (ours by name, Bevy's by name when they cost
//!   something, else grouped by crate), per schedule, and per thread (the
//!   main app; the render app, with its wait for a swapchain image: the
//!   display's refresh, or the GPU's backlog), from Bevy's `trace` spans
//!   ([`spans`]).
//! - GPU time per group of render passes, from timestamp queries ([`gpu`]).
//! - Counts that drive the cost: terrain chunks and triangles in view,
//!   meshes in view, line points, egui primitives, terrain chunks built in
//!   the background, the fleet's advance.
//!
//! Off (the default), the spans are filtered out where they are made
//! ([`log_filter`]) and nothing here runs.

pub mod gpu;
pub mod report;
pub mod spans;

use bevy::log::BoxedLayer;
use bevy::prelude::*;
use report::{Counts, Window};
use std::io::Write as _;
use std::path::PathBuf;

/// Seconds between reports.
pub const PERIOD: f64 = 5.0;

/// Whether the profiler is on (`SUNSCATTER_PROFILE` set, not `0`).
pub fn enabled() -> bool {
    std::env::var("SUNSCATTER_PROFILE").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// The log filter: Bevy's default; when not profiling, the spans made every
/// frame (per system, per schedule, per render pass's commands) are off at
/// their source, so the `trace` feature costs nothing.
pub fn log_filter() -> String {
    let base = bevy::log::DEFAULT_FILTER;
    if enabled() {
        base.to_string()
    } else {
        format!(
            "{base},bevy_ecs=warn,bevy_app=warn,bevy_render::renderer::render_context=warn,\
             bevy_render::batching=warn,bevy_render::view::visibility=warn"
        )
    }
}

/// `LogPlugin::custom_layer`: the span timer, when profiling.
pub fn layer(_app: &mut App) -> Option<BoxedLayer> {
    enabled().then(|| Box::new(spans::SpanTimes) as BoxedLayer)
}

/// The profiler's state in the main world.
#[derive(Resource, Default)]
pub struct Profile {
    /// The latest report's summary (the performance overlay).
    pub summary: Vec<(String, String)>,
    /// The latest report.
    pub last: String,
    start: f64,
    dts: Vec<f64>,
    sums: Counts,
    label: String,
    /// A report was asked for now (an agent's `profile`, at its window's end).
    cut: bool,
    file: Option<PathBuf>,
}

impl Profile {
    /// Starts a new window named `label` (what is measured).
    pub fn restart(&mut self, label: &str) {
        self.label = label.to_string();
        self.dts.clear();
        self.sums = Counts::default();
        self.start = f64::NAN;
        let _ = spans::take();
        self.cut = false;
    }

    /// Reports the current window at the next frame's end.
    pub fn cut(&mut self) {
        self.cut = true;
    }
}

/// Adds the profiler when `SUNSCATTER_PROFILE` is set.
pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        let on = enabled();
        let file = crate::persist::Dirs::resolve()
            .settings_file
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("profile.log");
        app.insert_resource(Profile { file: on.then_some(file), label: "flight".into(), start: f64::NAN, ..default() })
            .init_resource::<gpu::GpuSink>();
        if on {
            app.add_systems(Last, measure);
        }
    }

    fn finish(&self, app: &mut App) {
        if enabled() {
            let sink = app.world().resource::<gpu::GpuSink>().clone();
            gpu::install(app, sink);
        }
    }
}

/// This frame's counts.
#[allow(clippy::type_complexity)]
fn frame_counts(
    terrain: &crate::terrain::Terrain,
    chunks: &Query<(&ViewVisibility, &crate::terrain::TerrainChunk)>,
    meshes: &Query<&ViewVisibility, With<Mesh3d>>,
    lines: &crate::trajectory::lines::Lines,
    egui: &Query<&bevy_egui::EguiRenderOutput>,
    sim_ms: f64,
) -> Counts {
    let (mut in_view, mut tris) = (0.0, 0.0);
    for (v, c) in chunks {
        if v.get() {
            in_view += 1.0;
            tris += f64::from(c.triangles);
        }
    }
    let points: usize = lines.vessels.iter().flat_map(|v| &v.pieces).map(|p| p.points.len()).sum();
    let (mut prims, mut verts) = (0.0, 0.0);
    for out in egui {
        prims += out.paint_jobs.len() as f64;
        verts += out
            .paint_jobs
            .iter()
            .map(|p| match &p.primitive {
                bevy_egui::egui::epaint::Primitive::Mesh(m) => m.vertices.len() as f64,
                bevy_egui::egui::epaint::Primitive::Callback(_) => 0.0,
            })
            .sum::<f64>();
    }
    Counts {
        chunks: terrain.drawn as f64,
        chunks_in_view: in_view,
        triangles: tris,
        meshes_in_view: meshes.iter().filter(|v| v.get()).count() as f64,
        line_points: points as f64,
        line_evaluated: lines.evaluated as f64,
        egui_primitives: prims,
        egui_vertices: verts,
        sim_ms,
        ..Counts::default()
    }
}

fn add(sum: &mut Counts, c: &Counts) {
    sum.chunks += c.chunks;
    sum.chunks_in_view += c.chunks_in_view;
    sum.triangles += c.triangles;
    sum.meshes_in_view += c.meshes_in_view;
    sum.line_points += c.line_points;
    sum.line_evaluated += c.line_evaluated;
    sum.egui_primitives += c.egui_primitives;
    sum.egui_vertices += c.egui_vertices;
    sum.sim_ms += c.sim_ms;
    sum.render_scale += c.render_scale;
}

fn mean(sum: &Counts, n: f64) -> Counts {
    let n = n.max(1.0);
    Counts {
        chunks: sum.chunks / n,
        chunks_in_view: sum.chunks_in_view / n,
        triangles: sum.triangles / n,
        meshes_in_view: sum.meshes_in_view / n,
        line_points: sum.line_points / n,
        line_evaluated: sum.line_evaluated / n,
        egui_primitives: sum.egui_primitives / n,
        egui_vertices: sum.egui_vertices / n,
        sim_ms: sum.sim_ms / n,
        render_scale: sum.render_scale / n,
        ..Counts::default()
    }
}

/// Gathers each frame and reports every [`PERIOD`] seconds (or when the
/// an agent's `profile` cuts a window).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn measure(
    time: Res<Time<Real>>,
    mut profile: ResMut<Profile>,
    sink: Res<gpu::GpuSink>,
    sim: Res<crate::state::SimState>,
    terrain: Res<crate::terrain::Terrain>,
    lines: Res<crate::trajectory::lines::Lines>,
    chunks: Query<(&ViewVisibility, &crate::terrain::TerrainChunk)>,
    meshes: Query<&ViewVisibility, With<Mesh3d>>,
    egui: Query<&bevy_egui::EguiRenderOutput>,
    scale: Res<crate::render_scale::RenderScale>,
) {
    let now = time.elapsed_secs_f64();
    if profile.start.is_nan() {
        // The first frame of a window only starts it.
        profile.start = now;
        let _ = spans::take();
        let _ = sink.take();
        let _ = crate::terrain::take_build_stats();
        return;
    }
    profile.dts.push(time.delta_secs_f64());
    let mut c = frame_counts(&terrain, &chunks, &meshes, &lines, &egui, sim.sim_ms);
    c.render_scale = f64::from(scale.scale());
    add(&mut profile.sums, &c);
    let seconds = now - profile.start;
    if seconds < PERIOD && !profile.cut {
        return;
    }
    let frames = report::frame_stats(&profile.dts);
    let (built, build_ns) = crate::terrain::take_build_stats();
    let mut counts = mean(&profile.sums, frames.frames as f64);
    counts.chunk_builds_per_s = built as f64 / seconds.max(1e-9);
    counts.chunk_build_ms_per_s = build_ns as f64 / 1e6 / seconds.max(1e-9);
    let window = Window {
        label: profile.label.clone(),
        seconds,
        frames,
        spans: spans::take(),
        gpu: sink.take().map(|t| gpu::GpuSink::per_frame(&t)),
        counts,
    };
    let text = report::format(&window);
    info!("\n{text}");
    if let Some(path) = &profile.file {
        let written =
            std::fs::OpenOptions::new().create(true).append(true).open(path).and_then(|mut f| writeln!(f, "{text}"));
        if let Err(e) = written {
            warn!("profile: cannot write {}: {e}", path.display());
        }
    }
    profile.summary = report::summary(&window);
    profile.last = text;
    let label = profile.label.clone();
    profile.restart(&label);
    profile.start = now;
}
