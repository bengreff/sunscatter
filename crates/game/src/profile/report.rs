//! The profiler's rules, pure: frame statistics, the ranked table of CPU
//! time per system (ours by name, Bevy's by name when they cost something,
//! else grouped by crate), GPU time per segment between the timestamp
//! markers, and the printed report.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// What a recorded span measures (`spans` keeps these only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// One system's run (`name`: the system's path).
    System,
    /// Applying a system's commands (counted with the system).
    Commands,
    /// A schedule's run, wall time (`name`: its label).
    Schedule,
    /// A whole app's frame on its thread: "main app", or the render app.
    App,
}

/// Time accumulated by one span over a window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Acc {
    pub ns: u64,
    pub count: u64,
}

impl Acc {
    pub fn add(&mut self, ns: u64) {
        self.ns += ns;
        self.count += 1;
    }

    /// Milliseconds per frame over `frames` frames.
    pub fn ms_per_frame(self, frames: u64) -> f64 {
        self.ns as f64 / 1e6 / frames.max(1) as f64
    }
}

/// Frame-time statistics over a window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    pub frames: u64,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub worst_ms: f64,
}

impl FrameStats {
    pub fn fps(&self) -> f64 {
        if self.avg_ms > 0.0 {
            1e3 / self.avg_ms
        } else {
            0.0
        }
    }
}

/// Statistics of frame durations `dt` (s).
pub fn frame_stats(dt: &[f64]) -> FrameStats {
    if dt.is_empty() {
        return FrameStats::default();
    }
    let mut s: Vec<f64> = dt.iter().map(|d| d * 1e3).collect();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    FrameStats {
        frames: n as u64,
        avg_ms: s.iter().sum::<f64>() / n as f64,
        p95_ms: s[((n - 1) * 95).div_ceil(100)],
        worst_ms: s[n - 1],
    }
}

/// One row of the CPU table.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    /// Milliseconds per frame (summed over threads).
    pub ms: f64,
    /// Runs per frame.
    pub runs: f64,
}

/// Our own systems are listed by name, always.
const OURS: &str = "game::";

/// The crate a system's path belongs to (`bevy_pbr::render::…` → `bevy_pbr`).
pub fn crate_of(path: &str) -> &str {
    let p = path.trim_start_matches(['<', '&']);
    p.split("::").next().unwrap_or(p)
}

/// The CPU table: each system's time per frame (its commands included),
/// ours by name, others by name from `min_ms`, the rest grouped per crate
/// ("bevy_pbr: 31 more"), ranked by time, ties by name.
pub fn system_rows(spans: &[(Kind, String, Acc)], frames: u64, min_ms: f64) -> Vec<Row> {
    let mut by_name: BTreeMap<&str, Acc> = BTreeMap::new();
    for (kind, name, acc) in spans {
        let e = by_name.entry(name.as_str()).or_default();
        match kind {
            Kind::System => {
                e.ns += acc.ns;
                e.count += acc.count;
            }
            Kind::Commands => e.ns += acc.ns,
            _ => {}
        }
    }
    let mut rows = Vec::new();
    let mut groups: BTreeMap<&str, (Acc, usize)> = BTreeMap::new();
    for (name, acc) in by_name {
        if acc.count == 0 && acc.ns == 0 {
            continue;
        }
        let ms = acc.ms_per_frame(frames);
        if name.starts_with(OURS) || ms >= min_ms {
            rows.push(Row { name: name.to_string(), ms, runs: acc.count as f64 / frames.max(1) as f64 });
        } else {
            let g = groups.entry(crate_of(name)).or_default();
            g.0.ns += acc.ns;
            g.0.count += acc.count;
            g.1 += 1;
        }
    }
    for (krate, (acc, n)) in groups {
        let name = format!("{krate}: {n} more");
        rows.push(Row { name, ms: acc.ms_per_frame(frames), runs: acc.count as f64 / frames.max(1) as f64 });
    }
    rows.sort_by(|a, b| b.ms.total_cmp(&a.ms).then_with(|| a.name.cmp(&b.name)));
    rows
}

/// Rows of one kind (schedules, apps), ranked by time per frame.
pub fn rows_of(spans: &[(Kind, String, Acc)], kind: Kind, frames: u64) -> Vec<Row> {
    let mut rows: Vec<Row> = spans
        .iter()
        .filter(|(k, _, _)| *k == kind)
        .map(|(_, name, acc)| Row {
            name: name.clone(),
            ms: acc.ms_per_frame(frames),
            runs: acc.count as f64 / frames.max(1) as f64,
        })
        .collect();
    rows.sort_by(|a, b| b.ms.total_cmp(&a.ms).then_with(|| a.name.cmp(&b.name)));
    rows
}

/// GPU time per segment from one frame's timestamps (`(marker, ticks)` in
/// the order written): each segment ends at a marker and is named by it;
/// a tick count that goes backwards (a reset counter) gives nothing.
pub fn gpu_segments(marks: &[(usize, u64)], period_ns: f32) -> Vec<(usize, f64)> {
    marks
        .windows(2)
        .filter(|w| w[1].1 >= w[0].1)
        .map(|w| (w[1].0, (w[1].1 - w[0].1) as f64 * f64::from(period_ns) / 1e6))
        .collect()
}

/// Per-frame counts that drive the cost (means over the window).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    /// Terrain chunks selected by the LOD, and those in the view frustum.
    pub chunks: f64,
    pub chunks_in_view: f64,
    /// Terrain triangles in view.
    pub triangles: f64,
    /// Meshes in the main view (about the draw calls of the main passes).
    pub meshes_in_view: f64,
    /// Trajectory line points drawn, and evaluated this frame.
    pub line_points: f64,
    pub line_evaluated: f64,
    /// egui: clipped primitives and vertices.
    pub egui_primitives: f64,
    pub egui_vertices: f64,
    /// Terrain chunks built per second on the background threads, and the
    /// CPU time they took (ms per second, all threads).
    pub chunk_builds_per_s: f64,
    pub chunk_build_ms_per_s: f64,
    /// The fleet's advance (ms per frame, `SimState::sim_ms`).
    pub sim_ms: f64,
    /// The 3D view's internal scale (`render_scale`; 1: full resolution).
    pub render_scale: f64,
}

/// Everything one report shows.
#[derive(Clone, Debug, Default)]
pub struct Window {
    pub label: String,
    pub seconds: f64,
    pub frames: FrameStats,
    pub spans: Vec<(Kind, String, Acc)>,
    /// GPU ms per frame per segment (`None`: no timestamp queries).
    pub gpu: Option<Vec<(String, f64)>>,
    pub counts: Counts,
}

/// Systems shown by name below this (ms per frame) are grouped.
pub const MIN_SYSTEM_MS: f64 = 0.05;
/// Rows of the system table printed.
pub const TOP_SYSTEMS: usize = 30;

/// The system whose run is the wait for the next swapchain image (the
/// display's refresh or the GPU's backlog): `get_current_texture`.
pub const ACQUIRE: &str = "bevy_render::view::window::prepare_windows";

/// The report as text (terminal and file).
pub fn format(w: &Window) -> String {
    let f = w.frames.frames;
    let mut out = String::new();
    let fs = &w.frames;
    let _ = writeln!(
        out,
        "=== profile: {} ({:.1} s, {} frames) ===\nframe {:.2} ms avg, p95 {:.2}, worst {:.2} ({:.1} fps)",
        w.label,
        w.seconds,
        f,
        fs.avg_ms,
        fs.p95_ms,
        fs.worst_ms,
        fs.fps()
    );
    let apps = rows_of(&w.spans, Kind::App, f);
    let app = |name: &str| apps.iter().find(|r| r.name == name).map_or(0.0, |r| r.ms);
    let acquire = system_ms(&w.spans, ACQUIRE, f);
    let _ = writeln!(
        out,
        "CPU: main thread {:.2} ms | render thread {:.2} ms (of which waiting for a swapchain image: {:.2} ms, vsync or GPU) | fleet (sim) {:.2} ms",
        app("main app"),
        app("render app"),
        acquire,
        w.counts.sim_ms
    );
    match &w.gpu {
        Some(segs) => {
            let total: f64 = segs.iter().map(|s| s.1).sum();
            let parts: Vec<String> = segs.iter().map(|(n, ms)| format!("{n} {ms:.2}")).collect();
            let _ = writeln!(out, "GPU {total:.2} ms: {}", parts.join(" | "));
        }
        None => {
            let _ = writeln!(
                out,
                "GPU: not measured per pass on this adapter (Metal); when GPU-bound the render thread waits for it (submit, camera_driver); the benchmark (bench_*.md) gives each part's cost"
            );
        }
    }
    let c = &w.counts;
    let _ = writeln!(
        out,
        "counts/frame: terrain chunks {:.0} ({:.0} in view, {:.2} M triangles) | meshes in view {:.0} | line points {:.0} ({:.0} evaluated) | egui {:.0} primitives, {:.0} k vertices",
        c.chunks,
        c.chunks_in_view,
        c.triangles / 1e6,
        c.meshes_in_view,
        c.line_points,
        c.line_evaluated,
        c.egui_primitives,
        c.egui_vertices / 1e3
    );
    let _ = writeln!(
        out,
        "background: terrain chunks built {:.0}/s, {:.0} ms CPU per s | render scale {:.2}",
        c.chunk_builds_per_s, c.chunk_build_ms_per_s, c.render_scale
    );
    let sched: Vec<String> =
        rows_of(&w.spans, Kind::Schedule, f).iter().take(12).map(|r| format!("{} {:.2}", r.name, r.ms)).collect();
    let _ = writeln!(out, "schedules (wall ms/frame): {}", sched.join(" | "));
    let _ = writeln!(out, "systems (CPU ms/frame, summed over threads; runs/frame):");
    for r in system_rows(&w.spans, f, MIN_SYSTEM_MS).iter().take(TOP_SYSTEMS) {
        let _ = writeln!(out, "  {:>7.3}  {:>5.1}  {}", r.ms, r.runs, r.name);
    }
    out
}

/// A system's time per frame (its commands included).
pub fn system_ms(spans: &[(Kind, String, Acc)], name: &str, frames: u64) -> f64 {
    spans
        .iter()
        .filter(|(k, n, _)| matches!(k, Kind::System | Kind::Commands) && n == name)
        .map(|(_, _, a)| a.ms_per_frame(frames))
        .sum()
}

/// The few lines the Utilities ▸ Diagnostics section shows.
pub fn summary(w: &Window) -> Vec<(String, String)> {
    let f = w.frames.frames;
    let fs = &w.frames;
    let mut rows =
        vec![("FRAME".to_string(), format!("{:.1} ms · p95 {:.1} · {:.0} fps", fs.avg_ms, fs.p95_ms, fs.fps()))];
    let apps = rows_of(&w.spans, Kind::App, f);
    let app = |name: &str| apps.iter().find(|r| r.name == name).map_or(0.0, |r| r.ms);
    rows.push((
        "CPU MAIN / RENDER".into(),
        format!("{:.1} / {:.1} ms (wait {:.1})", app("main app"), app("render app"), system_ms(&w.spans, ACQUIRE, f)),
    ));
    if let Some(segs) = &w.gpu {
        let total: f64 = segs.iter().map(|s| s.1).sum();
        rows.push(("GPU".into(), format!("{total:.1} ms")));
        let mut top: Vec<&(String, f64)> = segs.iter().collect();
        top.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (n, ms) in top.into_iter().take(3) {
            rows.push((format!("  {}", n.to_uppercase()), format!("{ms:.2} ms")));
        }
    }
    for r in system_rows(&w.spans, f, MIN_SYSTEM_MS).iter().filter(|r| r.name.starts_with(OURS)).take(3) {
        rows.push((format!("  {}", r.name.trim_start_matches(OURS)), format!("{:.2} ms", r.ms)));
    }
    let c = &w.counts;
    rows.push(("TERRAIN".into(), format!("{:.0} chunks · {:.2} M tris", c.chunks_in_view, c.triangles / 1e6)));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(ms: f64, count: u64) -> Acc {
        Acc { ns: (ms * 1e6) as u64, count }
    }

    #[test]
    fn frame_statistics() {
        // (frame times in s, avg ms, p95 ms, worst ms)
        let cases: [(&[f64], f64, f64, f64); 3] = [
            (&[], 0.0, 0.0, 0.0),
            (&[0.010], 10.0, 10.0, 10.0),
            (&[0.008, 0.008, 0.008, 0.008, 0.040], 14.4, 40.0, 40.0),
        ];
        for (dt, avg, p95, worst) in cases {
            let s = frame_stats(dt);
            assert!((s.avg_ms - avg).abs() < 1e-9 && s.p95_ms == p95 && s.worst_ms == worst, "{dt:?}: {s:?}");
        }
        let twenty: Vec<f64> = (1..=20).map(|k| f64::from(k) * 1e-3).collect();
        assert_eq!(frame_stats(&twenty).p95_ms, 20.0);
        assert!((frame_stats(&[0.025; 4]).fps() - 40.0).abs() < 1e-9);
    }

    #[test]
    fn the_system_table_lists_ours_by_name_and_groups_bevys_small_ones() {
        let spans = vec![
            (Kind::System, "game::terrain::lod::update".to_string(), acc(20.0, 10)),
            (Kind::Commands, "game::terrain::lod::update".to_string(), acc(5.0, 10)),
            (Kind::System, "game::sky::update_stars".to_string(), acc(0.1, 10)),
            (Kind::System, "bevy_render::view::window::prepare_windows".to_string(), acc(40.0, 10)),
            (Kind::System, "bevy_pbr::a".to_string(), acc(0.2, 10)),
            (Kind::System, "bevy_pbr::b".to_string(), acc(0.1, 10)),
            (Kind::System, "<bevy_egui::X as Y>::z".to_string(), acc(0.3, 10)),
            (Kind::Schedule, "Update".to_string(), acc(99.0, 10)),
        ];
        let rows = system_rows(&spans, 10, MIN_SYSTEM_MS);
        let names: Vec<(&str, f64)> = rows.iter().map(|r| (r.name.as_str(), (r.ms * 1e3).round() / 1e3)).collect();
        assert_eq!(
            names,
            [
                ("bevy_render::view::window::prepare_windows", 4.0),
                ("game::terrain::lod::update", 2.5), // its commands included
                ("bevy_egui: 1 more", 0.03),         // a tie: by name
                ("bevy_pbr: 2 more", 0.03),
                ("game::sky::update_stars", 0.01), // ours, however small
            ]
        );
        assert_eq!(rows[1].runs, 1.0);
        assert_eq!(system_ms(&spans, "game::terrain::lod::update", 10), 2.5);
    }

    #[test]
    fn crate_of_a_system_path() {
        // (path, crate)
        let cases = [
            ("bevy_pbr::render::light::per_view_shadow_pass<0>", "bevy_pbr"),
            ("<bevy_egui::X as Y>::z", "bevy_egui"),
            ("game::fleet::advance", "game"),
            ("plain", "plain"),
        ];
        for (path, want) in cases {
            assert_eq!(crate_of(path), want, "{path}");
        }
    }

    #[test]
    fn gpu_segments_end_at_their_marker() {
        // Markers 0, 1, 3 written (2 did not run): two segments.
        let segs = gpu_segments(&[(0, 1_000), (1, 3_000_000), (3, 4_000_000)], 1.0);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].0, 1);
        assert!((segs[0].1 - 2.999).abs() < 1e-9);
        assert_eq!(segs[1].0, 3);
        assert!((segs[1].1 - 1.0).abs() < 1e-9);
        // A period of 2 ns per tick doubles them; a counter going back is dropped.
        assert!((gpu_segments(&[(0, 0), (1, 1_000_000)], 2.0)[0].1 - 2.0).abs() < 1e-9);
        assert!(gpu_segments(&[(0, 5), (1, 4)], 1.0).is_empty());
        assert!(gpu_segments(&[(0, 5)], 1.0).is_empty());
    }

    #[test]
    fn the_report_names_the_frame_the_gpu_and_the_top_systems() {
        let w = Window {
            label: "test".into(),
            seconds: 1.0,
            frames: frame_stats(&[0.037; 10]),
            spans: vec![
                (Kind::App, "main app".into(), acc(50.0, 10)),
                (Kind::App, "render app".into(), acc(300.0, 10)),
                (Kind::System, ACQUIRE.into(), acc(200.0, 10)),
                (Kind::System, "game::terrain::lod::update".into(), acc(10.0, 10)),
                (Kind::Schedule, "Update".into(), acc(30.0, 10)),
            ],
            gpu: Some(vec![("opaque".into(), 20.0), ("sky".into(), 12.0)]),
            counts: Counts { chunks: 400.0, chunks_in_view: 150.0, triangles: 345_600.0, ..Counts::default() },
        };
        let text = format(&w);
        assert!(text.contains("frame 37.00 ms avg"), "{text}");
        assert!(text.contains("main thread 5.00 ms | render thread 30.00 ms"), "{text}");
        assert!(text.contains("waiting for a swapchain image: 20.00 ms"), "{text}");
        assert!(text.contains("GPU 32.00 ms: opaque 20.00 | sky 12.00"), "{text}");
        assert!(text.contains("terrain chunks 400 (150 in view, 0.35 M triangles)"), "{text}");
        assert!(text.contains("game::terrain::lod::update"), "{text}");
        assert!(text.contains("Update 3.00"), "{text}");
        let s = summary(&w);
        assert_eq!(s[0], ("FRAME".to_string(), "37.0 ms · p95 37.0 · 27 fps".to_string()));
        assert!(s.iter().any(|(k, v)| k == "  OPAQUE" && v == "20.00 ms"), "{s:?}");
        assert!(s.iter().any(|(k, _)| k == "  terrain::lod::update"), "{s:?}");
        let none = Window { gpu: None, ..w };
        assert!(format(&none).contains("GPU: not measured per pass"));
    }
}
