use crate::bodies::{SECTOR_GRID_SIZE, SECTOR_SIDE_LY, LIGHT_YEAR};

/// Compute how many stars a sector should contain based on its galactic position.
///
/// Calibrated against observed Milky Way stellar densities:
/// - Solar neighborhood (R≈26,000 ly): ~4.3 ly average inter-star distance
///   (0.14 stars/pc³ midplane, projected into 2D → ~540 stars per sector)
/// - Galactic center: ~0.6 ly average distance (~29,000 stars/sector)
/// - Outskirts (R≈50,000 ly): ~18 ly average distance (~30 stars/sector)
///
/// Models:
/// - Exponential disk (scale length 8,500 ly, matching thin disk observations)
/// - Gaussian bulge (σ = 2,000 ly, half-light radius ~3,300 ly)
pub fn sector_star_count(sector_x: u16, sector_y: u16, galaxy_seed: u64) -> u32 {
    // Sector center in light-years relative to galactic center (Sgr A*)
    let cx_ly = (sector_x as f64 - SECTOR_GRID_SIZE as f64 / 2.0 + 0.5) * SECTOR_SIDE_LY;
    let cy_ly = (sector_y as f64 - SECTOR_GRID_SIZE as f64 / 2.0 + 0.5) * SECTOR_SIDE_LY;
    let r_ly = (cx_ly * cx_ly + cy_ly * cy_ly).sqrt();

    // --- Exponential disk (thin disk scale length: 2.6 kpc ≈ 8,500 ly) ---
    let disk_scale = 8_500.0;
    let disk = (-r_ly / disk_scale).exp();

    // --- Galactic bulge (Gaussian, σ = 2,000 ly) ---
    let bulge_sigma = 2_000.0;
    let bulge = 1.5 * (-r_ly * r_ly / (2.0 * bulge_sigma * bulge_sigma)).exp();

    let density = disk + bulge;

    // Multiplier calibrated so R=26,000 ly gives ~540 stars/sector (avg 4.3 ly spacing).
    // At that radius: disk≈0.047, bulge≈0 → density≈0.047.
    // 0.047 * 11,500 ≈ 540.
    let base_count = density * 11_500.0;

    // Deterministic jitter ±15%
    let hash = super::prng::Rng64::mix(galaxy_seed, sector_x as u64 | ((sector_y as u64) << 16));
    let variation = (hash & 0xFFFF) as f64 / 65536.0;
    let jitter = 0.85 + 0.30 * variation;

    let count = (base_count * jitter).round() as u32;
    // Cap to bound galactic center sectors (which would otherwise have ~29,000 stars)
    // to a manageable count while preserving visual density.
    const MAX_STARS_PER_SECTOR: u32 = 2_000;
    count.clamp(1, MAX_STARS_PER_SECTOR)
}

/// Compute the world-space origin of a sector (bottom-left corner) in meters.
pub fn sector_origin_meters(sector_x: u16, sector_y: u16) -> [f64; 2] {
    let half = (SECTOR_GRID_SIZE as f64 / 2.0) * SECTOR_SIDE_LY * LIGHT_YEAR;
    [
        sector_x as f64 * SECTOR_SIDE_LY * LIGHT_YEAR - half,
        sector_y as f64 * SECTOR_SIDE_LY * LIGHT_YEAR - half,
    ]
}
