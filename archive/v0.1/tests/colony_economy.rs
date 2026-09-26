mod common;

use sunscatter_app::colony::economy::{format_money, fuel_price_per_kg, orbit_science_reward, landing_science_reward, MaterialBreakdown};
use sunscatter_app::colony::contracts::Destination;
use sunscatter_app::colony::trade::CargoManifest;
use sunscatter_app::colony::ResourceType;
use sunscatter_app::parts::FuelType;

#[test]
fn format_money_zero() {
    assert_eq!(format_money(0.0), "$0", "Zero dollars should format as '$0'");
}

#[test]
fn format_money_ranges() {
    assert_eq!(format_money(999.0), "$999", "999 should format without suffix");
    assert_eq!(format_money(1000.0), "$1.0K", "1000 should format as $1.0K");
    assert_eq!(format_money(1_500_000.0), "$1.5M", "1.5 million should format as $1.5M");
    assert_eq!(format_money(1_000_000_000.0), "$1.0B", "1 billion should format as $1.0B");
    assert_eq!(format_money(1_000_000_000_000.0), "$1.0T", "1 trillion should format as $1.0T");
}

#[test]
fn format_money_negative() {
    assert_eq!(format_money(-5_000_000.0), "-$5.0M", "Negative 5 million should format as '-$5.0M'");
}

#[test]
fn fuel_price_per_kg_known_values() {
    assert_eq!(fuel_price_per_kg(FuelType::Rp1), 1.0, "RP-1 should cost $1/kg");
    assert_eq!(fuel_price_per_kg(FuelType::Methane), 2.0, "Methane should cost $2/kg");
    assert_eq!(fuel_price_per_kg(FuelType::Xenon), 3000.0, "Xenon should cost $3000/kg");
    assert_eq!(fuel_price_per_kg(FuelType::Empty), 0.0, "Empty should cost $0/kg");
    assert_eq!(fuel_price_per_kg(FuelType::Antimatter), 0.0, "Antimatter should cost $0/kg (not purchasable on Earth)");
    assert_eq!(fuel_price_per_kg(FuelType::NuclearPulse), 100_000.0, "Nuclear pulse units should cost $100,000/kg");
}

#[test]
fn orbit_science_reward_at_zero() {
    // Formula: 50.0 + 30.0 * ln(1.0 + 0.0) = 50.0 + 30.0 * 0.0 = 50.0
    let reward = orbit_science_reward(0.0);
    assert!(
        (reward - 50.0).abs() < 1e-10,
        "Orbit science reward at 0 AU should be 50.0, got {reward}"
    );
}

#[test]
fn landing_science_reward_at_zero() {
    // Formula: 100.0 + 80.0 * ln(1.0 + 0.0) = 100.0 + 80.0 * 0.0 = 100.0
    let reward = landing_science_reward(0.0);
    assert!(
        (reward - 100.0).abs() < 1e-10,
        "Landing science reward at 0 AU should be 100.0, got {reward}"
    );
}

#[test]
fn science_rewards_monotonic() {
    let distances = [0.4, 1.0, 5.2, 30.0];
    for window in distances.windows(2) {
        let r_near = orbit_science_reward(window[0]);
        let r_far = orbit_science_reward(window[1]);
        assert!(
            r_far > r_near,
            "Orbit science reward should increase with distance: reward({}) = {r_near} should be < reward({}) = {r_far}",
            window[0], window[1]
        );
    }
}

#[test]
fn landing_always_exceeds_orbit() {
    let distances = [0.0, 0.4, 1.0, 5.2, 30.0];
    for &d in &distances {
        let orbit = orbit_science_reward(d);
        let landing = landing_science_reward(d);
        assert!(
            landing > orbit,
            "Landing reward ({landing}) should exceed orbit reward ({orbit}) at distance {d} AU"
        );
    }
}

#[test]
fn material_breakdown_to_masses_conservation() {
    let breakdown = MaterialBreakdown {
        metal: 0.5,
        hta: 0.2,
        elec: 0.15,
        super_: 0.1,
        pi: 0.05,
    };
    let masses = breakdown.to_masses(1000.0);
    let total = masses.metal_kg + masses.hta_kg + masses.elec_kg + masses.super_kg + masses.pi_kg;
    assert!(
        (total - 1000.0).abs() < 1e-10,
        "Material masses should sum to dry mass: expected 1000.0, got {total}"
    );
}

#[test]
fn destination_price_per_kg_monotonic() {
    let sub = Destination::Suborbital.price_per_kg();
    let leo = Destination::LowEarthOrbit.price_per_kg();
    let lunar_orbit = Destination::LunarOrbit.price_per_kg();
    let lunar_surface = Destination::LunarSurface.price_per_kg();
    let mars_orbit = Destination::MarsOrbit.price_per_kg();
    let mars_surface = Destination::MarsSurface.price_per_kg();

    assert!(sub <= leo, "Suborbital ({sub}) should be <= LEO ({leo})");
    assert!(leo < lunar_orbit, "LEO ({leo}) should be < LunarOrbit ({lunar_orbit})");
    assert!(lunar_orbit < lunar_surface, "LunarOrbit ({lunar_orbit}) should be < LunarSurface ({lunar_surface})");
    assert!(lunar_surface < mars_orbit, "LunarSurface ({lunar_surface}) should be < MarsOrbit ({mars_orbit})");
    assert!(mars_orbit < mars_surface, "MarsOrbit ({mars_orbit}) should be < MarsSurface ({mars_surface})");
}

#[test]
fn tourism_three_valid_three_none() {
    assert!(
        Destination::Suborbital.tourism_price_per_seat().is_some(),
        "Suborbital should have a tourism price"
    );
    assert!(
        Destination::LowEarthOrbit.tourism_price_per_seat().is_some(),
        "LowEarthOrbit should have a tourism price"
    );
    assert!(
        Destination::LunarOrbit.tourism_price_per_seat().is_some(),
        "LunarOrbit should have a tourism price"
    );
    assert!(
        Destination::LunarSurface.tourism_price_per_seat().is_none(),
        "LunarSurface should NOT have a tourism price"
    );
    assert!(
        Destination::MarsOrbit.tourism_price_per_seat().is_none(),
        "MarsOrbit should NOT have a tourism price"
    );
    assert!(
        Destination::MarsSurface.tourism_price_per_seat().is_none(),
        "MarsSurface should NOT have a tourism price"
    );
}

#[test]
fn cargo_manifest_empty_and_mass() {
    let empty = CargoManifest { items: vec![] };
    assert!(empty.is_empty(), "Default CargoManifest with no items should be empty");

    let loaded = CargoManifest {
        items: vec![
            (ResourceType::MetalOre, 100.0),
            (ResourceType::Water, 200.0),
        ],
    };
    assert!(!loaded.is_empty(), "CargoManifest with items should not be empty");
    assert!(
        (loaded.total_mass() - 300.0).abs() < 1e-10,
        "Total mass should be 300.0, got {}",
        loaded.total_mass()
    );
}

#[test]
fn cargo_manifest_zero_amounts_is_empty() {
    let manifest = CargoManifest {
        items: vec![
            (ResourceType::MetalOre, 0.0),
            (ResourceType::Water, 0.0),
        ],
    };
    assert!(
        manifest.is_empty(),
        "CargoManifest with all zero amounts should be considered empty"
    );
}
