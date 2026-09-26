# Editor Camera

Camera controls for panning, zooming, keyboard movement, and auto-focus in the vehicle editor.

### Requirement: Camera zoom range

The editor camera zoom SHALL be clamped to `[0.1, 16666.0]`. Zoom is in pixels per meter (higher = more zoomed in).

### Requirement: Camera initial state

The camera SHALL initialize centered on the middle of a grid square: offset `[GRID_SIZE / 2, GRID_SIZE / 2]` = `[0.25, 0.25]`, with zoom `1.0`.

### Requirement: Scroll wheel zoom

Scrolling SHALL multiply the camera zoom by a factor. The resulting zoom SHALL be clamped to the valid range.

### Requirement: Camera panning

Camera panning SHALL adjust the offset by `delta / zoom` in world units, translating screen-space drag distance into world-space movement.

### Requirement: Keyboard camera movement

Arrow keys SHALL move the camera at a speed of `1.67 / zoom` world units per second (inversely proportional to zoom). Movement SHALL be applied each frame based on held keys and delta time.

### Requirement: Auto-focus on load

`focus_on_parts` SHALL compute the axis-aligned bounding box of all placed parts (using visual dimensions), center the camera on it, and set zoom to `0.6 / max(width, height)` (fitting the craft in ~60% of the smaller screen dimension), clamped to valid range. If no parts exist, it SHALL do nothing.
