use serde::{Deserialize, Serialize};

/// Mirror deployment state in the Dyson swarm deployment queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployingMirror {
    /// Simulation time when the mirror arrives and becomes operational.
    pub arrival_time: f64,
}

/// State of the Dyson swarm around a star.
/// Currently only the Sun (index 0) has a swarm, but the struct is per-star ready.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DysonSwarm {
    /// Number of operational mirrors in the swarm.
    pub mirror_count: u64,
    /// Mirrors currently in transit (launched but not yet operational).
    pub deploying: Vec<DeployingMirror>,
    /// Number of operational collector stations in the swarm.
    pub collector_count: u64,
    /// Collector stations currently in transit.
    pub deploying_collectors: Vec<DeployingMirror>,
}

/// Solar irradiance at 0.1 AU in W/m².
const IRRADIANCE_01AU: f64 = 136_100.0;

/// Mirror deployed area in m² (1 km²).
const MIRROR_AREA_M2: f64 = 1_000_000.0;

/// Reflective efficiency (fraction of irradiance reflected usefully).
pub const MIRROR_EFFICIENCY: f64 = 0.95;

/// Power per mirror in watts at 0.1 AU.
const POWER_PER_MIRROR_W: f64 = IRRADIANCE_01AU * MIRROR_AREA_M2 * MIRROR_EFFICIENCY;

/// Deployment time: ~1.1 days for solar pressure circularization at 0.1 AU.
/// Stored as seconds.
pub const MIRROR_DEPLOY_TIME_S: f64 = 1.1 * 86_400.0;

/// Mass of a single mirror segment (kg) at base sail technology.
pub const BASE_MIRROR_MASS_KG: f64 = 3_500.0;

/// Base sail loading (g/m²) at tier 0.
const BASE_SAIL_LOADING: f64 = 3.5;

/// Critical sail loading for statite (g/m²).
const STATITE_THRESHOLD: f64 = 1.53;

/// Required launch velocity from Mercury surface to 0.1 AU orbit (m/s).
pub const MERCURY_TO_SWARM_DV: f64 = 17_712.0;

// === Collector station / power delivery constants ===

/// Solar luminosity in watts.
const L_SUN_W: f64 = 3.828e26;

/// Surface area of a sphere at 0.1 AU (m²).
const A_SPHERE_01AU: f64 = 4.0 * std::f64::consts::PI * (0.1 * 1.496e11) * (0.1 * 1.496e11);

/// Maximum input power per collector station (watts). 500 GW.
const MAX_COLLECTOR_INPUT_W: f64 = 500e9;

/// PV + laser conversion efficiency for collector stations.
pub const COLLECTOR_EFFICIENCY: f64 = 0.50;

/// Laser → electricity conversion efficiency at receiver arrays.
pub const RECEIVER_EFFICIENCY: f64 = 0.90;

/// Maximum laser power input per receiver array (watts). 50 GW.
pub const MAX_RECEIVER_INPUT_W: f64 = 50e9;

/// Mass of a single collector station (kg).
pub const COLLECTOR_MASS_KG: f64 = 5_000.0;

/// Deployment time for collector stations (seconds). 2 days.
pub const COLLECTOR_DEPLOY_TIME_S: f64 = 2.0 * 86_400.0;

impl DysonSwarm {
    /// Total reflected power of the swarm in watts (before collection).
    pub fn total_power_w(&self) -> f64 {
        self.mirror_count as f64 * POWER_PER_MIRROR_W
    }

    /// Total mirror area in km².
    pub fn total_area_km2(&self) -> f64 {
        self.mirror_count as f64
    }

    /// Add mirrors that have completed deployment (arrival_time <= sim_time).
    pub fn process_deployments(&mut self, sim_time: f64) {
        let before = self.deploying.len();
        self.deploying.retain(|m| m.arrival_time > sim_time);
        let deployed = before - self.deploying.len();
        self.mirror_count += deployed as u64;

        // Process collector station deployments
        let before_c = self.deploying_collectors.len();
        self.deploying_collectors.retain(|c| c.arrival_time > sim_time);
        let deployed_c = before_c - self.deploying_collectors.len();
        self.collector_count += deployed_c as u64;
    }

    /// Queue a mirror for deployment.
    pub fn launch_mirror(&mut self, sim_time: f64) {
        self.deploying.push(DeployingMirror {
            arrival_time: sim_time + MIRROR_DEPLOY_TIME_S,
        });
    }

    /// Queue a collector station for deployment.
    pub fn launch_collector(&mut self, sim_time: f64) {
        self.deploying_collectors.push(DeployingMirror {
            arrival_time: sim_time + COLLECTOR_DEPLOY_TIME_S,
        });
    }

    /// Number of mirrors currently in transit.
    pub fn in_transit(&self) -> usize {
        self.deploying.len()
    }

    /// Number of collector stations currently in transit.
    pub fn collectors_in_transit(&self) -> usize {
        self.deploying_collectors.len()
    }

    /// Collection efficiency based on average lightness number β.
    /// η = β / (1 + β). Continuous function — no step at statite threshold.
    pub fn collection_efficiency(beta: f64) -> f64 {
        beta / (1.0 + beta)
    }

    /// Total laser power available to receiver arrays (watts).
    /// Accounts for occlusion, mirror efficiency, collection efficiency, and collector capacity.
    pub fn available_laser_power(&self, beta: f64) -> f64 {
        if self.mirror_count == 0 || self.collector_count == 0 {
            return 0.0;
        }
        let filling = self.mirror_count as f64 * MIRROR_AREA_M2 / A_SPHERE_01AU;
        let intercepted = L_SUN_W * (1.0 - (-filling).exp());
        let reflected = intercepted * MIRROR_EFFICIENCY;
        let collected = reflected * Self::collection_efficiency(beta);
        let collector_cap = self.collector_count as f64 * MAX_COLLECTOR_INPUT_W;
        let collector_limited = collected.min(collector_cap);
        collector_limited * COLLECTOR_EFFICIENCY
    }
}

/// Sail loading (g/m²) at a given sail technology tier.
/// Tiers reduce film areal density: base 3.0 g/m², halving every 5 tiers.
/// Total loading = film + structure (0.5 g/m²).
pub fn sail_loading_at_tier(tier: u32) -> f64 {
    let film_density = 3.0 * 0.5_f64.powf(tier as f64 / 5.0);
    film_density + 0.5 // structure mass
}

/// Lightness number β at a given sail technology tier.
/// β = σ_critical / σ. β ≥ 1 = statite capability.
pub fn lightness_number_at_tier(tier: u32) -> f64 {
    STATITE_THRESHOLD / sail_loading_at_tier(tier)
}

/// Mirror mass (kg) at a given sail technology tier.
/// Mass scales with sail loading relative to base.
pub fn mirror_mass_at_tier(tier: u32) -> f64 {
    BASE_MIRROR_MASS_KG * sail_loading_at_tier(tier) / BASE_SAIL_LOADING
}

/// Whether mirrors at this tier can act as statites (β ≥ 1).
pub fn is_statite_capable(tier: u32) -> bool {
    lightness_number_at_tier(tier) >= 1.0
}
