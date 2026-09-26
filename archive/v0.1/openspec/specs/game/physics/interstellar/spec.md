# Interstellar Parts

Interstellar engines, reactors, shields, radiators, and fuel tanks for relativistic spaceflight.

## Part Category

### Requirement: Interstellar category

Interstellar engines and shields SHALL use `category: Interstellar`. Interstellar reactors (Prometheus, Vulcan, Stellarator, Tokamak, Penning, Ixion) use `category: Electricity` — they are power sources and belong with batteries, solar panels, RTGs, and small fission reactors in the Electricity tab. The Interstellar tab appears in the editor parts palette alongside other categories and displays parts in a flat list without size sub-grouping.

### Requirement: Engine rendering for Interstellar category

Engine rendering fallbacks (procedural engine details) SHALL trigger for parts with `engine.is_some()` in both `Propulsion` and `Interstellar` categories. Sprite-based rendering takes priority when sprites are available in the atlas.

### Requirement: Engine sprites in atlas

Interstellar engine sprites SHALL be included in the sprite atlas (not excluded). Sprite filenames in `data/sprites/engines/` match part definition IDs. Per-engine plume sprites in `data/sprites/plumes/` are looked up by stripping the `engine_` prefix from the part ID, with shared plumes for engine families (e.g., `daedalus` for both stage 1 and 2, `zpinch` for both probe and advanced).

## Data Model

### Requirement: Propellant types

`Propellant` enum SHALL include interstellar variants: `FusionFuel` (D+He3), `Antimatter`, `NuclearPulse` (Orion pulse units). Each maps to a corresponding `FuelType` variant.

### Requirement: Dual propellant engines

`EngineData` SHALL support optional `secondary_propellant: Option<Propellant>` and `secondary_fuel_fraction: f64` fields. The AM-Cat Fusion engine uses FusionFuel as primary propellant with Antimatter as secondary catalyst (0.24% of mass flow). The editor displays both propellant types (e.g. "D+He3 + Antimatter") and mass flow breakdown shows consumption of each.

### Requirement: Fuel types

`FuelType` enum SHALL include interstellar variants with `propellant_per_grid_square()` values:
- `FusionFuel`: (0.0, 158.0) — D+He3 mix ~79 kg/m³ (40% D at 162 kg/m³, 60% He3 at 59 kg/m³), FF 82%
- `Antimatter`: (0.0, 5.0) — mostly containment mass, milligrams of actual antimatter
- `NuclearPulse`: (0.0, 500.0) — heavy fissile pulse units

Fuel resource names: `fusion_fuel`, `antimatter`, `nuclear_pulse`.

### Requirement: Standard-tank fuel compatibility

`FuelType::is_standard_tank_compatible(self)` SHALL return `true` for Empty, RP-1, Methane, Hydrogen (hydrolox), Monopropellant, PureHydrogen (LH2), and FusionFuel; and `false` for Xenon, Antimatter, and NuclearPulse. The editor SHALL use this method to filter the fuel-type selector for tanks without a `fixed_fuel_type`. Standard rectangular tanks can therefore hold any of the seven compatible fuels (e.g. D+He3 may be loaded into a kerolox tank), but specialized fuels are reserved for their dedicated tank parts.

### Requirement: Locked-fuel tanks

`TankData` SHALL include an optional `fixed_fuel_type: Option<FuelType>` field (serde-defaulting to `None`). When `Some(ft)`:
- The editor fuel-type selector SHALL be replaced by a "Fuel Type: {ft} (locked)" label.
- On placement, `PlacedPart::fuel_type` SHALL be initialized to `ft` and `fill_fraction` to `1.0`.
- The palette tank-info panel SHALL show capacity for `ft` only, not the generic RP-1/CH4/LH2 trio.

Xenon, Antimatter, and Pulse-Unit tanks SHALL set `fixed_fuel_type`; standard fuel tanks and fusion spheres SHALL leave it `None`.

### Requirement: Fusion fuel in standard tanks

D+He3 (`FuelType::FusionFuel`) is standard-tank-compatible and may be selected in any non-locked tank. Standard rectangular tanks lack the cryogenic insulation modeled in the fusion sphere structural density and carry far less `grid_area` per part, so they are usable in early fusion stages but cannot match the bulk capacity of XL fusion spheres.

### Requirement: Pure hydrogen tank capacity

`PureHydrogen` tanks SHALL have capacity (0.0, 140.0) kg per grid square — FF 80%, a bit below the Hydrolox FF of 83.7%. Full tank volume available for hydrogen with no LOX.

### Requirement: ReactorData

`PartDefinition` SHALL support `reactor: Option<ReactorData>`. `ReactorData { output_watts: f64, fuel: Option<ReactorFuelData> }` defines constant power output in Watts. By default reactors generate continuously like RTGs — `fuel: None` means constant output with no fuel consumption (fission and fusion reactors).

### Requirement: Fuel-consuming reactors (ReactorFuelData)

`ReactorData::fuel` MAY be `Some(ReactorFuelData)`, in which case the reactor produces no power on any tick where its fuel reserves are insufficient. `ReactorFuelData` contains:
- `primary: FuelType` — primary fuel resource consumed
- `secondary: Option<FuelType>` — optional second fuel (for matter+antimatter mixes)
- `secondary_fraction: f64` — fraction of total mass flow that is the secondary fuel (0.0–1.0)
- `total_kg_s: f64` — total combined mass flow while running

In `FlightVessel::update_power`, fuel-consuming reactors are processed in a first phase before all other generation/consumption. The runtime calls `try_consume_reactor_fuel(fuel_data, dt)` which atomically checks both fuels' availability across the whole vessel; if either is insufficient the reactor produces no power that tick and **no fuel is drained**. If both are available, the reactor drains them proportionally across all non-destroyed, non-decoupled parts and adds `output_watts` to the generation pool.

Antimatter reactors (`reactor_am_small`, `reactor_am_large`) SHALL set `fuel.primary = Antimatter`, `fuel.secondary = Some(PureHydrogen)`, `fuel.secondary_fraction = 0.5` — they consume antimatter and LH2 in equal mass. Total mass-flow rates approximate published antimatter-reactor figures: 0.0000209 kg/s for the Penning (small) and 0.0000656 kg/s for the Ixion (large), corresponding to roughly 75 g/hr and 236 g/hr of combined reactant.

### Requirement: ShieldData

`PartDefinition` SHALL support `shield: Option<ShieldData>`. `ShieldData` contains:
- `shield_type: ShieldType` — enum with variants `Whipple`, `FRES`, `Geodesic`
- `max_velocity_c: f64` — maximum rated velocity as fraction of c
- `power_base_watts: f64` — base power consumption (0 for passive Whipple)

### Requirement: RadiatorData

`PartDefinition` SHALL support `radiator: Option<RadiatorData>`. `RadiatorData` contains:
- `tier: RadiatorTier` — enum with variants `HeatPipe`, `Droplet`, `Phononic`
- `rejection_watts: f64` — heat rejection at full deployment (Watts)
- `deploy_time_sec: f64` — seconds to fully deploy/retract (serde default 5.0)

Radiators reject `rejection_watts × deploy_fraction` per tick from the ship-wide waste-heat pool.
See `physics/aerodynamic_heating/spec.md` for the integration with per-part temperatures.

### Requirement: EngineData waste heat

`EngineData` SHALL support an optional `waste_heat_watts: f64` field (serde default 0.0).
When non-zero and `engine_active == true`, the engine contributes
`waste_heat_watts × throttle` to the ship-wide thermal pool each tick. Chemical, NTR, and
electric engines leave this at 0; interstellar engines set it according to the doc table
(`docs/part_ideas.md` "Engine waste heat notes").

### Requirement: ReactorData waste heat

`ReactorData` SHALL support an optional `waste_heat_watts: f64` field (serde default 0.0).
When non-zero and the reactor is producing power (i.e. not tripped and fuel available),
the reactor contributes `waste_heat_watts` to the ship-wide thermal pool each tick.

## Interstellar Engines

All interstellar engines are vacuum-only (`thrust_asl: 0`, `isp_asl: 0`), `shape: Trapezoid`, `category: Interstellar`, `size: XL`.

### Requirement: No gimbal on interstellar engines

Every engine in the Interstellar category SHALL have `gimbal_range: 0.0`. Interstellar-scale propulsion (Orion pulse, fusion drives, antimatter drives) is too physically constrained — by pusher-plate geometry, magnetic-nozzle alignment, or annihilation-chamber rigidity — to vector thrust. Vessels using these engines steer via RCS only. Verified by a unit test that iterates every Interstellar-category part with an `engine` and asserts `gimbal_range == 0.0`.

### Requirement: Antimatter engines consume antimatter + LH2 in equal mass

`Antimatter Torch` (`engine_am_torch`) and `Gamma Converter` (`engine_gamma_conversion`) SHALL set:
- `propellant: Antimatter`
- `secondary_propellant: Some(Hydrogen)` (pure LH2)
- `secondary_fuel_fraction: 0.5`

Both engines therefore consume an equal mass of antimatter and LH2 every tick. (`Propellant::Hydrogen` maps to `FuelType::PureHydrogen`, which shares the `"hydrogen"` resource name with the hydrolox `FuelType::Hydrogen`, so the engines draw from any LH2-bearing tank in their fuel zone — pure LH2 or hydrolox.) The existing `EngineData::secondary_propellant` mechanism handles consumption end-to-end; no new runtime code is required.

`AM-Cat Fusion` (`engine_amcat_fusion`) keeps its existing FusionFuel + Antimatter (0.24%) configuration — it uses antimatter as a catalyst, not a reactant.

### Nuclear Pulse (Tier 1 — Fission)

| ID | Name | Grid | Mass (t) | Thrust (kN) | Isp (s) | Propellant |
|----|------|------|----------|-------------|---------|------------|
| engine_orion_pulse | Orion Pulse Drive | 70x52 | 7000 | 60,000 | 4,079 | NuclearPulse |

### Fusion (Tier 2)

| ID | Name | Grid | Mass (t) | Thrust (kN) | Isp (s) | Propellant |
|----|------|------|----------|-------------|---------|------------|
| engine_daedalus_s1 | Daedalus Stage 1 | 60x48 | 4500 | 7,000 | 1,019,716 | FusionFuel |
| engine_daedalus_s2 | Daedalus Stage 2 | 44x34 | 2000 | 2,000 | 1,019,716 | FusionFuel |
| engine_zpinch_probe | Z-Pinch Probe | 10x18 | 600 | 800 | 815,773 | FusionFuel |
| engine_zpinch_advanced | Z-Pinch Advanced | 16x26 | 2500 | 5,000 | 2,039,432 | FusionFuel |

### Antimatter (Tier 3)

| ID | Name | Grid | Mass (t) | Thrust (kN) | Isp (s) | Propellant |
|----|------|------|----------|-------------|---------|------------|
| engine_amcat_fusion | AM-Cat Fusion | 22x16 | 2000 | 4,000 | 3,059,148 | FusionFuel + Antimatter (0.24%) |
| engine_am_torch | Antimatter Torch | 8x12 | 800 | 2,000 | 15,295,740 | Antimatter + LH2 (50/50) |
| engine_gamma_conversion | Gamma Converter | 34x26 | 1800 | 1,300 | 26,002,758 | Antimatter + LH2 (50/50) |

## Interstellar Reactors

All interstellar reactors are `category: Electricity`, `shape: Rectangle`. Constant power output, no fuel consumption. Listed in the Electricity tab alongside batteries, solar panels, RTGs, and small fission reactors.

| ID | Name | Size | Grid | Mass (t) | Output |
|----|------|------|------|----------|--------|
| reactor_fission_small | Prometheus | Medium | 5x7 | 250 | 500 MW |
| reactor_fission_large | Vulcan | Large | 9x9 | 800 | 1.6 GW |
| reactor_fusion_small | Stellarator | Large | 7x7 | 400 | 10 GW |
| reactor_fusion_large | Tokamak | XL | 11x9 | 1200 | 30 GW |
| reactor_am_small | Penning Reactor | Large | 7x7 | 800 | 800 GW |
| reactor_am_large | Ixion Reactor | XL | 11x9 | 2500 | 2.5 TW |

## Shields

All shields are `category: Interstellar`, `shape: Rectangle`. Three types across three sizes.

**Whipple** — passive debris protection, max 0.1c, no power:
| ID | Size | Grid | Mass (t) |
|----|------|------|----------|
| shield_whipple_small | Large | 9x3 | 300 |
| shield_whipple_medium | XL | 13x5 | 1200 |
| shield_whipple_large | XL | 19x5 | 4800 |

**FRES** — electromagnetic, max 0.85c, requires reactor power:
| ID | Size | Grid | Mass (t) | Power |
|----|------|------|----------|-------|
| shield_fres_small | Large | 9x9 | 300 | 1 GW |
| shield_fres_medium | XL | 13x11 | 750 | 2 GW |
| shield_fres_large | XL | 17x13 | 2400 | 4 GW |

**Geodesic** — force field, max 0.85c, highest power:
| ID | Size | Grid | Mass (t) | Power |
|----|------|------|----------|-------|
| shield_geodesic_small | Large | 9x9 | 800 | 4 GW |
| shield_geodesic_medium | XL | 13x11 | 2000 | 8 GW |
| shield_geodesic_large | XL | 17x13 | 6000 | 16 GW |

## Specialized Fuel Tanks

Three specialized tank families lock to a single fuel via `TankData::fixed_fuel_type`. They share the same Tiny / Small / Medium / Large dimensions as the existing Xenon tanks: 1x1, 3x1, 5x1, 9x1 grid with `grid_area` values 0.4 / 2.3 / 5.0 / 12.1. Mass varies by family to reflect containment requirements. All three families use `category: FuelTanks` (alongside Xenon tanks and standard chemical tanks) — they appear in the editor's Fuel Tanks palette tab under their respective size sub-groupings, NOT in the Interstellar tab.

The previously-existing LH2-specific tanks (`tank_h2_*`) were removed; PureHydrogen is now stored in standard tanks via the fuel-type selector. Stale `LH2-T`, `LH2-S`, `LH2-M`, `LH2-L` references were removed from the tech tree.

### Xenon Tanks (`category: FuelTanks`, locked to Xenon)

Supercritical xenon storage for ion / Hall / MPD propulsion. Structural density 28.1 kg/m³.

| ID | Name | Size | Mass (t) | Xenon Capacity |
|----|------|------|----------|----------------|
| tank_xe_tiny | Xe-1 | Tiny (1x1) | 0.006 | ~126 kg |
| tank_xe_small | Xe-3 | Small (3x1) | 0.032 | ~725 kg |
| tank_xe_medium | Xe-5 | Medium (5x1) | 0.069 | ~1,575 kg |
| tank_xe_large | Xe-9 | Large (9x1) | 0.167 | ~3,812 kg |

### Antimatter Penning Arrays (`category: FuelTanks`, locked to Antimatter)

Rectangular Penning-trap arrays for proton-antiproton storage. Mass dominated by superconducting magnets, cryocoolers, and radiation-hardened vessel; structural density ~40 kg/grid-area. Capacity reflects the global Antimatter density of 5 kg per grid-area.

| ID | Name | Size | Mass (t) | Antimatter Capacity |
|----|------|------|----------|---------------------|
| tank_am_tiny | AM-1 | Tiny (1x1) | 0.016 | ~2 kg |
| tank_am_small | AM-3 | Small (3x1) | 0.092 | ~11.5 kg |
| tank_am_medium | AM-5 | Medium (5x1) | 0.200 | ~25 kg |
| tank_am_large | AM-9 | Large (9x1) | 0.484 | ~60.5 kg |

### Antimatter Spheres — endgame bulk containment (`category: FuelTanks`, locked to Antimatter)

Three spherical bulk-storage tanks sharing the fusion sphere silhouette (20×20, 40×40, 60×60) but built around an advanced topological-confinement field rather than a Penning-trap array. Dry mass is 2× the corresponding fusion sphere; capacity equals what the same-size fusion sphere would hold as PureHydrogen (140 kg/grid-area × fusion-sphere grid_area). Implemented by inflating `TankData::grid_area` by a factor of 28 relative to the matching fusion sphere — a containment-efficiency abstraction, since the global Antimatter density (5 kg/grid-area) stays unchanged. Unlocked by the `bulk_am_storage` tech node (prereq: `am_torch`).

| ID | Name | Grid | Mass (t) | grid_area | Antimatter Capacity |
|----|------|------|----------|-----------|---------------------|
| tank_am_sphere_s | AM Sphere S | 20x20 | 10.388 | 18289.6 | ~91.4 t |
| tank_am_sphere_m | AM Sphere M | 40x40 | 58.768 | 103460.0 | ~517.3 t |
| tank_am_sphere_l | AM Sphere L | 60x60 | 161.944 | 285104.4 | ~1,425.5 t |

### Pulse-Unit Magazines (`category: FuelTanks`, locked to NuclearPulse)

Rectangular radiation-shielded steel magazines for Orion-style fission pulse units. Structural density ~60 kg/grid-area.

| ID | Name | Size | Mass (t) | Pulse-Unit Capacity |
|----|------|------|----------|---------------------|
| tank_pulse_tiny | PU-1 | Tiny (1x1) | 0.024 | ~200 kg |
| tank_pulse_small | PU-3 | Small (3x1) | 0.138 | ~1,150 kg |
| tank_pulse_medium | PU-5 | Medium (5x1) | 0.300 | ~2,500 kg |
| tank_pulse_large | PU-9 | Large (9x1) | 0.726 | ~6,050 kg |

### Fusion Spheres (`category: FuelTanks`, not locked)

Spherical cryogenic D+He3 tanks for bulk fusion-stage propellant. Structural density 16.2 kg/m³ (~95.2% fuel fraction). Geometric mean scaling: `grid_area = sqrt(d² × V_sphere / 0.491)`. Fusion spheres are NOT locked to FusionFuel — they may hold any standard-tank-compatible fuel — but their dimensions and structural density are optimized for cryogenic D+He3 storage.

| ID | Name | Grid | Mass (t) | grid_area | D+He3 Capacity |
|----|------|------|----------|-----------|----------------|
| tank_sphere_s | Fusion Sphere S | 20x20 | 5.194 | 653.2 | ~103 t |
| tank_sphere_m | Fusion Sphere M | 40x40 | 29.384 | 3695.0 | ~584 t |
| tank_sphere_l | Fusion Sphere L | 60x60 | 80.972 | 10182.3 | ~1,609 t |

## Radiators

All radiators are `category: Interstellar`, `shape: Rectangle`, deployable wings. Twelve parts
across three tier × four sizes. Mass and rejection scale with deployed wing area.

| ID | Name | Tier | Size | Grid | Mass (t) | Rejection |
|----|------|------|------|------|----------|-----------|
| radiator_heatpipe_tiny | Heat Pipe Panel T | HeatPipe | Tiny | 1x2 | 1.85 | 50 MW |
| radiator_heatpipe_small | Heat Pipe Panel S | HeatPipe | Small | 1x4 | 9.3 | 250 MW |
| radiator_heatpipe_medium | Heat Pipe Panel M | HeatPipe | Medium | 1x8 | 37 | 1 GW |
| radiator_heatpipe_large | Heat Pipe Panel L | HeatPipe | Large | 1x16 | 148 | 4 GW |
| radiator_droplet_tiny | Droplet Radiator T | Droplet | Tiny | 1x2 | 0.71 | 1 GW |
| radiator_droplet_small | Droplet Radiator S | Droplet | Small | 1x4 | 3.57 | 5 GW |
| radiator_droplet_medium | Droplet Radiator M | Droplet | Medium | 1x8 | 14.3 | 20 GW |
| radiator_droplet_large | Droplet Radiator L | Droplet | Large | 1x16 | 57 | 80 GW |
| radiator_phononic_tiny | Phononic Emitter T | Phononic | Tiny | 1x2 | 0.36 | 35 GW |
| radiator_phononic_small | Phononic Emitter S | Phononic | Small | 1x4 | 1.81 | 175 GW |
| radiator_phononic_medium | Phononic Emitter M | Phononic | Medium | 1x8 | 7.24 | 700 GW |
| radiator_phononic_large | Phononic Emitter L | Phononic | Large | 1x16 | 28.9 | 2.8 TW |

`max_heat_tolerance` per tier matches operating temp: Heat Pipe 1500K, Droplet 2800K,
Phononic 6500K. Deployed radiators expose the full wing area to per-part aero heating
(via existing `update_part_temperatures`) and tear off if deployed at speed in atmosphere.

Tech-tree gating:
- `interstellar_fission` unlocks all four Heat Pipe sizes.
- `fusion_full` unlocks all four Droplet sizes.
- `am_torch` unlocks all four Phononic sizes.

## Attachment Diameters

Interstellar parts attach at their top edge. Engines (trapezoid shape) have a narrower top attachment and wider bottom exhaust. Reactors and shields (rectangle shape) have equal top and bottom width. All values in grid squares (1 grid square = 0.5m).

### Engines (Trapezoid)

| Part | Bottom (grid) | Top (grid) | Bottom (m) | Top (m) |
|------|---------------|------------|------------|---------|
| Orion Pulse Drive | 71 | 31 | 35.5 | 15.5 |
| Daedalus Stage 1 | 61 | 21 | 30.5 | 10.5 |
| Daedalus Stage 2 | 45 | 17 | 22.5 | 8.5 |
| Z-Pinch Probe | 11 | 7 | 5.5 | 3.5 |
| Z-Pinch Advanced | 17 | 11 | 8.5 | 5.5 |
| AM-Cat Fusion | 23 | 13 | 11.5 | 6.5 |
| Antimatter Torch | 9 | 7 | 4.5 | 3.5 |
| Gamma Converter | 35 | 15 | 17.5 | 7.5 |

Note: Engine hitbox widths are set to the next odd integer above `grid_width` (for grid-center alignment). Hitbox heights equal `grid_height` (no override needed since all heights are whole numbers).

### Reactors (Rectangle) — category: Electricity

| Part | Width (grid) | Width (m) |
|------|-------------|-----------|
| Prometheus | 5 | 2.5 |
| Vulcan | 9 | 4.5 |
| Stellarator | 7 | 3.5 |
| Tokamak | 11 | 5.5 |
| Penning Reactor | 7 | 3.5 |
| Ixion Reactor | 11 | 5.5 |

### Shields (Rectangle)

| Part | Width (grid) | Width (m) |
|------|-------------|-----------|
| Whipple Shield S | 9 | 4.5 |
| Whipple Shield M | 13 | 6.5 |
| Whipple Shield L | 19 | 9.5 |
| FRES Shield S | 9 | 4.5 |
| FRES Shield M | 13 | 6.5 |
| FRES Shield L | 17 | 8.5 |
| Geodesic Shield S | 9 | 4.5 |
| Geodesic Shield M | 13 | 6.5 |
| Geodesic Shield L | 17 | 8.5 |

## Files

- `src/parts/definition.rs` — ReactorData, ShieldData, ShieldType structs; Propellant/FuelType interstellar variants
- `src/parts/vessel.rs` — Reactor power generation in update_power()
- `src/editor/state.rs` — Reactor power in editor stats calculation
- `data/parts/engines_interstellar.ron` — 8 interstellar engines
- `data/parts/reactors_interstellar.ron` — 6 interstellar reactors
- `data/parts/shields.ron` — 9 shields (3 types × 3 sizes)
- `data/parts/radiators.ron` — 12 radiators (3 tiers × 4 sizes), procedural rendering, deployable wings
- `data/parts/tanks_fusion.ron` — 3 fusion spheres (`category: FuelTanks`)
- `data/parts/tanks_antimatter.ron` — 4 Penning-array antimatter tanks (Tiny/Small/Medium/Large, unlocked by `am_catalyzed`) plus 3 endgame antimatter spheres (S/M/L, unlocked by `bulk_am_storage`). All `category: FuelTanks`, locked to Antimatter.
- `data/parts/tanks_pulse.ron` — 4 nuclear pulse magazines (Tiny/Small/Medium/Large), `category: FuelTanks`, locked to NuclearPulse, unlocked by the `nuclear_pulse` tech node
- `data/parts/tanks_xenon.ron` — 4 xenon tanks (Tiny/Small/Medium/Large), `category: FuelTanks`, locked to Xenon (Xe-1 / Xe-3 / Xe-5+Xe-9 unlocked progressively across xenon-propulsion tech nodes)
- `data/tech/tree.ron` — tech-tree node `unlocks_parts` reference part names (e.g. `"AM-1"`, `"PU-3"`), not IDs
