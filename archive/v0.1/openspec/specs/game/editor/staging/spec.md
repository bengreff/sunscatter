# Staging

Stage management, ordering, drag-and-drop, and delta-v calculation in the vehicle editor.

## Data Structure

### Requirement: Stage representation

Stages SHALL be represented as `Vec<Vec<PlacedPartId>>` where the outer Vec is the ordered list of stages and the inner Vec contains the part IDs in each stage. Stage index 0 fires first in flight.

### Requirement: Auto-staging on placement

When an engine, decoupler, or fairing base is placed, it SHALL be automatically added to stage 0. If no stages exist, a new stage SHALL be created first. No other part categories are auto-staged.

### Requirement: Auto-staging on blueprint load

When loading a blueprint with empty stages, all engine parts SHALL be collected into a single stage 0. Decouplers are NOT auto-staged during this fallback.

## Staging Panel UI

### Requirement: Reverse display order

Stages SHALL be displayed in reverse index order: highest stage index at top, lowest at bottom. Stage labels SHALL be 1-indexed in the UI (e.g., "Stage 1" for index 0).

### Requirement: Part names in stages

Each part in a stage SHALL display its `PartDefinition.name`. If the definition is not found, it SHALL display "Part {id}".

### Requirement: Mirrored pair display

When a part and its `mirror_partner` are both in the same stage, they SHALL be displayed as a single entry with " x2" appended to the part name. Drag-reordering a mirrored entry in staging SHALL move both parts together.

#### Scenario: Mirrored pair shows as single entry

- **WHEN** engine A and its mirror partner engine B are both in stage 0
- **THEN** the staging panel SHALL show one entry: "Engine Name x2"

#### Scenario: Only one partner in stage

- **WHEN** engine A is in stage 0 but its mirror partner engine B is in stage 1
- **THEN** each stage SHALL show its respective engine as a normal entry (no x2 badge)

### Requirement: Empty stage display

Empty stages SHALL display "(empty)" in weak/gray style.

### Requirement: Per-stage delta-v display

Each stage with delta-v > 0 SHALL display its delta-v value to the right of the stage label, formatted in green (`rgb(120, 200, 120)`) at font size 10.

### Requirement: Per-stage burn time display

Each stage with burn time > 0 SHALL display burn time at 100% thrust to the right of the delta-v value, formatted as "Burn: {duration}" in green (`rgb(120, 200, 120)`) at font size 10. Duration SHALL use the standard `format_duration()` format (e.g., "1m 30s", "2h 5m 0s").

#### Calculation

Burn time SHALL be computed as: `burn_time = burnable_fuel_tonnes * Isp * g0 / total_thrust_kN`. This uses the same intermediate values from the delta-v calculation (burnable fuel mass, thrust-weighted Isp, total thrust). The units cancel correctly: tonnes * s * (m/s²) / kN = seconds.

### Requirement: Total delta-v display

The total delta-v across all stages SHALL be displayed at the top of the staging panel as "Total dv: {value}" in bold at font size 12. It SHALL also appear in the stats bar.

## Drag and Drop

### Requirement: Part drag between stages

Dragging a part from one stage and dropping it onto another stage SHALL remove the part from all stages and append it to the target stage.

### Requirement: Stage reordering

Dragging a stage and dropping it onto another stage's position SHALL move the stage to that position. The insertion index SHALL be adjusted if the source was before the target.

### Requirement: Insert stage gaps

Between each pair of stages (and at top/bottom), a "+" button SHALL be rendered inside a drop zone. Clicking "+" SHALL insert a new empty stage at that position. Dropping a stage onto a gap SHALL move it to that position.

### Requirement: Empty stage deletion

Empty stages (containing no parts) SHALL be deletable via a "✕" button displayed in the stage header row. The button SHALL only appear when `stage.is_empty()`. Clicking it removes the stage from the stage list. Non-empty stages SHALL NOT show a delete button.

### Requirement: Single action per frame

Only one staging action SHALL be processed per frame, in priority order: stage move via gap, stage insert, part/stage drop onto stage zone.

## Delta-V Calculation

### Requirement: Tsiolkovsky rocket equation

Per-stage delta-v SHALL be calculated using the Tsiolkovsky equation: `dv = Isp * g0 * ln(wet_mass / dry_mass)`, where `g0 = 9.80665 m/s^2`.

### Requirement: Sequential stage simulation

Delta-v calculation SHALL simulate stages sequentially in order. For each stage:

1. **Fire decouplers**: For each decoupler in the stage:
   - **Stack decouplers** (`is_radial = false`): Mark decoupler and all parts with top edge at or below the decoupler's bottom edge (0.01m tolerance) as decoupled.
   - **Radial decouplers** (`is_radial = true`): Mark only the decoupler itself as decoupled.
2. **BFS connectivity check**: After all decouplers fire, build weld adjacency among remaining non-decoupled parts and BFS from root. Mark any parts not reachable from root as decoupled. This handles radial decouplers disconnecting side-mounted subassemblies and catches any parts orphaned by stack decouplers.
3. **Fire fairings**: Mark fairing bases in this stage as decoupled. Only the fairing base itself is decoupled -- parts inside the fairing envelope remain connected to the vessel.
4. **Enable engines**: Mark non-decoupled engines in this stage as active.
5. **Compute fuel zones**: Perform BFS through welding hitbox overlap among non-decoupled parts, treating decouplers with `crossfeed_enabled = false` as barriers that block traversal.
6. **Compute drain priorities** (flight only): For each crossfeed-enabled decoupler in stage S, BFS from root excluding that decoupler; unreachable parts get `priority = min(current, S)`. Always-reachable parts keep `priority = usize::MAX`.
7. **Per-zone parallel burn**: For each engine zone with active engines:
   - `zone_thrust` = sum of engine vacuum thrusts in zone
   - `zone_thrust_over_isp` = sum of `thrust_vac / isp_vac` for engines in zone
   - `zone_fuel` = min-priority burnable fuel in zone (flight) or all fuel in zone (editor)
   - `zone_flow` = `zone_thrust_over_isp / g0` (tonnes/s)
   - `zone_burn_time` = `zone_fuel / zone_flow`
8. **Phase time**: `min(zone_burn_time)` across all zones with fuel > 0 and flow > 0.
9. **Fuel consumed**: `zone_consumed[z] = zone_flow[z] * phase_time`, clamped to `zone_fuel[z]`.
10. **Effective Isp**: `sum(zone_thrust) / sum(zone_thrust_over_isp)` (harmonic weighted mean).
11. **Apply Tsiolkovsky**: `dv = effective_isp * g0 * ln(wet_mass / (wet_mass - total_consumed))`. Burn time = phase_time.
12. **Update fuel**: Distribute `zone_consumed[z]` proportionally among the zone's min-priority tanks: `tank_consumed = zone_consumed * (tank_fuel / zone_fuel)`.

#### Scenario: Zero delta-v cases
- **WHEN** Isp <= 0, dry mass <= 0, or wet mass <= dry mass
- **THEN** the stage delta-v SHALL be 0.0

#### Scenario: Multi-stage fuel isolation
- **WHEN** a vessel has two stages separated by a non-crossfeed decoupler, each with its own engines and tanks
- **THEN** firing the first stage SHALL only consume fuel from tanks in the same fuel zone as the active engines, preserving upper stage fuel for later

#### Scenario: Parallel burn (side boosters)
- **WHEN** a stage ignites engines in multiple separate fuel zones (e.g., center sustainer + side boosters)
- **THEN** delta-v SHALL reflect the burn until the first zone empties (phase time = min burn time across zones)
- **AND** the next stage (firing decouplers to jettison empty boosters) SHALL show remaining delta-v from the center zone with correctly reduced fuel and jettisoned booster mass

#### Scenario: Single zone equivalence
- **WHEN** all active engines are in a single fuel zone
- **THEN** the parallel burn algorithm SHALL produce identical results to the previous single-burn algorithm

### Requirement: Fuel tracking initialization

Fuel remaining per tank SHALL be initialized from `propellant_capacity(fuel_type)` (oxidizer + fuel mass), converted from kg to tonnes. Only tanks with `tank_filled == true` and `fuel_type != Empty` SHALL contribute fuel.
