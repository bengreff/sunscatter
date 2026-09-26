# Economy & Science System

Vessel cost computation, money management, science from discoveries and R&D, tech gating, and contracts.

## Module Structure

```
src/colony/
  economy.rs    — MaterialBreakdown, cost computation, format_money, science reward functions
  contracts.rs  — Destination, ContractKind, Contract, ContractPayload, GovernmentMilestone, ContractManager
```

## Material Cost System (`economy.rs`)

### Requirement: Material breakdown by part type

Each part's dry mass is decomposed into five construction materials using `material_breakdown(def: &PartDefinition) -> MaterialBreakdown`. Dispatch priority:
1. **Engines**: by propellant type — Chemical (Kerolox/Methalox/Hydrolox), NTR (Hydrogen/NuclearPulse), Ion/Hall (Xenon, mass < 250 kg), MPD (Xenon, mass >= 250 kg), Fusion (FusionFuel), Antimatter (primary or secondary propellant)
2. **Shields**: by ShieldType — Whipple, FRES, Geodesic
3. **Reactors**: by ID pattern — `reactor_am*` (antimatter), `reactor_fusion*` (fusion), `reactor_fission*` (large fission), else small fission
4. **RTG, Solar, Battery**: by component data presence
5. **Pods**: Command (crew + control), Probe (control only), Crew Quarters (crew only), Greenhouse (neither)
6. **Tanks, Decouplers, RCS, Cargo**: by component data
7. **Aerodynamic**: Heat shields, Parachutes, Fairings, then default (Nose Cones)

### Requirement: Earth cost formula

`MaterialMasses::earth_cost()` computes: `metal_kg * $100 + hta_kg * $1,000 + elec_kg * $10,000 + super_kg * $50,000 + pi_kg * $200,000`.

### Requirement: Vessel cost computation

`EditorState::calculate_vessel_cost()` sums material cost (dry mass * breakdown * prices) plus fuel cost (LOX at $0.50/kg + fuel at `fuel_price_per_kg()`) for all filled tanks.

### Requirement: Money deduction on launch

`Game::launch_from_editor()` computes vessel cost before creating the vessel. If cost > company money, returns `Err("Insufficient funds: ...")`. Otherwise deducts cost from company money.

### Requirement: Money display formatting

`format_money(dollars)` returns compact format: `$1.2K`, `$3.5M`, `$12.4B`, `$1.5T`. Handles negative values with leading `-`.

## Discovery Science (`economy.rs`, `game.rs`)

### Requirement: Earth milestones

`Game::check_discovery_milestones()` awards one-time science:
- **Suborbital**: altitude > 100 km in Earth SOI while Flying → 25 science
- **First orbit**: not suborbital + Flying in Earth SOI → 50 science
- **Geostationary**: altitude > 35,786 km + in orbit in Earth SOI → 25 science

### Requirement: Per-body discovery

- **Orbit**: Flying + not suborbital + body not in `body_orbited` set → `50 + 30 * ln(1 + dist_AU)` science
- **Landing**: Landed on body not in `body_landed` set → `100 + 80 * ln(1 + dist_AU)` science
- Earth landings and Earth orbit are excluded (handled by Earth milestones)

### Requirement: Body distance computation

`body_distance_au()` returns the body's distance from the Sun in AU. For moons, uses the parent body's semi-major axis.

## R&D Science Generation (`game.rs`)

### Requirement: R&D science formula

`Game::update_rd_science(dt_sim)`:
- `base_rate = 25 * ln(1 + budget / 1,000,000)` per year
- `effective_rate = base_rate * max(0, 1 - cumulative_rd / 50,000)` per year
- Deducts R&D budget proportionally from company money; stops if broke

## Tech Gating

### Requirement: Parts palette filtering

Parts in the editor palette are filtered by `tech_tree.is_part_available(&part.name)`. `TechTree::load()` automatically unlocks `basic_rocketry` so every new game starts with starter parts available. Default unlocked parts (from `tree.ron`): Gecko, Tiny Fuel Tank 1/2/4/8, Tiny Probe Core, NC-1 Nose Cone, AE-FF0 Fairing, TD-1 Decoupler, and additional starter parts (Hummingbird, Firefly, NC-1R/NC-1L Side Cones, HS-1 Heat Shield, Battery Bank Z-1). These parts all appear in the `basic_rocketry` node's `unlocks_parts` for discoverability. Size groups with no unlocked parts are hidden entirely.

## Tech Tree UI (`render/tech_tree_ui.rs`)

### Requirement: Tech tree window

A resizable egui::Window "Research" with two tabs:
- **Tech Nodes**: grouped by era (CollapsingHeaders), each node as a colored card — green (unlocked), blue (purchasable), yellow (need science), grey (locked). Click "Unlock" to spend science.
- **Efficiency Lines**: list of 11 lines showing current tier/15, efficiency multiplier, upgrade button with cost. Max tier = 15.

Accessed via "Research" button in editor toolbar.

## Contract System (`contracts.rs`)

See `openspec/specs/game/colony/contracts/spec.md` for full specification.

Pool-based contract system with payload delivery (price-per-kg), tourism (price-per-seat, requires recovery), and government milestones (automatic one-time awards). Contracts are the manual income bridge until trade routes automate income.

### Requirement: Contract persistence

`ContractManager` serialized in `SaveGame` with `#[serde(default)]` for backward compatibility. Pool refilled on load.
