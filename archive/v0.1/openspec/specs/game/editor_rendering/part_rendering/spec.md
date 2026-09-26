# Part Rendering

Vertex generation for placed parts. Parts with sprites use textured quads from the sprite atlas; parts without sprites fall back to procedural shape rendering.

## Sprite Atlas

### Requirement: Sprite atlas loading

At startup, all PNG sprites from `data/sprites/{engines,parts,plumes}/` SHALL be packed into a single GPU texture atlas (4096px wide, height rounded to next power-of-2). A shelf-based packer arranges sprites sorted by height descending. Interstellar engines (orion_pulse, daedalus_s1/s2, zpinch_probe/advanced, amcat_fusion, am_torch, gamma_conversion) SHALL be excluded from the atlas.

### Requirement: Sprite rendering priority

When rendering a part, if the sprite atlas contains a sprite matching the part's definition ID, a textured quad SHALL be emitted instead of procedural geometry. Fairings SHALL always use procedural rendering (their shells are user-shaped geometry). If no sprite is found, the procedural fallback SHALL be used.

### Requirement: Sprite UV convention

Sprite vertices SHALL use `uv.x >= 2.0` as a flag for the shader to sample from the sprite atlas. The atlas UV is stored with a +2.0 offset on the U coordinate. The fragment shader subtracts 2.0 before sampling and discards fragments with `alpha < 0.01`.

### Requirement: Sprite tint multiplication

The shader SHALL multiply the sampled sprite color by the vertex tint color. Default tint `[1,1,1,1]` produces no change. Ghost previews use tinted sprites: valid = `[0.3, 0.9, 0.3, 0.5]`, invalid = `[0.9, 0.3, 0.3, 0.5]`. Heat tinting in flight applies via the existing `apply_heat_tint()` on the tint color.

### Requirement: Sprite quad placement

Sprite quads SHALL be placed according to per-category alignment rules via `sprite_placement()`:

- **Engines**: width = flight hitbox width, height = flight hitbox height, centered on the editor hitbox center. The flight hitbox may be narrower than the editor hitbox (which is always odd for grid alignment). Sprites render at their natural size within the wider editor placement zone.
- **Stack decouplers** (non-radial): width = hitbox width, height = visual height, bottom-aligned within the hitbox (y offset = `-(hitbox_half_h - visual_half_h)`).
- **Heat shields**: width = hitbox width, height = visual height, top-aligned within the hitbox (y offset = `+(hitbox_half_h - visual_half_h)`). Each heat shield size has `grid_height` matching its sprite's natural aspect ratio (Tiny: 0.1, Small: 0.28, Medium: 0.48, Large: 0.86 grid squares) and `flight_hitbox_height` matching `grid_height`. The editor hitbox remains 1 tile for grid placement. In flight, `click_local_y` is shifted to the visual center for correct click detection.
- **RCS thrusters** (standalone, not pods): visual dimensions, side-offset to the appropriate edge of the hitbox based on `is_mirrored`.
- **Default** (tanks, pods, radial decouplers, fairings): hitbox width x hitbox height, centered. This eliminates visual gaps between stacked tanks.

In the game coordinate system (+Y up), the sprite's top row maps to `v_min` (top of atlas region) and bottom row maps to `v_max`.

### Requirement: Nose cone sprite rendering

Parts with Triangle, TriangleLeft, or TriangleRight shapes (nose cones) SHALL use sprite rendering like other parts. Nose cone sprites are generated with tank-matching colors (tip: RGB 210,215,225; base: RGB 195,200,210; outline: RGB 145,150,160) so they blend visually with adjacent tank bodies. The procedural fallback for nose cones uses `[0.76, 0.78, 0.82, alpha]` (tank-matching gray) instead of white.

### Requirement: Pod RCS nozzle overlay

When a pod has built-in RCS (`def.rcs.is_some()`), after rendering the pod sprite, RCS nozzle bumps SHALL be overlaid using `generate_pod_rcs_nozzles()`. This draws small triangular nozzle tips at ~80% pod height on both the left and right edges.

## Plume Sprites

### Requirement: Sprite plume animation

When a sprite plume animation exists for an engine's propellant type (kerolox/methalox/hydrolox), the plume SHALL be rendered as a textured quad instead of procedural triangles. The animation cycles through 4 frames at ~10fps using wall-clock time (independent of time warp). Plume width = nozzle width, plume height = `nozzle_width * 2.5 * throttle`. Brightness tint scales with throttle (`0.5 + 0.5 * throttle`).

### Requirement: Plume nozzle position

The plume nozzle Y position SHALL be the bottom of the flight hitbox (top-aligned within the editor hitbox): `y + editor_half_h - flight_hitbox_height_m`. This aligns the plume origin with the visible nozzle tip rather than the sprite quad bottom (which includes transparent padding).

### Requirement: Procedural plume fallback

Engines without sprite plumes SHALL fall back to the procedural two-triangle plume (red outer, yellow inner).

## Colors

### Requirement: Part color scheme

Placed parts SHALL use the following colors:
- Default: `[0.4, 0.4, 0.45, 1.0]`
- Selected: `[0.5, 0.7, 1.0, 1.0]`
- Hovered: `[0.55, 0.55, 0.6, 1.0]`

Mirror partners SHALL share the same visual state as the primary selected/hovered part.

## Shape Rendering

### Requirement: Rectangle part rendering

Rectangle parts SHALL be rendered as two triangles forming a quad, using the part's visual width and height centered at the part position.

### Requirement: Triangle part rendering

Triangle parts SHALL be rendered as a single tank-colored (`[0.76, 0.78, 0.82, alpha]`) triangle with the base at the bottom and the apex at top center. This procedural fallback matches tank body color for visual consistency when sprites are unavailable.

### Requirement: TriangleRight part rendering

TriangleRight parts SHALL be rendered as a single tank-colored right triangle with vertices at bottom-left, bottom-right, and top-right (vertical edge on the right side, hypotenuse on the left). Used for right-side booster nose cones.

### Requirement: TriangleLeft part rendering

TriangleLeft parts SHALL be rendered as a single tank-colored right triangle with vertices at bottom-left, bottom-right, and top-left (vertical edge on the left side, hypotenuse on the right). Used for left-side booster nose cones.

### Requirement: Trapezoid part rendering

Trapezoid parts SHALL be rendered as two triangles forming a quad with `width` at the bottom edge and `top_width` at the top edge.

## Overlays

### Requirement: Invalid drag overlay for generic parts

When a generic part is being dragged to an invalid position, a red overlay (`[0.9, 0.2, 0.2, 0.4]`) SHALL be rendered on top of the part using the same shape geometry.

## Rendering Pipeline

### Requirement: Part rendering pass order

Parts SHALL be rendered in two passes: first pass draws all part shapes (engines, pods, decouplers, generic shapes), second pass draws decoupler adapter trapezoids. This ensures adapters render on top of adjacent parts.

### Requirement: Camera-relative part output

All part vertices SHALL be output in camera-relative coordinates by subtracting the camera offset from the world position of each part.
