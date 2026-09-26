# Transfer Planner

## Overview
Automated transfer planning: calculates delta-v and timing for orbital transfers to other bodies, then creates a maneuver node with one click.

## Modes

### Hohmann Transfer (Mode 0)
- Available for transfers to bodies within the same SOI (children of ship_soi)
- Example: ship orbits Earth → targets are Moon (also orbits Earth)
- Computes circular Hohmann transfer: departure dv (signed: positive=prograde, negative=retrograde), arrival dv, transfer time
- Calculates phase angles (current vs required) and time to optimal departure window
- Places departure burn node at the correct true anomaly with prograde-only delta-v (sign preserved for direction)

### Lambert Transfer (Mode 1)
- Available for transfers to siblings of the body you orbit (same parent SOI)
- Example: ship orbits Earth → targets are Mars, Venus, etc. (also orbit Sun)
- Uses universal variable Lambert solver with Stumpff functions
- Adjustable departure and transfer time offsets via logarithmic sliders
- Defaults to Hohmann-optimal timing
- Computes ejection burn from parking orbit as a **prograde-only** burn
- Calculates the correct **ejection angle** on the parking orbit where a tangential burn produces a hyperbolic escape trajectory with the desired v_infinity direction
- Ejection angle physics: computes actual turn angle at SOI exit (not asymptotic acos(-1/e)), accounting for gravity curving the trajectory at finite distance → burn position = v_inf_angle ∓ turn_angle
- **Escape time compensation**: Lambert solver is solved for SOI exit time (departure_time + escape_time), not the LEO burn time. The escape time is the time to traverse the hyperbolic escape trajectory from parking orbit periapsis to the SOI boundary. This accounts for Earth moving in its orbit during the escape (~3° for Mars transfers), ensuring the v_inf direction is correct at SOI exit rather than at the burn point.
- Node placed at the computed ejection position angle (inertial), not at current ship position
- Shows v_infinity at departure and arrival

## UI
- **XFER button** at the bottom of the left throttle panel (with ellipse orbit icon)
- **Floating window** (egui Area) with:
  - Mode selector (Hohmann / Lambert tabs)
  - Target dropdown (filtered by mode)
  - Lambert sliders for departure/transfer time offset
  - Results display: delta-v, transfer time, phase angles, time to window
  - Phase angle color coding: green (< 5 deg), yellow (< 20 deg), white (> 20 deg)
  - "Create Node" button

## Implementation Files
- `src/ship/transfer.rs` - All transfer math (Hohmann, Lambert, interplanetary)
- `src/ship/mod.rs` - Module export
- `src/render/state.rs` - UI window and planner state fields
- `src/render/maneuver.rs` - `create_maneuver_node_with_dv()` method
- `src/main.rs` - Transfer computation and node creation wiring

## Key Functions
- `compute_hohmann()` - Hohmann transfer with phase angle timing
- `solve_lambert_2d()` - Universal variable Lambert solver
- `compute_interplanetary()` - Full interplanetary transfer with ejection burn
- `hohmann_optimal_times()` - Default departure/arrival times for Lambert defaults
- `hohmann_targets()` / `lambert_targets()` - Valid target listing

## Bidirectional Target Sync
- **Nav → Planner**: When the navigation target (`selected_target`) is a body that appears in the planner's target list, the planner dropdown auto-selects it and switches mode (Hohmann or Lambert) accordingly
- **Planner → Nav**: When the user selects a target from the planner dropdown, the navigation target is synced to match (`selected_target = Body(idx)`, `selected_target_name = name`)
- This ensures clicking through Lambert targets in the dropdown also updates the closest approach marker, target arrow, and other nav-target-dependent features

## Data Flow
1. `main.rs` updates target lists and computes transfers each frame (when planner open)
2. Navigation target and planner target are kept in bidirectional sync
3. Results stored as `TransferDisplay` on `RenderState` (pre-formatted for UI)
4. UI reads `TransferDisplay` and shows results
5. "Create Node" pushes `RenderRequest::TransferNode { position_angle, prograde_dv, radial_dv, time_to_window }` onto `RenderState::render_requests`
6. `main.rs` drains the queue and handles the variant: converts the inertial position angle to true anomaly using the trajectory segment's arg_peri, then calls `create_maneuver_node_with_epoch()`

## Phase Angle and Time-to-Window

### Requirement: Phase angle computation
- **Current phase** = `normalize_angle(target_angle - departure_body_angle)` at current sim_time. Positive means target is ahead.
- **Required phase** = `PI - omega_target * hohmann_transfer_time` (Hohmann approximation). For Earth→Mars this is ~+44° (Mars ahead of Earth).

### Requirement: Time-to-window uses signed synodic rate
- Phase angle `φ = target_angle - ship_angle` changes at rate `ω_target - ω_ship` (signed).
- For outward transfers (inner→outer), the inner body orbits faster, so `ω_target - ω_ship < 0` and the phase decreases over time.
- `time_to_window = normalize_angle(required_phase - current_phase) / synodic_rate_signed`, then normalized to `[0, synodic_period)` via `rem_euclid`.
- For Lambert mode, `time_to_window = departure_time - sim_time` directly (the departure time is already determined by the solver/user).

### Requirement: Current phase in Lambert mode shows current state
- The `current_phase_angle` in interplanetary results is computed at `sim_time` (not `departure_time`), showing the user the current planetary alignment.

## Technical Notes
- **Inertial position angles**: Transfer functions return inertial position angles (not true anomalies) to avoid dependence on `argument_of_periapsis`, which is ill-defined for near-circular parking orbits. Conversion to true anomaly happens at node creation using the trajectory segment's stable arg_peri.
- **Escape time**: Computed from hyperbolic trajectory parameters (true anomaly at SOI radius → hyperbolic anomaly → mean anomaly → time via mean motion). Uses single-iteration correction (initial Lambert → escape time → corrected Lambert).
