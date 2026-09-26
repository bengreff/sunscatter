mod common;

use sunscatter_app::galaxy::generation::generate_sector;
use sunscatter_app::galaxy::density::sector_star_count;
use sunscatter_app::galaxy::prng::Rng64;
use sunscatter_app::galaxy::star_color::stellar_color;
use sunscatter_app::galaxy::solve_kepler_nr;

const GALAXY_SEED: u64 = 0xDEAD_BEEF_CAFE_1234;

// ============================================================
// Deterministic generation
// ============================================================

#[test]
fn generation_is_deterministic() {
    let stars1 = generate_sector(500, 500, GALAXY_SEED);
    let stars2 = generate_sector(500, 500, GALAXY_SEED);

    assert_eq!(stars1.len(), stars2.len(), "Star count should be identical");

    for (i, (s1, s2)) in stars1.iter().zip(stars2.iter()).enumerate() {
        assert_eq!(s1.pos[0], s2.pos[0], "Star {i} x mismatch");
        assert_eq!(s1.pos[1], s2.pos[1], "Star {i} y mismatch");
        assert_eq!(s1.temperature, s2.temperature, "Star {i} temp mismatch");
    }
}

// ============================================================
// Sun exclusion zone
// ============================================================

#[test]
fn no_procedural_stars_near_sun() {
    // Sun's sector (approximately sector 260, 290 based on its galactic position)
    // We use the same calculation as in generation.rs to find the Sun's position
    let sun_pos = {
        let (a, e, m0) = (1.996e20_f64, 0.07_f64, 4.8534_f64);
        let big_e = sunscatter_app::galaxy::solve_kepler_nr(m0, e);
        let nu = 2.0 * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
            .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
        let r = a * (1.0 - e * big_e.cos());
        [r * nu.cos(), r * nu.sin()]
    };

    let exclusion_ly = 15.0;
    let ly = 9.461e15_f64;
    let exclusion_r = exclusion_ly * ly;

    // Get Sun's sector
    let sun_sector = sunscatter_app::bodies::position_to_sector(sun_pos)
        .expect("Sun should be in a valid sector");

    // Check Sun's sector + all 8 neighbors
    for dy in -1i16..=1 {
        for dx in -1i16..=1 {
            let sx = (sun_sector.x as i16 + dx) as u16;
            let sy = (sun_sector.y as i16 + dy) as u16;
            let stars = generate_sector(sx, sy, GALAXY_SEED);
            for star in &stars {
                let ddx = star.pos[0] - sun_pos[0];
                let ddy = star.pos[1] - sun_pos[1];
                let dist = (ddx * ddx + ddy * ddy).sqrt();
                assert!(
                    dist >= exclusion_r,
                    "Star at ({:.1}, {:.1}) ly is {:.2} ly from Sun, expected >= {exclusion_ly} ly",
                    star.pos[0] / ly, star.pos[1] / ly, dist / ly
                );
            }
        }
    }
}

// ============================================================
// Density model sanity
// ============================================================

#[test]
fn solar_neighborhood_reasonable_count() {
    // Sun is at ~sector (260, 290) — approximately 21,100 ly from center
    // Use the Sun's actual sector
    let sun_pos = {
        let (a, e, m0) = (1.996e20_f64, 0.07_f64, 4.8534_f64);
        let big_e = sunscatter_app::galaxy::solve_kepler_nr(m0, e);
        let nu = 2.0 * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
            .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
        let r = a * (1.0 - e * big_e.cos());
        [r * nu.cos(), r * nu.sin()]
    };
    let sun_sector = sunscatter_app::bodies::position_to_sector(sun_pos).unwrap();

    let count = sector_star_count(sun_sector.x, sun_sector.y, GALAXY_SEED);
    assert!(count >= 100 && count <= 2000,
        "Solar neighborhood count {count} out of expected range [100, 2000]");
}

#[test]
fn galactic_edge_low_count() {
    // Far from center: sector (50, 500) ≈ 45,000 ly from center
    let count = sector_star_count(50, 500, GALAXY_SEED);
    assert!(count < 100,
        "Galactic edge count {count} should be low (< 100)");
}

#[test]
fn galactic_center_high_count() {
    // At galactic center: sector (500, 500) = Sgr A*
    let count = sector_star_count(500, 500, GALAXY_SEED);
    assert_eq!(count, 2000, "Galactic center should be capped at MAX_STARS_PER_SECTOR (2000)");
}

// ============================================================
// Stellar color
// ============================================================

#[test]
fn stellar_color_valid_rgb() {
    for temp in [2000.0_f32, 5800.0, 10000.0, 40000.0] {
        let [r, g, b] = stellar_color(temp);
        assert!(r >= 0.0 && r <= 1.0, "Red channel out of range for {temp}K: {r}");
        assert!(g >= 0.0 && g <= 1.0, "Green channel out of range for {temp}K: {g}");
        assert!(b >= 0.0 && b <= 1.0, "Blue channel out of range for {temp}K: {b}");
    }
}

#[test]
fn stellar_color_clamped_extremes() {
    let below = stellar_color(500.0);
    let at_min = stellar_color(2000.0);
    assert_eq!(below, at_min, "Temps below 2000K should clamp to 2000K color");

    let above = stellar_color(100_000.0);
    let at_max = stellar_color(40_000.0);
    assert_eq!(above, at_max, "Temps above 40000K should clamp to 40000K color");
}

// ============================================================
// PRNG determinism and range
// ============================================================

#[test]
fn rng64_deterministic() {
    let mut rng1 = Rng64::new(42);
    let mut rng2 = Rng64::new(42);
    let seq1: Vec<u64> = (0..100).map(|_| rng1.next_u64()).collect();
    let seq2: Vec<u64> = (0..100).map(|_| rng2.next_u64()).collect();
    assert_eq!(seq1, seq2, "Same seed must produce identical sequence");
}

#[test]
fn rng64_f64_in_range() {
    let mut rng = Rng64::new(12345);
    for i in 0..10_000 {
        let v = rng.next_f64();
        assert!(v >= 0.0 && v < 1.0, "next_f64() sample {i} out of [0,1): {v}");
    }
}

#[test]
fn rng64_range_f64_bounds() {
    let mut rng = Rng64::new(99999);
    for i in 0..10_000 {
        let v = rng.range_f64(5.0, 10.0);
        assert!(v >= 5.0 && v < 10.0, "range_f64(5,10) sample {i} out of bounds: {v}");
    }
}

// ============================================================
// Kepler equation solver
// ============================================================

#[test]
fn solve_kepler_nr_roundtrip() {
    for e in [0.0, 0.3, 0.8, 0.95] {
        for m_input in [0.5, 1.0, 3.0, 5.5] {
            let big_e = solve_kepler_nr(m_input, e);
            // Verify: M = E - e*sin(E)
            let m_recovered = big_e - e * big_e.sin();
            let m_normalized = m_input.rem_euclid(std::f64::consts::TAU);
            let diff = (m_recovered - m_normalized).abs();
            assert!(
                diff < 1e-8,
                "Kepler roundtrip failed for e={e}, M={m_input}: E={big_e}, recovered M={m_recovered}, expected {m_normalized}, diff={diff}"
            );
        }
    }
}
