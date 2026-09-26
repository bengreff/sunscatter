# Flight Camera

Camera position, zoom, body tracking, and coordinate conversions for flight mode.

## Camera State

### Requirement: Camera position uses f64 precision

The camera position is stored as `[f64; 2]` to prevent precision loss at large distances (billions of meters from origin). The GPU uniform receives a truncated `[f32; 2]` copy, but all camera math (panning, tracking, coordinate conversion) operates in f64.

### Requirement: Two-step camera subtraction for vertex precision

The camera position is decomposed into `body_center` (SOI body position in render units) and `ship_offset` (ship's relative position in render units) as two separate f64 values. When generating vertex positions, both are subtracted from world positions as two separate f64 operations before casting to f32:

```
vertex_f32 = ((world_pos - body_center) - ship_offset) as f32
```

This preserves full f32 precision for vertices near the ship. Adding `ship_offset` (~0.006 WU) to `body_center` (~2.46e11 WU) would introduce ~3e-5 WU error (f64 ULP at galaxy-scale), which is 30 km in physical units. The two-step subtraction keeps each f64 operation between values of similar magnitude.

When tracking the ship, parts are rendered at `(0, 0)` in camera-relative space (their `body_center` and `ship_offset` exactly cancel), so part vertex offsets (~1e-9 WU) have full f32 precision. When the camera is focused on a body instead, the ship's camera-relative position uses the same two-step decomposition: `RenderState` stores `ship_body_center` (SOI body position in render units) and `ship_rel_offset` (ship's local offset) separately, then computes `((ship_body_center - cam_body_center) + (ship_rel_offset - cam_ship_offset)) as f32`. Each subtraction is between values of similar magnitude, preserving f64 precision. The shader's `fine_offset` uniform is set to `[0.0, 0.0]` — all precision work happens on the CPU.

When the user pans the camera, `body_center` is set to `camera.position` and `ship_offset` is zeroed, gracefully degrading to single-step subtraction.

### Requirement: Camera initial state

A new Camera SHALL initialize with: position `[0.0, 0.0]`, zoom `1.0`, rotation `0.0`, `is_dragging = false`, `last_mouse_pos = [0.0, 0.0]`.

### Requirement: Camera zoom clamping

Zoom SHALL be clamped to `[0.00001, 1e10]` on `zoom_by()` and `[0.001, 1e10]` on `zoom_at()`.

### Requirement: Zoom at world position preserves cursor position

When zooming at a world position, the camera SHALL reposition so that the world coordinate under the cursor remains stationary on screen: `position[i] = world_pos[i] - (world_pos[i] - old_pos[i]) * (old_zoom / new_zoom)`.

### Requirement: Camera rotation field

The Camera SHALL have a `rotation: f32` field representing the camera rotation in radians. The `CameraUniform` sent to the GPU SHALL include this rotation value. The rotation is applied in the vertex shader, `world_to_screen`, `screen_to_world`, and panning calculations.

### Requirement: Camera surface-down rotation

When the ship is both below landing altitude AND suborbital, the camera SHALL rotate so that the body's surface is oriented downward on screen. The target rotation SHALL be `PI/2 - atan2(rel_y, rel_x)`, where `rel_x` and `rel_y` are the ship's position relative to the SOI body center. When the ship is not below landing altitude or not suborbital, the target rotation SHALL be 0.0 (no rotation).

### Requirement: Camera rotation interpolation

Camera rotation SHALL smoothly interpolate toward the target rotation at a rate of approximately 5 radians per second. Angle wrapping SHALL be handled by normalizing the difference to [-PI, PI]. When the remaining difference is less than one step, the rotation SHALL snap to the target. The rotation SHALL be normalized to [-PI, PI] after each update.

### Requirement: Camera panning

The `pan(dx, dy)` method SHALL be rotation-aware: the screen-space drag delta SHALL be rotated by the negative camera rotation before applying the pan, so that dragging feels natural regardless of camera orientation. The adjusted delta is `rotated_dx = dx*cos(r) + dy*sin(r)`, `rotated_dy = -dx*sin(r) + dy*cos(r)`, then position is updated by `(-rotated_dx / zoom, +rotated_dy / zoom)`.

### Requirement: Camera aspect ratio updates on resize

When the window is resized, `camera.aspect_ratio = new_width / new_height` SHALL be recalculated.

## Coordinate Conversion

### Requirement: Screen to world coordinate conversion

`screen_to_world` SHALL convert pixel coordinates to world coordinates, accounting for camera rotation:
1. NDC: `ndc_x = (screen_x / width) * 2 - 1`, `ndc_y = 1 - (screen_y / height) * 2`
2. Undo aspect ratio and zoom: `view_x = ndc_x * aspect_ratio / zoom`, `view_y = ndc_y / zoom`
3. Undo rotation (inverse rotation): `unrotated_x = view_x * cos(r) + view_y * sin(r)`, `unrotated_y = -view_x * sin(r) + view_y * cos(r)`
4. World: `world_x = unrotated_x + position[0]`, `world_y = unrotated_y + position[1]`

### Requirement: World to screen coordinate conversion

`world_to_screen` SHALL convert world coordinates to pixel coordinates, accounting for camera rotation:
1. Relative: `rel_x = world_x - position[0]`, `rel_y = world_y - position[1]`
2. Apply rotation: `rotated_x = rel_x * cos(r) - rel_y * sin(r)`, `rotated_y = rel_x * sin(r) + rel_y * cos(r)`
3. Apply zoom: `view_x = rotated_x * zoom`, `view_y = rotated_y * zoom`
4. NDC: `ndc_x = view_x / aspect_ratio`, `ndc_y = view_y`
5. Screen: `screen_x = (ndc_x + 1) * 0.5 * width`, `screen_y = (1 - ndc_y) * 0.5 * height`

### Requirement: Shader rotation support

The vertex shader SHALL apply camera rotation to vertex positions before zoom and aspect ratio correction:
1. Rotate: `rotated = (position.x * cos(r) - position.y * sin(r), position.x * sin(r) + position.y * cos(r))`
2. Zoom: `view_pos = rotated * zoom`
3. Aspect ratio: `corrected_x = view_pos.x / aspect_ratio`

The `CameraUniform` struct SHALL include `rotation: f32` and padding to maintain 32-byte alignment.

## Body Tracking

### Requirement: Camera body tracking

The camera can track a celestial body by index. When tracking, the camera position SHALL be set to the tracked body's world position each frame via `update_tracking()`.

### Requirement: Focus on body

Double-clicking a body calls `focus_on_body(index)`, which centers the camera on the body's world position and sets `tracked_body = Some(index)`.

## View Modes

### Requirement: Ship view vs map view threshold

The ship is in "map view" (indicator visible) when its screen size is less than 5 pixels. It is in "ship view" when its screen size is 5 pixels or larger. Ship pixel size is calculated as `ship_data.size * pixels_per_world_unit * 2.0`, where `pixels_per_world_unit = camera.zoom * screen_height / 2.0`.

#### Scenario: Map view
- **WHEN** `ship_size * pixels_per_world_unit * 2.0 < 5.0`
- **THEN** the ship triangle indicator is drawn, orbit lines are visible

#### Scenario: Ship view
- **WHEN** `ship_size * pixels_per_world_unit * 2.0 >= 5.0`
- **THEN** ship parts are rendered, trees/launchpad are visible, orbit lines are hidden

### Requirement: Galaxy view tracked body redirect

When the camera enters galaxy view (screen spans >= 0.1 light-years), the tracked body SHALL be redirected to its nearest star ancestor. This prevents the camera from tracking an invisible planet/moon in galaxy view. The redirect walks up the body hierarchy until it finds a body whose parent is the root body (a star). This applies in both flight mode and tracking station mode.

#### Scenario: Ship tracking redirect in galaxy view
- **WHEN** in flight mode with ship tracking active and camera enters galaxy view
- **THEN** ship tracking is disabled, and the tracked body is set to the ship's SOI body's parent star
- **AND** the camera follows the star instead of the ship

#### Scenario: Body tracking redirect in galaxy view
- **WHEN** in flight mode tracking a planet or moon and camera enters galaxy view
- **THEN** the tracked body is redirected to its parent star

### Requirement: Galaxy view rendering restrictions

In galaxy view, only stars (direct children of the root body) and the root body SHALL be visible. All other objects are hidden.

#### Scenario: Galaxy view hides non-star bodies
- **WHEN** in galaxy view
- **THEN** planet and moon body circles have radius set to 0 (invisible and non-interactive)

#### Scenario: Galaxy view hides ship and vessels
- **WHEN** in galaxy view
- **THEN** the active ship indicator, orbit lines, parts, and launchpad are not rendered
- **AND** background vessel indicators and orbit lines are not rendered

#### Scenario: Galaxy view hides non-star orbits
- **WHEN** in galaxy view
- **THEN** only star orbits around the root body are shown (when the star is tracked)
- **AND** all planetary and moon orbit lines are hidden
