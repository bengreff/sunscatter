use crate::colony::resources::ResourceType;
use crate::parts::{FuelType, PartDefinition, PartDefinitions, Propellant, ShieldType, VesselBlueprint};

/// Material breakdown fractions for computing part cost.
/// All fields sum to 1.0.
#[derive(Debug, Clone, Copy)]
pub struct MaterialBreakdown {
    pub metal: f64,
    pub hta: f64,
    pub elec: f64,
    pub super_: f64,
    pub pi: f64,
}

/// Material masses in kg, computed from dry mass * breakdown fractions.
#[derive(Debug, Clone, Copy)]
pub struct MaterialMasses {
    pub metal_kg: f64,
    pub hta_kg: f64,
    pub elec_kg: f64,
    pub super_kg: f64,
    pub pi_kg: f64,
}

impl MaterialMasses {
    /// Earth cost in dollars using fixed $/kg prices from colonies.md §7.
    pub fn earth_cost(&self) -> f64 {
        self.metal_kg * 100.0
            + self.hta_kg * 1_000.0
            + self.elec_kg * 10_000.0
            + self.super_kg * 50_000.0
            + self.pi_kg * 200_000.0
    }
}

impl MaterialBreakdown {
    /// Convert to material masses given the total dry mass in kg.
    pub fn to_masses(&self, dry_mass_kg: f64) -> MaterialMasses {
        MaterialMasses {
            metal_kg: dry_mass_kg * self.metal,
            hta_kg: dry_mass_kg * self.hta,
            elec_kg: dry_mass_kg * self.elec,
            super_kg: dry_mass_kg * self.super_,
            pi_kg: dry_mass_kg * self.pi,
        }
    }
}

const FUEL_TANKS: MaterialBreakdown = MaterialBreakdown { metal: 0.90, hta: 0.05, elec: 0.05, super_: 0.0, pi: 0.0 };
const CHEMICAL_ENGINES: MaterialBreakdown = MaterialBreakdown { metal: 0.50, hta: 0.35, elec: 0.15, super_: 0.0, pi: 0.0 };
const NTR_ENGINES: MaterialBreakdown = MaterialBreakdown { metal: 0.40, hta: 0.30, elec: 0.15, super_: 0.15, pi: 0.0 };
const ION_HALL: MaterialBreakdown = MaterialBreakdown { metal: 0.25, hta: 0.15, elec: 0.35, super_: 0.25, pi: 0.0 };
const MPD: MaterialBreakdown = MaterialBreakdown { metal: 0.25, hta: 0.15, elec: 0.30, super_: 0.30, pi: 0.0 };
const FUSION_ENGINES: MaterialBreakdown = MaterialBreakdown { metal: 0.25, hta: 0.15, elec: 0.20, super_: 0.38, pi: 0.02 };
const ANTIMATTER_ENGINES: MaterialBreakdown = MaterialBreakdown { metal: 0.20, hta: 0.10, elec: 0.15, super_: 0.50, pi: 0.05 };
const COMMAND_PODS: MaterialBreakdown = MaterialBreakdown { metal: 0.50, hta: 0.10, elec: 0.40, super_: 0.0, pi: 0.0 };
const PROBE_CORES: MaterialBreakdown = MaterialBreakdown { metal: 0.20, hta: 0.10, elec: 0.70, super_: 0.0, pi: 0.0 };
const CREW_QUARTERS: MaterialBreakdown = MaterialBreakdown { metal: 0.60, hta: 0.10, elec: 0.30, super_: 0.0, pi: 0.0 };
const DECOUPLERS: MaterialBreakdown = MaterialBreakdown { metal: 0.85, hta: 0.10, elec: 0.05, super_: 0.0, pi: 0.0 };
const FAIRINGS: MaterialBreakdown = MaterialBreakdown { metal: 0.90, hta: 0.05, elec: 0.05, super_: 0.0, pi: 0.0 };
const NOSE_CONES: MaterialBreakdown = MaterialBreakdown { metal: 0.80, hta: 0.15, elec: 0.05, super_: 0.0, pi: 0.0 };
const HEAT_SHIELDS: MaterialBreakdown = MaterialBreakdown { metal: 0.25, hta: 0.70, elec: 0.05, super_: 0.0, pi: 0.0 };
const PARACHUTES: MaterialBreakdown = MaterialBreakdown { metal: 0.60, hta: 0.25, elec: 0.15, super_: 0.0, pi: 0.0 };
const SOLAR_PANELS: MaterialBreakdown = MaterialBreakdown { metal: 0.20, hta: 0.10, elec: 0.70, super_: 0.0, pi: 0.0 };
const BATTERIES: MaterialBreakdown = MaterialBreakdown { metal: 0.30, hta: 0.05, elec: 0.65, super_: 0.0, pi: 0.0 };
const RTG: MaterialBreakdown = MaterialBreakdown { metal: 0.25, hta: 0.40, elec: 0.35, super_: 0.0, pi: 0.0 };
const SMALL_FISSION: MaterialBreakdown = MaterialBreakdown { metal: 0.45, hta: 0.25, elec: 0.20, super_: 0.10, pi: 0.0 };
const LARGE_FISSION: MaterialBreakdown = MaterialBreakdown { metal: 0.40, hta: 0.20, elec: 0.20, super_: 0.20, pi: 0.0 };
const FUSION_REACTORS: MaterialBreakdown = MaterialBreakdown { metal: 0.25, hta: 0.15, elec: 0.18, super_: 0.40, pi: 0.02 };
const ANTIMATTER_REACTORS: MaterialBreakdown = MaterialBreakdown { metal: 0.15, hta: 0.10, elec: 0.15, super_: 0.55, pi: 0.05 };
const WHIPPLE_SHIELDS: MaterialBreakdown = MaterialBreakdown { metal: 0.95, hta: 0.05, elec: 0.0, super_: 0.0, pi: 0.0 };
const FRES_SHIELDS: MaterialBreakdown = MaterialBreakdown { metal: 0.25, hta: 0.10, elec: 0.25, super_: 0.40, pi: 0.0 };
const GEODESIC_SHIELDS: MaterialBreakdown = MaterialBreakdown { metal: 0.15, hta: 0.05, elec: 0.25, super_: 0.55, pi: 0.0 };
const RCS_THRUSTERS: MaterialBreakdown = MaterialBreakdown { metal: 0.40, hta: 0.35, elec: 0.25, super_: 0.0, pi: 0.0 };
const CARGO_CONTAINERS: MaterialBreakdown = MaterialBreakdown { metal: 0.95, hta: 0.0, elec: 0.05, super_: 0.0, pi: 0.0 };
const GREENHOUSES_SHIP: MaterialBreakdown = MaterialBreakdown { metal: 0.50, hta: 0.05, elec: 0.45, super_: 0.0, pi: 0.0 };

/// Determine the material breakdown for a part based on its type and components.
pub fn material_breakdown(def: &PartDefinition) -> MaterialBreakdown {
    // 1. Engines: dispatch on propellant type
    if let Some(ref engine) = def.engine {
        // Check for antimatter (primary or secondary propellant)
        if engine.propellant == Propellant::Antimatter
            || engine.secondary_propellant == Some(Propellant::Antimatter)
        {
            return ANTIMATTER_ENGINES;
        }
        return match engine.propellant {
            Propellant::Kerolox | Propellant::Methalox | Propellant::Hydrolox => CHEMICAL_ENGINES,
            Propellant::Hydrogen | Propellant::NuclearPulse => NTR_ENGINES,
            Propellant::Xenon => {
                // Ion/Hall vs MPD: MPD >= 250 kg (0.250 tonnes)
                if def.mass >= 0.250 {
                    MPD
                } else {
                    ION_HALL
                }
            }
            Propellant::FusionFuel => FUSION_ENGINES,
            Propellant::Antimatter => ANTIMATTER_ENGINES, // already handled above
        };
    }

    // 2. Shields: dispatch on shield type
    if let Some(ref shield) = def.shield {
        return match shield.shield_type {
            ShieldType::Whipple => WHIPPLE_SHIELDS,
            ShieldType::FRES => FRES_SHIELDS,
            ShieldType::Geodesic => GEODESIC_SHIELDS,
        };
    }

    // 3. Reactors: dispatch on ID pattern
    if def.reactor.is_some() {
        if def.id.starts_with("reactor_am") {
            return ANTIMATTER_REACTORS;
        } else if def.id.starts_with("reactor_fusion") {
            return FUSION_REACTORS;
        } else if def.id.starts_with("reactor_fission") {
            return LARGE_FISSION;
        } else {
            // Small fission reactors (Ember, Hearth, Crucible)
            return SMALL_FISSION;
        }
    }

    // 4. RTG (check before battery since RTGs might also have battery-like props)
    if def.rtg.is_some() {
        return RTG;
    }

    // 5. Solar panels
    if def.solar_panel.is_some() {
        return SOLAR_PANELS;
    }

    // 6. Batteries (only if no reactor/solar/rtg)
    if def.battery.is_some() && def.pod.is_none() {
        return BATTERIES;
    }

    // 7. Pods: command pods vs probe cores vs crew quarters
    if let Some(ref pod) = def.pod {
        if pod.can_control {
            if pod.crew_capacity > 0 {
                return COMMAND_PODS;
            } else {
                return PROBE_CORES;
            }
        } else if pod.crew_capacity > 0 {
            return CREW_QUARTERS;
        }
        // Greenhouses have pod data for power_draw but no crew/control
        return GREENHOUSES_SHIP;
    }

    // 8. Tanks
    if def.tank.is_some() {
        return FUEL_TANKS;
    }

    // 9. Decouplers
    if def.decoupler.is_some() {
        return DECOUPLERS;
    }

    // 10. RCS
    if def.rcs.is_some() {
        return RCS_THRUSTERS;
    }

    // 11. Cargo
    if def.cargo.is_some() {
        return CARGO_CONTAINERS;
    }

    // 12. Aerodynamic category parts
    if def.is_heat_shield {
        return HEAT_SHIELDS;
    }
    if def.parachute.is_some() {
        return PARACHUTES;
    }
    if def.fairing.is_some() {
        return FAIRINGS;
    }

    // 13. Default: nose cones / generic structural
    NOSE_CONES
}

/// Earth $/kg price for the fuel component (non-oxidizer) of a tank fuel type.
/// LOX cost ($0.50/kg) is handled separately in vessel cost calculation.
pub fn fuel_price_per_kg(fuel_type: FuelType) -> f64 {
    match fuel_type {
        FuelType::Empty => 0.0,
        FuelType::Rp1 => 1.0,
        FuelType::Methane => 2.0,
        FuelType::Hydrogen | FuelType::PureHydrogen => 6.0,
        FuelType::Monopropellant => 5.0,
        FuelType::Xenon => 3_000.0,
        FuelType::FusionFuel => 20.0,       // Deuterium component; He-3 not purchasable on Earth
        FuelType::Antimatter => 0.0,         // Not purchasable on Earth
        FuelType::NuclearPulse => 100_000.0,
    }
}

/// LOX price in $/kg.
pub const LOX_PRICE_PER_KG: f64 = 0.50;

/// Earth-launch resource cost of a single dry (empty) part, in dollars.
/// Derived from the part's material breakdown × dry mass × per-material $/kg
/// (per `colonies.md` §7). This is the single source of truth for the per-part
/// cost shown in the editor and the per-part contribution to the vessel total.
pub fn part_dry_earth_cost(def: &PartDefinition) -> f64 {
    let dry_mass_kg = def.mass * 1000.0;
    material_breakdown(def).to_masses(dry_mass_kg).earth_cost()
}

/// Earth-launch cost of the propellant currently loaded in a tank, in dollars.
/// Returns 0 for non-tank parts, empty tanks, and zero fill fractions.
pub fn part_filled_fuel_cost(
    def: &PartDefinition,
    fuel_type: FuelType,
    fill_fraction: f64,
) -> f64 {
    let tank = match &def.tank {
        Some(t) => t,
        None => return 0.0,
    };
    if fuel_type == FuelType::Empty || fill_fraction <= 0.0 {
        return 0.0;
    }
    let (ox_kg, fuel_kg) = tank.propellant_capacity(fuel_type);
    let ox_cost = ox_kg * fill_fraction * LOX_PRICE_PER_KG;
    let fuel_cost = fuel_kg * fill_fraction * fuel_price_per_kg(fuel_type);
    ox_cost + fuel_cost
}

/// Combined Earth $/kg cost of a fuel type at full load (fuel + LOX, blended
/// by mass ratio). The single $/kg number that determines how much fuel cost
/// each kg of loaded propellant adds. Used for editor-selector unit-price
/// displays. Returns 0 for Empty and for fuels with no Earth purchase price
/// (Antimatter).
pub fn fuel_blended_cost_per_kg(fuel_type: FuelType) -> f64 {
    let (ox_per_sq, fuel_per_sq) = fuel_type.propellant_per_grid_square();
    let total = ox_per_sq + fuel_per_sq;
    if total <= 0.0 {
        return 0.0;
    }
    let total_cost = ox_per_sq * LOX_PRICE_PER_KG
        + fuel_per_sq * fuel_price_per_kg(fuel_type);
    total_cost / total
}

/// Build the per-material lines for a part's dry composition. Each entry is
/// already formatted ("Metal: 1.5 t @ $100/kg" etc.). Materials with zero
/// fraction are skipped. Used to make the dry cost transparent in part info.
pub fn material_breakdown_lines(def: &PartDefinition) -> Vec<String> {
    let dry_kg = def.mass * 1000.0;
    let bd = material_breakdown(def);
    let mut lines = Vec::new();
    let push = |lines: &mut Vec<String>, name: &str, kg: f64, price_per_kg: u64| {
        if kg > 0.0 {
            lines.push(format!("  {}: {} @ ${}/kg", name, format_mass(kg), price_per_kg));
        }
    };
    push(&mut lines, "Metal",          dry_kg * bd.metal,  100);
    push(&mut lines, "High-Temp Alloy", dry_kg * bd.hta,   1_000);
    push(&mut lines, "Electronics",    dry_kg * bd.elec,   10_000);
    push(&mut lines, "Superconductor", dry_kg * bd.super_, 50_000);
    push(&mut lines, "Precision Inst.", dry_kg * bd.pi,    200_000);
    lines
}

fn format_mass(kg: f64) -> String {
    if kg >= 1000.0 {
        format!("{:.1} t", kg / 1000.0)
    } else if kg >= 1.0 {
        format!("{:.0} kg", kg)
    } else {
        format!("{:.1} kg", kg)
    }
}

/// Format a dollar amount for display: $1.2K, $3.5M, $12.4B, etc.
pub fn format_money(dollars: f64) -> String {
    let abs = dollars.abs();
    let sign = if dollars < 0.0 { "-" } else { "" };
    if abs >= 1e12 {
        format!("{}${:.1}T", sign, abs / 1e12)
    } else if abs >= 1e9 {
        format!("{}${:.1}B", sign, abs / 1e9)
    } else if abs >= 1e6 {
        format!("{}${:.1}M", sign, abs / 1e6)
    } else if abs >= 1e3 {
        format!("{}${:.1}K", sign, abs / 1e3)
    } else {
        format!("{}${:.0}", sign, abs)
    }
}

// Science reward constants
pub const SCIENCE_SUBORBITAL: f64 = 25.0;
pub const SCIENCE_FIRST_ORBIT: f64 = 50.0;
pub const SCIENCE_GEOSTATIONARY: f64 = 25.0;

/// Distance from the Sun in AU for a body (uses parent body SMA for moons).
pub fn body_distance_au(body_index: usize, solar_system: &crate::bodies::SolarSystem) -> f64 {
    const AU: f64 = 1.496e11;
    let body = &solar_system.bodies[body_index];
    if let Some(parent_idx) = body.parent {
        // Moon: use parent body's SMA
        let parent = &solar_system.bodies[parent_idx];
        if let Some(grandparent) = parent.parent {
            // Moon of a planet: use parent's distance from grandparent
            if grandparent == solar_system.sun_index {
                return parent.orbit.as_ref().map_or(0.0, |o| o.semi_major_axis / AU);
            }
        }
        // Direct child of sun
        if parent_idx == solar_system.sun_index {
            return body.orbit.as_ref().map_or(0.0, |o| o.semi_major_axis / AU);
        }
        // Fallback: use parent's SMA
        parent.orbit.as_ref().map_or(0.0, |o| o.semi_major_axis / AU)
    } else {
        // Sun itself
        0.0
    }
}

/// Science reward for achieving orbit around a body at given AU distance.
pub fn orbit_science_reward(dist_au: f64) -> f64 {
    50.0 + 30.0 * (1.0 + dist_au).ln()
}

/// Science reward for landing on a body at given AU distance.
pub fn landing_science_reward(dist_au: f64) -> f64 {
    100.0 + 80.0 * (1.0 + dist_au).ln()
}

/// Compute the material costs of a blueprint as colony resources.
/// Returns Vec of (ResourceType, kg) for each material used.
pub fn blueprint_material_costs(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
) -> Vec<(ResourceType, f64)> {
    let mut totals = std::collections::HashMap::<ResourceType, f64>::new();

    for part in &blueprint.parts {
        let Some(def) = part_defs.get(&part.definition_id) else {
            continue;
        };
        let dry_mass_kg = def.mass * 1000.0; // tonnes → kg
        let breakdown = material_breakdown(def);
        let masses = breakdown.to_masses(dry_mass_kg);

        *totals.entry(ResourceType::StructuralMetal).or_default() += masses.metal_kg;
        *totals.entry(ResourceType::HighTempAlloys).or_default() += masses.hta_kg;
        *totals.entry(ResourceType::Electronics).or_default() += masses.elec_kg;
        if masses.super_kg > 0.0 {
            *totals.entry(ResourceType::Superconductors).or_default() += masses.super_kg;
        }
        if masses.pi_kg > 0.0 {
            *totals.entry(ResourceType::PrecisionInstruments).or_default() += masses.pi_kg;
        }
    }

    totals.into_iter().filter(|(_, v)| *v > 0.0).collect()
}

/// Compute the total dry mass of a blueprint in kg.
pub fn blueprint_dry_mass_kg(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
) -> f64 {
    blueprint
        .parts
        .iter()
        .filter_map(|p| part_defs.get(&p.definition_id))
        .map(|d| d.mass * 1000.0) // tonnes → kg
        .sum()
}
