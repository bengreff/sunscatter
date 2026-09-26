//! Star catalog: 67 named star systems with real positions and planetary data.
//!
//! Catalog stars are injected into the procedural sector system as ProceduralStar instances
//! with catalog_index > 0. Planetary data is stored in static arrays, looked up when a
//! catalog star is focused for the info panel.

pub mod catalog_data;

use crate::bodies::{G, LIGHT_YEAR, SectorCoord, galactic_enclosed_mass, position_to_sector};
use crate::colony::ResourceType;
use super::{ProceduralStar, StarType};
use super::star_color;

// ── Data structures ──────────────────────────────────────────────────────────

/// A complete star system from the catalog.
pub struct CatalogSystem {
    pub name: &'static str,
    pub description: &'static str,
    pub zone: u8,
    pub distance_ly: f32,
    pub galactic_pos_ly: [f64; 2],
    pub orbit_sma_ly: f64,
    pub orbit_ecc: f64,
    pub orbit_arg_peri_deg: f64,
    pub orbit_mean_anom_deg: f64,
    pub orbit_period_myr: f64,
    pub is_sgr_a_orbit: bool,
    pub stars: &'static [CatalogStar],
    pub bodies: &'static [CatalogBody],
    pub binary_orbits: &'static [StarOrbitData],
}

/// Orbital parameters for a binary/hierarchical stellar pair within a system.
pub struct StarOrbitData {
    pub star_a: usize,       // index into sys.stars[]
    pub star_b: usize,       // index into sys.stars[]
    pub sma_au: f64,         // semi-major axis of the binary pair (AU)
    pub eccentricity: f64,
    pub period_years: f64,
}

/// A single stellar component (e.g. Alpha Centauri A).
pub struct CatalogStar {
    pub name: &'static str,
    pub description: &'static str,
    pub spectral_type: &'static str,
    pub mass_solar: f64,
    pub radius_solar: f64,
    pub luminosity_solar: f64,
}

/// A planet or moon in a catalog system.
pub struct CatalogBody {
    pub name: &'static str,
    pub designation: &'static str,
    pub is_moon: bool,
    pub parent_body_idx: Option<usize>,
    pub orbit_sma_au: f64,
    pub orbit_sma_km: f64,       // used for moons (AU field ignored when > 0)
    pub orbit_ecc: f64,
    pub orbit_period_days: f64,
    pub mass_earth: f64,
    pub radius_earth: f64,
    pub gravity_g: f64,
    pub temperature_k: f64,
    pub atmosphere: Option<CatalogAtmosphere>,
    pub habitability: u32,
    pub science: u32,
    pub resources: &'static [ResourceType],
    pub solar_power_mult: f32,
    pub is_gas_giant: bool,
    pub has_life: bool,
    pub description: &'static str,
}

/// Atmospheric properties for a catalog body.
pub struct CatalogAtmosphere {
    pub pressure_atm: f64,
    pub composition: &'static str,
    pub scale_height_km: f64,
}

/// Determine which star a planet orbits based on its designation string.
/// Rules: "Ab/Ac/..." → star A, "Bb/Bc/..." → star B, "Cb/Cc/..." → star C,
/// "Proxima ..." → last star, "AB..." → 0 (barycenter, treat as primary),
/// bare lowercase "b/c/d" → 0 (single star system).
///
/// Searches the stars array by name to find the correct index, handling systems
/// where sub-components (e.g. "Regulus A companion") shift the letter→index mapping.
pub fn host_star_index(designation: &str, stars: &[CatalogStar]) -> usize {
    let num_stars = stars.len();
    let bytes = designation.as_bytes();
    if bytes.is_empty() || num_stars == 0 { return 0; }

    // "Proxima ..." → last star in the system
    if designation.starts_with("Proxima") {
        return num_stars.saturating_sub(1);
    }

    // Check if first char is uppercase letter (star indicator)
    let first = bytes[0];
    if first.is_ascii_uppercase() {
        // "AB..." → barycenter, assign to primary
        if bytes.len() > 1 && bytes[1].is_ascii_uppercase() {
            return 0;
        }
        // Search for the first star whose name has a word starting with this letter.
        // Handles Regulus (A, A-companion, B, C) and Castor (Aa, Ab, Ba, Bb, Ca, Cb).
        for (i, star) in stars.iter().enumerate() {
            for word in star.name.rsplit(' ') {
                let wb = word.as_bytes();
                if wb.is_empty() { continue; }
                // Skip parenthetical words like "(YY", "Gem)"
                if wb[0] == b'(' || wb[wb.len()-1] == b')' { continue; }
                if wb[0] == first {
                    return i;
                }
                // Stop at the first non-parenthetical word starting with an uppercase letter
                // (skip trailing words like "companion")
                if wb[0].is_ascii_uppercase() {
                    break;
                }
            }
        }
        // Fallback: letter-based index
        let idx = (first - b'A') as usize;
        return idx.min(num_stars.saturating_sub(1));
    }

    // Bare lowercase "b", "c", etc. → single star, index 0
    0
}

// ── Lookup ───────────────────────────────────────────────────────────────────

/// Look up a catalog system by 1-based index (catalog_index on ProceduralStar).
/// Returns None for index 0 (procedural) or out-of-range.
pub fn lookup_system(catalog_index: u16) -> Option<&'static CatalogSystem> {
    if catalog_index == 0 {
        return None;
    }
    catalog_data::CATALOG.get((catalog_index - 1) as usize)
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Convert all catalog systems into ProceduralStar instances grouped by sector.
/// Returns a map from SectorCoord to the catalog stars that belong in that sector.
pub fn build_catalog_stars() -> std::collections::HashMap<SectorCoord, Vec<ProceduralStar>> {
    use std::collections::HashMap;

    const M_SUN: f64 = 1.989e30;
    const M_BH: f64 = 8.26e36; // Sgr A* mass for Zone 5

    // Compute the Sun's actual t=0 galactic position using exact Newton-Raphson.
    // Must match bodies.rs Orbit::position_at() (which renders the Sun dot), NOT
    // galaxy::kepler_position() which uses a first-order approximation for e < 0.1.
    // Sun: a=1.996e20 m, e=0.07, ω=0.0, M₀=4.8534 rad (from bodies.rs)
    let sun_pos = {
        let (a, e, m0) = (1.996e20_f64, 0.07_f64, 4.8534_f64);
        let big_e = super::solve_kepler_nr(m0, e);
        let nu = 2.0 * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
            .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
        let r = a * (1.0 - e * big_e.cos());
        [r * nu.cos(), r * nu.sin()]
    };

    let mut by_sector: HashMap<SectorCoord, Vec<ProceduralStar>> = HashMap::new();

    for (i, sys) in catalog_data::CATALOG.iter().enumerate() {
        let catalog_index = (i + 1) as u16;

        // Skip empty systems (e.g. Sgr A* — Crucible is a real body in bodies.rs)
        let primary = match sys.stars.first() {
            Some(p) => p,
            None => continue,
        };
        let mass_kg = primary.mass_solar * M_SUN;
        let luminosity = primary.luminosity_solar as f32;
        let temperature = spectral_temperature(primary.spectral_type);
        let star_type = spectral_to_star_type(primary.spectral_type);

        // Orbital elements
        let (semi_major_axis, mean_motion, pos, ecc, arg_peri, mean_anomaly_0) = if sys.is_sgr_a_orbit {
            // Zone 5: orbits Sgr A* at AU scale — keep original position
            let sma_m = sys.orbit_sma_ly * 1.496e11; // field stores AU for Zone 5
            let n = (G * M_BH / (sma_m * sma_m * sma_m)).sqrt();
            let pos = [
                sys.galactic_pos_ly[0] * LIGHT_YEAR,
                sys.galactic_pos_ly[1] * LIGHT_YEAR,
            ];
            let ecc = sys.orbit_ecc as f32;
            let arg_peri = sys.orbit_arg_peri_deg.to_radians() as f32;
            let mean_anomaly_0 = sys.orbit_mean_anom_deg.to_radians();
            (sma_m, n, pos, ecc, arg_peri, mean_anomaly_0)
        } else {
            // Non-Zone-5: position star at correct distance from the Sun's actual position.
            // Use each star's own orbital elements (kepler_position) for direction —
            // galactic_pos_ly can't be used because it's in a different coordinate frame
            // than the Sun's kepler_position, causing all directions to collapse.
            let sma_m = sys.orbit_sma_ly * LIGHT_YEAR;
            let ecc_f64 = sys.orbit_ecc;
            let omega = sys.orbit_arg_peri_deg.to_radians();
            let catalog_m0 = sys.orbit_mean_anom_deg.to_radians();

            // Compute where this star's orbit places it at t=0
            let star_kepler_t0 = super::kepler_position(sma_m, ecc_f64, omega, catalog_m0);
            let dx = star_kepler_t0[0] - sun_pos[0];
            let dy = star_kepler_t0[1] - sun_pos[1];
            let dist = (dx * dx + dy * dy).sqrt();
            let (dir_x, dir_y) = if dist > 1.0 {
                (dx / dist, dy / dist)
            } else {
                (1.0, 0.0)
            };
            let corrected_pos = [
                sun_pos[0] + sys.distance_ly as f64 * LIGHT_YEAR * dir_x,
                sun_pos[1] + sys.distance_ly as f64 * LIGHT_YEAR * dir_y,
            ];

            // Recompute orbital elements so kepler_position() reproduces corrected_pos at t=0.
            // Must use the SAME math as kepler_position(): for e < 0.1 it uses first-order
            // approximations ν ≈ M + 2e·sin(M), r ≈ a·(1 − e·cos(M)), so we invert those.
            let r_corr = (corrected_pos[0] * corrected_pos[0] + corrected_pos[1] * corrected_pos[1]).sqrt();
            let angle = corrected_pos[1].atan2(corrected_pos[0]); // = ν + ω in kepler_position

            let (sma_corrected, mean_anomaly_0) = if ecc_f64 < 0.1 && r_corr > 0.0 {
                // Invert first-order: angle = M + 2e·sin(M) + ω → solve for M
                // Fixed-point iteration: M = angle − ω − 2e·sin(M)
                let target = angle - omega; // = ν = M + 2e·sin(M)
                let mut m = target;
                for _ in 0..6 {
                    m = target - 2.0 * ecc_f64 * m.sin();
                }
                // Invert r = a·(1 − e·cos(M)) → a = r / (1 − e·cos(M))
                let a = r_corr / (1.0 - ecc_f64 * m.cos());
                (a, m)
            } else if r_corr > 0.0 {
                // Exact inverse for higher eccentricities (kepler_position uses NR for e >= 0.1)
                let nu = angle - omega;
                let a = r_corr * (1.0 + ecc_f64 * nu.cos()) / (1.0 - ecc_f64 * ecc_f64);
                let big_e = ((1.0 - ecc_f64 * ecc_f64).sqrt() * nu.sin()).atan2(ecc_f64 + nu.cos());
                let m0 = big_e - ecc_f64 * big_e.sin();
                (a, m0)
            } else {
                (sma_m, 0.0)
            };
            // Mean motion from corrected SMA
            let n = if sma_corrected > 0.0 {
                (G * galactic_enclosed_mass(sma_corrected) / (sma_corrected * sma_corrected * sma_corrected)).sqrt()
            } else {
                0.0
            };

            (sma_corrected, n, corrected_pos, ecc_f64 as f32, omega as f32, mean_anomaly_0)
        };

        let color = star_color::stellar_color(temperature);
        let radius_m = StarType::radius_meters(luminosity, temperature);

        let sector = match position_to_sector(pos) {
            Some(s) => s,
            None => continue, // outside grid
        };

        let star = ProceduralStar {
            pos,
            semi_major_axis,
            mean_motion,
            mean_anomaly_0,
            eccentricity: ecc,
            arg_periapsis: arg_peri,
            mass: mass_kg,
            temperature,
            luminosity,
            color,
            radius_m,
            sector_index: 10_000 + i as u32, // high index to avoid collisions
            flags: 1, // bit 0: is_predefined
            star_type,
            catalog_index,
        };

        by_sector.entry(sector).or_default().push(star);
    }

    by_sector
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Estimate effective temperature from spectral type string (e.g. "G2V", "M4V", "A1V").
fn spectral_temperature(spec: &str) -> f32 {
    let bytes = spec.as_bytes();
    if bytes.is_empty() {
        return 5800.0;
    }
    let class = bytes[0] as char;
    let subtype: f32 = if bytes.len() > 1 && bytes[1].is_ascii_digit() {
        (bytes[1] - b'0') as f32
    } else {
        5.0
    };

    // Linear interpolation within each spectral class
    match class {
        'O' => 50000.0 - subtype * 2000.0,
        'B' => 30000.0 - subtype * 2000.0,
        'A' => 10000.0 - subtype * 250.0,
        'F' => 7500.0 - subtype * 150.0,
        'G' => 6000.0 - subtype * 80.0,
        'K' => 5200.0 - subtype * 150.0,
        'M' => 3700.0 - subtype * 185.0,
        'D' => 25000.0, // white dwarf (DA2 etc.)
        _ => 5800.0,
    }
}

/// Map spectral type string to StarType enum.
pub fn spectral_to_star_type(spec: &str) -> StarType {
    let bytes = spec.as_bytes();
    if bytes.is_empty() {
        return StarType::MainSequence('G');
    }
    match bytes[0] as char {
        'D' => StarType::WhiteDwarf,
        c @ ('O' | 'B' | 'A' | 'F' | 'G' | 'K' | 'M') => StarType::MainSequence(c),
        _ => StarType::MainSequence('G'),
    }
}
