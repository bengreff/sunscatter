# Procedural Parts

Procedural rendering of engines, pods, and decouplers with per-engine visual identity, adapter trapezoids, and exhaust plumes.

## Engine Rendering

### Requirement: Engine nozzle bell base shape

Engines SHALL be rendered as a trapezoid (nozzle bell) using the part's full width at the bottom and `top_width` at the top. The nozzle color SHALL depend on whether the engine is a "first stage" engine (ASL thrust > 70% of vacuum thrust): first stage engines use dark nozzle color `[0.08, 0.08, 0.10, 1.0]`, upper stage engines use lighter nozzle color `[0.15, 0.15, 0.17, 1.0]`.

#### Scenario: First stage engine detection
- **WHEN** an engine has `thrust_asl > thrust_vac * 0.7`
- **THEN** it SHALL be classified as a first stage engine and use the dark nozzle color

#### Scenario: Upper stage engine detection
- **WHEN** an engine has `thrust_asl <= thrust_vac * 0.7`
- **THEN** it SHALL be classified as an upper stage engine and use the lighter nozzle color

### Requirement: Engine combustion chamber

Each engine SHALL render a rectangular combustion chamber near the top of the nozzle. The chamber position and size vary per engine ID, expressed as fractions of the nozzle's half-dimensions. The chamber color SHALL be `[0.18, 0.18, 0.20, 1.0]`.

### Requirement: Nozzle cooling rings

Each engine SHALL render horizontal ring bands across the nozzle. Rings are evenly distributed from the bottom upward (covering the lower 70% of the nozzle height). Ring count varies per engine ID (2 to 14). Ring width at each Y position SHALL be interpolated between bottom and top width. Ring thickness SHALL be `half_h * 0.04`. Ring color: dark `[0.14, 0.14, 0.16, 1.0]` for first stage, light `[0.22, 0.22, 0.24, 1.0]` for upper stage.

#### Scenario: Ring interpolation
- **GIVEN** `half_w = 1.0` at bottom and `half_top_w = 0.5` at top
- **WHEN** ring is at height fraction `t = 0.35`
- **THEN** ring width = `half_w + (half_top_w - half_w) * t = 0.825`

### Requirement: Gimbal actuator brackets

Engines with gimbal range > 2.0 degrees MAY render gimbal actuator brackets as angled triangles on left and right sides of the nozzle, extending outward by `half_w * 0.3`. Per-engine-ID.

### Requirement: Turbopump housing

Some engines SHALL render a rectangular turbopump box near the top of the engine. Color: `[0.20, 0.20, 0.22, 1.0]`. Per-engine-ID.

### Requirement: Gas generator box

Some engines SHALL render a rectangular gas generator box on one side (offset `half_top_w * 0.85-0.9`). Color: `[0.16, 0.16, 0.18, 1.0]`. Per-engine-ID.

### Requirement: Per-engine visual identity

Each of the 16 engine IDs SHALL have a unique combination of: combustion chamber size/position, ring count, gimbal actuators, turbopump, and gas generator. IDs: `engine_hummingbird`, `engine_gecko`, `engine_firefly`, `engine_wolf`, `engine_falcon`, `engine_wren`, `engine_owl`, `engine_viper`, `engine_bear`, `engine_eagle`, `engine_panther`, `engine_crane`, `engine_mammoth`, `engine_whale`, `engine_bison`, `engine_titan`.

### Requirement: Unknown engine fallback

Unrecognized engine IDs SHALL render with: default combustion chamber (Y offset `0.75`, width `0.8`, height `0.25`), ring count 8 (first stage) or 4 (upper stage), gimbal actuators only if `gimbal_range > 2.0`.

### Requirement: Engine alpha modulation

All engine detail colors SHALL support alpha modulation: `0.5` for ghost previews, `1.0` for placed parts.

## Pod Rendering

### Requirement: Pod body and window

Pods SHALL render as a dark grey trapezoid body (`[0.15, 0.15, 0.18, 1.0]`) with a white circular window (`[0.9, 0.9, 0.95, 1.0]`) centered horizontally at `y + half_h * 0.2`. Window radius = `min(half_w, half_h) * 0.25`.

### Requirement: Window as filled circle

The pod window SHALL be rendered as a 12-segment triangle fan.

### Requirement: Built-in RCS nozzle bumps

When a pod definition has `rcs: Some(...)`, two small triangular nozzles SHALL protrude from the left and right edges of the pod trapezoid at 80% of the pod height. Each nozzle is a triangle with its base on the pod edge (half-width 0.04m) and tip pointing outward (length 0.08m), matching the style of standalone RCS nozzles. Nozzle X position SHALL be interpolated along the trapezoid edge at that height. Color: `RCS_NOZZLE_COLOR` (`[0.12, 0.12, 0.14, 1.0]`).

## Decoupler Rendering

### Requirement: Decoupler ring band

Stack decouplers SHALL render as a horizontal band from hitbox bottom upward by visual height. Color: `[0.25, 0.25, 0.28, 1.0]`. Radial decouplers (`is_radial: true`) SHALL instead render as a simple dark rectangle (`[0.1, 0.1, 0.1, alpha]`) filling the part's visual bounds, with no ring band.

### Requirement: Decoupler adapter trapezoid

A second pass SHALL draw adapter trapezoids connecting stack decouplers to the closest aligned fuel tank **or command pod** above (same center X within 0.01 tolerance). Bottom edge matches decoupler width, top edge matches tank/pod width. Radial decouplers SHALL NOT render adapter trapezoids.

#### Scenario: Adapter with size transition
- **GIVEN** medium decoupler (2.5m) with small tank (1.5m) above
- **THEN** bottom edge 2.5m, top edge 1.5m (tapered)

#### Scenario: Adapter to command pod
- **GIVEN** decoupler with command pod directly above (same center X)
- **THEN** adapter trapezoid drawn connecting decoupler ring to pod bottom

#### Scenario: No aligned tank or pod above
- **WHEN** no tank or pod shares the decoupler's center X or is above the ring
- **THEN** no adapter drawn

### Requirement: Adapter frustum detail lines

The adapter SHALL render detail lines as frustum projections. Line count = `round(grid_width * 3)`. Each line X = `center_x + half_width * sin(theta)` for evenly-spaced theta in `[-PI/2, PI/2]`. Half-thickness = 0.008 world units. Color: `[0.18, 0.18, 0.20, alpha]`.

### Requirement: Ghost decoupler adapter preview

Ghost previews of decouplers SHALL also render the adapter trapezoid by checking against existing placed parts.

## RCS Thruster Rendering

### Requirement: RCS thin side-mount shape

RCS thrusters SHALL be rendered as thin side-mount parts. Small RCS (RV-1) uses `grid_height: 0.75` for a more compact visual, while medium RCS (RV-3) uses `grid_height: 1.0`. Both share `grid_width: 0.5` and a 1x1 hitbox. The visual sprite SHALL be offset to the side of the hitbox: right-mount (default) offsets to the right side, left-mount (`is_mirrored: true`) offsets to the left side. The offset SHALL be `sign * (hitbox_half_w - visual_half_w)` where sign is 1.0 for right-mount, -1.0 for left-mount.

### Requirement: RCS body rectangle

The RCS body SHALL be a dark grey rectangle (`[0.20, 0.20, 0.22, 1.0]`) covering 80% of the visual extents in both dimensions.

### Requirement: RCS three directional nozzles

Each RCS thruster SHALL render 3 triangular nozzles pointing outward:
- **Lateral nozzle**: Points away from the vessel (left for right-mount, right for left-mount)
- **Top nozzle**: Points upward from the body
- **Bottom nozzle**: Points downward from the body

No nozzle SHALL be rendered on the attachment face (the face against the vessel body). Nozzle color: `[0.12, 0.12, 0.14, 1.0]`.

### Requirement: RCS mirror variants

RCS parts SHALL come in mirror pairs via `mirror_def_id`. Right-mount parts (default, `is_mirrored: false`) have nozzles pointing left/up/down. Left-mount parts (`is_mirrored: true`) have nozzles pointing right/up/down. The `RcsData` struct SHALL include an `is_mirrored: bool` field (serde-default false).

#### Scenario: Right-mount RCS placement
- **WHEN** `rcs_small` is placed (right-mount, `is_mirrored: false`)
- **THEN** the sprite offsets to the right side of the hitbox with nozzles pointing left, up, and down

#### Scenario: Left-mount RCS mirror
- **WHEN** `rcs_small_left` is the mirror variant (`is_mirrored: true`)
- **THEN** the sprite offsets to the left side of the hitbox with nozzles pointing right, up, and down

### Requirement: RCS plume rendering

When RCS nozzles are active during rotation, white plume rectangles (`[0.95, 0.95, 1.0, 0.85]`) SHALL extend outward from each firing nozzle tip. Plume length SHALL be 1.5x the nozzle length, plume width SHALL be 60% of nozzle base width. Plumes SHALL only appear on nozzles whose torque contribution matches the desired rotation direction.

### Requirement: Per-nozzle activation logic

Each RCS nozzle SHALL fire when its torque contribution matches the desired rotation direction OR when translation demands it. The final activation is the union of rotation-driven and translation-driven activations.

**Rotation-driven activation** — torque is computed as the 2D cross product of the part position vector (relative to COM) and the nozzle reaction force direction (opposite exhaust):
- **Lateral nozzle** (non-mirrored, right-mount): Exhausts left, reaction force is rightward. Torque sign = `-ry`. For mirrored (left-mount): exhausts right, reaction force is leftward. Torque sign = `ry`.
- **Up nozzle**: Torque sign = `-rx`
- **Down nozzle**: Torque sign = `rx`
- A nozzle fires when its torque sign matches the desired rotation direction sign.

**Translation-driven activation**:
- **Forward** (`translate[0] > 0`): down nozzles fire on all RCS parts
- **Backward** (`translate[0] < 0`): up nozzles fire on all RCS parts
- **Left** (`translate[1] < 0`): right-mount (non-mirrored) lateral nozzles fire
- **Right** (`translate[1] > 0`): left-mount (mirrored) lateral nozzles fire

## Fairing Base Rendering

### Requirement: Fairing base disc shape

Fairing bases (parts with `fairing: Some(...)`) SHALL be rendered via `generate_fairing_base_details()` as a filled rectangle covering the full hitbox — from `y - hitbox_half_h` (bottom) to `y + hitbox_half_h` (top). The disc fills the entire 1-grid-square-tall hitbox. Color: `FAIRING_BASE_COLOR = [0.30, 0.30, 0.33, 1.0]` (lighter metallic disc, distinct from the darker decoupler ring).

### Requirement: Fairing base alpha modulation

The fairing base color SHALL support alpha modulation: `0.5` for ghost previews, `0.5` when the fairing is hovered (so interior parts become visible), `1.0` otherwise.

## Fairing Shell Rendering (Editor)

### Requirement: Completed fairing shell geometry

`generate_fairing_shell_vertices()` SHALL render the shell of a completed `FairingShape` as a series of symmetric trapezoid segments from the base top edge upward. For each vertex `(hw_grid, y_off_grid)`:
- Left half: two triangles from `(x - prev_hw, prev_y)` to `(x, prev_y)` to `(x, seg_y)` to `(x - hw, seg_y)`
- Right half: mirrored from `(x, prev_y)` to `(x + prev_hw, prev_y)` to `(x + hw, seg_y)` to `(x, seg_y)`

Shell color: `FAIRING_SHELL_COLOR = [0.35, 0.35, 0.38, 1.0]` (light grey panels).

The function accepts a `fairing_half: Option<FairingHalf>` parameter. When `Some(Left)`, only left-side triangles are emitted. When `Some(Right)`, only right-side. When `None`, both halves are drawn (default).

### Requirement: Fairing shell seam lines

Each shell segment SHALL render a horizontal seam line at the vertex Y position (when `hw > 0.001`). A vertical seam line SHALL run down the center of the shell from base top to the top vertex. Line half-thickness: 0.008 world units. Color: `FAIRING_SHELL_LINE_COLOR = [0.20, 0.20, 0.22, 1.0]`.

### Requirement: Fairing build preview

During fairing build mode, `generate_fairing_build_preview()` SHALL render:
1. Completed segments at alpha 0.7 using the same trapezoid geometry as the final shell
2. A ghost segment from the last vertex (or base top) to the current cursor point, colored green `[0.3, 0.9, 0.3, 0.3]` when valid or red `[0.9, 0.3, 0.3, 0.3]` when invalid
3. Diamond-shaped ghost point markers at both the left and right mirrored positions when the ghost is valid (marker size 0.03 world units, color `[0.3, 0.9, 0.3, 0.8]`)

### Requirement: Fairing shell z-ordering

Completed fairing shells SHALL be rendered in a dedicated third pass (after the decoupler adapter pass), so they always draw on top of all other parts. When the fairing is hovered, the shell alpha SHALL be `0.5` for transparency. The fairing base still renders in the first pass.

### Requirement: Fairing build preview pass order

The fairing build preview SHALL be rendered in a fourth pass after the shell z-ordering pass, ensuring in-progress shells draw on top of completed shells.

### Requirement: Ghost fairing base preview

Ghost previews of fairing bases SHALL render the base disc via `generate_fairing_base_details()` at ghost alpha, followed by a green/red validity overlay rectangle over the hitbox area.

## Highlight Overlays

### Requirement: Procedural part selection/hover overlay

For procedural parts, highlighting SHALL be a semi-transparent overlay on top:

| State        | Overlay Color             |
|-------------|--------------------------|
| Selected    | `[0.5, 0.7, 1.0, 0.3]`  |
| Hovered     | `[0.55, 0.55, 0.6, 0.2]` |
| Invalid drag | `[0.9, 0.2, 0.2, 0.4]`  |

### Requirement: Ghost validity overlay for procedural parts

Ghost previews SHALL render procedural details at `alpha = 0.5`, then overlay green `[0.3, 0.9, 0.3, 0.25]` or red `[0.9, 0.3, 0.3, 0.25]` based on validity.

## Exhaust Plume

### Requirement: Engine exhaust plume geometry

`generate_engine_plume_vertices` SHALL render exhaust plumes from engine nozzle exits. No plume when throttle = 0.0.

#### Scenario: Single-nozzle engine (default)
- Two nested triangles from the nozzle exit
- Outer red (`[1.0, 0.2, 0.0, 0.9]`): full width × 1.2, length = `nozzle_width * 4.0 * throttle`
- Inner yellow (`[1.0, 0.9, 0.1, 1.0]`): 60% width, 40% length

#### Scenario: Multi-nozzle engine (`nozzle_offsets`)
- `EngineData.nozzle_offsets: Option<Vec<f64>>` specifies X offsets in grid squares from part center
- Each nozzle renders a separate plume at `x + offset * GRID_SQUARE_SIZE`
- Per-nozzle plume width and height scaled by `1 / nozzle_count`
- For sprite plumes: per-nozzle width = `half_nozzle / n * 1.2`, height = `nozzle_width / n * 5.0 * throttle`
- For procedural plumes: same scaling applied to both outer and inner triangles

#### Scenario: Full throttle plume
- **GIVEN** engine width 1.5m at throttle 1.0
- **THEN** red plume 3.0m long, yellow plume 0.9m wide and 1.2m long

#### Scenario: Partial throttle
- **GIVEN** throttle 0.5
- **THEN** plume length halved
