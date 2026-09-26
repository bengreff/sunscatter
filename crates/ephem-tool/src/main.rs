//! Offline ephemeris generation for the Solar System.
//!
//! ```text
//! cargo run -p ephem-tool --release -- sol [--bsp data/external/de440s.bsp] [--years 50]
//! ```
//!
//! 1. Reads Sun/planet/Earth/Moon states at the epoch from JPL DE440 (via ANISE,
//!    build time only) and writes them to `data/ephemeris/sol-2030-initial.ron`.
//! 2. Writes DE440 reference states at later dates to
//!    `data/ephemeris/de440-reference.ron` (used by sim's golden tests, so CI
//!    doesn't need the 32 MB kernel).
//! 3. Runs `sim::gen::generate` and writes `data/ephemeris/sol-2030.bin`,
//!    printing the per-node report and the error against DE440.
//!
//! Download the kernel first:
//! `curl -L -o data/external/de440s.bsp https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de440s.bsp`

mod sol;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    match args.first().map(String::as_str) {
        Some("sol") => {
            let bsp = PathBuf::from(flag("--bsp").unwrap_or_else(|| "data/external/de440s.bsp".into()));
            let years: f64 = flag("--years").map_or(50.0, |y| y.parse().expect("--years must be a number"));
            let extras = flag("--extras").unwrap_or_else(|| "all".into());
            sol::run(&bsp, years, &extras);
        }
        _ => {
            eprintln!("usage: ephem-tool sol [--bsp PATH] [--years N] [--extras all|none|gr|j2]");
            std::process::exit(2);
        }
    }
}
