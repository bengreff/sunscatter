pub mod prng;
pub mod density;
pub mod generation;
pub mod star_color;
pub mod catalog;

use std::collections::HashMap;
use crate::bodies::SectorCoord;

/// Classification of a procedural star's evolutionary state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StarType {
    MainSequence(char), // spectral class: 'O','B','A','F','G','K','M'
    WhiteDwarf,
    RedGiant,
    Supergiant,
    NeutronStar,
}

impl StarType {
    /// Human-readable display name, e.g. "G-type Main Sequence"
    pub fn display_name(&self) -> &'static str {
        match self {
            StarType::MainSequence('O') => "O-type Main Sequence",
            StarType::MainSequence('B') => "B-type Main Sequence",
            StarType::MainSequence('A') => "A-type Main Sequence",
            StarType::MainSequence('F') => "F-type Main Sequence",
            StarType::MainSequence('G') => "G-type Main Sequence",
            StarType::MainSequence('K') => "K-type Main Sequence",
            StarType::MainSequence('M') => "M-type Main Sequence",
            StarType::MainSequence(_) => "Main Sequence",
            StarType::WhiteDwarf => "White Dwarf",
            StarType::RedGiant => "Red Giant",
            StarType::Supergiant => "Supergiant",
            StarType::NeutronStar => "Neutron Star",
        }
    }

    /// Short prefix for catalog naming, e.g. "G" for G-type main sequence, "WD" for white dwarf
    pub fn catalog_prefix(&self) -> &'static str {
        match self {
            StarType::MainSequence(c) => match c {
                'O' => "O",
                'B' => "B",
                'A' => "A",
                'F' => "F",
                'G' => "G",
                'K' => "K",
                'M' => "M",
                _ => "MS",
            },
            StarType::WhiteDwarf => "WD",
            StarType::RedGiant => "RG",
            StarType::Supergiant => "SG",
            StarType::NeutronStar => "NS",
        }
    }

    /// Physical radius in meters from luminosity and temperature via Stefan-Boltzmann law.
    /// R = R_sun * sqrt(L / L_sun) / (T / T_sun)^2
    pub fn radius_meters(luminosity_solar: f32, temperature_k: f32) -> f64 {
        const R_SUN: f64 = 6.957e8; // meters
        const T_SUN: f64 = 5778.0;  // Kelvin
        let l = luminosity_solar.max(1e-10) as f64;
        let t_ratio = temperature_k as f64 / T_SUN;
        R_SUN * l.sqrt() / (t_ratio * t_ratio)
    }
}

/// A procedurally generated star. Lives only in the sector cache.
#[derive(Clone, Debug)]
pub struct ProceduralStar {
    pub pos: [f64; 2],          // world position at t=0 (meters, Sgr A* origin)
    pub semi_major_axis: f64,   // orbital semi-major axis around galactic center (meters)
    pub mean_motion: f64,       // mean motion n = sqrt(G * M_enclosed(a) / a^3) (rad/s)
    pub mean_anomaly_0: f64,    // mean anomaly at t=0 (radians)
    pub eccentricity: f32,      // orbital eccentricity (0..0.6)
    pub arg_periapsis: f32,     // argument of periapsis (radians)
    pub mass: f64,              // kg
    pub temperature: f32,       // Kelvin (determines render color)
    pub luminosity: f32,        // solar luminosities (determines dot size)
    pub color: [f32; 3],        // pre-computed RGB from temperature (cached)
    pub radius_m: f64,          // physical radius in meters (cached from Stefan-Boltzmann)
    pub sector_index: u32,      // unique ID within sector
    pub flags: u32,             // bit 0: is_predefined, bit 1: has_system
    pub star_type: StarType,    // evolutionary classification
    pub catalog_index: u16,     // 0 = procedural, 1-67 = catalog system index
}

/// Solve Kepler's equation M = E - e·sin(E) via Newton-Raphson.
/// Returns eccentric anomaly E in radians.
///
/// Reduces `m` modulo TAU before iterating so sin/cos retain precision when
/// callers pass large accumulated mean anomalies (e.g. `M₀ + n·game_time` at
/// high time warp). Must match `bodies.rs::Orbit::solve_kepler` semantics.
pub fn solve_kepler_nr(m: f64, e: f64) -> f64 {
    let m = m.rem_euclid(std::f64::consts::TAU);
    let mut big_e = m; // initial guess
    for _ in 0..10 {
        let sin_e = big_e.sin();
        let cos_e = big_e.cos();
        let f = big_e - e * sin_e - m;
        let f_prime = 1.0 - e * cos_e;
        big_e -= f / f_prime;
        if f.abs() < 1e-10 {
            break;
        }
    }
    big_e
}

/// Compute (x, y) position on an elliptical orbit given orbital elements.
/// - `a`: semi-major axis (meters)
/// - `e`: eccentricity
/// - `arg_peri`: argument of periapsis (radians)
/// - `mean_anomaly`: mean anomaly M at current time (radians)
///
/// Uses a first-order expansion for e < 0.1 (covers ~80%+ of stars),
/// avoiding Newton-Raphson entirely. Full NR for higher eccentricities.
///
/// The mean anomaly is reduced modulo TAU at entry. This is CRITICAL for
/// precision at high time warp: callers compute `M = M₀ + mean_motion · game_time`,
/// which can reach 1e10+ radians after extended play at max warp. Passing such a
/// large value directly to sin/cos loses ~log₂(M/2π) bits of precision, causing
/// orbiting bodies to drift off their (static, parametrically rendered) orbit
/// lines. Matches the pattern in `bodies.rs::Orbit::solve_kepler`.
pub fn kepler_position(a: f64, e: f64, arg_peri: f64, mean_anomaly: f64) -> [f64; 2] {
    let mean_anomaly = mean_anomaly.rem_euclid(std::f64::consts::TAU);
    if e < 0.1 {
        // First-order expansion: ν ≈ M + 2e·sin(M), r ≈ a·(1 − e·cos(M))
        // Error O(e²) ≈ 0.01 rad at e=0.1, negligible for rendering
        let sin_m = mean_anomaly.sin();
        let cos_m = mean_anomaly.cos();
        let nu = mean_anomaly + 2.0 * e * sin_m;
        let r = a * (1.0 - e * cos_m);
        let angle = nu + arg_peri;
        [r * angle.cos(), r * angle.sin()]
    } else {
        let big_e = solve_kepler_nr(mean_anomaly, e);
        let nu = 2.0 * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
            .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
        let r = a * (1.0 - e * big_e.cos());
        let angle = nu + arg_peri;
        [r * angle.cos(), r * angle.sin()]
    }
}

struct CachedSector {
    stars: Vec<ProceduralStar>,
    last_used_frame: u64,
}

/// Galaxy-wide procedural star system. Owned by Game.
/// Stars are generated per-sector from deterministic seeds and cached with LRU eviction.
pub struct GalaxyState {
    pub galaxy_seed: u64,
    cache: HashMap<SectorCoord, CachedSector>,
    frame_counter: u64,
    /// Pre-computed catalog stars grouped by sector (never evicted).
    pub catalog_by_sector: HashMap<SectorCoord, Vec<ProceduralStar>>,
}

/// Maximum number of sectors to keep cached before eviction.
/// 1000 ly render radius / 100 ly per sector = 10 sector radius → π×10² ≈ 314 + margin
const MAX_CACHED_SECTORS: usize = 500;
/// Sectors unused for this many frames get evicted.
const EVICTION_AGE_FRAMES: u64 = 300; // ~5 seconds at 60fps

impl GalaxyState {
    pub fn new() -> Self {
        let catalog_by_sector = catalog::build_catalog_stars();
        Self {
            galaxy_seed: 0xDEAD_BEEF_CAFE_1234,
            cache: HashMap::new(),
            frame_counter: 0,
            catalog_by_sector,
        }
    }

    /// Get stars for a sector, generating and caching if needed.
    /// Includes both procedural and catalog stars. Catalog stars are appended
    /// after procedural generation, and the nearest procedural star within 2 ly
    /// of each catalog star is removed to avoid visual overlap.
    pub fn get_sector(&mut self, coord: SectorCoord) -> &[ProceduralStar] {
        let frame = self.frame_counter;
        let seed = self.galaxy_seed;

        // Check if this sector has catalog stars to inject
        let catalog_stars = self.catalog_by_sector.get(&coord).cloned();

        let entry = self.cache
            .entry(coord)
            .and_modify(|cs| cs.last_used_frame = frame)
            .or_insert_with(|| {
                let mut stars = generation::generate_sector(coord.x, coord.y, seed);

                // Inject catalog stars: remove nearest procedural star within 2 ly
                if let Some(ref cat_stars) = catalog_stars {
                    const DEDUP_RADIUS: f64 = 2.0 * 9.461e15; // 2 light-years in meters
                    let dedup_r2 = DEDUP_RADIUS * DEDUP_RADIUS;

                    for cat in cat_stars {
                        // Find and remove the nearest procedural star within dedup radius
                        let mut best_idx = None;
                        let mut best_dist2 = dedup_r2;
                        for (j, proc_star) in stars.iter().enumerate() {
                            if proc_star.catalog_index > 0 {
                                continue; // skip already-injected catalog stars
                            }
                            let dx = proc_star.pos[0] - cat.pos[0];
                            let dy = proc_star.pos[1] - cat.pos[1];
                            let d2 = dx * dx + dy * dy;
                            if d2 < best_dist2 {
                                best_dist2 = d2;
                                best_idx = Some(j);
                            }
                        }
                        if let Some(idx) = best_idx {
                            stars.swap_remove(idx);
                        }
                        stars.push(cat.clone());
                    }
                }

                CachedSector {
                    stars,
                    last_used_frame: frame,
                }
            });

        entry.stars.as_slice()
    }

    /// Find procedural stars within `radius` meters of `pos` at the given simulation time.
    /// Returns (sector_coord, star_index_in_sector, star, current_position).
    pub fn stars_near(&mut self, pos: [f64; 2], radius: f64, time: f64) -> Vec<(u16, u16, u32, ProceduralStar, [f64; 2])> {
        use crate::bodies::{SECTOR_SIDE_METERS, SECTOR_GRID_HALF_METERS, SECTOR_GRID_SIZE, G, galactic_enclosed_mass};

        let mut results = Vec::new();
        let sector_radius = (radius / SECTOR_SIDE_METERS).ceil() as i64 + 1;

        let center_sx = ((pos[0] + SECTOR_GRID_HALF_METERS) / SECTOR_SIDE_METERS).floor() as i64;
        let center_sy = ((pos[1] + SECTOR_GRID_HALF_METERS) / SECTOR_SIDE_METERS).floor() as i64;

        for dy in -sector_radius..=sector_radius {
            for dx in -sector_radius..=sector_radius {
                let sx = center_sx + dx;
                let sy = center_sy + dy;
                if sx < 0 || sx >= SECTOR_GRID_SIZE as i64 || sy < 0 || sy >= SECTOR_GRID_SIZE as i64 {
                    continue;
                }
                let coord = crate::bodies::SectorCoord { x: sx as u16, y: sy as u16 };
                let stars: Vec<ProceduralStar> = self.get_sector(coord).to_vec();
                for (i, star) in stars.iter().enumerate() {
                    let mu = G * galactic_enclosed_mass(star.semi_major_axis);
                    let n = (mu / star.semi_major_axis.powi(3)).sqrt();
                    let mean_anomaly = star.mean_anomaly_0 + n * time;
                    let e = star.eccentricity as f64;
                    let ea = solve_kepler_nr(mean_anomaly, e);
                    let nu = if e < 1.0 {
                        2.0 * ((1.0 + e).sqrt() * (ea / 2.0).sin()).atan2(
                            (1.0 - e).sqrt() * (ea / 2.0).cos())
                    } else {
                        2.0 * ((e + 1.0).sqrt() * (ea / 2.0).tanh()).atan2((e - 1.0).sqrt())
                    };
                    let r = if e < 1.0 {
                        star.semi_major_axis * (1.0 - e * ea.cos())
                    } else {
                        star.semi_major_axis.abs() * (e * ea.cosh() - 1.0)
                    };
                    let angle = nu + star.arg_periapsis as f64;
                    let star_pos = [r * angle.cos(), r * angle.sin()];
                    let dx = star_pos[0] - pos[0];
                    let dy = star_pos[1] - pos[1];
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < radius {
                        results.push((sx as u16, sy as u16, i as u32, star.clone(), star_pos));
                    }
                }
            }
        }
        results
    }

    /// Advance frame counter and evict stale sectors.
    pub fn tick(&mut self) {
        self.frame_counter += 1;

        // Only evict when over capacity
        if self.cache.len() > MAX_CACHED_SECTORS {
            let cutoff = self.frame_counter.saturating_sub(EVICTION_AGE_FRAMES);
            self.cache.retain(|_, cs| cs.last_used_frame >= cutoff);
        }
    }
}
