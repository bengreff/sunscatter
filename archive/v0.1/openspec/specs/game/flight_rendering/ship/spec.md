# Ship Rendering

Ship triangle indicator, part-based rendering in ship view, exhaust plumes, and orbit lines.

## Ship Indicator

### Requirement: Ship triangle indicator in map view

When the ship is smaller than 5 pixels on screen, a fixed-size triangle indicator SHALL be drawn at 16 pixels screen radius, pointing in the direction of ship rotation.

#### Scenario: Indicator triangle geometry
- Nose vertex at angle R from center, distance = `indicator_size`
- Back_left vertex at angle `R + PI * 0.8`, distance = `indicator_size * 0.6`
- Back_right vertex at angle `R - PI * 0.8`, distance = `indicator_size * 0.6`
- `indicator_size = 16.0 / pixels_per_world_unit`

#### Scenario: Indicator outline effect
- Outer triangle at full indicator_size with ship color
- Inner triangle at `indicator_size * 0.6` with darkened color `[color[0]*0.3, color[1]*0.3, color[2]*0.3, color[3]]`

### Requirement: Ship rendered at actual world size when partially visible

When `ship_pixels >= 1.0` and `ship_pixels < 5.0`, the ship SHALL be rendered at actual world scale AND the fixed-size indicator SHALL also be drawn on top.

### Requirement: Ship default color and size

Default ship color SHALL be `[1.0, 0.2, 0.2, 1.0]` (bright red). Default ship size (no vessel loaded) SHALL be `SHIP_SIZE = 10.0` meters.

## Part-Based Rendering

### Requirement: Part-based rendering in ship view

When the ship has parts and part definitions are available and `ship_pixels >= 1.0`, individual parts SHALL be rendered at their positions relative to the vessel center of mass.

#### Scenario: Part vertex transformation pipeline
1. Scale local position by `render_scale` (= SCALE * BODY_SCALE = 1e-9)
2. Rotate by `visual_rotation = ship.rotation - PI/2` (editor Y-up maps to physics rotation where 0 = +X)
3. Translate to vessel world position relative to camera

#### Scenario: Engine gimbal rotation
- After scaling but before vessel rotation, rotate engine part vertices by gimbal_angle in part-local space

### Requirement: Ship fallback triangle when no vessel parts

When no vessel parts are available, the ship SHALL be rendered as a simple triangle at actual world size using the same nose/back geometry as the indicator.

### Requirement: Decoupler adapter fairings in second pass

Decoupler adapter fairings SHALL be rendered in a separate pass after all base parts for proper layering.

### Requirement: Fairing shell rendering in third pass

Fairing shells SHALL be rendered in a third pass after decoupler adapters. For each part with a `fairing_shape` and `fairing: Some(...)` definition, `generate_flight_fairing_shell()` SHALL draw the shell using the same trapezoid segment + seam line geometry as the editor shell renderer, passing `fairing_half` to support half-shell debris rendering. The shell is positioned relative to the part's local coordinates, transformed through the standard vertex pipeline (scale, rotate, translate). Shell color: `FAIRING_SHELL_COLOR = [0.35, 0.35, 0.38, 1.0]`. Seam line color: `FAIRING_SHELL_LINE_COLOR = [0.20, 0.20, 0.22, 1.0]`.

### Requirement: Fairing shape in render data

`ShipPartRenderData` SHALL include `fairing_shape: Option<FairingShape>` to pass the shell geometry from the vessel to the renderer each frame. This is populated from `FlightPart.fairing_shape` during render data construction. `ShipPartRenderData` SHALL also include `is_fairing: bool` (true when the definition has `fairing: Some(...)`) and `fairing_half: Option<FairingHalf>` (from `FlightPart.fairing_half`).

### Requirement: Fairing deploy button

The flight part info popup SHALL show a "Deploy" button for fairing parts (`is_fairing == true`). Clicking deploy SHALL push `RenderRequest::FairingDeploy { part_index }` onto `RenderState::render_requests`, which main.rs drains each frame; the handler marks the fairing as decoupled and calls `handle_post_decouple()`.

### Requirement: Two-half fairing debris

When a fairing is decoupled (via staging or deploy button), `extract_fairing_halves()` SHALL be called before `extract_decoupled_parts()`. It finds decoupled fairing parts with a shape but no `fairing_half` set, and for each creates two shell-only debris vessels — one with `fairing_half = Left` and one with `fairing_half = Right`. The base disc stays on the vessel: the original part is un-decoupled (`decoupled = false`) and its `fairing_shape` is cleared. Shell debris mass is 10% of total fairing mass per half.

Each fairing half debris vessel SHALL receive 5 m/s perpendicular separation velocity: left half gets -5 m/s in vessel-local X, right half gets +5 m/s. The velocity is rotated to world coordinates using the vessel's heading.

### Requirement: Shell-only debris rendering

When rendering a part with `fairing_half.is_some()`, the base disc SHALL be skipped (`generate_part_shape_vertices` is not called). Only the fairing shell half is rendered via the fairing shell pass.

## Exhaust Plumes

### Requirement: Engine exhaust plumes during thrust

When an engine is active and throttle > 0, an exhaust plume SHALL be rendered below the engine nozzle. Multi-nozzle engines (with `nozzle_offsets`) render separate plumes at each nozzle position, scaled by `1/nozzle_count`.

#### Scenario: Plume geometry
- `plume_length = nozzle_width * 2.0 * throttle`
- Nozzle exit at `y - half_height`

#### Scenario: Outer plume
- Red triangle `[1.0, 0.2, 0.0, 0.9]`, full nozzle width, full plume length

#### Scenario: Inner plume
- Yellow triangle `[1.0, 0.9, 0.1, 1.0]`, 60% width, 40% length

## RCS Plumes

### Requirement: RCS plume rendering in flight

When RCS nozzles are active (vessel is rotating or translating via manual input or autopilot), white plume rectangles SHALL be rendered extending outward from each firing nozzle. Plumes are generated in part-local space at origin (0,0) and transformed with the rest of the part vertices (scale, gimbal rotation if applicable, vessel rotation).

#### Scenario: RCS plume activation from rotation
- **WHEN** the player presses Q (rotate left/CCW, `rcs_direction = 1.0`) and a pod RCS is above the center of mass
- **THEN** the right lateral nozzle SHALL fire (exhaust right, pushing the top left = CCW torque)
- **WHEN** the player presses E (rotate right/CW, `rcs_direction = -1.0`) and a pod RCS is above the center of mass
- **THEN** the left lateral nozzle SHALL fire (exhaust left, pushing the top right = CW torque)

#### Scenario: RCS plume activation from translation
- **WHEN** the player presses W (translate forward, `rcs_translate[0] > 0`)
- **THEN** down nozzles on all RCS parts SHALL fire (push ship forward=up)
- **WHEN** S pressed (translate backward, `rcs_translate[0] < 0`): up nozzles fire
- **WHEN** D pressed (translate right, `rcs_translate[1] > 0`): right-mount lateral nozzles fire (exhaust left, push right)
- **WHEN** A pressed (translate left, `rcs_translate[1] < 0`): left-mount lateral nozzles fire (exhaust right, push left)

#### Scenario: Combined rotation and translation
- **WHEN** both rotation and translation inputs are active simultaneously
- **THEN** nozzle activation SHALL be the union of rotation-driven and translation-driven activations

#### Scenario: No plumes when idle
- **WHEN** `rcs_direction = 0.0` AND `rcs_translate = [0.0, 0.0]` (no RCS input)
- **THEN** no RCS plumes SHALL be rendered

#### Scenario: Background vessel RCS
- **WHEN** rendering an inactive/background vessel
- **THEN** `rcs_nozzle_state` SHALL be `None` (no plumes)

### Requirement: Pod RCS plume rendering

When a pod has built-in RCS (`def.rcs.is_some()` AND `def.category == Pods`), plumes SHALL be rendered from bilateral nozzle positions near the top of the pod (80% up, at the trapezoid edges). The `lateral` flag fires the left nozzle (exhaust left) and `lateral_mirrored` fires the right nozzle (exhaust right). Pod RCS plumes use `generate_pod_rcs_plume_vertices()` instead of the standalone RCS plume function.

## Orbit Lines

### Requirement: Ship orbit line displayed in map view

The ship's orbit line SHALL only be visible when `ship_pixels < 5.0` (map view). It supports multiple patched conic segments.

#### Scenario: Orbit line width and color
- Line width = `0.002 / camera.zoom`
- Outer edge = segment color, inner edge = `[color[0]*0.5, color[1]*0.5, color[2]*0.5, color[3]*0.7]`

#### Scenario: Segment color
- Base color = `[ship_color[0]*0.6, ship_color[1]*0.6, ship_color[2]*0.6, alpha]`
- First segment alpha = 0.7, subsequent = 0.4

#### Scenario: Elliptical orbit rendering
- Parametrize using eccentric anomaly, segment count = `max(16, floor((|angle_span| / TAU) * 512))`

#### Scenario: Smooth orbit trimming for first segment
- The first segment's `start_true_anomaly` SHALL be recomputed each frame from the ship's current `rel_position` and the cached segment's `argument_of_periapsis`, rather than using the stale value from the cached trajectory. This ensures the orbit line trims smoothly as the ship moves, instead of jumping when the trajectory cache refreshes (every ~30 frames). True anomaly normalization SHALL match elliptical (`rem_euclid(TAU)`) vs hyperbolic (`(-PI, PI)` range) conventions.

#### Scenario: Hyperbolic trajectory rendering
- 1024 sample points, skip points within `HYPERBOLIC_SKIP_MARGIN = 0.005` rad of asymptote

### Requirement: Predicted trajectory rendering (post-maneuver)

Predicted trajectories from maneuver nodes SHALL be drawn as green lines `[0.0, 1.0, 0.0, alpha]` where alpha = 0.9 (first segment), 0.6 (subsequent). Line width = `0.0015 / camera.zoom`.

### Requirement: Camera-relative orbit precision

All orbit line rendering (ship orbits, predicted trajectories, background vessel orbits, body orbits) SHALL subtract the camera position from the parent body position FIRST (`pcam = parent - camera`), then compute orbit geometry relative to `pcam`. This prevents f64 precision loss at galaxy-scale coordinates (~1e11). Vertex positions are already camera-relative and SHALL NOT subtract camera again. Ap/Pe markers and closest approach markers follow the same pattern.

## Apoapsis and Periapsis Markers

### Requirement: Ap/Pe markers on orbit lines

Filled circle markers SHALL be drawn at apoapsis and periapsis points.

#### Scenario: Main orbit markers
- Radius = `0.008 / camera.zoom`, 16 segments
- Periapsis: `[0.3, 0.8, 1.0, alpha]` (cyan), Apoapsis: `[1.0, 0.6, 0.2, alpha]` (orange)
- First segment alpha = 1.0, subsequent = 0.6

#### Scenario: Predicted trajectory markers
- Radius = `0.006 / camera.zoom`, 12 segments
- Periapsis: `[0.2, 0.7, 0.9, alpha]`, Apoapsis: `[0.9, 0.5, 0.1, alpha]`
- First segment alpha = 0.7, subsequent = 0.5

#### Scenario: Partial orbit marker visibility
- Only show periapsis if true anomaly 0 is within the traversed arc
- Only show apoapsis if true anomaly PI is within the traversed arc
- Full orbits always show both markers

### Requirement: Ap/Pe label overlays

egui text labels "Ap" and "Pe" SHALL be rendered at each marker's screen position.

#### Scenario: Label formatting
- "Ap" at `(x, y - 12)`, font size 11, color `rgb(255, 153, 51)` (orange)
- "Pe" at `(x, y - 12)`, font size 11, color `rgb(77, 204, 255)` (cyan)

#### Scenario: Altitude on hover
- Within 20 pixels of marker: show altitude at `(x, y + 14)`, font size 10
- Format: `>= 1e9` -> "X.XX Gm", `>= 1e6` -> "X.X Mm", `>= 1e3` -> "X.X km", else "X m"

## Prograde Direction Arrow

### Requirement: Prograde arrow at screen edge in ship view

When zoomed into the ship (`!needs_indicator`, i.e. ship_pixels >= 5.0) and velocity > 0.1 m/s, a small white filled triangle SHALL be drawn at the viewport edge pointing in the prograde (velocity) direction.

#### Scenario: Arrow geometry
- Color: `[1.0, 1.0, 1.0, 0.85]` (white, slightly transparent)
- Fixed screen size: 16px arrow length, 10px base width (divided by `pixels_per_world_unit` for world coords)
- Filled triangle: tip at edge point, two base corners behind tip perpendicular to velocity

#### Scenario: Arrow positioning
- Arrow tip is positioned at the screen edge in the velocity direction
- A ray is cast from the ship center along the velocity unit vector, clamped to the viewport boundary with 25px margin
- Direction is based on `velocity_direction` (normalized velocity unit vector), independent of ship rotation

#### Scenario: Arrow not shown
- **WHEN** `needs_indicator` is true (map view) **THEN** no prograde arrow is drawn
- **WHEN** velocity < 0.1 m/s **THEN** `velocity_direction` is `[0, 0]` and no arrow is drawn

## Flight Part Info — Electrical Parts

### Requirement: Battery info in flight

The flight part info popup for batteries SHALL display capacity and a progress bar showing "current / max Wh". The progress bar fill color SHALL be gold/yellow.

### Requirement: Solar panel info in flight

The flight part info popup for solar panels SHALL display current output in Watts, adjusted for distance from the Sun using inverse-square law: `output_1au * (AU / sun_distance)^2`.

### Requirement: RTG info in flight

The flight part info popup for RTGs SHALL display constant output in Watts.

### Requirement: Electrical render data fields

`ShipPartRenderData` SHALL include:
- `battery_current: Option<f64>` — current Wh stored
- `battery_max: Option<f64>` — maximum Wh capacity
- `solar_output: Option<f64>` — current watts (distance-adjusted in flight)
- `rtg_output: Option<f64>` — constant watts

## Flight Part Selection

### Requirement: Flight part click detection

Clicking on a part in ship view SHALL select it and testing against part hitboxes.

#### Scenario: Precision-safe click coordinate conversion

The click-to-vessel-local conversion SHALL compute the screen offset from camera center directly in f32, avoiding the precision loss that would occur from adding a tiny screen offset (~1e-8 render units) to a galaxy-scale camera position (~1e10 render units) in f64:
1. Convert screen position to NDC
2. Undo zoom and aspect ratio in f32
3. Undo camera rotation in f32
4. Cast to f64 — this is the click offset from camera center in render units
5. Add `(camera.position - ship_render_position)` drift term (zero when tracking, non-zero after panning)
6. Un-rotate by vessel visual rotation (`rotation - PI/2`)
7. Divide by `ship_render_scale` (= SCALE = 1e-9) to get meters
8. Test against each part's `local_x/y` and `hitbox_half_w/h` (all in meters relative to vessel COM)

The naive approach (`screen_to_world()` then subtract `ship_render_position`) fails because `screen_to_world` adds the f32 screen offset to the f64 camera position, and the offset vanishes into the f64 ULP at galaxy-scale magnitudes.

#### Scenario: Part selection with egui overlap

Part selection runs unconditionally on left-click (before the egui consumption check), so clicking on a part works even when an egui window (e.g., part info popup) overlaps the ship. If no part is hit and egui did not consume the click, the selection is cleared.

### Requirement: Background vessel orbit visibility

Each background vessel's orbit line SHALL be visible when that vessel's own triangle indicator is showing (`needs_indicator` = true), independent of the active ship's zoom level.
