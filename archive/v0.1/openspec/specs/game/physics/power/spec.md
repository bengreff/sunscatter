# Power System Spec

## Overview

Electricity system providing power generation, storage, and consumption. Electricity is stored in Watt-hours (Wh) and generated/consumed in Watts (W).

## Data Model

### Dedicated Electricity Fields

Electricity uses dedicated fields on `FlightPart` (`electricity` / `max_electricity`) rather than the `resources` HashMap. This prevents Wh values from being summed as kg in mass calculations (~10 sites sum `resources.values()` as kilograms).

### Part Definition Structs

- `BatteryData { capacity_wh: f64 }` — storage capacity in Wh
- `SolarPanelData { output_1au: f64 }` — power output in Watts at 1 AU from the Sun
- `RtgData { output_watts: f64 }` — constant power output in Watts
- `ReactorData { output_watts: f64 }` — constant power output in Watts (fission/fusion/antimatter reactors)
- `EngineData.alternator_power: f64` — Watts generated when engine is running (default 0)
- `PodData.power_draw: f64` — Watts consumed by command pod (default 0)

### Part Definitions

**Batteries** (Category: Electricity, Shape: Rectangle, 1 grid tall):
| ID | Size | Grid | Mass (t) | Capacity (Wh) |
|----|------|------|----------|---------------|
| battery_z1 | Tiny | 1x1 | 0.025 | 5,000 |
| battery_z3 | Small | 3x1 | 0.075 | 15,000 |
| battery_z5 | Medium | 5x1 | 0.125 | 25,000 |
| battery_z9 | Large | 9x1 | 0.225 | 45,000 |
| battery_z13 | XL | 13x1 | 0.325 | 65,000 |

**Solar Panels** (Category: Electricity, 150 W per grid square at 1 AU):
| ID | Size | Grid | Mass (t) | Output @1AU (W) |
|----|------|------|----------|----------------|
| solar_sp3 | Tiny | 1x3 | 0.015 | 450 |
| solar_sp6 | Tiny | 1x6 | 0.03 | 900 |
| solar_sp12 | Small | 2x6 | 0.06 | 1,800 |
| solar_sp24 | Small | 2x12 | 0.12 | 3,600 |

**RTG** (Category: Electricity, Size: Tiny):
| ID | Grid | Mass (t) | Output (W) |
|----|------|----------|-----------|
| rtg_pbnuk | 1x2 | 0.08 | 300 |

**Small Fission Reactors** (Category: Electricity, constant output):
| ID | Size | Grid | Mass (t) | Output |
|----|------|------|----------|--------|
| reactor_ember | Tiny | 1x3 | 0.4 | 10 kW |
| reactor_hearth | Small | 3x5 | 1.5 | 100 kW |
| reactor_crucible | Medium | 5x5 | 4.0 | 500 kW |

**Interstellar Reactors** (Category: Electricity, constant output):
| ID | Size | Grid | Mass (t) | Output |
|----|------|------|----------|--------|
| reactor_fission_small | Medium | 5x7 | 250 | 500 MW |
| reactor_fission_large | Large | 9x9 | 800 | 1.6 GW |
| reactor_fusion_small | Large | 7x7 | 400 | 10 GW |
| reactor_fusion_large | XL | 11x9 | 1200 | 30 GW |
| reactor_am_small | Large | 7x7 | 800 | 800 GW |
| reactor_am_large | XL | 11x9 | 2500 | 2.5 TW |

**Engine Alternators**: Chemical engines have `alternator_power: 0.0` (no power generation). Interstellar engines (fusion, antimatter) generate power when active via their `alternator_power` values.

**Pod Power Draw** (added to existing pods):
- Small Pod: 200 W
- Medium Pod: 500 W

## Solar Panel Deployment

### Data Model

`PlacedPart` (editor) has `deployed: bool` (default `true`). `BlueprintPart` serializes `deployed` with `serde(default = "default_true")` for backward compatibility.

`FlightPart` has three deployment fields:
- `deploy_fraction: f64` — 0.0 (retracted) to 1.0 (fully deployed), initialized to 0.0 on launch
- `deploy_target: bool` — desired state (false = retract, true = deploy), initialized to false
- `mirror_partner: Option<usize>` — index of mirror partner in parts vec, mapped from blueprint

### Deploy Animation

`FlightVessel::update_solar_deploy(dt)` runs each physics frame. For each non-destroyed, non-decoupled part, moves `deploy_fraction` toward `deploy_target` at 0.5 per second (2 seconds for full deploy/retract).

### Power Gating

Solar panel output is multiplied by `deploy_fraction`. At 0.0 (retracted), output is 0W. At 1.0 (fully deployed), full output. Partial deployment gives proportional output.

### Mirror Sync

Toggling deploy/retract on a solar panel also sets the same state on its `mirror_partner` (if any), in both editor and flight.

### Rendering

Retracted/deploying panels render as a grey base rectangle (0.2 grid squares tall) plus a partial sprite showing only the deployed portion. The sprite UV is clipped from the bottom up proportionally to `deploy_fraction`. Fully deployed panels use normal sprite rendering.

### Click Hitbox

When `deploy_fraction < 1.0`, the click area shrinks to just the base square (1 grid square centered at the panel bottom), so clicking above the retracted panel doesn't select it.

### Editor UI

Solar panel info section shows an "Extend"/"Retract" button that toggles `PlacedPart.deployed`. Toggling syncs to mirror partner.

### Flight UI

Solar panel info popup shows an "Extend"/"Retract" button based on `deploy_fraction >= 0.5`. Request is processed via `RenderState.solar_deploy_request` field, setting `deploy_target` on the part and its mirror partner.

## Flight Behavior

### Power Update (`FlightVessel::update_power`)

Called each physics frame after fuel consumption and mass recalculation.

**Generation sources** (summed across non-decoupled, non-destroyed parts):
1. Solar panels: `output_1au * (AU / sun_distance)^2 * deploy_fraction` — inverse-square law, gated by deployment
2. RTGs: constant `output_watts`
3. Reactors: constant `output_watts` (fission/fusion/antimatter — same pattern as RTGs but much higher output)
4. Engine alternators: `alternator_power` when `engine_active == true`

**Consumption sources**:
1. Pod power draw: `power_draw` from all active pods

**Net energy calculation**:
- `net_wh = (generation - consumption) * dt / 3600`
- Distributed proportionally across battery parts by capacity fraction
- Each battery clamped to `[0, max_electricity]`

### Sun Distance Computation

Ship absolute position computed via `ship.absolute_position()`. Sun is body index 1. Distance is Euclidean from ship to Sun position.

## Part Info Panels

### Flight Part Info
- **Battery**: Shows capacity and a progress bar with "current / max Wh"
- **Solar Panel**: Shows current output in Watts (distance-adjusted)
- **RTG**: Shows constant output in Watts

### Editor Part Info
- **Battery**: Shows capacity in Wh
- **Solar Panel**: Shows output at Earth distance (1 AU) in Watts
- **RTG**: Shows constant output in Watts

## Editor Display

`ShipStats` includes:
- `electricity_capacity`: total battery Wh
- `power_generation`: total solar + RTG watts
- `power_consumption`: total pod watts

Displayed in stats bar: "EC: X Wh" and "Power: +XW / -YW"

## Flight HUD

ELEC bar displayed between FUEL and HEAT bars when vessel has batteries:
- Gold/yellow fill at >30% charge
- Orange fill at 10-30%
- Red fill at <10%
- Shows stored Wh (e.g. "5.0k Wh") and "+Xw / -Yw" generation/consumption text

## Sprite Generation

Three generator functions in `generate_parts.py`:
- `generate_battery()`: Black rectangle with cell division lines, terminal indicators
- `generate_solar_panel()`: Silver frame with blue photovoltaic cell grid, hinge detail
- `generate_rtg()`: Dark grey body with red/orange heat dissipation fin stripes

## Files

- `data/parts/electrical.ron` — 10 electrical part definitions (5 batteries, 4 solar panels, 1 RTG)
- `data/parts/reactors_small.ron` — 3 small fission reactors (Ember, Hearth, Crucible)
- `data/parts/reactors_interstellar.ron` — 6 interstellar reactors (fission/fusion/antimatter)
- `data/parts/engines.ron` — alternator_power added to all 16 engines
- `data/parts/pods.ron` — power_draw added to both pods
- `src/parts/definition.rs` — BatteryData, SolarPanelData, RtgData, ReactorData structs
- `src/parts/vessel.rs` — FlightPart electricity/deploy fields, update_power(), update_solar_deploy(), helpers
- `src/render/types.rs` — ShipRenderData power fields
- `src/render/state.rs` — ELEC bar in flight HUD
- `src/editor/state.rs` — ShipStats power fields, calculate_stats() updates
- `src/editor/ui.rs` — Power stats in editor stats bar
- `src/main.rs` — update_power() call in game loop, ShipRenderData wiring
- `tools/sprite_gen/generate_parts.py` — Battery, solar panel, RTG sprite generators
