mod common;

use sunscatter_app::colony::{ResourceType, ResourceInventory};

#[test]
fn empty_inventory_returns_zero() {
    let inv = ResourceInventory::new();
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        0.0,
        "Empty inventory should return 0.0 for any resource"
    );
    assert_eq!(
        inv.total_mass(),
        0.0,
        "Empty inventory should have 0.0 total mass"
    );
}

#[test]
fn set_and_get_roundtrip() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 1000.0);
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        1000.0,
        "get() should return the value that was set()"
    );
}

#[test]
fn set_zero_removes_entry() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 500.0);
    inv.set(ResourceType::MetalOre, 0.0);
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        0.0,
        "Setting a resource to 0.0 should remove it from the inventory"
    );
    assert_eq!(
        inv.total_mass(),
        0.0,
        "Total mass should be 0.0 after removing the only resource"
    );
}

#[test]
fn set_negative_removes_entry() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 500.0);
    inv.set(ResourceType::MetalOre, -1.0);
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        0.0,
        "Setting a resource to a negative value should remove it (impl does `if amount <= 0.0 {{ remove }}`)"
    );
}

#[test]
fn add_ignores_negative() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 100.0);
    inv.add(ResourceType::MetalOre, -50.0);
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        100.0,
        "add() with a negative amount should be a no-op"
    );
}

#[test]
fn add_accumulates() {
    let mut inv = ResourceInventory::new();
    inv.add(ResourceType::MetalOre, 100.0);
    inv.add(ResourceType::MetalOre, 250.0);
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        350.0,
        "Successive add() calls should accumulate"
    );
}

#[test]
fn remove_succeeds_within_tolerance() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 100.0);
    let removed = inv.remove(ResourceType::MetalOre, 100.0 + 5e-10);
    assert!(
        removed,
        "remove() should succeed when the overshoot is within the 1e-9 tolerance"
    );
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        0.0,
        "Resource should be fully consumed after removing within tolerance"
    );
}

#[test]
fn remove_fails_beyond_tolerance() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 100.0);
    let removed = inv.remove(ResourceType::MetalOre, 100.0 + 2e-9);
    assert!(
        !removed,
        "remove() should fail when the overshoot exceeds the 1e-9 tolerance"
    );
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        100.0,
        "Inventory should be unchanged after a failed remove()"
    );
}

#[test]
fn remove_all_atomic_success() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 200.0);
    inv.set(ResourceType::Water, 500.0);
    let result = inv.remove_all(&[
        (ResourceType::MetalOre, 100.0),
        (ResourceType::Water, 200.0),
    ]);
    assert!(result, "remove_all() should succeed when all resources are sufficient");
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        100.0,
        "MetalOre should be reduced by the requested amount"
    );
    assert_eq!(
        inv.get(ResourceType::Water),
        300.0,
        "Water should be reduced by the requested amount"
    );
}

#[test]
fn remove_all_atomic_failure() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 50.0);
    inv.set(ResourceType::Water, 500.0);
    let result = inv.remove_all(&[
        (ResourceType::MetalOre, 100.0),
        (ResourceType::Water, 200.0),
    ]);
    assert!(
        !result,
        "remove_all() should fail when any resource is insufficient"
    );
    assert_eq!(
        inv.get(ResourceType::MetalOre),
        50.0,
        "MetalOre should be unchanged after atomic failure (atomicity guarantee)"
    );
    assert_eq!(
        inv.get(ResourceType::Water),
        500.0,
        "Water should be unchanged after atomic failure (atomicity guarantee)"
    );
}

#[test]
fn total_mass_sums_all() {
    let mut inv = ResourceInventory::new();
    inv.set(ResourceType::MetalOre, 100.0);
    inv.set(ResourceType::Water, 200.0);
    inv.set(ResourceType::Food, 300.0);
    assert_eq!(
        inv.total_mass(),
        600.0,
        "total_mass() should sum all resource amounts"
    );
}

#[test]
fn resource_type_display_name_roundtrip() {
    let all = ResourceType::all();
    assert!(
        all.len() >= 26,
        "ResourceType::all() should contain at least 26 resource types, got {}",
        all.len()
    );
    for r in all {
        let name = r.display_name();
        let roundtripped = ResourceType::from_display_name(name);
        assert_eq!(
            roundtripped,
            Some(*r),
            "from_display_name({:?}) should roundtrip back to {:?}",
            name,
            r
        );
    }
}
