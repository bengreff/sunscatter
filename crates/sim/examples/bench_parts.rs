//! What splitting a craft of parts costs (`docs/features/parts.md`, D083):
//! the pieces' craft (union surface, cells, mass) and designs (aero bake,
//! volume grid, thermal network), built fresh as the registry would, and
//! the split itself once they exist. Stress craft (`sim::craft::stress`).
//! Release only:
//!
//! `cargo run -p sim --release --example bench_parts [parts=300] [runs=3]`
//!
//! Each timing is the median of `runs` runs.
use sim::craft::registry::{cell_options, definition};
use sim::craft::{craft_of, stress, Craft, CraftId, Resolution};
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::vessel::{Controls, Vessel, VesselId, VesselIds};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let clock = Instant::now();
    let out = f();
    (out, clock.elapsed().as_secs_f64() * 1e3)
}

fn median(runs: usize, mut f: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..runs.max(1)).map(|_| f()).collect();
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// A piece's craft built fresh (not from the registry's cache): build and
/// design times (ms), cells and nodes.
fn piece(id: &CraftId, runs: usize) -> (f64, f64, usize, usize) {
    let def = definition(id).unwrap();
    let opts = cell_options(id, &def);
    let mut built: Option<Craft> = None;
    let build = median(runs, || {
        let (c, ms) = timed(|| Craft::from_def(id.clone(), def.clone(), &Resolution::default(), &opts));
        built = Some(c);
        ms
    });
    let c = built.unwrap();
    let design = median(runs, || timed(|| sim::craft::CraftDesign::new(&c)).1);
    (build, design, c.cells.cells.len(), c.design().nodes())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| args.iter().find_map(|a| a.strip_prefix(key).map(str::to_owned));
    let n: usize = value("parts=").map_or(300, |s| s.parse().unwrap());
    let runs: usize = value("runs=").map_or(3, |s| s.parse().unwrap());
    let w = world();
    let c = craft_of(&stress::id(n));
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 0.0, argp: 0.0, mean_anomaly: 0.0 }.to_state(earth.gm);
    let mut base = Vessel::coasting(&w, VesselId(1), sol::sol_epoch(), earth.node, r, v, &c);
    base.advance(&w, sol::sol_epoch().add_seconds(10.0), &Controls::default(), usize::MAX);
    let named = |prefix: &str| (0..c.parts.len() as u32).find(|&i| c.parts[i as usize].name.starts_with(prefix));
    println!(
        "bench_parts: {n}-part stress craft ({} cells, {} nodes), {runs} runs per timing",
        c.cells.cells.len(),
        c.design().nodes()
    );
    println!("\n| split | pieces (parts) | plan | split (crafts built) | per piece: build, design (cells, nodes) |");
    println!("|---|---|---|---|---|");
    let cases = [
        ("decoupler 1", "upper stack at the first decoupler"),
        ("booster 1.0 strut 0", "a booster"),
        ("panel", "a panel"),
        ("fin 0", "a fin and its leg"),
    ];
    for (part, what) in cases {
        let Some(p) = named(part) else { continue };
        let plan_ms = median(runs, || timed(|| base.plan_detach(&[p])).1);
        let plan = base.plan_detach(&[p]);
        let sizes: Vec<usize> = plan.pieces.iter().map(Vec::len).collect();
        let pieces: Vec<String> = plan
            .ids
            .iter()
            .map(|id| {
                let (b, d, cells, nodes) = piece(id, runs);
                format!("{b:.0} + {d:.0} ms ({cells}, {nodes})")
            })
            .collect();
        let crafts = plan.crafts();
        let split_ms = median(runs, || {
            let mut v = base.clone();
            timed(|| v.split(&w, &plan, &crafts, &mut VesselIds::default())).1
        });
        println!("| {what} | {sizes:?} | {plan_ms:.2} ms | {split_ms:.1} ms | {} |", pieces.join("; "));
    }
}
