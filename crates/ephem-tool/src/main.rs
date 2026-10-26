//! Offline ephemeris generation for the Solar System.
//!
//! ```text
//! cargo run -p ephem-tool --release -- fetch
//! cargo run -p ephem-tool --release -- sol [--bsp data/external/de440s.bsp] [--years 50]
//!     [--out data/ephemeris] [--bodies data/bodies] [--extras all|none|gr|j2]
//! ```
//!
//! `fetch` downloads the JPL Horizons states the catalog
//! (`data/ephemeris/sol-systems.ron`) needs into `data/external/horizons/`
//! (with `curl`; cached). `sol` then:
//!
//! 1. Reads Sun/planet/Earth/Moon states at the epoch from JPL DE440 (via ANISE,
//!    build time only), adds the dwarf planets' from Horizons, and writes them
//!    to `sol-2030-initial.ron`; the moon systems' states (relative to their
//!    barycenters) to `sol-2030-systems-initial.ron`.
//! 2. Writes DE440 reference states at later dates to `de440-reference.ron`
//!    and Horizons ones to `horizons-reference.ron` (used by sim's golden
//!    tests, so CI needs neither the kernel nor the network).
//! 3. Runs `sim::sol::generate_sol` and writes `sol-2030.bin`, printing the
//!    per-node report and the errors against DE440 and Horizons.
//!
//! Download the kernel first:
//! `curl -L -o data/external/de440s.bsp https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de440s.bsp`

mod horizons;
mod sol;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    match args.first().map(String::as_str) {
        Some("fetch") => sol::fetch(),
        Some("sol") => {
            let bsp = PathBuf::from(flag("--bsp").unwrap_or_else(|| "data/external/de440s.bsp".into()));
            let years: f64 = flag("--years").map_or(50.0, |y| y.parse().expect("--years must be a number"));
            let extras = flag("--extras").unwrap_or_else(|| "all".into());
            let out = PathBuf::from(flag("--out").unwrap_or_else(|| "data/ephemeris".into()));
            let bodies = PathBuf::from(flag("--bodies").unwrap_or_else(|| "data/bodies".into()));
            sol::run(&sol::Options { bsp: &bsp, years, extras: &extras, out: &out, bodies: &bodies });
        }
        _ => {
            eprintln!("usage: ephem-tool fetch | sol [--bsp PATH] [--years N] [--out DIR] [--bodies DIR] [--extras all|none|gr|j2]");
            std::process::exit(2);
        }
    }
}
