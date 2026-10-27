//! The look's benchmark: frame times of the one look (D073) and of the look
//! without each of its parts (`look::Parts::each_off`), measured in the
//! running app from the current view: what each part costs. The parts are
//! switched for the measurement only (never a setting). Started from
//! Settings ▸ Interface or by the demo.

use crate::look::Parts;
use bevy::prelude::*;

/// Seconds to let a configuration settle (pipelines compile, LOD builds)
/// before measuring, and how long to measure.
const WARMUP: f64 = 1.5;
const MEASURE: f64 = 2.5;

/// The baseline's label: the whole look.
pub const LOOK: &str = "the look";

#[derive(Clone, Debug)]
pub struct BenchResult {
    pub label: String,
    pub avg_ms: f64,
    pub p95_ms: f64,
}

#[derive(Resource, Default)]
pub struct Bench {
    queue: Vec<(String, Parts)>,
    current: Option<(String, Parts)>,
    timer: f64,
    samples: Vec<f64>,
    pub results: Vec<BenchResult>,
    /// Label of the view the results were measured from.
    pub view: String,
}

/// Every configuration measured: the look, then the look without each part.
pub fn configurations() -> Vec<(String, Parts)> {
    let mut v = vec![(LOOK.to_string(), Parts::default())];
    v.extend(Parts::each_off().into_iter().map(|(name, p)| (format!("−{name}"), p)));
    v
}

impl Bench {
    pub fn running(&self) -> bool {
        self.current.is_some() || !self.queue.is_empty()
    }

    /// Starts measuring every configuration, or only those named in
    /// `SUNSCATTER_BENCH_ONLY` (a developer filter: comma-separated label
    /// fragments, run in that order).
    pub fn start(&mut self, view: &str) {
        let mut q = configurations();
        if let Ok(only) = std::env::var("SUNSCATTER_BENCH_ONLY") {
            let all = q;
            q = only
                .split(',')
                .filter_map(|want| all.iter().find(|(label, _)| label.contains(want.trim())).cloned())
                .collect();
        }
        q.reverse();
        self.queue = q;
        self.current = None;
        self.results.clear();
        self.view = view.to_string();
    }

    /// Progress as (done, total).
    pub fn progress(&self) -> (usize, usize) {
        let done = self.results.len();
        (done, done + self.queue.len() + usize::from(self.current.is_some()))
    }
}

/// The whole look's frame time (ms), the cost column's reference.
pub fn baseline(results: &[BenchResult]) -> Option<f64> {
    results.iter().find(|r| r.label == LOOK).map(|r| r.avg_ms)
}

pub fn run(time: Res<Time>, mut bench: ResMut<Bench>, mut parts: ResMut<Parts>) {
    if !bench.running() {
        return;
    }
    let dt = time.delta_secs_f64();
    if bench.current.is_none() {
        let next = bench.queue.pop().expect("running");
        parts.set_if_neq(next.1);
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
            parts.set_if_neq(Parts::default());
        }
    }
}

/// Results as a Markdown table: each part's cost is the look's frame time
/// less the look's without it.
pub fn markdown(bench: &Bench) -> String {
    let base = baseline(&bench.results).unwrap_or(0.0);
    let mut out =
        format!("View: {}\n\n| Configuration | avg ms | p95 ms | part's cost |\n|---|---|---|---|\n", bench.view);
    for r in &bench.results {
        out += &format!("| {} | {:.2} | {:.2} | {:+.2} ms |\n", r.label, r.avg_ms, r.p95_ms, base - r.avg_ms);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_gives_each_parts_cost_against_the_look() {
        let bench = Bench {
            results: vec![
                BenchResult { label: LOOK.into(), avg_ms: 8.0, p95_ms: 9.0 },
                BenchResult { label: "−atmosphere".into(), avg_ms: 5.5, p95_ms: 6.0 },
            ],
            view: "orbit".into(),
            ..Bench::default()
        };
        let md = markdown(&bench);
        assert!(md.contains("| the look | 8.00 | 9.00 | +0.00 ms |"), "{md}");
        assert!(md.contains("| −atmosphere | 5.50 | 6.00 | +2.50 ms |"), "{md}");
        let labels: Vec<String> = configurations().into_iter().map(|c| c.0).collect();
        assert_eq!(labels[0], LOOK);
        assert!(labels.contains(&"−relief".to_string()) && labels.len() == 10);
    }
}
