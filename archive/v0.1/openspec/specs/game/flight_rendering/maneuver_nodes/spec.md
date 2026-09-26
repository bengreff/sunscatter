# Maneuver Nodes

Maneuver node creation, editing, dragging, burning, and predicted trajectory display.

## Creation

### Requirement: Maneuver node creation via orbit click

Clicking near any visible orbit line in map view SHALL detect the closest orbit point and show a "Create Maneuver Node" popup button. Click detection searches all current trajectory segments and all predicted trajectories (from maneuver node burns).

#### Scenario: Orbit click detection
- Sample 256 points along each trajectory segment (current and predicted)
- If any point is within 15 pixels of click position, store as `pending_orbit_click`
- Closest match across all segments wins

#### Scenario: Node creation details
- ManeuverNode created with: unique auto-incrementing id, orbit parameters copied from segment
- `delta_v` and `remaining_delta_v` = ManeuverDeltaV::default() (prograde: 0.0, radial_out: 0.0)
- Automatically selected (`selected_maneuver_node = Some(id)`)

## Rendering

### Requirement: Maneuver node rendered as diamond marker

Each maneuver node SHALL be drawn as a filled diamond on the egui foreground layer.

#### Scenario: Diamond geometry
- 4 vertices: `(x, y-size)`, `(x+size, y)`, `(x, y+size)`, `(x-size, y)`
- Selected: size = 10.0, fill = `rgb(255, 200, 100)` (bright gold)
- Unselected: size = 8.0, fill = `rgb(200, 150, 50)` (dimmer gold)
- Stroke = 1.5px white

#### Scenario: Delta-v label on hover or select
- When selected or mouse within 15 pixels: show "{remaining_dv:.0} m/s" at `(x, y-18)`, font size 10, white

### Requirement: Maneuver node click detection

Clicking within 20 pixels of a maneuver node marker SHALL select it.

## Editing Panel

### Requirement: Maneuver node editing panel

When a maneuver node is selected, an egui right panel named "maneuver_panel" (width 200) SHALL show editing controls.

#### Scenario: Panel contents
- "Maneuver Node" heading, "Remaining Dv: {total:.1} m/s"
- Prograde/Retrograde slider: range -100 to +100, snap-back to 0
- Radial Out/In slider: range -100 to +100, snap-back to 0
- Delete and Close buttons

#### Scenario: Non-linear slider scaling
- normalized = `delta / 100.0`
- curved = `sign(normalized) * |normalized|^2.0` (quadratic)
- change per frame = `curved * 16.67` m/s
- Full deflection ~1000 m/s/s, small deflection ~1 m/s/s

### Requirement: Maneuver node deletion

Clicking "Delete" SHALL remove the node and set `selected_maneuver_node = None`.

### Requirement: Closing maneuver panel

Clicking "Close" SHALL set `selected_maneuver_node = None` without deleting.

## Dragging

### Requirement: Maneuver node dragging along orbit

Nodes can be dragged along their stored orbit to change position. Sample 360 points along the orbit and set `true_anomaly` to the closest point's value. Epoch is updated using a delta approach: compute the signed time delta between old and new true anomaly on the node's own orbit (via mean anomaly difference), then `new_epoch = old_epoch + delta_time`. This works correctly for nodes on any orbit, not just the current trajectory.

## Position Calculation

### Requirement: Maneuver node world position

Node world position SHALL be computed from stored orbital parameters and current parent body position.

#### Scenario: Elliptical position
- `p = a * (1 - e^2)`, `r = p / (1 + e * cos(ta))`
- `world_x = parent_x + r * cos(ta + arg_peri)`

#### Scenario: Hyperbolic position
- `p = |a| * (e^2 - 1)`, `denom = 1 + e * cos(ta)`
- If `denom <= 0.001`, fallback `r = |a|`

## Delta-V Calculation

### Requirement: Maneuver node total delta-v

Total delta-v magnitude SHALL be `sqrt(prograde^2 + radial_out^2)`, computed separately for `delta_v` (trajectory prediction) and `remaining_delta_v` (burn display).

### Requirement: Prograde and radial unit vectors at node

Velocity at node position SHALL be computed via vis-viva equation. Prograde unit = normalized velocity vector. Radial out unit = `[prograde_y, -prograde_x]` (perpendicular, pointing away from parent).

## Burn Application

### Requirement: Maneuver burn application

During a burn with autopilot in ManeuverNode mode, `apply_burn_to_maneuver` SHALL project the ship's thrust onto the maneuver's coordinate system and reduce `remaining_delta_v`. The acceleration used for burn tracking SHALL be atmosphere-adjusted (interpolated between vacuum and sea-level thrust based on atmospheric pressure fraction), matching the actual physics thrust.

#### Scenario: Burn projection
- prograde_contribution = `dot(burn_dir, prograde_unit) * delta_v_magnitude`
- radial_contribution = `dot(burn_dir, radial_unit) * delta_v_magnitude`

#### Scenario: Remaining delta-v tracking
- Burn contribution projected onto prograde/radial axes is subtracted from remaining delta-v
- Correct-direction burns reduce remaining delta-v toward zero
- Wrong-direction burns increase remaining delta-v (moving further from target)
- Components can cross zero and flip sign if the ship overshoots

## Time-to-Node

### Requirement: Maneuver node epoch

Each ManeuverNode SHALL store an `epoch: f64` — the absolute simulation time at which the ship will reach the node. This is computed at node creation and when dragging.

#### Scenario: Epoch computation
- Each `OrbitSegmentData` carries a `base_epoch` field: the absolute sim time at the trajectory's origin
  - Current trajectory: `base_epoch = simulation_time`
  - Predicted trajectories (post-maneuver): `base_epoch = source_maneuver_node.epoch`
- Uses the trajectory segment's own `start_true_anomaly` and `start_time` — both the segment start TA and the node TA are in the same orbit frame, avoiding arg_peri mismatch issues
- Convert segment `start_true_anomaly` and node TA to mean anomalies on the segment's orbit
- Compute delta MA from segment start to node position, accounting for orbit direction
- `epoch = base_epoch + segment.start_time + delta_ma / mean_motion`
- Epoch is absolute simulation time; supports nodes on any trajectory segment

### Requirement: Time-to-node countdown

The maneuver panel SHALL display a countdown to the first maneuver node, computed live each frame from the ship's current orbit to prevent drift.

#### Scenario: Live time-to-node computation
- Compute node's inertial angle: `node.true_anomaly + node.argument_of_periapsis` (fixed in space)
- Project onto ship's current orbit: `node_ta = (inertial_angle - ship_orbit.arg_peri) mod TAU`
- Convert both ship MA and node TA→MA using the **same** orbit parameters (errors cancel even for circular orbits with jittering arg_peri)
- `time_one_pass = delta_ma / mean_motion` — time to reach node on current orbit (0 to 1 period)
- Use stored epoch to determine full orbit count: `full_orbits = round((epoch_remaining - time_one_pass) / period)` when epoch is more than 1.5 passes away
- `time_to_node = time_one_pass + full_orbits * period`
- This is drift-free because it's recomputed from live state each frame, not accumulated

#### Scenario: Burn time estimation
- `burn_time = remaining_dv / (thrust_kN * 1000 / (mass_tonnes * 1000))`
- Only displayed when burn_time > 0.5 seconds and vessel has thrust

#### Scenario: Panel display
- Below "Remaining Δv": show `T- {formatted_duration}` and `Burn: {formatted_duration}`
- Duration formatted as `Xd Xh Xm Xs` (omitting leading zero components)

## Warp-to-Node

### Requirement: Warp-to-node auto-warp

A "Warp to Node" button in the maneuver panel SHALL engage automatic time warp that scales down as the node approaches.

#### Scenario: Button visibility
- "Warp to Node" shown when time_to_node > 10 seconds and auto-warp is not active
- "Cancel Warp" shown when auto-warp is active

#### Scenario: Auto-warp level selection
- Minimum auto-warp: 100x (WARP_LEVELS index 5)
- Find highest warp level (≥100x) where `effective_time / warp_level >= 5.0` real seconds
- `effective_time = time_to_node - burn_time / 2`
- Recalculated each frame, progressively stepping down warp

#### Scenario: Auto-warp termination
- Stop when `effective_time <= 0`: set warp to 1x, deactivate auto-warp
- Stop when `effective_time / 100x < 2.0` real seconds: even minimum on-rails warp would overshoot, drop to 1x
- Cancel if node is deleted (first node specifically)
- Cancel if time_to_node becomes unavailable (e.g., SOI change, node epoch in the past)
- Cancel if user manually clicks a warp button
