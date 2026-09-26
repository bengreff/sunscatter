# Colony Foundation Data Model

Layer 0 of the colony system. Defines all data types for resources, buildings, colonies, tech tree, and science. No UI or simulation logic.

## Module Structure

```
src/colony/
  mod.rs          — Re-exports from submodules
  resources.rs    — ResourceType, ResourceInventory, Company
  buildings.rs    — BuildingType, FactoryRecipe, BuildingInstance, Colony, ColonyManager
  tech.rs         — TechNodeData, TechLineData, TechTree, DiscoveryTracker, ScienceState
  economy.rs      — MaterialBreakdown, cost computation, format_money, science rewards
  contracts.rs    — ContractType, Contract, ContractManager
```

## Resources (`resources.rs`)

### ResourceType
27-variant enum covering raw, processed, fuel, manufactured, and consumable resources (includes MirrorSegment). Each variant has:
- `display_name()` — human-readable name
- `earth_price()` — `Option<f64>` $/kg purchase price on Earth. `None` for non-purchasable resources (Regolith, AtmosphericCo2, GasGiantAtmosphere, Helium3, Antimatter).
- `all()` — all variants as static slice
- `from_display_name(name)` — reverse lookup by display name
- `is_ship_fuel()` — true for refined fuels that go in ship tanks (RP-1, Methane, LH2, LOX, Xenon, He-3, Antimatter); used to filter cargo container resource dropdown

### ResourceInventory
`HashMap<ResourceType, f64>` wrapper tracking kg of each resource. Methods: `get`, `set`, `add`, `remove` (returns false if insufficient), `has_enough`, `has_enough_all`, `remove_all` (atomic), `iter`, `total_mass`. Implements `Default`, `Serialize`, `Deserialize`.

### StorageAllocation
Per-resource storage allocation within a colony's total stockpile capacity. Resources can be "pinned" to a fixed percentage of total storage, or left as "auto" to share the remainder equally. Food is excluded (has its own capacity system).

- `capacity_for(resource, total_capacity, active_resources)` — returns max kg for this resource
- `set_pinned(resource, percent)` — pin a resource at a specific % (clamped to `[0, 100 - other_pinned_total]`)
- `unpin(resource)` — remove manual override, return to auto allocation
- `is_pinned(resource)` — whether a resource has a manual override
- `effective_pcts(active_resources)` — compute all percentages for display

Auto-allocation logic: `auto_pct = (100 - pinned_total) / num_auto_resources`. Each pinned resource keeps its set value.

### compute_active_resources
`compute_active_resources(resources, production)` — returns Vec of active resource types. A resource is active if stock > 0 OR production > 0. Food is excluded.

### Company
Player's financial state: `money: f64` (starting $25M), `rd_budget: f64` (starting $1M/yr).

## Buildings (`buildings.rs`)

### BuildingType
25-variant enum (includes MassDriverMk1-4, ParticleAcceleratorMk1-4). Each variant provides:
- `display_name()` — human-readable name
- `build_cost()` — `Vec<(ResourceType, f64)>` base build cost in kg
- `power_draw_kw()` — power consumption (Factory power varies by recipe, returns 0)
- `power_output_kw()` — power generation (solar farms at 1 AU, reactors constant)
- `maintenance_cost_per_30d()` — resources consumed per 30 days
- `total_build_mass()` — sum of build cost; Habitat adds 1,000 kg for pre-stocked food
- `affected_by_habitability()` — true for Habitat, BasicGreenhouse, AdvancedGreenhouse only
- `size_multiplier(body_radius_m)` — returns `2π · radius_km` for `ParticleAcceleratorMk4`, `1.0` otherwise
- `all()` — all variants as static slice
- `from_display_name(name)` — reverse lookup by display name
- `is_mass_driver()` — true for MassDriverMk1-4
- `mass_driver_track_m()` — track length in meters (None for non-drivers)
- `mass_driver_max_payload_kg()` — max payload in kg
- `mass_driver_launch_velocity(mass, is_mirror)` — launch velocity for given payload
- `mass_driver_launch_energy_j(mass, velocity)` — energy per launch (joules, 90% efficiency)
- `mass_driver_recharge_time_s(energy, power)` — recharge time from power

Particle Accelerator Mk IV stores per-km values; simulation multiplies by body circumference via `size_multiplier`. This multiplier SHALL be applied to `build_cost`, `total_build_mass`, `power_draw_kw`, and `maintenance_cost_per_30d` at every consumption site (construction queueing, construction display, power balance, maintenance processing). `Colony::queue_building` and `Colony::can_queue_building` SHALL accept `body_radius_m` so the multiplier can be applied at the call site.

### FactoryRecipe
16-variant enum (includes MirrorSegmentAssembly). Each provides:
- `display_name()`, `inputs()`, `outputs()` — per-batch resource amounts
- `power_draw_kw()` — power while running
- `batch_time_hours()` — time per batch
- `requires_colocation()` — `Option<BuildingType>` (TritiumBreeding and NpuAssembly require FissionReactor)

### BuildingInstance
Runtime state for a single building: type, assigned_resource (mines), assigned_recipe (factories), operational flag, degradation (0-1), water_fill (greenhouses).

### Colony
Per-body colony state: body_index, name, buildings vec, resources inventory, crew count, food, power balance, construction queue, is_orbital_station flag, lab_science_extracted, lab_elapsed_years, food_depleted_notified, mass_driver_energy_j, mirrors_launched, storage_allocation, receiver_power_kw, receiver_laser_power_kw. Helper methods: `best_mass_driver()`, `has_mass_driver()`.

Food capacity: `Colony::food_capacity()` returns total food storage capacity in kg:
- Habitat: 3,000 kg per operational building
- FoodStorage: 10,000 kg per operational building

Crew capacity: `Colony::crew_capacity()` returns 20 per operational Habitat.

### ColonyManager
`Vec<Colony>` with lookup by body_index: `get_by_body()`, `get_by_body_mut()`, `has_colony()`, `add_colony()`.

## Tech Tree (`tech.rs`)

### Data File: `data/tech/tree.ron`
39 tech nodes across 9 eras, 11 efficiency lines, default unlocked parts list. Loaded as `TechTreeFile`.

### TechTree (Runtime)
Static definitions (nodes, lines) loaded from RON file at startup. Dynamic state (unlocked set, line tiers) persisted in saves.
- `is_unlocked()`, `can_unlock()`, `unlock()` — node unlock management
- `is_part_available()` — checks default parts + unlocked nodes
- `is_building_available()` — checks unlocked nodes
- `is_recipe_available()` — checks efficiency line tier gates
- `line_tier()`, `tier_cost()`, `upgrade_line()` — efficiency line management
- `tier_multiplier(tier)` — `1.11^tier` (4.78x at tier 15)
- `apply_save_state()` — restore from save data

### ScienceState
Tracks available science points and cumulative sources (discovery, R&D, lab).

### DiscoveryTracker
One-time milestone flags (first_suborbital, first_orbit, geostationary) and per-body sets (body_orbited, body_landed).

## CelestialBody Extensions (`bodies.rs`)

Three new fields on `CelestialBody`:
- `mineable_resources: Vec<ResourceType>` — resources extractable by Mine
- `atmospheric_resources: Vec<ResourceType>` — resources extractable by Atmospheric Collector
- `habitability_score: u32` — 0-100, affects Habitat/Greenhouse cost multiplier

Populated for all 21 bodies per colonies.md.

## Game Integration (`game.rs`)

Fields on `Game`:
- `colony_manager: ColonyManager`
- `company: Company`
- `science: ScienceState`
- `tech_tree: TechTree`
- `contracts: ContractManager`
- `colony_view_body_index: Option<usize>` — body index of colony being viewed in Colony mode
- `colony_return_mode: Option<GameMode>` — which mode to return to when leaving Colony mode

`GameMode::Colony` variant for full-screen colony management. `enter_colony(body_index, from_mode)` sets view state and switches mode. `leave_colony()` returns to the previous mode.

Initialized in `new()` (tech tree loaded from file) and reset in `reset_for_new_game()`.

## Save System (`save.rs`)

Fields on `SaveGame`, all with `#[serde(default)]`:
- `colonies: ColonyManager`
- `company: Company`
- `science: ScienceState`
- `tech_unlocked: HashSet<String>` — which nodes are researched
- `tech_line_tiers: HashMap<String, u32>` — efficiency line progress
- `contracts: ContractManager`

Tech tree static data NOT saved — reloaded from `data/tech/tree.ron` at startup. Version check changed from `!=` to `>` to accept current and older save versions.
