//! Smoke test for `save::test_fixture::build()`. Verifies the fixture
//! constructs without panicking, satisfies key invariants, and round-trips
//! through `write_to_path` → `read_to_string` → `from_str` cleanly. Drops
//! the resulting RON at `target/test_artifacts/Mass_Driver_Test/save.ron`
//! as a side effect so the file is available for ad-hoc inspection.
//!
//! Run with: cargo test --test generate_mass_driver_save -- --nocapture

use std::fs;
use std::path::PathBuf;

use sunscatter_app::save::{test_fixture, SaveGame};

#[test]
fn generate_mass_driver_test_save() {
    let mut save = test_fixture::build();

    // Pin the on-disk name so the output path is the legacy
    // `target/test_artifacts/Mass_Driver_Test/save.ron` — unchanged from
    // before the fixture was extracted, so any ad-hoc tooling that points
    // at that path keeps working.
    save.name = "Mass Driver Test".to_string();

    // ---- invariants the fixture must satisfy ----
    assert!(save.simulation_time > 0.0, "simulation_time should be positive");
    assert!(
        save.company.money >= 1.0e9,
        "fixture should have generous funds (got {})",
        save.company.money,
    );
    assert!(
        save.colonies.colonies.len() >= 2,
        "fixture should have at least 2 colonies (Mercury + Moon)",
    );
    assert!(
        save.tech_unlocked.len() >= 40,
        "fixture should unlock the full tech tree (got {} nodes)",
        save.tech_unlocked.len(),
    );
    assert!(
        !save.dyson_swarms.is_empty(),
        "fixture should include at least one Dyson swarm",
    );
    assert!(!save.vessels.is_empty(), "fixture should spawn a test vessel");

    let total_buildings: usize = save
        .colonies
        .colonies
        .iter()
        .map(|c| c.buildings.len())
        .sum();
    assert!(
        total_buildings > 100,
        "fixture should have substantial building counts (got {})",
        total_buildings,
    );

    // ---- write to target/ and round-trip ----
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test_artifacts");
    let written = save
        .write_to_path(&out_dir)
        .expect("Failed to write fixture save file");
    println!("Save written to {}", written.display());

    let roundtrip_content = fs::read_to_string(&written)
        .expect("Failed to read fixture back from disk");
    let reloaded: SaveGame = ron::from_str(&roundtrip_content)
        .expect("Failed to parse fixture RON");
    assert_eq!(reloaded.name, "Mass Driver Test");
    assert_eq!(reloaded.colonies.colonies.len(), save.colonies.colonies.len());
    assert_eq!(reloaded.tech_unlocked.len(), save.tech_unlocked.len());
}
