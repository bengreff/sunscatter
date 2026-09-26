# Editor Rendering

Editor GUI, building grid, part drawing, and UI infrastructure for the vehicle editor.

## Capabilities

- [Grid](grid/spec.md) - Editor building grid line generation
- [Camera](camera/spec.md) - Editor camera panning, zooming, keyboard movement, auto-focus
- [Part Rendering](part_rendering/spec.md) - Generic part shape rendering (rectangle, triangle, trapezoid)
- [Procedural Parts](procedural_parts/spec.md) - Engine nozzles, pods, decouplers, exhaust plumes
- [Toolbar](toolbar/spec.md) - Top toolbar with vessel actions, symmetry, launch
- [Stats Bar](stats_bar/spec.md) - Mass, thrust, TWR, delta-v, resource display
- [Ship Stats](ship_stats/spec.md) - Vessel statistics calculation from placed parts

## Coordinate Transforms

### Requirement: Screen to world coordinate conversion

`screen_to_world` SHALL convert screen pixel coordinates (origin at top-left) to world coordinates using:
1. Convert screen position to NDC: `ndc_x = (screen_x / width) * 2 - 1`, `ndc_y = 1 - (screen_y / height) * 2` (Y is flipped).
2. Invert the shader transform: `world_x = ndc_x * aspect_ratio / zoom + camera_offset_x`, `world_y = ndc_y / zoom + camera_offset_y`.

Where `aspect_ratio = screen_width / screen_height`.

### Requirement: World to screen coordinate conversion

`world_to_screen` SHALL convert world coordinates to screen pixel coordinates using:
1. Compute camera-relative position: `rel_x = world_x - offset_x`, `rel_y = world_y - offset_y`.
2. Convert to screen: `screen_x = rel_x * zoom + width / 2`, `screen_y = height / 2 - rel_y * zoom` (Y is flipped).

### Requirement: Hit testing uses hitbox dimensions

`part_at_screen_pos` SHALL convert the screen position to world coordinates, then test against each part's hitbox AABB (using `hitbox_width()` and `hitbox_height()`). It SHALL return the first matching part ID, or None if no part is hit.

## UI Input Passthrough

### Requirement: Mouse over UI detection

`is_mouse_over_ui` SHALL return true when `egui::Context::is_pointer_over_area()` is true. When this returns true, editor click/drag/placement handlers SHALL NOT process the event.

## Editor Actions

### Requirement: Editor action types

The editor UI SHALL return one of the following actions per frame: `None`, `Launch`, `SaveBlueprint(name)`, `LoadBlueprint(name)`, `NewVessel`, or `ExitToFlight`. The game layer processes these actions after UI rendering.

## Bottom Instructions

### Requirement: Instruction bar content

The bottom panel SHALL display: "Click part to select . Click build area to place . Right-click to deselect . Scroll to zoom . Drag to pan" with a dark translucent background (`rgba(20, 20, 30, 200)`).
