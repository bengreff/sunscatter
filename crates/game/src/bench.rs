//! Graphics benchmark: frame times per tier and per feature (each feature
//! alone on top of Minimal), measured in the running app from the current
//! view. Started from the settings window or by the demo.

use crate::settings::{AtmosphereQuality, GraphicsSettings, MsaaLevel, Tier};
use bevy::prelude::*;

/// Seconds to let a configuration settle (pipelines compile, LOD builds)
/// before measuring, and how long to measure.
const WARMUP: f64 = 1.5;
const MEASURE: f64 = 2.5;

#[derive(Clone, Debug)]
pub struct BenchResult {
    pub label: String,
    pub avg_ms: f64,
    pub p95_ms: f64,
}

#[derive(Resource, Default)]
pub struct Bench {
    queue: Vec<(String, GraphicsSettings)>,
    current: Option<(String, GraphicsSettings)>,
    timer: f64,
    samples: Vec<f64>,
    /// Settings to restore when finished.
    restore: Option<GraphicsSettings>,
    pub results: Vec<BenchResult>,
    /// Label of the view the results were measured from.
    pub view: String,
}

/// Every configuration measured: the five tiers, then each feature alone.
pub fn configurations() -> Vec<(String, GraphicsSettings)> {
    let mut v: Vec<(String, GraphicsSettings)> =
        Tier::ALL.iter().map(|&t| (format!("tier {}", t.name()), GraphicsSettings::preset(t))).collect();
    let min = GraphicsSettings { tier: None, ..GraphicsSettings::preset(Tier::Minimal) };
    let ultra = GraphicsSettings::preset(Tier::Ultra);
    let features: [(&str, GraphicsSettings); 11] = [
        ("terrain (Ultra LOD)", GraphicsSettings { terrain: true, terrain_error_px: 1.0, ..min }),
        ("textures 8k", GraphicsSettings { texture_size: ultra.texture_size, ..min }),
        ("detail layer", GraphicsSettings { terrain: true, detail: true, ..min }),
        ("atmosphere LUT", GraphicsSettings { atmosphere: AtmosphereQuality::Lut, ..min }),
        ("atmosphere raymarched", GraphicsSettings { atmosphere: AtmosphereQuality::Raymarched, ..min }),
        ("ocean glint", GraphicsSettings { ocean_glint: true, ..min }),
        ("stars mag 8", GraphicsSettings { star_magnitude: 8.0, ..min }),
        ("bloom", GraphicsSettings { bloom: true, ..min }),
        ("flare", GraphicsSettings { flare: true, ..min }),
        ("shadows", GraphicsSettings { shadows: true, ..min }),
        ("MSAA 4x", GraphicsSettings { msaa: MsaaLevel::X4, ..min }),
    ];
    v.extend(features.into_iter().map(|(n, s)| (format!("+{n}"), s)));
    v.push(("+earthshine".into(), GraphicsSettings { earthshine: true, ..min }));
    v
}

impl Bench {
    pub fn running(&self) -> bool {
        self.current.is_some() || !self.queue.is_empty()
    }

    /// Starts measuring every configuration, or only those named in
    /// `SUNSCATTER_BENCH_ONLY` (a developer filter: comma-separated label
    /// fragments, run in that order).
    pub fn start(&mut self, view: &str, current: GraphicsSettings) {
        let mut q = configurations();
        if let Ok(only) = std::env::var("SUNSCATTER_BENCH_ONLY") {
            // A comma-separated list of labels, run in that order.
            let all = q;
            q = only
                .split(',')
                .filter_map(|want| all.iter().find(|(label, _)| label.contains(want.trim())).cloned())
                .collect();
        }
        q.reverse();
        self.queue = q;
        self.current = None;
        self.restore = Some(current);
        self.results.clear();
        self.view = view.to_string();
    }

    /// Progress as (done, total).
    pub fn progress(&self) -> (usize, usize) {
        let done = self.results.len();
        (done, done + self.queue.len() + usize::from(self.current.is_some()))
    }
}

/// Minimal-tier baseline (ms) for cost columns.
pub fn baseline(results: &[BenchResult]) -> Option<f64> {
    results.iter().find(|r| r.label == "tier Minimal").map(|r| r.avg_ms)
}

pub fn run(time: Res<Time>, mut bench: ResMut<Bench>, mut settings: ResMut<GraphicsSettings>) {
    if !bench.running() {
        return;
    }
    let dt = time.delta_secs_f64();
    if bench.current.is_none() {
        let next = bench.queue.pop().expect("running");
        *settings = next.1;
        bench.current = Some(next);
        bench.timer = 0.0;
        bench.samples.clear();
        return;
    }
    bench.timer += dt;
    if bench.timer > WARMUP {
        bench.samples.push(dt);
    }
    if bench.timer > WARMUP + MEASURE {
        let (label, _) = bench.current.take().expect("current");
        let mut s = std::mem::take(&mut bench.samples);
        s.sort_by(f64::total_cmp);
        let avg = s.iter().sum::<f64>() / s.len().max(1) as f64;
        let p95 = s.get(s.len() * 95 / 100).copied().unwrap_or(avg);
        info!("bench [{}] {label}: {:.2} ms avg, {:.2} ms p95", bench.view, avg * 1e3, p95 * 1e3);
        bench.results.push(BenchResult { label, avg_ms: avg * 1e3, p95_ms: p95 * 1e3 });
        if bench.queue.is_empty() {
            if let Some(r) = bench.restore.take() {
                *settings = r;
            }
        }
    }
}

/// Results as a Markdown table (for the plan's review and the demo log).
pub fn markdown(bench: &Bench) -> String {
    let base = baseline(&bench.results).unwrap_or(0.0);
    let mut out =
        format!("View: {}\n\n| Configuration | avg ms | p95 ms | cost vs Minimal |\n|---|---|---|---|\n", bench.view);
    for r in &bench.results {
        out += &format!("| {} | {:.2} | {:.2} | {:+.2} ms |\n", r.label, r.avg_ms, r.p95_ms, r.avg_ms - base);
    }
    out
}
