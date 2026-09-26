use crate::bodies::{G, SECTOR_SIDE_LY, LIGHT_YEAR, galactic_enclosed_mass};
use super::prng::Rng64;
use super::density;
use super::star_color;
use super::{ProceduralStar, StarType, solve_kepler_nr};

/// No procedural stars within this radius of the Sun. The catalog handles
/// the solar neighborhood; without this, random stars appear closer than
/// Alpha Centauri (4.37 ly) due to the ~4.3 ly average procedural spacing.
const SUN_EXCLUSION_RADIUS_LY: f64 = 15.0;

/// Generate all stars for a sector, deterministically from the galaxy seed.
pub fn generate_sector(
    sector_x: u16,
    sector_y: u16,
    galaxy_seed: u64,
) -> Vec<ProceduralStar> {
    let count = density::sector_star_count(sector_x, sector_y, galaxy_seed);
    let origin = density::sector_origin_meters(sector_x, sector_y);
    let sector_meters = SECTOR_SIDE_LY * LIGHT_YEAR;

    // Sun's t=0 galactic position (matches catalog.rs / bodies.rs)
    let sun_pos = {
        let (a, e, m0) = (1.996e20_f64, 0.07_f64, 4.8534_f64);
        let big_e = solve_kepler_nr(m0, e);
        let nu = 2.0 * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
            .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
        let r = a * (1.0 - e * big_e.cos());
        [r * nu.cos(), r * nu.sin()]
    };
    let exclusion_r2 = (SUN_EXCLUSION_RADIUS_LY * LIGHT_YEAR) * (SUN_EXCLUSION_RADIUS_LY * LIGHT_YEAR);

    let mut stars = Vec::with_capacity(count as usize);

    for i in 0..count {
        // Deterministic seed per star
        let star_seed = Rng64::mix(
            Rng64::mix(galaxy_seed, sector_x as u64 | ((sector_y as u64) << 16)),
            i as u64,
        );
        let mut rng = Rng64::new(star_seed);

        // Position: uniform within sector
        let px = origin[0] + rng.next_f64() * sector_meters;
        let py = origin[1] + rng.next_f64() * sector_meters;

        // Skip stars inside the Sun's exclusion zone (catalog stars cover this region)
        let dx = px - sun_pos[0];
        let dy = py - sun_pos[1];
        if dx * dx + dy * dy < exclusion_r2 {
            continue;
        }

        // Spectral type by initial mass function (IMF)
        // Weights: M=76%, K=12%, G=7%, F=3%, A=1.5%, B=0.4%, O=0.1%
        let spectral_roll = rng.next_f64() * 100.0;
        let (temp_lo, temp_hi, mass_lo, mass_hi) = if spectral_roll < 76.0 {
            // M: 2400-3700K, 0.08-0.45 solar masses
            (2400.0, 3700.0, 0.08, 0.45)
        } else if spectral_roll < 88.0 {
            // K: 3700-5200K, 0.45-0.8
            (3700.0, 5200.0, 0.45, 0.8)
        } else if spectral_roll < 95.0 {
            // G: 5200-6000K, 0.8-1.04
            (5200.0, 6000.0, 0.8, 1.04)
        } else if spectral_roll < 98.0 {
            // F: 6000-7500K, 1.04-1.4
            (6000.0, 7500.0, 1.04, 1.4)
        } else if spectral_roll < 99.5 {
            // A: 7500-10000K, 1.4-2.1
            (7500.0, 10000.0, 1.4, 2.1)
        } else if spectral_roll < 99.9 {
            // B: 10000-30000K, 2.1-16.0
            (10000.0, 30000.0, 2.1, 16.0)
        } else {
            // O: 30000-50000K, 16.0-90.0
            (30000.0, 50000.0, 16.0, 90.0)
        };

        let mut temperature = rng.range_f64(temp_lo, temp_hi) as f32;
        let mut mass_solar = rng.range_f64(mass_lo, mass_hi);

        // Determine spectral class from initial temperature range
        let spectral_class = if spectral_roll < 76.0 { 'M' }
            else if spectral_roll < 88.0 { 'K' }
            else if spectral_roll < 95.0 { 'G' }
            else if spectral_roll < 98.0 { 'F' }
            else if spectral_roll < 99.5 { 'A' }
            else if spectral_roll < 99.9 { 'B' }
            else { 'O' };

        // Secondary evolution roll: override to evolved stellar type
        // ~6% white dwarfs, ~2% red giants, ~0.01% supergiants, ~0.1% neutron stars
        let evolution_roll = rng.next_f64() * 100.0;
        let luminosity;
        let star_type;
        if evolution_roll < 6.0 {
            // White Dwarf: remnant core, hot but tiny surface area → very dim
            mass_solar = rng.range_f64(0.5, 0.8);
            temperature = rng.range_f64(4_000.0, 40_000.0) as f32;
            luminosity = rng.range_f64(0.0001, 0.01) as f32;
            star_type = StarType::WhiteDwarf;
        } else if evolution_roll < 8.0 {
            // Red Giant: evolved off main sequence, cool but enormous → very bright
            mass_solar = rng.range_f64(0.8, 8.0);
            temperature = rng.range_f64(3_000.0, 5_000.0) as f32;
            luminosity = rng.range_f64(50.0, 2_000.0) as f32;
            star_type = StarType::RedGiant;
        } else if evolution_roll < 8.01 {
            // Supergiant: massive evolved star, enormous luminosity
            mass_solar = rng.range_f64(10.0, 70.0);
            temperature = rng.range_f64(3_500.0, 30_000.0) as f32;
            luminosity = rng.range_f64(10_000.0, 500_000.0) as f32;
            star_type = StarType::Supergiant;
        } else if evolution_roll < 8.11 {
            // Neutron Star: ultra-dense remnant, extremely hot but tiny → nearly invisible
            mass_solar = rng.range_f64(1.4, 2.1);
            temperature = rng.range_f64(100_000.0, 1_000_000.0) as f32;
            luminosity = rng.range_f64(0.00001, 0.0001) as f32;
            star_type = StarType::NeutronStar;
        } else {
            // Main Sequence: L ∝ M^3.5
            luminosity = (mass_solar.powf(3.5)) as f32;
            star_type = StarType::MainSequence(spectral_class);
        }

        let mass_kg = mass_solar * 1.989e30; // solar mass in kg

        // Elliptical galactic orbital elements
        let galactic_r = (px * px + py * py).sqrt();
        let theta_0 = py.atan2(px);

        // Generate eccentricity from Rayleigh distribution with radius-dependent σ
        // σ(r) = 0.05 + 0.20·exp(−r/2kpc) + 0.03·(r/20kpc)
        const KPC: f64 = 3.086e19; // 1 kiloparsec in meters
        let sigma = 0.05 + 0.20 * (-galactic_r / (2.0 * KPC)).exp()
            + 0.03 * (galactic_r / (20.0 * KPC));
        let u = rng.next_f64().max(1e-10);
        let ecc = (sigma * (-2.0 * u.ln()).sqrt()).min(0.95) as f32;
        let e = ecc as f64;

        // Random mean anomaly at t=0
        let mean_anomaly_0 = rng.next_f64() * std::f64::consts::TAU;

        // Derive consistent orbital elements so kepler_position(a,e,ω,M₀) = (px, py).
        // IMPORTANT: kepler_position() uses a first-order approximation when e < 0.1,
        // so we must derive elements using the SAME approximation. Otherwise the rendered
        // t=0 position diverges from (px, py) by O(e²·a) — up to ~100 ly at galactic
        // distances — breaking the exclusion zone check which uses (px, py).
        let (arg_periapsis, semi_major_axis) = if e < 0.1 {
            // Match kepler_position's first-order path: ν ≈ M + 2e·sin(M), r = a·(1−e·cos(M))
            let nu_approx = mean_anomaly_0 + 2.0 * e * mean_anomaly_0.sin();
            let omega = theta_0 - nu_approx;
            let a = galactic_r / (1.0 - e * mean_anomaly_0.cos());
            (omega, a)
        } else {
            // Exact Newton-Raphson (matches kepler_position's e >= 0.1 path)
            let big_e0 = solve_kepler_nr(mean_anomaly_0, e);
            let nu_0 = 2.0 * ((1.0 + e).sqrt() * (big_e0 / 2.0).sin())
                .atan2((1.0 - e).sqrt() * (big_e0 / 2.0).cos());
            let omega = theta_0 - nu_0;
            let a = if e > 0.0 {
                galactic_r / (1.0 - e * big_e0.cos())
            } else {
                galactic_r
            };
            (omega, a)
        };
        // Mean motion from enclosed mass at semi-major axis
        let mean_motion = if semi_major_axis > 0.0 {
            (G * galactic_enclosed_mass(semi_major_axis) / (semi_major_axis * semi_major_axis * semi_major_axis)).sqrt()
        } else {
            0.0
        };

        stars.push(ProceduralStar {
            pos: [px, py],
            semi_major_axis,
            mean_motion,
            mean_anomaly_0,
            eccentricity: ecc,
            arg_periapsis: arg_periapsis as f32,
            mass: mass_kg,
            temperature,
            luminosity,
            color: star_color::stellar_color(temperature),
            radius_m: StarType::radius_meters(luminosity, temperature),
            sector_index: i,
            flags: 0,
            star_type,
            catalog_index: 0,
        });
    }

    stars
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::{LIGHT_YEAR, position_to_sector};

    #[test]
    fn no_procedural_stars_within_exclusion_zone() {
        // Compute Sun's t=0 position (same as generate_sector)
        let sun_pos = {
            let (a, e, m0) = (1.996e20_f64, 0.07_f64, 4.8534_f64);
            let big_e = solve_kepler_nr(m0, e);
            let nu = 2.0 * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
                .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
            let r = a * (1.0 - e * big_e.cos());
            [r * nu.cos(), r * nu.sin()]
        };
        let sun_ly = [sun_pos[0] / LIGHT_YEAR, sun_pos[1] / LIGHT_YEAR];
        eprintln!("Sun galactic pos: ({:.4}, {:.4}) ly", sun_ly[0], sun_ly[1]);

        // Get Sun's sector
        let sun_sector = position_to_sector(sun_pos).expect("Sun should be in a valid sector");
        eprintln!("Sun sector: ({}, {})", sun_sector.x, sun_sector.y);

        let galaxy_seed = 0xDEAD_BEEF_CAFE_1234_u64;
        let exclusion_r = SUN_EXCLUSION_RADIUS_LY * LIGHT_YEAR;

        // Check Sun's sector and all 8 neighbors
        let mut closest_dist_ly = f64::MAX;
        let mut total_excluded = 0u32;
        for dy in -1i16..=1 {
            for dx in -1i16..=1 {
                let sx = (sun_sector.x as i16 + dx) as u16;
                let sy = (sun_sector.y as i16 + dy) as u16;
                let stars = generate_sector(sx, sy, galaxy_seed);
                for star in &stars {
                    let ddx = star.pos[0] - sun_pos[0];
                    let ddy = star.pos[1] - sun_pos[1];
                    let dist = (ddx * ddx + ddy * ddy).sqrt();
                    let dist_ly = dist / LIGHT_YEAR;
                    if dist_ly < closest_dist_ly {
                        closest_dist_ly = dist_ly;
                    }
                    if dist < exclusion_r {
                        total_excluded += 1;
                    }
                }
            }
        }
        eprintln!("Closest procedural star: {:.2} ly from Sun", closest_dist_ly);
        eprintln!("Stars found inside exclusion zone: {}", total_excluded);
        assert!(closest_dist_ly >= SUN_EXCLUSION_RADIUS_LY,
            "Found star at {:.2} ly, expected >= {} ly", closest_dist_ly, SUN_EXCLUSION_RADIUS_LY);
    }

    #[test]
    fn kepler_position_matches_generated_position_at_t0() {
        // Verify that kepler_position(a, e, ω, M₀) returns the same position
        // as the generated (px, py). A mismatch here means the exclusion zone
        // check (on px, py) wouldn't match what's actually rendered.
        use crate::galaxy::kepler_position;

        let galaxy_seed = 0xDEAD_BEEF_CAFE_1234_u64;
        let sun_sector = position_to_sector({
            let (a, e, m0) = (1.996e20_f64, 0.07_f64, 4.8534_f64);
            let big_e = solve_kepler_nr(m0, e);
            let nu = 2.0 * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
                .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
            let r = a * (1.0 - e * big_e.cos());
            [r * nu.cos(), r * nu.sin()]
        }).unwrap();

        let mut max_error_ly = 0.0_f64;
        let mut star_count = 0u32;
        for dy in -1i16..=1 {
            for dx in -1i16..=1 {
                let sx = (sun_sector.x as i16 + dx) as u16;
                let sy = (sun_sector.y as i16 + dy) as u16;
                let stars = generate_sector(sx, sy, galaxy_seed);
                for star in &stars {
                    let [kx, ky] = kepler_position(
                        star.semi_major_axis,
                        star.eccentricity as f64,
                        star.arg_periapsis as f64,
                        star.mean_anomaly_0,
                    );
                    let err_x = kx - star.pos[0];
                    let err_y = ky - star.pos[1];
                    let err_ly = (err_x * err_x + err_y * err_y).sqrt() / LIGHT_YEAR;
                    if err_ly > max_error_ly {
                        max_error_ly = err_ly;
                    }
                    star_count += 1;
                }
            }
        }
        eprintln!("Checked {} stars, max kepler_position error: {:.6} ly", star_count, max_error_ly);
        // Position error must be sub-ly so the 15 ly exclusion zone is meaningful
        assert!(max_error_ly < 0.01,
            "kepler_position at t=0 diverges from generated pos by {:.2} ly", max_error_ly);
    }
}
