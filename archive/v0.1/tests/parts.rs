mod common;

use sunscatter_app::parts::{
    PartDefinitions, PartSize, PartCategory, FuelType,
    BlueprintPart, VesselBlueprint, AttachmentType, FlightVessel,
};
use std::f64::consts::PI;

fn load_parts() -> PartDefinitions {
    PartDefinitions::load_from_directory("data/parts").unwrap()
}

#[test]
fn part_size_grid_widths() {
    assert_eq!(PartSize::Tiny.grid_width(), 1, "Tiny should be 1 grid square wide");
    assert_eq!(PartSize::Small.grid_width(), 3, "Small should be 3 grid squares wide");
    assert_eq!(PartSize::Medium.grid_width(), 5, "Medium should be 5 grid squares wide");
    assert_eq!(PartSize::Large.grid_width(), 9, "Large should be 9 grid squares wide");
    assert_eq!(PartSize::XL.grid_width(), 13, "XL should be 13 grid squares wide");
}

#[test]
fn hitbox_width_equals_grid_times_constant() {
    let defs = load_parts();
    for part in defs.all() {
        let expected = part.hitbox_grid_width() as f64 * 0.5;
        assert_eq!(
            part.hitbox_width(),
            expected,
            "Part '{}': hitbox_width() should equal hitbox_grid_width() * GRID_SQUARE_SIZE (0.5)",
            part.name
        );
    }
}

#[test]
fn weld_hitbox_is_105_percent() {
    let defs = load_parts();
    for part in defs.all() {
        let width_diff = (part.weld_hitbox_width() - part.hitbox_width() * 1.05).abs();
        assert!(
            width_diff < 1e-9,
            "Part '{}': weld_hitbox_width ({}) should be 1.05 * hitbox_width ({})",
            part.name,
            part.weld_hitbox_width(),
            part.hitbox_width()
        );

        let height_diff = (part.weld_hitbox_height() - part.hitbox_height() * 1.05).abs();
        assert!(
            height_diff < 1e-9,
            "Part '{}': weld_hitbox_height ({}) should be 1.05 * hitbox_height ({})",
            part.name,
            part.weld_hitbox_height(),
            part.hitbox_height()
        );
    }
}

#[test]
fn rotated_hitbox_swaps_at_90_degrees() {
    let defs = load_parts();
    let non_square = defs
        .all()
        .find(|p| p.hitbox_grid_width() != p.hitbox_grid_height())
        .expect("Should find at least one non-square part");

    let orig_w = non_square.hitbox_grid_width();
    let orig_h = non_square.hitbox_grid_height();

    // At 90 degrees, width and height should swap
    let rot_w = non_square.rotated_hitbox_grid_width(PI / 2.0);
    let rot_h = non_square.rotated_hitbox_grid_height(PI / 2.0);
    assert_eq!(
        rot_w, orig_h,
        "Part '{}': rotated_hitbox_grid_width at 90deg should equal original hitbox_grid_height",
        non_square.name
    );
    assert_eq!(
        rot_h, orig_w,
        "Part '{}': rotated_hitbox_grid_height at 90deg should equal original hitbox_grid_width",
        non_square.name
    );

    // At 180 degrees, should revert to original
    let rot_w_180 = non_square.rotated_hitbox_grid_width(PI);
    let rot_h_180 = non_square.rotated_hitbox_grid_height(PI);
    assert_eq!(
        rot_w_180, orig_w,
        "Part '{}': rotated_hitbox_grid_width at 180deg should equal original hitbox_grid_width",
        non_square.name
    );
    assert_eq!(
        rot_h_180, orig_h,
        "Part '{}': rotated_hitbox_grid_height at 180deg should equal original hitbox_grid_height",
        non_square.name
    );
}

#[test]
fn rotated_weld_hitbox_swaps() {
    let defs = load_parts();
    let non_square = defs
        .all()
        .find(|p| p.hitbox_grid_width() != p.hitbox_grid_height())
        .expect("Should find at least one non-square part");

    let orig_weld_w = non_square.weld_hitbox_width();
    let orig_weld_h = non_square.weld_hitbox_height();

    // At 90 degrees, weld hitbox width and height should swap
    let rot_weld_w = non_square.rotated_weld_hitbox_width(PI / 2.0);
    let rot_weld_h = non_square.rotated_weld_hitbox_height(PI / 2.0);
    assert!(
        (rot_weld_w - orig_weld_h).abs() < 1e-9,
        "Part '{}': rotated_weld_hitbox_width at 90deg ({}) should equal original weld_hitbox_height ({})",
        non_square.name,
        rot_weld_w,
        orig_weld_h
    );
    assert!(
        (rot_weld_h - orig_weld_w).abs() < 1e-9,
        "Part '{}': rotated_weld_hitbox_height at 90deg ({}) should equal original weld_hitbox_width ({})",
        non_square.name,
        rot_weld_h,
        orig_weld_w
    );

    // At 180 degrees, should revert to original
    let rot_weld_w_180 = non_square.rotated_weld_hitbox_width(PI);
    let rot_weld_h_180 = non_square.rotated_weld_hitbox_height(PI);
    assert!(
        (rot_weld_w_180 - orig_weld_w).abs() < 1e-9,
        "Part '{}': rotated_weld_hitbox_width at 180deg ({}) should equal original weld_hitbox_width ({})",
        non_square.name,
        rot_weld_w_180,
        orig_weld_w
    );
    assert!(
        (rot_weld_h_180 - orig_weld_h).abs() < 1e-9,
        "Part '{}': rotated_weld_hitbox_height at 180deg ({}) should equal original weld_hitbox_height ({})",
        non_square.name,
        rot_weld_h_180,
        orig_weld_h
    );
}

#[test]
fn engine_mass_flow_formula() {
    let defs = load_parts();
    for part in defs.all() {
        if let Some(engine) = part.engine.as_ref() {
            let expected = engine.thrust_vac * 1000.0 / (9.80665 * engine.isp_vac);
            let actual = engine.total_mass_flow_kg_s();
            let rel_diff = if expected.abs() > 1e-30 {
                (actual - expected).abs() / expected.abs()
            } else {
                actual.abs()
            };
            assert!(
                rel_diff < 1e-3,
                "Part '{}': mass flow {} should match thrust_vac*1000/(g0*isp_vac) = {} (rel diff {:.4e})",
                part.name,
                actual,
                expected,
                rel_diff
            );
        }
    }
}

#[test]
fn fuel_type_propellant_positive() {
    for ft in FuelType::all() {
        if *ft == FuelType::Empty {
            continue;
        }
        let (ox, fuel) = ft.propellant_per_grid_square();
        assert!(
            ox > 0.0 || fuel > 0.0,
            "FuelType::{:?} should have at least one positive propellant value, got ({}, {})",
            ft,
            ox,
            fuel
        );
    }
}

#[test]
fn empty_fuel_type_zero() {
    let (ox, fuel) = FuelType::Empty.propellant_per_grid_square();
    assert_eq!(
        (ox, fuel),
        (0.0, 0.0),
        "FuelType::Empty should return (0.0, 0.0)"
    );
}

#[test]
fn fuel_type_all_covers_all_variants() {
    // Sentinel: if a new FuelType is added, `all()` must be extended.
    let count = FuelType::all().len();
    assert!(
        count >= 10,
        "FuelType::all() should enumerate at least 10 variants, got {}",
        count
    );

    // Exhaustive match: adding a new variant forces this block to fail to compile,
    // reminding the author to also update FuelType::all() and related lookups.
    for ft in FuelType::all() {
        match ft {
            FuelType::Empty
            | FuelType::Rp1
            | FuelType::Methane
            | FuelType::Hydrogen
            | FuelType::Monopropellant
            | FuelType::PureHydrogen
            | FuelType::Xenon
            | FuelType::FusionFuel
            | FuelType::Antimatter
            | FuelType::NuclearPulse => {}
        }
    }
}

#[test]
fn every_fuel_type_has_price() {
    use sunscatter_app::colony::economy::fuel_price_per_kg;
    for ft in FuelType::all() {
        let price = fuel_price_per_kg(*ft);
        assert!(
            price >= 0.0,
            "FuelType::{:?} has negative price {}",
            ft,
            price
        );
        if *ft == FuelType::Empty {
            assert_eq!(price, 0.0, "Empty fuel should have zero price");
        }
    }
}

#[test]
fn non_empty_fuel_types_have_resource_name() {
    for ft in FuelType::all() {
        let name = ft.fuel_resource_name();
        if *ft == FuelType::Empty {
            assert!(name.is_none(), "Empty fuel should have no resource name");
        } else {
            assert!(
                name.is_some(),
                "FuelType::{:?} must have a fuel_resource_name() (used by colony/trade lookups)",
                ft
            );
            assert!(
                !name.unwrap().is_empty(),
                "FuelType::{:?} resource name is empty string",
                ft
            );
        }
    }
}

#[test]
fn fuel_resource_names_map_to_resource_types() {
    // Every fuel type must have a documented mapping to a ResourceType (or None).
    // If a new FuelType is added, this table must be extended — the final assert
    // guards that every variant in FuelType::all() appears in `expected`.
    use sunscatter_app::colony::ResourceType;

    let expected: &[(FuelType, Option<ResourceType>)] = &[
        (FuelType::Rp1, Some(ResourceType::Rp1)),
        (FuelType::Methane, Some(ResourceType::Methane)),
        (FuelType::Hydrogen, Some(ResourceType::LiquidHydrogen)),
        (FuelType::PureHydrogen, Some(ResourceType::LiquidHydrogen)),
        (FuelType::Xenon, Some(ResourceType::Xenon)),
        (FuelType::FusionFuel, Some(ResourceType::Deuterium)),
        (FuelType::Antimatter, Some(ResourceType::Antimatter)),
        (FuelType::NuclearPulse, Some(ResourceType::NuclearPulseUnits)),
        (FuelType::Monopropellant, None), // intentionally not a colony resource
        (FuelType::Empty, None),
    ];

    for ft in FuelType::all() {
        assert!(
            expected.iter().any(|(f, _)| f == ft),
            "FuelType::{:?} missing from expected ResourceType mapping table — \
             add a mapping in tests/parts.rs::fuel_resource_names_map_to_resource_types \
             and verify the corresponding match arm in src/colony/trade.rs",
            ft
        );
    }
}

#[test]
fn all_engines_positive_thrust_isp() {
    let defs = load_parts();
    for part in defs.all() {
        if let Some(engine) = part.engine.as_ref() {
            assert!(
                engine.thrust_vac > 0.0,
                "Part '{}': engine thrust_vac ({}) should be positive",
                part.name,
                engine.thrust_vac
            );
            assert!(
                engine.isp_vac > 0.0,
                "Part '{}': engine isp_vac ({}) should be positive",
                part.name,
                engine.isp_vac
            );
        }
    }
}

#[test]
fn all_9_categories_have_parts() {
    let defs = load_parts();
    for cat in PartCategory::all() {
        let count = defs.by_category(*cat).len();
        assert!(
            count > 0,
            "Category {:?} should have at least one part, but has {}",
            cat,
            count
        );
    }
}

#[test]
fn loaded_parts_count() {
    let defs = load_parts();
    let count = defs.all().count();
    assert!(
        count >= 140,
        "Expected at least 140 parts loaded from data/parts/, got {}",
        count
    );
}

// ============================================================
// Vessel construction and delta-v
// ============================================================

fn make_simple_rocket(defs: &PartDefinitions) -> FlightVessel {
    let mut blueprint = VesselBlueprint::new("test_rocket".to_string());
    // Positions are in meters (grid_squares * GRID_SQUARE_SIZE where GRID_SQUARE_SIZE = 0.5m).
    // Pod: hitbox 3x3 grid = 1.5x1.5m. Tank: hitbox 3x2 grid = 1.5x1.0m.
    // Engine: hitbox 3x4 grid = 1.5x2.0m.
    // Stack tightly so weld hitboxes overlap at boundaries.
    blueprint.parts = vec![
        BlueprintPart {
            definition_id: "pod_small".to_string(),
            position: [0.0, 1.25],  // center of pod
            attachment_type: AttachmentType::Root,
            ..Default::default()
        },
        BlueprintPart {
            definition_id: "tank_small_2".to_string(),
            position: [0.0, 0.0],   // center of tank, top touches pod bottom
            parent_index: Some(0),
            attachment_type: AttachmentType::Stack,
            fuel_type: FuelType::Rp1,
            fill_fraction: 1.0,
            ..Default::default()
        },
        BlueprintPart {
            definition_id: "engine_wolf".to_string(),
            position: [0.0, -1.5],  // center of engine, top touches tank bottom
            parent_index: Some(1),
            attachment_type: AttachmentType::Stack,
            ..Default::default()
        },
    ];
    blueprint.root_part_index = 0;
    blueprint.stages = vec![vec![2]]; // engine in stage 0

    FlightVessel::from_blueprint(
        &blueprint,
        defs,
        [0.0, 6.471e6], // surface of Earth
        [0.0, 0.0],
        0, // soi_body
    )
    .expect("Simple rocket should construct from blueprint")
}

#[test]
fn vessel_from_blueprint_succeeds() {
    let defs = load_parts();
    let vessel = make_simple_rocket(&defs);
    assert!(
        vessel.total_mass > 0.0,
        "Vessel should have positive total_mass"
    );
}

#[test]
fn vessel_delta_v_positive() {
    let defs = load_parts();
    let vessel = make_simple_rocket(&defs);
    let stage_dvs = vessel.calculate_stage_delta_v(&defs);
    assert!(
        !stage_dvs.is_empty(),
        "Stage delta-v list should not be empty"
    );
    let total_dv: f64 = stage_dvs.iter().map(|(dv, _)| dv).sum();
    assert!(
        total_dv > 0.0,
        "Total delta-v should be positive, got {}",
        total_dv
    );
}

#[test]
fn vessel_delta_v_tsiolkovsky() {
    let defs = load_parts();
    let vessel = make_simple_rocket(&defs);

    // Manually compute expected delta-v using Tsiolkovsky equation:
    // dv = Isp * g0 * ln(wet_mass / dry_mass)
    let pod = defs.get("pod_small").unwrap();
    let tank = defs.get("tank_small_2").unwrap();
    let engine = defs.get("engine_wolf").unwrap();
    let eng = engine.engine.as_ref().unwrap();

    let dry_mass_kg = (pod.mass + tank.mass + engine.mass) * 1000.0; // tonnes -> kg
    let (ox_per_sq, fuel_per_sq) = FuelType::Rp1.propellant_per_grid_square();
    let tank_area = tank.tank.as_ref().unwrap().grid_area;
    let fuel_mass_kg = (ox_per_sq + fuel_per_sq) * tank_area;
    let wet_mass_kg = dry_mass_kg + fuel_mass_kg;

    let expected_dv = eng.isp_vac * 9.80665 * (wet_mass_kg / dry_mass_kg).ln();

    let stage_dvs = vessel.calculate_stage_delta_v(&defs);
    let actual_dv: f64 = stage_dvs.iter().map(|(dv, _)| dv).sum();

    let rel_diff = (actual_dv - expected_dv).abs() / expected_dv;
    assert!(
        rel_diff < 0.05,
        "Delta-v {:.1} should match Tsiolkovsky {:.1} (rel diff {:.4})",
        actual_dv,
        expected_dv,
        rel_diff
    );
}

#[test]
fn part_size_ordering() {
    assert!(
        PartSize::Tiny.grid_width() < PartSize::Small.grid_width(),
        "Tiny ({}) should be less than Small ({})",
        PartSize::Tiny.grid_width(),
        PartSize::Small.grid_width()
    );
    assert!(
        PartSize::Small.grid_width() < PartSize::Medium.grid_width(),
        "Small ({}) should be less than Medium ({})",
        PartSize::Small.grid_width(),
        PartSize::Medium.grid_width()
    );
    assert!(
        PartSize::Medium.grid_width() < PartSize::Large.grid_width(),
        "Medium ({}) should be less than Large ({})",
        PartSize::Medium.grid_width(),
        PartSize::Large.grid_width()
    );
    assert!(
        PartSize::Large.grid_width() < PartSize::XL.grid_width(),
        "Large ({}) should be less than XL ({})",
        PartSize::Large.grid_width(),
        PartSize::XL.grid_width()
    );
}
